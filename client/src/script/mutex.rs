use crate::script::control::SessionControl;
use rquickjs::{class::Trace, Class, Ctx, Exception, JsLifetime};
use std::{cell::{Cell, RefCell}, collections::HashMap, rc::Rc, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(super) struct MutexRegistry {
    channels: RefCell<HashMap<String, Arc<Semaphore>>>,
    closed: Cell<bool>,
}

impl Default for MutexRegistry {
    fn default() -> Self {
        Self { channels: RefCell::new(HashMap::new()), closed: Cell::new(false) }
    }
}

impl MutexRegistry {
    pub(super) fn close(&self) {
        self.closed.set(true);
        for semaphore in self.channels.borrow().values() {
            semaphore.close();
        }
    }

    pub(super) async fn acquire<'js>(
        self: Rc<Self>,
        ctx: Ctx<'js>,
        session: SessionControl,
        channel: String,
    ) -> rquickjs::Result<Class<'js, MutexGuard>> {
        if session.is_stopped() || self.closed.get() {
            return Err(Exception::throw_message(&ctx, SessionControl::error_message()));
        }
        let semaphore = self.channels.borrow_mut()
            .entry(channel)
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone();
        let permit = semaphore.acquire_owned().await
            .map_err(|_| Exception::throw_message(&ctx, SessionControl::error_message()))?;
        if session.is_stopped() || self.closed.get() {
            drop(permit);
            return Err(Exception::throw_message(&ctx, SessionControl::error_message()));
        }
        Class::instance(ctx, MutexGuard { permit: Some(permit) })
    }
}

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub(super) struct MutexGuard {
    #[qjs(skip_trace)]
    permit: Option<OwnedSemaphorePermit>,
}

#[rquickjs::methods]
impl MutexGuard {
    pub fn release(&mut self) {
        self.permit.take();
    }
}

impl Drop for MutexGuard {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::MutexRegistry;
    use crate::script::{bindings::{Button, HostControls, MouseInput, MouseInputError, SharedMouse}, control::SessionControl, session::ScriptSession};
    use std::{cell::RefCell, rc::Rc, time::Duration};

    struct Mouse(tokio::sync::mpsc::UnboundedSender<i32>);

    impl MouseInput for Mouse {
        fn click_at(&mut self, x: i32, _: i32, _: Button) -> Result<(), MouseInputError> {
            self.0.send(x).unwrap();
            Ok(())
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mutex_is_shared_across_runtimes_and_independent_channels() {
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let controls: HostControls = (Rc::new(RefCell::new(Mouse(events))) as SharedMouse).into();
        let session = ScriptSession::new(r#"
            export function afterLoad() {
                rev.daemon('worker', async function() {
                    while (!rev.global.__mutex_ready) await rev.sleep(1);
                    using shared = await rev.mutex('__mutex_shared');
                    const before = rev.global.__mutex_count ?? 0;
                    await rev.sleep(1);
                    rev.global.__mutex_count = before + 1;
                    rev.click(2, 1);
                    shared.release();
                    shared.release();
                });
                rev.daemon('independent', async function() {
                    while (!rev.global.__mutex_ready) await rev.sleep(1);
                    using other = await rev.mutex('__mutex_other');
                    rev.global.__mutex_independent = true;
                    rev.click(3, 1);
                    other.release();
                });
            }
            export default async function() {
                if (rev.global.__mutex_done) {
                    rev.click(rev.global.__mutex_count, 1);
                    delete rev.global.__mutex_ready;
                    delete rev.global.__mutex_independent;
                    delete rev.global.__mutex_count;
                    delete rev.global.__mutex_done;
                    return;
                }
                using shared = await rev.mutex('__mutex_shared');
                rev.global.__mutex_ready = true;
                rev.click(1, 1);
                while (!rev.global.__mutex_independent) await rev.sleep(1);
                const before = rev.global.__mutex_count ?? 0;
                await rev.sleep(1);
                rev.global.__mutex_count = before + 1;
                rev.global.__mutex_done = true;
                shared.release();
                shared.release();
            }
        "#).await.unwrap();
        let mut background = Box::pin(session.drive_background(controls.clone()));
        session.run_after_load(controls.clone()).await.unwrap();
        let invocation = session.invoke((), controls.clone());
        tokio::pin!(invocation);
        let mut observed = Vec::new();
        let mut invocation_complete = false;
        tokio::time::timeout(Duration::from_secs(1), async {
            while observed.len() < 3 {
                tokio::select! {
                    _ = &mut background => panic!("background service stopped unexpectedly"),
                    result = &mut invocation, if !invocation_complete => { result.unwrap(); invocation_complete = true; },
                    event = received.recv() => observed.push(event.unwrap()),
                }
            }
        }).await.unwrap();
        assert_eq!(observed, vec![1, 3, 2]);
        session.invoke((), controls).await.unwrap();
        assert_eq!(tokio::time::timeout(Duration::from_secs(1), received.recv()).await.unwrap().unwrap(), 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mutex_using_releases_after_caught_throw() {
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let controls: HostControls = (Rc::new(RefCell::new(Mouse(events))) as SharedMouse).into();
        let session = ScriptSession::new(r#"
            export default async function() {
                try {
                    using guard = await rev.mutex('__mutex_throw');
                    rev.click(1, 1);
                    throw new Error('expected');
                } catch (_) {}
                using next = await rev.mutex('__mutex_throw');
                rev.click(2, 1);
            }
        "#).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), session.invoke((), controls)).await.unwrap().unwrap();
        assert_eq!(received.try_recv().unwrap(), 1);
        assert_eq!(received.try_recv().unwrap(), 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mutex_close_cancels_polled_waiters_and_rejects_new_channels() {
        use rquickjs::{AsyncContext, AsyncRuntime};
        use std::time::Duration;

        let runtime = AsyncRuntime::new().unwrap();
        let context = AsyncContext::full(&runtime).await.unwrap();
        let registry = Rc::new(MutexRegistry::default());
        let session = SessionControl::standalone();
        context.async_with(async |ctx| {
            let holder = registry.clone().acquire(ctx.clone(), session.clone(), "__mutex_close".to_owned()).await.unwrap();
            let waiting = registry.clone().acquire(ctx.clone(), session.clone(), "__mutex_close".to_owned());
            tokio::pin!(waiting);
            tokio::select! {
                biased;
                _ = &mut waiting => panic!("waiter acquired before close"),
                _ = tokio::task::yield_now() => {}
            }
            registry.close();
            assert!(tokio::time::timeout(Duration::from_secs(1), waiting).await.unwrap().is_err());
            ctx.catch();
            assert!(registry.clone().acquire(ctx, session, "__mutex_new".to_owned()).await.is_err());
            holder.borrow_mut().release();
        }).await;
    }
}
