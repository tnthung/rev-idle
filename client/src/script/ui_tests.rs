use rquickjs::{Context, Runtime};

struct CallbackMouse(std::rc::Rc<std::cell::RefCell<Vec<i32>>>);

impl crate::script::bindings::MouseInput for CallbackMouse {
    fn click_at(&mut self, x: i32, _: i32, _: crate::script::bindings::Button) -> Result<(), crate::script::bindings::MouseInputError> {
        self.0.borrow_mut().push(x);
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn ui_and_detached_jobs_run_during_default_cadence_delay() {
    use crate::{app::{PauseUpdate, ScriptCommand, StateUpdate}, bridge::{QueuedScriptUiEvent, ScriptUiEvent, ScriptUiEventKind, ScriptUiPublisher, WsConnection}, capture::{CaptureState, LockState}, script::{bindings::{HostControls, MouseInput, SharedMouse}, lifecycle::run_with_controls_and_lifecycle_with_control, ScriptControl}};
    use std::{cell::RefCell, rc::Rc, sync::{Arc, atomic::AtomicBool}, time::{Duration, Instant}};
    use tokio::sync::{mpsc, watch};
    struct TimedMouse(Rc<RefCell<Vec<(i32, Instant)>>>);
    impl MouseInput for TimedMouse {
        fn click_at(&mut self, x: i32, _: i32, _: crate::script::bindings::Button) -> Result<(), crate::script::bindings::MouseInputError> {
            self.0.borrow_mut().push((x, Instant::now()));
            Ok(())
        }
    }
    let path = std::env::temp_dir().join(format!("rev-idle-ui-cadence-{}.js", uuid::Uuid::new_v4()));
    std::fs::write(&path, r#"
        export function afterLoad() {
            rev.ui('button', {});
            rev.ui.button.setOnClick(() => { rev.click(7, 7); });
            (async () => { await rev.sleep(10); rev.click(9, 9); })();
        }
        export default function() { rev.click(1, 1); }
    "#).unwrap();
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let controls: HostControls = (Rc::new(RefCell::new(TimedMouse(clicks.clone()))) as SharedMouse).into();
    let (commands, command_rx) = mpsc::channel(8);
    let (events, event_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let (states, _) = watch::channel(StateUpdate::new(false, false, false, false, false));
    let publisher = ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    CaptureState.set_enabled(false);
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_lifecycle_with_control(
            command_rx, event_rx, WsConnection::disconnected_for_test(), pause_rx, Some(path.clone()), controls,
            Duration::from_millis(500), shutdown_rx, Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)),
            CaptureState, LockState::default(), None, states, ScriptControl::default(), publisher,
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            while !clicks.borrow().iter().any(|(id, _)| *id == 1) { tokio::task::yield_now().await; }
        }).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if snapshots.borrow().elements[0].events.iter().any(|event| matches!(event, ScriptUiEventKind::Click)) { break; }
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
        let snapshot = snapshots.borrow().clone();
        events.send(QueuedScriptUiEvent {
            event: ScriptUiEvent {
                session_id: snapshot.session_id.unwrap(), element_id: "button".to_owned(),
                instance_id: snapshot.elements[0].instance_id, events_version: snapshot.elements[0].events_version,
                event: ScriptUiEventKind::Click,
            },
            generation: 0,
            capture_epoch: CaptureState.epoch(),
        }).await.unwrap();
        tokio::time::timeout(Duration::from_millis(250), async {
            while ![7, 9].into_iter().all(|expected| clicks.borrow().iter().any(|(id, _)| *id == expected)) {
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
        assert_eq!(clicks.borrow().iter().filter(|(id, _)| *id == 1).count(), 1);
        tokio::time::timeout(Duration::from_secs(1), async {
            while clicks.borrow().iter().filter(|(id, _)| *id == 1).count() < 2 { tokio::task::yield_now().await; }
        }).await.unwrap();
        let defaults: Vec<Instant> = clicks.borrow().iter().filter_map(|(id, time)| (*id == 1).then_some(*time)).collect();
        assert!(defaults[1].duration_since(defaults[0]) >= Duration::from_millis(500));
        commands.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
    }).await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn ui_callback_runs_while_main_awaits_and_while_paused() {
    use crate::{bridge::{ScriptUiEvent, ScriptUiEventKind, ScriptUiPublisher, WsConnection}, script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession}};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(clicks.clone()))) as SharedMouse).into();
    let publisher = ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(r#"
        export function afterLoad() {
            rev.ui('button', { text: 'ready' });
            rev.ui.button.setOnClick(function() { rev.pause(); rev.click(7, 1); this.text = 'callback'; rev.global.released = true; });
            rev.ui('release', {});
            rev.ui.release.setOnClick(function() { rev.resume(); rev.global.released = true; });
        }
        export default async function() {
            rev.ui.button.text = 'waiting';
            while (!rev.global.released) await rev.sleep(1);
            rev.ui.button.text += '-default';
        }
    "#, "callback.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
    let mut background = Box::pin(session.drive_background(controls.clone()));
    tokio::select! {
        _ = &mut background => panic!("background service stopped during afterLoad"),
        result = session.run_after_load(controls.clone()) => result.unwrap(),
    }
    let invocation = session.invoke((), controls.clone());
    tokio::pin!(invocation);
    assert!(tokio::time::timeout(Duration::from_millis(10), &mut invocation).await.is_err());
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if snapshots.borrow().elements.iter().all(|element| element.events.iter().any(|event| matches!(event, crate::bridge::ScriptUiEventKind::Click))) { break; }
            tokio::select! { _ = &mut background => panic!("background service stopped installing handlers"), _ = tokio::task::yield_now() => {} }
        }
    }).await.unwrap();
    let snapshot = snapshots.borrow().clone();
    tokio::select! {
        _ = &mut background => panic!("background service stopped during callback"),
        result = session.dispatch_ui_event(ScriptUiEvent {
            session_id: snapshot.session_id.unwrap(), element_id: "button".to_owned(),
            instance_id: snapshot.elements[0].instance_id, events_version: snapshot.elements[0].events_version,
            event: ScriptUiEventKind::Click,
        }, controls.clone()) => result.unwrap(),
    }
    assert_eq!(*clicks.borrow(), vec![7]);
    assert_eq!(snapshots.borrow().elements[0].text, "callback");
    assert!(controls.actions_paused.is_paused());
    session.acknowledge_pause(true);
    tokio::select! {
        _ = &mut background => panic!("background service stopped during release"),
        result = session.dispatch_ui_event(ScriptUiEvent {
            session_id: snapshot.session_id.unwrap(), element_id: "release".to_owned(),
            instance_id: snapshot.elements[1].instance_id, events_version: snapshot.elements[1].events_version,
            event: ScriptUiEventKind::Click,
        }, controls.clone()) => result.unwrap(),
    }
    assert!(!controls.actions_paused.is_paused());
    tokio::time::timeout(Duration::from_secs(1), invocation).await.unwrap().unwrap();
    assert_eq!(snapshots.borrow().elements[0].text, "callback-default");
}

#[tokio::test]
async fn ui_callbacks_overlap_after_await() {
    use crate::{bridge::{ScriptUiEvent, ScriptUiEventKind, ScriptUiPublisher, WsConnection}, script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession}};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(clicks.clone()))) as SharedMouse).into();
    let publisher = ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(r#"
        export function afterLoad() {
            rev.ui('button', {});
            rev.ui.button.setOnClick(async function() {
                const id = (this.states.count ?? 0) + 1;
                this.states.count = id;
                rev.click(id, 1);
                if (id === 1) { while (!this.states.release) await rev.sleep(1); }
                else { await rev.sleep(1); this.states.release = true; rev.click(-id, 1); return; }
                rev.click(-id, 1);
            });
        }
        export default function() {}
    "#, "callback.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
    let mut background = Box::pin(session.drive_background(controls.clone()));
    tokio::select! {
        _ = &mut background => panic!("background service stopped during afterLoad"),
        result = session.run_after_load(controls.clone()) => result.unwrap(),
    }
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if snapshots.borrow().elements[0].events.iter().any(|event| matches!(event, crate::bridge::ScriptUiEventKind::Click)) { break; }
            tokio::select! { _ = &mut background => panic!("background service stopped installing handler"), _ = tokio::task::yield_now() => {} }
        }
    }).await.unwrap();
    let snapshot = snapshots.borrow().clone();
    let event = ScriptUiEvent {
        session_id: snapshot.session_id.unwrap(), element_id: "button".to_owned(),
        instance_id: snapshot.elements[0].instance_id, events_version: snapshot.elements[0].events_version,
        event: ScriptUiEventKind::Click,
    };
    let (first, second) = tokio::time::timeout(Duration::from_secs(1), async {
        tokio::select! {
            _ = &mut background => panic!("background service stopped during overlap"),
            result = async {
                tokio::join!(session.dispatch_ui_event(event.clone(), controls.clone()), session.dispatch_ui_event(event, controls))
            } => result,
        }
    }).await.unwrap();
    first.unwrap(); second.unwrap();
    assert_eq!(*clicks.borrow(), vec![1, 2, -2, -1]);
}

#[tokio::test]
async fn ui_callback_error_is_isolated() {
    use crate::{bridge::{ScriptUiEvent, ScriptUiEventKind, ScriptUiPublisher, WsConnection}, script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession}};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(clicks.clone()))) as SharedMouse).into();
    let publisher = ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(r#"
        export function afterLoad() {
            rev.ui('sync', {});
            rev.ui.sync.setOnClick(() => { throw new Error('sync broken'); });
            rev.ui('async', {});
            rev.ui.async.setOnClick(async () => { await rev.sleep(1); throw new Error('async broken'); });
            rev.ui('good', {});
            rev.ui.good.setOnClick(() => { rev.click(9, 1); });
        }
        export default function() { rev.click(10, 1); }
    "#, "callback.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
    let mut background = Box::pin(session.drive_background(controls.clone()));
    tokio::select! {
        _ = &mut background => panic!("background service stopped during afterLoad"),
        result = session.run_after_load(controls.clone()) => result.unwrap(),
    }
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if snapshots.borrow().elements.iter().all(|element| element.events.iter().any(|event| matches!(event, crate::bridge::ScriptUiEventKind::Click))) { break; }
            tokio::select! { _ = &mut background => panic!("background service stopped installing handlers"), _ = tokio::task::yield_now() => {} }
        }
    }).await.unwrap();
    let snapshot = snapshots.borrow().clone();
    for element in &snapshot.elements {
        let result = tokio::select! {
            _ = &mut background => panic!("background service stopped during error isolation"),
            result = session.dispatch_ui_event(ScriptUiEvent {
                session_id: snapshot.session_id.unwrap(), element_id: element.id.clone(),
                instance_id: element.instance_id, events_version: element.events_version, event: ScriptUiEventKind::Click,
            }, controls.clone()) => result,
        };
        if element.id == "good" { result.unwrap(); }
        else { assert!(result.unwrap_err().contains(&format!("{} broken", element.id))); }
    }
    session.invoke((), controls).await.unwrap();
    assert_eq!(*clicks.borrow(), vec![9, 10]);
    assert!(!session.is_stopped());
}

#[tokio::test]
async fn ui_state_update_observes_shallow_changes_from_both_runtimes() {
    use crate::{bridge::{ScriptUiPublisher, WsConnection}, script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession}};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(Rc::new(RefCell::new(Vec::new()))))) as SharedMouse).into();
    let publisher = ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(r#"
        export async function afterLoad() {
            rev.global.stateUpdateCount = 0;
            const element = rev.ui('counter', { states: { count: 0, options: { enabled: false } } });
            if (typeof element.setOnStateUpdate !== 'function') throw new Error('missing state update setter');
            if (element.setOnStateUpdate(async function() {
                if (this !== rev.ui.counter) throw new Error('incorrect callback receiver');
                await rev.sleep(1);
                rev.global.stateUpdateCount++;
                this.text = String(rev.global.stateUpdateCount);
            }) !== element) throw new Error('setter must return element');
            element.states.count = 1;
            element.states.count = 1;
            element.states.options.enabled = true;
            if (element.states.options.enabled !== false) throw new Error('nested edit changed stored state');
            element.states.options = { enabled: false };
            element.states.options = { enabled: true };
            delete element.states.missing;
            delete element.states.count;
            element.states = { count: 2 };
            element.states = { count: 2 };
            rev.ui('counter', { states: { count: 3 } });
            rev.ui('counter', { states: { count: 3 } });
            try { rev.ui('counter', { states: { count: 4 }, size: 0 }); } catch (_) {}
            delete element.states;
            delete element.states;
            if (rev.global.stateUpdateCount !== 0) throw new Error('callback ran synchronously');
            while (rev.global.stateUpdateCount < 6) await rev.sleep(1);
            rev.daemon('update', function() { rev.ui.counter.states.background = true; });
            while (rev.global.stateUpdateCount < 7) await rev.sleep(1);
            await rev.sleep(20);
            if (rev.global.stateUpdateCount !== 7) throw new Error('unexpected state update count: ' + rev.global.stateUpdateCount);
        }
        export default function() {}
    "#, "state_update.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = session.drive_background(controls.clone()) => panic!("background service stopped during state updates"),
            result = session.run_after_load(controls) => result.unwrap(),
        }
    }).await.unwrap();
    assert_eq!(snapshots.borrow().elements[0].text, "7");
    assert!(snapshots.borrow().elements[0].events.is_empty());
}

#[tokio::test]
async fn ui_update_triggers_callbacks_without_state_changes() {
    use crate::{bridge::{ScriptUiPublisher, WsConnection}, script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession}};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(Rc::new(RefCell::new(Vec::new()))))) as SharedMouse).into();
    let publisher = ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(r#"
        export async function afterLoad() {
            rev.global.forcedStateUpdateCount = 0;
            const element = rev.ui('counter', { states: { count: 7 } });
            if (typeof element.update !== 'function') throw new Error('missing update method');
            if (element.update() !== element) throw new Error('update must return element');
            element.setOnStateUpdate(async function() {
                if (this !== rev.ui.counter || this.states.count !== 7) throw new Error('incorrect callback receiver or state');
                await rev.sleep(1);
                rev.global.forcedStateUpdateCount++;
                this.text = String(rev.global.forcedStateUpdateCount);
            }).update().update();
            if (rev.global.forcedStateUpdateCount !== 0) throw new Error('callback ran synchronously');
            while (rev.global.forcedStateUpdateCount < 2) await rev.sleep(1);
            rev.daemon('update', function() { rev.ui.counter.update(); });
            while (rev.global.forcedStateUpdateCount < 3) await rev.sleep(1);
            element.setOnStateUpdate(null).update();
            await rev.sleep(20);
            if (rev.global.forcedStateUpdateCount !== 3) throw new Error('unexpected update count');
            const update = element.update;
            rev.ui('counter', null);
            rev.ui('counter', { text: 'replacement' });
            let rejected = false;
            try { update(); } catch (_) { rejected = true; }
            if (!rejected) throw new Error('stale update method accepted');
        }
        export default function() {}
    "#, "forced_state_update.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = session.drive_background(controls.clone()) => panic!("background service stopped during forced state updates"),
            result = session.run_after_load(controls) => result.unwrap(),
        }
    }).await.unwrap();
    assert_eq!(snapshots.borrow().elements[0].text, "replacement");
}

#[tokio::test]
async fn ui_native_numbers_preserve_callback_types_and_value_equality() {
    use crate::{bridge::{ScriptUiPublisher, WsConnection}, script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession}};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(Rc::new(RefCell::new(Vec::new()))))) as SharedMouse).into();
    let session = ScriptSession::new_with_connection_and_control(r#"
        export async function afterLoad() {
            rev.global.__nativeUiUpdates = 0;
            const element = rev.ui('native', { states: { amount: { a: '1e3', b: BigNum.ONE } } });
            element.setOnStateUpdate(function() {
                if (!(this.states.amount.a instanceof BigNum) || !this.states.amount.a.eq(1000)) throw Error('native callback state lost');
                rev.global.__nativeUiUpdates++;
            });
            element.states.amount = { a: new BigNum(1000), b: BigNum.ONE };
            element.states.amount = { b: BigNum.ONE, a: new BigNum(1000) };
            element.states = { amount: { a: new BigNum(1000), b: BigNum.ONE } };
            while (rev.global.__nativeUiUpdates < 1) await rev.sleep(1);
            await rev.sleep(20);
            if (rev.global.__nativeUiUpdates !== 1) throw Error('native equality changed');
            delete rev.global.__nativeUiUpdates;
        }
        export default function() {}
    "#, "native_ui_state.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), ScriptUiPublisher::default()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = session.drive_background(controls.clone()) => panic!("background service stopped during native state update"),
            result = session.run_after_load(controls) => result.unwrap(),
        }
    }).await.unwrap();
}

#[tokio::test]
async fn ui_state_update_discards_cleared_replaced_and_removed_handlers() {
    use crate::{bridge::{ScriptUiPublisher, WsConnection}, script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession}};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(Rc::new(RefCell::new(Vec::new()))))) as SharedMouse).into();
    let publisher = ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(r#"
        export async function afterLoad() {
            rev.global.stateLifecycleUnexpected = 0;
            rev.global.stateLifecycleCount = 0;
            const element = rev.ui('counter', {});
            if (typeof element.setOnStateUpdate !== 'function') throw new Error('missing state update setter');
            const obsolete = function() { rev.global.stateLifecycleUnexpected++; };
            element.setOnStateUpdate(obsolete);
            element.states.count = 1;
            element.setOnStateUpdate(null);
            element.states.count = 2;
            element.setOnStateUpdate(obsolete);
            element.states.count = 3;
            const current = function() { rev.global.stateLifecycleCount++; this.text = 'updated'; };
            element.setOnStateUpdate(current);
            element.states.count = 4;
            element.setOnStateUpdate(current);
            while (rev.global.stateLifecycleCount < 1) await rev.sleep(1);
            element.states.count = 5;
            rev.ui('counter', null);
            rev.ui('counter', { text: 'replacement' }).setOnStateUpdate(function() {
                rev.global.stateLifecycleCount++;
                this.text = 'replacement updated';
            });
            rev.ui.counter.states.count = 6;
            while (rev.global.stateLifecycleCount < 2) await rev.sleep(1);
            rev.ui.counter.setOnStateUpdate(null);
            rev.ui.counter.states.count = 7;
            await rev.sleep(20);
            if (rev.global.stateLifecycleUnexpected !== 0 || rev.global.stateLifecycleCount !== 2) throw new Error('obsolete state handler ran');
        }
        export default function() {}
    "#, "state_update_lifecycle.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = session.drive_background(controls.clone()) => panic!("background service stopped during state updates"),
            result = session.run_after_load(controls) => result.unwrap(),
        }
    }).await.unwrap();
    assert_eq!(snapshots.borrow().elements[0].text, "replacement updated");
}

#[tokio::test]
async fn ui_state_update_runs_while_paused_and_isolates_errors() {
    use crate::{bridge::{ScriptUiPublisher, WsConnection}, script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession}};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(Rc::new(RefCell::new(Vec::new()))))) as SharedMouse).into();
    let publisher = ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(r#"
        export function afterLoad() {
            rev.ui('sync', {}).setOnStateUpdate(function() {
                this.text = 'ran';
                throw new Error('sync state update failure');
            });
            rev.ui('async', {}).setOnStateUpdate(async function() {
                await rev.sleep(1);
                this.text = 'ran';
                throw new Error('async state update failure');
            });
            rev.ui('good', {}).setOnStateUpdate(async function() {
                await rev.sleep(1);
                if (this.states.count === 1) {
                    this.states.count = 2;
                } else {
                    this.setOnStateUpdate(null);
                    this.states.count = 3;
                    this.text = 'done';
                }
            });
            rev.ui.sync.states.count = 1;
            rev.ui.async.states.count = 1;
            rev.ui.good.states.count = 1;
        }
        export default function() {}
    "#, "state_update_errors.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
    session.run_after_load(controls.clone()).await.unwrap();
    session.acknowledge_pause(true);
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = session.drive_background(controls) => panic!("background service stopped during state updates"),
            _ = async {
                while snapshots.borrow().elements.iter().any(|element| element.text.is_empty()) {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            } => {}
        }
    }).await.unwrap();
    assert_eq!(snapshots.borrow().elements.iter().map(|element| element.text.as_str()).collect::<Vec<_>>(), ["ran", "ran", "done"]);
    assert!(!session.is_stopped());
    assert!(session.is_paused());
}

#[test]
fn ui_registry_native_roots_drop_before_runtime() {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    let publisher = crate::bridge::ScriptUiPublisher::default();
    let session = crate::script::control::SessionControl::standalone();
    let state = std::rc::Rc::new(super::ScriptUiState::new(publisher, session.clone()));
    let background = std::rc::Rc::new(crate::script::background::BackgroundRegistry::new(session.clone()));
    context.with(|ctx| {
        let transfer = std::rc::Rc::new(crate::script::transfer::FunctionTransfer::new(&ctx).unwrap());
        let codec = crate::script::value::codec(&ctx).unwrap();
        let ui = super::ScriptUiBindings::new(
            &ctx,
            codec,
            state.clone(),
            crate::bridge::WsConnection::disconnected_for_test(),
            session.clone(),
            transfer,
            background.clone(),
        ).unwrap();
        ctx.globals().set("ui", ui.registry(&ctx).unwrap()).unwrap();
        ctx.eval::<(), _>("ui('a', {text: 'root'});").unwrap();
        drop(ui);
    });
    drop(state);
    drop(context);
    drop(runtime);
}

#[tokio::test]
async fn ui_registry_survives_invocations_and_pause() {
    use crate::script::{bindings::{Button, HostControls, MouseInput, SharedMouse}, control::SessionControl, session::ScriptSession};
    use std::{cell::RefCell, rc::Rc};
    struct Mouse;
    impl MouseInput for Mouse {
        fn click_at(&mut self, _: i32, _: i32, _: Button) -> Result<(), crate::script::bindings::MouseInputError> { panic!("no input expected"); }
    }
    let controls: HostControls = (Rc::new(RefCell::new(Mouse)) as SharedMouse).into();
    let publisher = crate::bridge::ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(
        r#"
            let original;
            export function afterLoad() { rev.ui('a', { text: 'loaded' }); original = rev.ui.a; }
            export default function() {
                if (rev.ui.a !== original) throw new Error('registry identity changed');
                rev.ui.a.text = rev.paused ? 'paused' : 'running';
            }
        "#,
        "ui-test.js", crate::bridge::WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher,
    ).await.unwrap();
    session.run_after_load(controls.clone()).await.unwrap();
    let instance = snapshots.borrow().elements[0].instance_id;
    session.invoke((), controls.clone()).await.unwrap();
    controls.actions_paused.set_paused(true);
    session.acknowledge_pause(true);
    session.invoke((), controls).await.unwrap();
    assert_eq!(snapshots.borrow().elements[0].instance_id, instance);
    assert_eq!(snapshots.borrow().elements[0].text, "paused");
    drop(session);
    assert_eq!(snapshots.borrow().session_id, None);
}

#[tokio::test]
async fn script_ui_same_value_writes_do_not_publish() {
    use crate::script::{bindings::{Button, HostControls, MouseInput, SharedMouse}, control::SessionControl, session::ScriptSession};
    use futures_util::StreamExt;
    use std::{cell::RefCell, rc::Rc, time::Duration};
    struct Mouse;
    impl MouseInput for Mouse {
        fn click_at(&mut self, _: i32, _: i32, _: Button) -> Result<(), crate::script::bindings::MouseInputError> { panic!("no input expected"); }
    }
    let controls: HostControls = (Rc::new(RefCell::new(Mouse)) as SharedMouse).into();
    let (address, server) = crate::bridge::test_support::raw_server().await;
    let connection = crate::bridge::WsConnection::connect_for_test(address, Duration::from_millis(10), Duration::from_secs(1));
    let publisher = crate::bridge::ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(
        r#"
            export default function() {
                if (!rev.ui.a) rev.ui('a', { text: 'same', color: [1,2,3] });
                rev.ui.a.text = rev.paused ? 'paused' : 'same';
                rev.ui.a.color = [1,2,3];
            }
        "#,
        "ui-test.js", connection.clone(), SessionControl::standalone(), publisher,
    ).await.unwrap();
    session.invoke((), controls.clone()).await.unwrap();
    let revision = snapshots.borrow().revision;
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(crate::bridge::publish_script_ui(connection.clone(), snapshots.clone(), shutdown_rx));
    let mut socket = server.await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), socket.next()).await.unwrap().unwrap().unwrap();
    for _ in 0..10 { session.invoke((), controls.clone()).await.unwrap(); }
    assert_eq!(snapshots.borrow().revision, revision);
    assert!(tokio::time::timeout(Duration::from_millis(40), socket.next()).await.is_err());
    controls.actions_paused.set_paused(true);
    session.acknowledge_pause(true);
    session.invoke((), controls).await.unwrap();
    let packet = tokio::time::timeout(Duration::from_secs(1), socket.next()).await.unwrap().unwrap().unwrap();
    let packet: serde_json::Value = serde_json::from_str(packet.to_text().unwrap()).unwrap();
    assert_eq!(packet["payload"]["elements"][0]["text"], "paused");
    drop(session);
    shutdown.send(true).unwrap();
    task.await.unwrap();
    connection.shutdown().await;
}

fn check(source: &str) {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    let publisher = crate::bridge::ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = crate::script::control::SessionControl::standalone();
    let state = std::rc::Rc::new(super::ScriptUiState::new(publisher, session.clone()));
    let background = std::rc::Rc::new(crate::script::background::BackgroundRegistry::new(session.clone()));
    context.with(|ctx| {
        crate::script::bignum::install(&ctx).unwrap();
        let transfer = std::rc::Rc::new(crate::script::transfer::FunctionTransfer::new(&ctx).unwrap());
        let codec = crate::script::value::codec(&ctx).unwrap();
        let keep_transfer = transfer.clone();
        let ui = super::ScriptUiBindings::new(
            &ctx,
            codec,
            state,
            crate::bridge::WsConnection::disconnected_for_test(),
            session,
            transfer,
            background,
        ).unwrap();
        ctx.globals().set("ui", ui.registry(&ctx).unwrap()).unwrap();
        ctx.globals().set("snapshot", rquickjs::Function::new(ctx.clone(), move || {
            serde_json::to_string(&*snapshots.borrow()).map_err(|error| rquickjs::Error::new_from_js_message("UI", "snapshot", error.to_string()))
        }).unwrap()).unwrap();
        let result = ctx.eval::<(), _>(format!(r#"
            const snapshots = {{ at(index) {{ return JSON.parse(snapshot()).elements; }}, get length() {{ return JSON.parse(snapshot()).revision; }} }};
            function assert(condition, message = 'assertion failed') {{ if (!condition) throw new Error(message); }}
            function throws(action) {{ let thrown = false; try {{ action(); }} catch (_) {{ thrown = true; }} assert(thrown, 'expected rejection'); }}
            {}
        "#, source));
        std::hint::black_box(&keep_transfer);
        result.unwrap_or_else(|error| {
            panic!("{}", rquickjs::CaughtError::from_error(&ctx, error));
        });
    });
}

#[test]
fn ui_registry_create_update_delete() {
    check(r#"
        assert(typeof ui === 'function');
        assert(ui.missing === undefined);
        const old = ui('a', { text: 'first', posX: 20 }); ui('b', {});
        ui.a.text = 'changed'; assert(ui.a === old && old.text === 'changed');
        assert(ui('a', { text: 'updated' }) === old);
        assert(old.text === 'updated' && old.posX === 20);
        assert(Object.keys(ui).join(',') === 'a,b');
        throws(() => ui.a = { text: 'replacement' });
        throws(() => ui.other = {});
        throws(() => delete ui.a);
        throws(() => delete ui.missing);
        assert(ui.a === old && Object.keys(ui).join(',') === 'a,b');
        assert(Object.getOwnPropertyDescriptor(ui, 'a').writable === false);
        assert(ui('a', null) === undefined && ui.a === undefined);
        const revision = snapshots.length;
        assert(ui('a', null) === undefined && snapshots.length === revision);
        throws(() => old.text = 'stale');
        assert(ui('a', {}) !== old); assert(Object.keys(ui).join(',') === 'b,a');
        ui('__proto__', { text: 'prototype key' }); ui('constructor', {});
        ui('name', { text: 'name key' }); ui('length', {}); ui('prototype', {});
        assert(ui.__proto__.text === 'prototype key' && Object.getPrototypeOf(ui) === null);
        assert(ui.name.text === 'name key' && Object.keys(ui).join(',') === 'b,a,__proto__,constructor,name,length,prototype');
        throws(() => ui('', {})); throws(() => ui(1, {}));
        throws(() => ui('', null)); throws(() => ui(1, null));
    "#);
}

#[test]
fn ui_registry_updates_attributes_atomically() {
    check(r#"
        const callback = function() {};
        const original = ui('a', { text: 'before', padding: { left: 4, right: 5 }, states: { count: 2 } })
            .setOnClick(callback).setOnHover(callback).setOnLeave(callback).setOnStateUpdate(callback);
        assert(original === ui.a && original.setOnClick(callback) === original);
        const states = original.states, first = snapshots.at(-1)[0], count = snapshots.length;
        throws(() => ui('a', { text: 'invalid', size: 0, states: {} }));
        assert(original.text === 'before' && original.states.count === 2 && snapshots.length === count);
        assert(ui('a', { text: 'after', padding: { top: 3 } }) === original);
        assert(original.states === states && states.count === 2 && original.padding.left === undefined);
        assert(snapshots.length === count + 1 && snapshots.at(-1)[0].padding.top === 3 && snapshots.at(-1)[0].padding.left === 0);
        assert(snapshots.at(-1)[0].instanceId === first.instanceId && snapshots.at(-1)[0].eventsVersion === first.eventsVersion);
        ui('a', { text: 'after', padding: { top: 3 }, states: { enabled: true } });
        assert(states.enabled === true && states.count === undefined && snapshots.length === count + 1);
        ui('a', {});
        assert(original.text === 'after' && states.enabled === true && snapshots.length === count + 1);
        assert(original.setOnClick(null).setOnHover(null).setOnLeave(null).setOnStateUpdate(null) === original);
    "#);
}

#[test]
fn ui_dimensions_are_readonly_methods_not_definition_fields() {
    check(r#"
        ui('a', { text: 'measure me' });
        assert(typeof ui.a.width === 'function' && typeof ui.a.height === 'function');
        assert(typeof ui.a.globalXPos === 'function' && typeof ui.a.globalYPos === 'function');
        assert(ui.a.width === ui.a.width && 'width' in ui.a && 'height' in ui.a);
        assert(Object.keys(ui.a).join() === 'text,states');
        throws(() => ui.a.width = () => 10);
        throws(() => delete ui.a.height);
        throws(() => ui('b', { width: () => 10 }));
        assert(snapshots.at(-1).length === 1);
    "#);
}

#[tokio::test]
async fn ui_dimensions_request_current_revision_and_reject_stale_elements() {
    use crate::script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession};
    use futures_util::{SinkExt, StreamExt};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    use tokio_tungstenite::tungstenite::Message;

    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(Rc::new(RefCell::new(Vec::new()))))) as SharedMouse).into();
    controls.actions_paused.set_paused(true);
    let (address, server) = crate::bridge::test_support::raw_server().await;
    let connection = crate::bridge::WsConnection::connect_for_test(address, Duration::from_millis(10), Duration::from_secs(1));
    let publisher = crate::bridge::ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(
        r#"
            export default async function() {
                rev.ui('a', { text: 'initial', basedOn: 'scene:7/Panel[0]', hidden: true, padding: { thickness: 3 }, border: { thickness: 9 } });
                rev.ui.a.text = 'updated';
                const original = rev.ui.a;
                if (await original.width() !== 123.5 || await original.height() !== 27.25)
                    throw new Error('wrong dimensions');
                rev.ui('a', null);
                async function rejects(read) {
                    try { await read(); } catch (error) {
                        if (error.message.includes('no longer exists')) return;
                        throw error;
                    }
                    throw new Error('stale measurement accepted');
                }
                await rejects(() => original.width());
                rev.ui('a', { text: 'replacement' });
                await rejects(() => original.height());
                const pending = rev.ui.a.width();
                await rev.sleep(0);
                rev.ui('a', null);
                rev.ui('a', { text: 'newer replacement' });
                await rejects(() => pending);
            }
        "#,
        "ui-dimensions.js", connection.clone(), SessionControl::standalone(), publisher,
    ).await.unwrap();
    let mut socket = server.await.unwrap();
    tokio::task::yield_now().await;
    let peer = async {
        for index in 0..3 {
            let message = tokio::time::timeout(Duration::from_secs(2), socket.next()).await.unwrap().unwrap().unwrap();
            let envelope: serde_json::Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
            assert_eq!(envelope["type"], "ScriptUiMeasureReq");
            assert_eq!(envelope["payload"]["elementId"], "a");
            if index == 2 {
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if snapshots.borrow().elements.first().map(|element| element.text.as_str()) == Some("newer replacement") { break; }
                        tokio::task::yield_now().await;
                    }
                }).await.unwrap();
            }
            let snapshot = snapshots.borrow();
            assert_eq!(envelope["payload"]["sessionId"], snapshot.session_id.unwrap().to_string());
            if index < 2 {
                assert_eq!(snapshot.elements[0].text, "updated");
                assert_eq!(envelope["payload"]["revision"], snapshot.revision);
                assert_eq!(envelope["payload"]["instanceId"], snapshot.elements[0].instance_id.to_string());
            } else if !snapshot.elements.is_empty() {
                assert!(envelope["payload"]["revision"].as_u64().unwrap() < snapshot.revision);
                assert_ne!(envelope["payload"]["instanceId"], snapshot.elements[0].instance_id.to_string());
            }
            drop(snapshot);
            socket.send(Message::Text(serde_json::json!({
                "uuid": envelope["uuid"], "type": "ScriptUiMeasureRes", "payload": {
                    "width": 123.5, "height": 27.25,
                    "globalX": serde_json::Value::Null,
                    "globalY": serde_json::Value::Null,
                },
            }).to_string().into())).await.unwrap();
        }
    };
    let (result, ()) = tokio::join!(session.invoke((), controls), peer);
    result.unwrap();
    connection.shutdown().await;
}

#[tokio::test]
async fn ui_global_position_getters_preserve_dimensions_when_mount_is_inaccessible() {
    use crate::script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession};
    use futures_util::{SinkExt, StreamExt};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    use tokio_tungstenite::tungstenite::Message;

    let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(Rc::new(RefCell::new(Vec::new()))))) as SharedMouse).into();
    controls.actions_paused.set_paused(true);
    let (address, server) = crate::bridge::test_support::raw_server().await;
    let connection = crate::bridge::WsConnection::connect_for_test(address, Duration::from_millis(10), Duration::from_secs(1));
    let publisher = crate::bridge::ScriptUiPublisher::default();
    let snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(
        r#"
            export default async function() {
                rev.ui('a', { basedOn: 'scene:7/Panel[0]' });
                try { await rev.ui.a.globalXPos(1); throw new Error('invalid relativeTo accepted'); }
                catch (error) { if (!error.message.includes('relativeTo must be a string')) throw error; }
                if ((await rev.ui.a.globalXPos()).join() !== '321.5,-1475' ||
                    (await rev.ui.a.globalYPos('')).join() !== '654.25,-398.5' ||
                    (await rev.ui.a.globalXPos('scene:7/Reference[0]')).join() !== '20,-230' ||
                    (await rev.ui.a.globalYPos('scene:7/Reference[0]')).join() !== '30,-170')
                    throw new Error('wrong global position');
                try { await rev.ui.a.globalXPos('scene:missing'); throw new Error('inaccessible relativeTo accepted'); }
                catch (error) { if (!error.message.includes('relativeTo target is inaccessible')) throw error; }
                try { await rev.ui.a.globalXPos(); throw new Error('inaccessible position accepted'); }
                catch (error) { if (!error.message.includes('basedOn or relativeTo target is inaccessible')) throw error; }
                if (await rev.ui.a.width() !== 123.5 || await rev.ui.a.height() !== 27.25)
                    throw new Error('dimensions rejected with inaccessible position');
            }
        "#,
        "ui-global-position-getters.js", connection.clone(), SessionControl::standalone(), publisher,
    ).await.unwrap();
    let mut socket = server.await.unwrap();
    let peer = async {
        for index in 0..8 {
            let message = tokio::time::timeout(Duration::from_secs(2), socket.next()).await.unwrap().unwrap().unwrap();
            let envelope: serde_json::Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
            assert_eq!(envelope["type"], "ScriptUiMeasureReq");
            let snapshot = snapshots.borrow();
            assert_eq!(envelope["payload"]["sessionId"], snapshot.session_id.unwrap().to_string());
            assert_eq!(envelope["payload"]["revision"], snapshot.revision);
            assert_eq!(envelope["payload"]["relativeTo"], match index {
                0 | 1 => "",
                2 | 3 => "scene:7/Reference[0]",
                4 => "scene:missing",
                _ => "",
            });
            assert_eq!(envelope["payload"]["instanceId"], snapshot.elements[0].instance_id.to_string());
            drop(snapshot);
            socket.send(Message::Text(serde_json::json!({
                "uuid": envelope["uuid"], "type": "ScriptUiMeasureRes", "payload": {
                    "width": 123.5, "height": 27.25,
                    "globalX": match index {
                        0 => serde_json::json!([321.5, -1475.0]),
                        2 => serde_json::json!([20.0, -230.0]),
                        _ => serde_json::Value::Null,
                    },
                    "globalY": match index {
                        1 => serde_json::json!([654.25, -398.5]),
                        3 => serde_json::json!([30.0, -170.0]),
                        _ => serde_json::Value::Null,
                    },
                },
            }).to_string().into())).await.unwrap();
        }
    };
    let (result, ()) = tokio::join!(session.invoke((), controls), peer);
    result.unwrap();
    connection.shutdown().await;
}

#[test]
fn ui_registry_font_updates_and_resets_without_replacing_element() {
    check(r#"
        ui('a', { text: 'default' }); ui.a.setOnClick(() => {});
        const original = ui.a, first = snapshots.at(-1)[0];
        assert(ui.a.font === '' && first.font === '');
        ui.a.font = 'Consolas';
        assert(ui.a === original && ui.a.font === 'Consolas');
        assert(snapshots.at(-1)[0].font === 'Consolas');
        assert(snapshots.at(-1)[0].instanceId === first.instanceId && snapshots.at(-1)[0].eventsVersion === first.eventsVersion);
        const count = snapshots.length;
        ui.a.font = 'Consolas';
        for (const value of [null, undefined, 14, {}, [], new String('Arial')]) {
            throws(() => ui.a.font = value);
            throws(() => ui('a', { text: 'invalid replacement', font: value }));
        }
        assert(ui.a === original && ui.a.font === 'Consolas' && snapshots.length === count);
        delete ui.a.font;
        assert(ui.a.font === '' && snapshots.at(-1)[0].font === '');
        const resetCount = snapshots.length;
        delete ui.a.font; ui.a.font = '';
        assert(snapshots.length === resetCount);
        ui('b', { text: 'named', font: 'Noto Sans CJK TC' });
        assert(snapshots.at(-1)[1].font === 'Noto Sans CJK TC');
    "#);
}

#[test]
fn ui_registry_size_updates_and_resets_without_replacing_element() {
    check(r#"
        ui('a', { text: 'default' }); ui.a.setOnClick(() => {});
        const original = ui.a, first = snapshots.at(-1)[0];
        assert(ui.a.size === 14 && first.size === 14);
        ui.a.size = 24;
        assert(ui.a === original && ui.a.size === 24);
        assert(snapshots.at(-1)[0].size === 24);
        assert(snapshots.at(-1)[0].instanceId === first.instanceId && snapshots.at(-1)[0].eventsVersion === first.eventsVersion);
        const count = snapshots.length;
        ui.a.size = 24;
        for (const value of [null, undefined, 0, -1, 1.5, Infinity, 2147483648, '14', {}, [], new Number(14)]) {
            throws(() => ui.a.size = value);
            throws(() => ui('a', { text: 'invalid replacement', size: value }));
        }
        assert(ui.a === original && ui.a.size === 24 && snapshots.length === count);
        delete ui.a.size;
        assert(ui.a.size === 14 && snapshots.at(-1)[0].size === 14);
        const resetCount = snapshots.length;
        delete ui.a.size; ui.a.size = 14;
        assert(snapshots.length === resetCount);
        ui('b', { text: 'large', size: 32 });
        assert(snapshots.at(-1)[1].size === 32);
    "#);
}

#[test]
fn ui_registry_hidden_defaults_updates_and_remains_reactive() {
    check(r#"
        ui('a', { text: 'visible' }); ui.a.setOnClick(() => {});
        const original = ui.a, first = snapshots.at(-1)[0];
        assert(ui.a.hidden === false && first.hidden === false);
        ui.a.hidden = true;
        assert(ui.a === original && ui.a.hidden === true && snapshots.at(-1)[0].hidden === true);
        ui.a.text = 'updated while hidden';
        assert(ui.a === original && snapshots.at(-1)[0].hidden === true && snapshots.at(-1)[0].text === 'updated while hidden');
        const count = snapshots.length;
        ui.a.hidden = true;
        for (const value of [null, undefined, 0, 1, '', {}, [], new Boolean(true)]) {
            throws(() => ui.a.hidden = value);
            throws(() => ui('b', { hidden: value }));
        }
        assert(ui.a === original && ui.a.hidden === true && snapshots.length === count);
        delete ui.a.hidden;
        assert(ui.a.hidden === false && snapshots.at(-1)[0].hidden === false);
        ui('b', { hidden: true });
        assert(ui.b.hidden === true && snapshots.at(-1)[1].hidden === true);
    "#);
}

#[test]
fn ui_registry_based_on_validates_publishes_updates_and_resets_on_delete() {
    check(r#"
        ui('a', { basedOn: 'scene:7/Panel[0]' });
        assert(snapshots.at(-1)[0].basedOn === 'scene:7/Panel[0]');
        ui.a.basedOn = 'scene:8/Panel[1]';
        assert(snapshots.at(-1)[0].basedOn === 'scene:8/Panel[1]');
        delete ui.a.basedOn;
        assert(snapshots.at(-1)[0].basedOn === '');
        throws(() => ui.a.basedOn = 1);
    "#);
}

#[test]
fn ui_registry_alignment_fields_validate_update_and_reset() {
    check(r#"
        ui('a', { text: 'aligned' }); ui.a.setOnClick(() => {});
        const original = ui.a, first = snapshots.at(-1)[0];
        assert(ui.a.alignX === 'left' && ui.a.alignY === 'center');
        assert(first.alignX === 'left' && first.alignY === 'center');
        for (const alignX of ['left', 'center', 'right']) {
            for (const alignY of ['top', 'center', 'bottom']) {
                ui.a.alignX = alignX; ui.a.alignY = alignY;
                const state = snapshots.at(-1)[0];
                assert(state.alignX === alignX && state.alignY === alignY);
                assert(ui.a === original && state.instanceId === first.instanceId && state.eventsVersion === first.eventsVersion);
            }
        }
        const count = snapshots.length;
        ui.a.alignX = 'right'; ui.a.alignY = 'bottom';
        for (const value of ['middle', 'LEFT', '', null, undefined, 1, {}, new String('center')]) {
            throws(() => ui.a.alignX = value); throws(() => ui.a.alignY = value);
            throws(() => ui('a', { text: 'invalid replacement', alignX: value }));
            throws(() => ui('a', { text: 'invalid replacement', alignY: value }));
        }
        throws(() => ui.a.alignX = 'top'); throws(() => ui.a.alignY = 'left');
        assert(ui.a === original && ui.a.alignX === 'right' && ui.a.alignY === 'bottom' && snapshots.length === count);
        delete ui.a.alignX; delete ui.a.alignY;
        assert(ui.a.alignX === 'left' && ui.a.alignY === 'center');
        assert(snapshots.at(-1)[0].alignX === 'left' && snapshots.at(-1)[0].alignY === 'center');
        ui('b', { alignX: 'center', alignY: 'top' });
        assert(snapshots.at(-1)[1].alignX === 'center' && snapshots.at(-1)[1].alignY === 'top');
    "#);
}

#[test]
fn ui_registry_accepts_styles_in_initial_definition() {
    check(r#"
        const original = { text: 'Click Me', border: { thickness: 2, color: [255,0,0] },
            corner: { radius: 5, topLeft: 0, bottomLeft: 0 }, padding: { thickness: 4, left: 0, right: 8 } };
        ui('a', original);
        ui.a.setOnClick(() => {});
        const a = snapshots.at(-1)[0];
        assert(a.border.thickness === 2 && a.border.color.join() === '255,0,0,255');
        assert(a.corner.topLeft === 0 && a.corner.topRight === 5 && a.corner.bottomLeft === 0);
        assert(JSON.stringify(a.padding) === '{"top":4,"right":8,"bottom":4,"left":0}');
        original.border.color[0] = 0; assert(ui.a.border.color[0] === 255);
        ui('b', {}); ui.b.border = original.border; ui.b.corner = original.corner; ui.b.padding = original.padding;
        assert(snapshots.at(-1)[1].corner.topRight === a.corner.topRight);
    "#);
}

#[test]
fn ui_registry_validates_atomically() {
    check(r#"
        ui('a', { text: 'valid' }); const original = ui.a; const count = snapshots.length;
        for (const value of [undefined, [], { unknown: 1 }, { posX: Infinity }, { lenX: { min: 2, max: 1 } },
            { border: { thickness: -1 } }, { corner: { radias: 5 } }, { padding: { left: NaN } }, { color: [256,0,0] }]) {
            throws(() => ui('a', value)); assert(ui.a === original && snapshots.length === count);
        }
        throws(() => ui.a.border = { color: [0,0,0,-1] });
        assert(ui.a.text === 'valid');
    "#);
}

#[test]
fn ui_registry_keeps_validation_independent_of_script_intrinsics() {
    check(r#"
        ui('a', { text: 'valid' });
        const original = ui.a, count = snapshots.length, first = snapshots.at(-1)[0];
        Number.isFinite = () => true;
        Number.isInteger = () => true;
        JSON.stringify = () => { throw new Error('script serializer'); };
        Object.prototype.min = 1000;
        Array.prototype.map = () => [];
        Map.prototype.get = () => undefined;
        throws(() => ui.a.posX = NaN);
        throws(() => ui.a.color = [1.5, 0, 0]);
        assert(ui.a === original && ui.a.posX === 0 && snapshots.length === count);
        ui.a.lenX = {};
        assert(snapshots.at(-1)[0].lenX.min === 0);
        ui.a.text = 'still valid';
        assert(ui.a.text === 'still valid' && snapshots.at(-1)[0].text === 'still valid');
    "#);
}

#[test]
fn ui_registry_invalid_update_preserves_requested_state() {
    check(r#"
        ui('a', { text: 'before' }); ui.a.setOnClick(() => {}); ui('b', {});
        const original = ui.a, count = snapshots.length, first = snapshots.at(-1)[0];
        throws(() => ui('a', { unknown: 1 }));
        throws(() => ui('a', { text: 'replacement', size: 0 }));
        assert(ui.a === original && ui.a.text === 'before');
        assert(Object.keys(ui).join() === 'a,b' && snapshots.length === count);
        ui.a.text = 'after';
        assert(snapshots.at(-1)[0].eventsVersion === first.eventsVersion && ui.a.text === 'after');
    "#);
}

#[test]
fn ui_registry_rejects_stale_proxy_and_nested_mutation() {
    check(r#"
        ui('a', { color: [1,2,3], border: { color: [4,5,6] }, lenX: { min: 2 }, corner: { radius: 3 }, padding: { left: 1 } });
        for (const field of ['color','border','lenX','corner','padding']) {
            assert(Object.isFrozen(ui.a[field])); throws(() => ui.a[field].unknown = 2);
        }
        throws(() => ui.a.border.color[0] = 9);
        throws(() => Object.defineProperty(ui.a, 'text', { value: 'bypass' }));
        throws(() => Object.setPrototypeOf(ui, {}));
        throws(() => Object.defineProperty(ui, 'a', { value: {} }));
        const old = ui.a; ui('a', null); throws(() => old.text = 'late');
    "#);
}

#[test]
fn ui_registry_same_value_assignment_is_noop() {
    check(r#"
        const handler = () => {};
        ui('a', { text: 'x', color: [1,2,3], corner: { radius: 5, topLeft: 0 } });
        ui.a.setOnClick(handler);
        const count = snapshots.length, first = snapshots.at(-1)[0];
        ui.a.text = 'x'; ui.a.color = [1,2,3]; ui.a.corner = { topLeft: 0, radius: 5 }; ui.a.setOnClick(handler);
        ui('absent', null); delete ui.a.padding;
        assert(snapshots.length === count);
        ui.a.text = 'y'; ui.a.text = 'y'; assert(snapshots.length === count + 1);
        assert(snapshots.at(-1)[0].eventsVersion === first.eventsVersion);
        ui.a.setOnClick(() => {}); assert(snapshots.at(-1)[0].eventsVersion > first.eventsVersion);
        const old = ui.a, revision = snapshots.length;
        assert(ui('a', { text: 'y' }) === old && snapshots.length === revision);
    "#);
}

#[test]
fn ui_registry_native_bignum_state_reads_replacements_and_callback_surface() {
    check(r#"
        const first = new BigNum('1.5');
        const element = ui('a', { states: { value: first } });
        assert(element.states.value instanceof BigNum && element.states.value.eq(first));
        assert(Object.getOwnPropertyDescriptor(element.states, 'value').value.eq(first));
        element.setOnStateUpdate(() => {});
        element.states.value = new BigNum('2.5');
        assert(element.states.value.eq(new BigNum('2.5')));
        ui('a', { states: { value: new BigNum('3.5') } });
        assert(element.states.value.eq(new BigNum('3.5')));
        const collision = ui('collision', { states: { states: new BigNum('4.5') } });
        collision.states.states = new BigNum('5.5');
        assert(collision.states.states.eq(new BigNum('5.5')));
        collision.states = { states: new BigNum('6.5') };
        assert(collision.states.states.eq(new BigNum('6.5')));
    "#);
}

#[test]
fn ui_registry_border_corner_padding_defaults_and_overrides() {
    check(r#"
        ui('a', {});
        let state = snapshots.at(-1)[0];
        assert(state.border.thickness === 0 && state.corner.topLeft === 0 && state.padding.left === 0);
        assert(state.color.join() === '0,0,0,0' && state.textColor.join() === '255,255,255,255');
        ui.a.border = { thickness: 2.5, color: [1,2,3] };
        ui.a.corner = { radius: 99.5, topLeft: 0 };
        ui.a.padding = { thickness: 1.5, bottom: 0 };
        state = snapshots.at(-1)[0];
        assert(state.corner.topRight === 99.5 && state.padding.top === 1.5 && state.padding.bottom === 0);
        const revision = snapshots.length;
        ui.a.corner = { radius: 199.5, topLeft: 0 };
        assert(snapshots.length === revision + 1 && snapshots.at(-1)[0].corner.topRight === 199.5);
        delete ui.a.border; delete ui.a.corner; delete ui.a.padding;
        state = snapshots.at(-1)[0];
        assert(state.border.thickness === 0 && state.corner.topRight === 0 && state.padding.top === 0);
    "#);
}

#[test]
fn ui_callback_setters_are_the_only_handler_surface() {
    check(r#"
        ui('a', { text: 'safe' });
        assert(ui.a.onClick === undefined && ui.a.onHover === undefined && ui.a.onLeave === undefined && ui.a.onStateUpdate === undefined);
        assert(!('onClick' in ui.a) && !('onHover' in ui.a) && !('onLeave' in ui.a) && !('onStateUpdate' in ui.a));
        assert(ui.a.setOnClick(() => {}) === ui.a);
        assert(ui.a.setOnClick(null) === ui.a);
        throws(() => ui.a.setOnClick(undefined));
        throws(() => ui('a', { onClick() {} }));
        const revision = snapshots.length;
        assert(ui.a.setOnStateUpdate(() => {}) === ui.a);
        assert(ui.a.setOnStateUpdate(null) === ui.a);
        assert(snapshots.length === revision);
        throws(() => ui.a.setOnStateUpdate(undefined));
        throws(() => ui.a.setOnStateUpdate(1));
        throws(() => ui.a.setOnStateUpdate = () => {});
        throws(() => ui('a', { onStateUpdate() {} }));
        throws(() => ui('a', { setOnStateUpdate() {} }));
    "#);
}

#[test]
fn ui_setter_replacement_invalidates_old_handler_registration() {
    check(r#"
        let first = () => {}, second = () => {};
        ui('a', {});
        const old = ui.a;
        old.setOnClick(first);
        ui('a', null);
        ui('a', {});
        assert(ui.a !== old);
        assert(ui.a.setOnClick(second) === ui.a);
        throws(() => old.setOnClick(null));
    "#);
}
