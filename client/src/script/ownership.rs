use crate::{app::StateUpdate, capture::LockState};
use crate::script::control::SessionControl;
use rquickjs::{class::Trace, Class, Ctx, Exception, JsLifetime};
use std::{cell::{Cell, RefCell}, rc::Rc, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};

pub(super) struct ScreenOwnershipState {
    semaphore: Arc<Semaphore>,
    owned: Cell<bool>,
    label: RefCell<Option<String>>,
    paused: Cell<bool>,
    manual_lock: Cell<bool>,
    output: RefCell<Option<(LockState, watch::Sender<StateUpdate>)>>,
}

impl Default for ScreenOwnershipState {
    fn default() -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(1)),
            owned: Cell::new(false),
            label: RefCell::new(None),
            paused: Cell::new(false),
            manual_lock: Cell::new(false),
            output: RefCell::new(None),
        }
    }
}

impl ScreenOwnershipState {
    pub(super) fn attach(&self, lock_state: LockState, state_updates: watch::Sender<StateUpdate>) {
        self.manual_lock.set(lock_state.is_enabled());
        *self.output.borrow_mut() = Some((lock_state, state_updates));
        self.update_lock();
    }

    pub(super) fn set_manual_lock(&self) {
        self.manual_lock.set(true);
        self.update_lock();
    }

    pub(super) fn set_paused(&self, paused: bool) {
        self.paused.set(paused);
        if paused {
            // Preserve the existing rule: plain resume does not restore a manual lock.
            self.manual_lock.set(false);
        }
        self.update_lock();
    }

    pub(super) fn close(&self) {
        self.semaphore.close();
        self.update_lock();
        // Old tokens may finalize after a replacement session has started.
        self.output.borrow_mut().take();
    }

    fn update_lock(&self) {
        if let Some((lock_state, state_updates)) = self.output.borrow().as_ref() {
            let locked = !self.semaphore.is_closed()
                && !self.paused.get()
                && (self.manual_lock.get() || self.owned.get());
            lock_state.set_enabled(locked);
            let label = if locked { self.label.borrow().clone() } else { None };
            state_updates.send_if_modified(|state| {
                if state.locked == locked && state.lock_label == label { return false; }
                state.locked = locked;
                state.lock_label = label;
                true
            });
        }
    }

    pub(super) async fn acquire<'js>(
        self: Rc<Self>,
        ctx: Ctx<'js>,
        session: SessionControl,
        label: Option<String>,
    ) -> rquickjs::Result<Class<'js, ScreenOwnership>> {
        if session.is_stopped() {
            return Err(Exception::throw_message(&ctx, SessionControl::error_message()));
        }
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| Exception::throw_message(&ctx, "screen ownership is unavailable: script session stopped"))?;
        if session.is_stopped() {
            drop(permit);
            return Err(Exception::throw_message(&ctx, SessionControl::error_message()));
        }
        self.owned.set(true);
        *self.label.borrow_mut() = label;
        self.update_lock();
        Class::instance(ctx, ScreenOwnership { state: self, permit: Some(permit) })
    }
}

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub(super) struct ScreenOwnership {
    #[qjs(skip_trace)]
    state: Rc<ScreenOwnershipState>,
    #[qjs(skip_trace)]
    permit: Option<OwnedSemaphorePermit>,
}

#[rquickjs::methods]
impl ScreenOwnership {
    pub fn release(&mut self) {
        if let Some(permit) = self.permit.take() {
            self.state.owned.set(false);
            self.state.label.borrow_mut().take();
            self.state.update_lock();
            drop(permit);
        }
    }
}

impl Drop for ScreenOwnership {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquickjs::{AsyncContext, AsyncRuntime};
    use std::{sync::atomic::AtomicBool, time::Duration};

    #[tokio::test(flavor = "current_thread")]
    async fn ownership_checkpoint_retains_token_and_fifo_order() {
        use crate::script::{bindings::{Button, HostControls, MouseInput, SharedMouse}, session::ScriptSession};
        static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
        struct Mouse(Rc<RefCell<Vec<i32>>>, tokio::sync::mpsc::UnboundedSender<i32>);
        impl MouseInput for Mouse {
            fn click_at(&mut self, x: i32, _: i32, _: Button) -> Result<(), crate::script::bindings::MouseInputError> {
                self.0.borrow_mut().push(x);
                self.1.send(x).unwrap();
                Ok(())
            }
        }
        let clicks = Rc::new(RefCell::new(Vec::new()));
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let controls: HostControls = (Rc::new(RefCell::new(Mouse(clicks.clone(), events))) as SharedMouse).into();
        let lock_state = LockState { enabled: &LOCK_ENABLED };
        let (state_updates, _) = watch::channel(StateUpdate::new(true, true, true, false, false));
        let session = ScriptSession::new(r#"
            export default async function() {
                using first = await rev.screenOwnership();
                rev.click(1, 1);
                await rev.sleep(10);
                const second = (async () => { using next = await rev.screenOwnership(); rev.click(2, 1); })();
                const third = (async () => { using next = await rev.screenOwnership(); rev.click(4, 1); })();
                const monitor = (async () => {
                    for (let i = 0; i < 3; i++) { await rev.sleep(1); rev.click(100, 1); }
                })();
                await rev.ensureRunning();
                rev.click(3, 1);
                first.release();
                await Promise.all([second, third, monitor]);
            }
        "#).await.unwrap();
        session.screen_ownership.attach(lock_state, state_updates);
        let invocation = session.invoke((), controls.clone());
        tokio::pin!(invocation);
        assert_eq!(tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                result = &mut invocation => panic!("invocation completed before acquiring ownership: {result:?}"),
                event = received.recv() => event.unwrap(),
            }
        }).await.unwrap(), 1);
        session.acknowledge_pause(true);
        session.screen_ownership.set_paused(true);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(*clicks.borrow(), vec![1]);
        assert!(!lock_state.is_enabled());
        session.acknowledge_pause(false);
        session.screen_ownership.set_paused(false);
        assert!(lock_state.is_enabled());
        tokio::time::timeout(Duration::from_secs(1), invocation).await.unwrap().unwrap();
        assert_eq!(clicks.borrow().iter().copied().filter(|event| *event < 100).collect::<Vec<_>>(), vec![1, 3, 2, 4]);
        assert!(!lock_state.is_enabled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ownership_is_shared_across_runtime_local_tokens() {
        use crate::script::{bindings::{Button, HostControls, MouseInput, SharedMouse}, session::ScriptSession};
        struct Mouse(tokio::sync::mpsc::UnboundedSender<i32>);
        impl MouseInput for Mouse {
            fn click_at(&mut self, x: i32, _: i32, _: Button) -> Result<(), crate::script::bindings::MouseInputError> {
                self.0.send(x).unwrap();
                Ok(())
            }
        }
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let controls: HostControls = (Rc::new(RefCell::new(Mouse(events))) as SharedMouse).into();
        let session = ScriptSession::new(r#"
            export function afterLoad() {
                rev.daemon('owner', async function() {
                    const owner = await rev.screenOwnership();
                    rev.click(10, 1);
                    await rev.sleep(30);
                    owner.release();
                    rev.click(11, 1);
                });
            }
            export default async function() {
                const owner = await rev.screenOwnership();
                rev.click(20, 1);
                owner.release();
            }
        "#).await.unwrap();
        let background = session.drive_background(controls.clone());
        tokio::pin!(background);
        let after_load = session.run_after_load(controls.clone());
        tokio::pin!(after_load);
        tokio::select! {
            _ = &mut background => panic!("background service stopped during afterLoad"),
            result = &mut after_load => result.unwrap(),
        }
        assert_eq!(tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                tokio::select! {
                    _ = &mut background => panic!("background service stopped before daemon acquired ownership"),
                    event = received.recv() => if event.unwrap() == 10 { break 10; },
                }
            }
        }).await.unwrap(), 10);
        let invocation = session.invoke((), controls);
        tokio::pin!(invocation);
        let mut observed = Vec::new();
        let mut invocation_complete = false;
        tokio::time::timeout(Duration::from_secs(1), async {
            while observed.len() < 2 {
                tokio::select! {
                    _ = &mut background => panic!("background service stopped while tokens were queued"),
                    result = &mut invocation, if !invocation_complete => {
                        result.unwrap();
                        invocation_complete = true;
                    }
                    event = received.recv() => observed.push(event.unwrap()),
                }
            }
        }).await.unwrap();
        assert_eq!(observed, vec![11, 20]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn screen_ownership_gc_releases_a_cycle_without_clearing_manual_lock() {
        static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
        let lock_state = LockState { enabled: &LOCK_ENABLED };
        let (state_updates, state_rx) = watch::channel(StateUpdate::new(true, true, false, false, false));
        let runtime = AsyncRuntime::new().unwrap();
        let context = AsyncContext::full(&runtime).await.unwrap();
        let state = Rc::new(ScreenOwnershipState::default());
        let session = SessionControl::standalone();
        state.attach(lock_state, state_updates);

        context.async_with(async |ctx| {
            let owner = state.clone().acquire(ctx, session.clone(), Some("First".into())).await.unwrap();
            owner.set("cycle", owner.clone()).unwrap();
        }).await;
        assert!(lock_state.is_enabled());
        assert_eq!(state_rx.borrow().lock_label.as_deref(), Some("First"));
        runtime.run_gc().await;
        assert!(!lock_state.is_enabled());
        assert_eq!(state_rx.borrow().lock_label, None);

        state.set_manual_lock();
        context.async_with(async |ctx| {
            let owner = tokio::time::timeout(Duration::from_secs(1), state.clone().acquire(ctx, session.clone(), Some("Second".into()))).await.unwrap().unwrap();
            owner.set("cycle", owner.clone()).unwrap();
        }).await;
        assert_eq!(state_rx.borrow().lock_label.as_deref(), Some("Second"));
        runtime.run_gc().await;
        assert!(lock_state.is_enabled());
        assert_eq!(state_rx.borrow().lock_label, None);
        state.close();
        assert!(!lock_state.is_enabled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn screen_ownership_close_cancels_waiters_and_isolates_old_tokens() {
        static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
        let lock_state = LockState { enabled: &LOCK_ENABLED };
        let (state_updates, state_rx) = watch::channel(StateUpdate::new(true, true, false, false, false));
        let runtime = AsyncRuntime::new().unwrap();
        let context = AsyncContext::full(&runtime).await.unwrap();
        let state = Rc::new(ScreenOwnershipState::default());
        let session = SessionControl::standalone();
        state.attach(lock_state, state_updates.clone());

        context.async_with(async |ctx| {
            let owner = state.clone().acquire(ctx.clone(), session.clone(), Some("Current".into())).await.unwrap();
            let waiting = state.clone().acquire(ctx.clone(), session.clone(), Some("Waiting".into()));
            tokio::pin!(waiting);
            tokio::select! {
                biased;
                _ = &mut waiting => panic!("a second owner acquired before release"),
                _ = tokio::task::yield_now() => {}
            }
            assert_eq!(state_rx.borrow().lock_label.as_deref(), Some("Current"));
            state.close();
            assert!(tokio::time::timeout(Duration::from_secs(1), waiting).await.unwrap().is_err());
            ctx.catch();
            assert!(!lock_state.is_enabled());
            assert_eq!(state_rx.borrow().lock_label, None);

            let replacement = Rc::new(ScreenOwnershipState::default());
            replacement.attach(lock_state, state_updates);
            let next = replacement.clone().acquire(ctx.clone(), session.clone(), Some("Replacement".into())).await.unwrap();
            owner.borrow_mut().release();
            assert!(lock_state.is_enabled());
            assert_eq!(state_rx.borrow().lock_label.as_deref(), Some("Replacement"));
            next.borrow_mut().release();
            assert!(!lock_state.is_enabled());
            assert_eq!(state_rx.borrow().lock_label, None);

            replacement.set_paused(true);
            let next = tokio::time::timeout(Duration::from_secs(1), replacement.clone().acquire(ctx, session.clone(), None)).await.unwrap().unwrap();
            assert!(!lock_state.is_enabled());
            replacement.set_paused(false);
            assert!(lock_state.is_enabled());
            next.borrow_mut().release();
            assert!(!lock_state.is_enabled());
        }).await;
    }
}
