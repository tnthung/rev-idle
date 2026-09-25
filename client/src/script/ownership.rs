use crate::{app::StateUpdate, capture::LockState};
use rquickjs::{class::Trace, Class, Ctx, Exception, JsLifetime};
use std::{cell::{Cell, RefCell}, rc::Rc, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};

pub(super) struct ScreenOwnershipState {
    semaphore: Arc<Semaphore>,
    owned: Cell<bool>,
    paused: Cell<bool>,
    manual_lock: Cell<bool>,
    output: RefCell<Option<(LockState, watch::Sender<StateUpdate>)>>,
}

impl Default for ScreenOwnershipState {
    fn default() -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(1)),
            owned: Cell::new(false),
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
            state_updates.send_if_modified(|state| {
                if state.locked == locked { return false; }
                state.locked = locked;
                true
            });
        }
    }

    pub(super) async fn acquire<'js>(self: Rc<Self>, ctx: Ctx<'js>) -> rquickjs::Result<Class<'js, ScreenOwnership>> {
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| Exception::throw_message(&ctx, "screen ownership is unavailable: script session stopped"))?;
        self.owned.set(true);
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
    async fn screen_ownership_gc_releases_a_cycle_without_clearing_manual_lock() {
        static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
        let lock_state = LockState { enabled: &LOCK_ENABLED };
        let (state_updates, _) = watch::channel(StateUpdate::new(true, true, false, false, false));
        let runtime = AsyncRuntime::new().unwrap();
        let context = AsyncContext::full(&runtime).await.unwrap();
        let state = Rc::new(ScreenOwnershipState::default());
        state.attach(lock_state, state_updates);

        context.async_with(async |ctx| {
            let owner = state.clone().acquire(ctx).await.unwrap();
            owner.set("cycle", owner.clone()).unwrap();
        }).await;
        assert!(lock_state.is_enabled());
        runtime.run_gc().await;
        assert!(!lock_state.is_enabled());

        state.set_manual_lock();
        context.async_with(async |ctx| {
            let owner = tokio::time::timeout(Duration::from_secs(1), state.clone().acquire(ctx)).await.unwrap().unwrap();
            owner.set("cycle", owner.clone()).unwrap();
        }).await;
        runtime.run_gc().await;
        assert!(lock_state.is_enabled());
        state.close();
        assert!(!lock_state.is_enabled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn screen_ownership_close_cancels_waiters_and_isolates_old_tokens() {
        static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
        let lock_state = LockState { enabled: &LOCK_ENABLED };
        let (state_updates, _) = watch::channel(StateUpdate::new(true, true, false, false, false));
        let runtime = AsyncRuntime::new().unwrap();
        let context = AsyncContext::full(&runtime).await.unwrap();
        let state = Rc::new(ScreenOwnershipState::default());
        state.attach(lock_state, state_updates.clone());

        context.async_with(async |ctx| {
            let owner = state.clone().acquire(ctx.clone()).await.unwrap();
            let waiting = state.clone().acquire(ctx.clone());
            tokio::pin!(waiting);
            tokio::select! {
                biased;
                _ = &mut waiting => panic!("a second owner acquired before release"),
                _ = tokio::task::yield_now() => {}
            }
            state.close();
            assert!(tokio::time::timeout(Duration::from_secs(1), waiting).await.unwrap().is_err());
            ctx.catch();
            assert!(!lock_state.is_enabled());

            let replacement = Rc::new(ScreenOwnershipState::default());
            replacement.attach(lock_state, state_updates);
            let next = replacement.clone().acquire(ctx.clone()).await.unwrap();
            owner.borrow_mut().release();
            assert!(lock_state.is_enabled());
            next.borrow_mut().release();
            assert!(!lock_state.is_enabled());

            replacement.set_paused(true);
            let next = tokio::time::timeout(Duration::from_secs(1), replacement.clone().acquire(ctx)).await.unwrap().unwrap();
            assert!(!lock_state.is_enabled());
            replacement.set_paused(false);
            assert!(lock_state.is_enabled());
            next.borrow_mut().release();
            assert!(!lock_state.is_enabled());
        }).await;
    }
}
