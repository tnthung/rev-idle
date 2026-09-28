use crate::bridge::{ScriptUiEvent, ScriptUiPublisher};
use rquickjs::{Ctx, Function, Object, Persistent, Value, promise::MaybePromise};
use uuid::Uuid;

pub(super) struct ScriptUiState {
    api: Persistent<Object<'static>>,
    session_id: Uuid,
    publisher: ScriptUiPublisher,
}

impl ScriptUiState {
    pub(super) fn new(
        ctx: &Ctx<'_>,
        publisher: ScriptUiPublisher,
        stopped: impl Fn() -> bool + 'static,
    ) -> rquickjs::Result<Self> {
        let session_id = Uuid::new_v4();
        let publish = publisher.clone();
        let factory: Function = ctx.eval(include_str!("ui.js"))?;
        let api: Object = factory.call((
            Function::new(ctx.clone(), move |json: String| {
                let elements = serde_json::from_str(&json).map_err(|error| {
                    rquickjs::Error::new_from_js_message("UI", "snapshot", error.to_string())
                })?;
                publish.replace(session_id, elements);
                Ok::<(), rquickjs::Error>(())
            })?,
            Function::new(ctx.clone(), || Uuid::new_v4().to_string())?,
            Function::new(ctx.clone(), stopped)?,
            session_id.to_string(),
        ))?;
        publisher.replace(session_id, Vec::new());
        Ok(Self { api: Persistent::save(ctx, api), session_id, publisher })
    }

    pub(super) fn registry<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<Object<'js>> {
        self.api.clone().restore(ctx)?.get("registry")
    }

    pub(super) fn dispatch<'js>(&self, ctx: &Ctx<'js>, event: &ScriptUiEvent) -> rquickjs::Result<MaybePromise<'js>> {
        let api = self.api.clone().restore(ctx)?;
        let dispatch: Function = api.get("dispatch")?;
        let event: Value = ctx.json_parse(serde_json::to_string(event).unwrap())?;
        dispatch.call((event,))
    }
}

impl Drop for ScriptUiState {
    fn drop(&mut self) {
        self.publisher.clear(self.session_id);
    }
}

#[cfg(test)]
mod tests {
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
                rev.ui.button = { onClick() { rev.click(7, 7); } };
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
            let release;
            export function afterLoad() {
                rev.ui.button = { text: 'ready', onClick() { rev.click(7, 1); rev.ui.button.text = 'callback'; } };
                rev.ui.release = { onClick() { release(); } };
            }
            export default async function() {
                rev.ui.button.text = 'waiting';
                await new Promise(resolve => { release = resolve; });
                rev.ui.button.text += '-default';
            }
        "#, "callback.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
        session.run_after_load(controls.clone()).await.unwrap();
        let invocation = session.invoke((), controls.clone());
        tokio::pin!(invocation);
        assert!(tokio::time::timeout(Duration::from_millis(10), &mut invocation).await.is_err());
        controls.actions_paused.set_paused(true);
        let snapshot = snapshots.borrow().clone();
        session.dispatch_ui_event(ScriptUiEvent {
            session_id: snapshot.session_id.unwrap(), element_id: "button".to_owned(),
            instance_id: snapshot.elements[0].instance_id, events_version: snapshot.elements[0].events_version,
            event: ScriptUiEventKind::Click,
        }, controls.clone()).await.unwrap();
        assert_eq!(*clicks.borrow(), vec![7]);
        assert_eq!(snapshots.borrow().elements[0].text, "callback");
        session.dispatch_ui_event(ScriptUiEvent {
            session_id: snapshot.session_id.unwrap(), element_id: "release".to_owned(),
            instance_id: snapshot.elements[1].instance_id, events_version: snapshot.elements[1].events_version,
            event: ScriptUiEventKind::Click,
        }, controls).await.unwrap();
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
            let count = 0, release;
            export function afterLoad() {
                rev.ui.button = { async onClick() {
                    const id = ++count;
                    rev.click(id, 1);
                    if (id === 1) await new Promise(resolve => { release = resolve; });
                    else { await rev.sleep(1); rev.click(-id, 1); release(); return; }
                    rev.click(-id, 1);
                } };
            }
            export default function() {}
        "#, "callback.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
        session.run_after_load(controls.clone()).await.unwrap();
        let snapshot = snapshots.borrow().clone();
        let event = ScriptUiEvent {
            session_id: snapshot.session_id.unwrap(), element_id: "button".to_owned(),
            instance_id: snapshot.elements[0].instance_id, events_version: snapshot.elements[0].events_version,
            event: ScriptUiEventKind::Click,
        };
        let (first, second) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(session.dispatch_ui_event(event.clone(), controls.clone()), session.dispatch_ui_event(event, controls))
        }).await.unwrap();
        first.unwrap(); second.unwrap();
        assert_eq!(*clicks.borrow(), vec![1, 2, -2, -1]);
    }

    #[tokio::test]
    async fn ui_callback_error_is_isolated() {
        use crate::{bridge::{ScriptUiEvent, ScriptUiEventKind, ScriptUiPublisher, WsConnection}, script::{bindings::{HostControls, SharedMouse}, control::SessionControl, session::ScriptSession}};
        use std::{cell::RefCell, rc::Rc};
        let clicks = Rc::new(RefCell::new(Vec::new()));
        let controls: HostControls = (Rc::new(RefCell::new(CallbackMouse(clicks.clone()))) as SharedMouse).into();
        let publisher = ScriptUiPublisher::default();
        let snapshots = publisher.subscribe();
        let session = ScriptSession::new_with_connection_and_control(r#"
            export function afterLoad() {
                rev.ui.sync = { onClick() { throw new Error('sync broken'); } };
                rev.ui.async = { async onClick() { await rev.sleep(1); throw new Error('async broken'); } };
                rev.ui.good = { onClick() { rev.click(9, 1); } };
            }
            export default function() { rev.click(10, 1); }
        "#, "callback.js", WsConnection::disconnected_for_test(), SessionControl::standalone(), publisher).await.unwrap();
        session.run_after_load(controls.clone()).await.unwrap();
        let snapshot = snapshots.borrow().clone();
        for element in &snapshot.elements {
            let result = session.dispatch_ui_event(ScriptUiEvent {
                session_id: snapshot.session_id.unwrap(), element_id: element.id.clone(),
                instance_id: element.instance_id, events_version: element.events_version, event: ScriptUiEventKind::Click,
            }, controls.clone()).await;
            if element.id == "good" { result.unwrap(); }
            else { assert!(result.unwrap_err().contains(&format!("{} broken", element.id))); }
        }
        session.invoke((), controls).await.unwrap();
        assert_eq!(*clicks.borrow(), vec![9, 10]);
        assert!(!session.is_stopped());
    }

    #[test]
    fn ui_registry_native_roots_drop_before_runtime() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        let publisher = crate::bridge::ScriptUiPublisher::default();
        let ui = context.with(|ctx| super::ScriptUiState::new(&ctx, publisher, || false).unwrap());
        context.with(|ctx| {
            ctx.globals().set("ui", ui.registry(&ctx).unwrap()).unwrap();
            ctx.eval::<(), _>("ui.a = {text: 'root', onClick() { ui.a.text = 'clicked'; }};").unwrap();
        });
        drop(ui);
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
                export function afterLoad() { rev.ui.a = { text: 'loaded' }; original = rev.ui.a; }
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
                    if (!rev.ui.a) rev.ui.a = { text: 'same', color: [1,2,3] };
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
        context.with(|ctx| {
            ctx.eval::<(), _>(format!(r#"
                let snapshots = [], serial = 0, stopped = false, failPublish = false;
                const api = ({})((json) => {{ if (failPublish) throw new Error('publish failed'); snapshots.push(JSON.parse(json)); }}, () => String(++serial), () => stopped, 'session');
                const ui = api.registry;
                function assert(condition, message = 'assertion failed') {{ if (!condition) throw new Error(message); }}
                function throws(action) {{ let thrown = false; try {{ action(); }} catch (_) {{ thrown = true; }} assert(thrown, 'expected rejection'); }}
                {}
            "#, include_str!("ui.js"), source)).unwrap_or_else(|error| {
                panic!("{}", rquickjs::CaughtError::from_error(&ctx, error));
            });
        });
    }

    #[test]
    fn ui_registry_create_update_replace_delete() {
        check(r#"
            assert(ui.missing === undefined);
            ui.a = { text: 'first' }; ui.b = {};
            const old = ui.a;
            ui.a.text = 'changed'; assert(ui.a === old && old.text === 'changed');
            ui.a = { text: 'changed' }; assert(ui.a !== old);
            assert(Object.keys(ui).join(',') === 'a,b');
            throws(() => old.text = 'stale');
            delete ui.a; assert(ui.a === undefined);
            ui.a = {}; assert(Object.keys(ui).join(',') === 'b,a');
            ui.__proto__ = { text: 'prototype key' }; ui.constructor = {};
            assert(ui.__proto__.text === 'prototype key' && Object.getPrototypeOf(ui) === null);
        "#);
    }

    #[test]
    fn ui_registry_accepts_styles_in_initial_definition() {
        check(r#"
            const original = { text: 'Click Me', border: { thickness: 2, color: [255,0,0] },
                corner: { radius: 5, topLeft: 0, bottomLeft: 0 }, padding: { thickness: 4, left: 0, right: 8 }, onClick() {} };
            ui.a = original;
            const a = snapshots.at(-1)[0];
            assert(a.border.thickness === 2 && a.border.color.join() === '255,0,0,255');
            assert(a.corner.topLeft === 0 && a.corner.topRight === 5 && a.corner.bottomLeft === 0);
            assert(JSON.stringify(a.padding) === '{"top":4,"right":8,"bottom":4,"left":0}');
            assert(a.events.join() === 'click');
            original.border.color[0] = 0; assert(ui.a.border.color[0] === 255);
            ui.b = {}; ui.b.border = original.border; ui.b.corner = original.corner; ui.b.padding = original.padding;
            assert(snapshots.at(-1)[1].corner.topRight === a.corner.topRight);
        "#);
    }

    #[test]
    fn ui_registry_validates_atomically() {
        check(r#"
            ui.a = { text: 'valid' }; const original = ui.a; const count = snapshots.length;
            for (const value of [null, undefined, [], { unknown: 1 }, { posX: Infinity }, { lenX: { min: 2, max: 1 } },
                { border: { thickness: -1 } }, { corner: { radias: 5 } }, { padding: { left: NaN } }, { color: [256,0,0] }]) {
                throws(() => ui.a = value); assert(ui.a === original && snapshots.length === count);
            }
            throws(() => ui.a.border = { color: [0,0,0,-1] });
            assert(ui.a.text === 'valid');
            stopped = true; throws(() => ui.a.text = 'late'); throws(() => delete ui.a); throws(() => ui.new = {});
        "#);
    }

    #[test]
    fn ui_registry_keeps_validation_independent_of_script_intrinsics() {
        check(r#"
            ui.a = { text: 'valid' };
            const original = ui.a, count = snapshots.length;
            Number.isFinite = () => true;
            Number.isInteger = () => true;
            JSON.stringify = () => { throw new Error('script serializer'); };
            Object.prototype.min = 1000;
            Object.prototype.toJSON = () => { throw new Error('script prototype serializer'); };
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
    fn ui_registry_publication_failure_preserves_requested_state() {
        check(r#"
            ui.a = { text: 'before', onClick() {} }; ui.b = {};
            const original = ui.a, count = snapshots.length, handler = ui.a.onClick;
            failPublish = true;
            throws(() => ui.a.text = 'after');
            throws(() => ui.a.onClick = () => {});
            throws(() => delete ui.a.text);
            throws(() => ui.a = { text: 'replacement' });
            throws(() => delete ui.a);
            assert(ui.a === original && ui.a.text === 'before' && ui.a.onClick === handler);
            assert(Object.keys(ui).join() === 'a,b' && snapshots.length === count);
            failPublish = false;
            ui.a.text = 'after';
            assert(snapshots.at(-1)[0].eventsVersion === 1 && ui.a.text === 'after');
        "#);
    }

    #[test]
    fn ui_registry_rejects_stale_proxy_and_nested_mutation() {
        check(r#"
            ui.a = { color: [1,2,3], border: { color: [4,5,6] }, lenX: { min: 2 }, corner: { radius: 3 }, padding: { left: 1 } };
            for (const field of ['color','border','lenX','corner','padding']) {
                assert(Object.isFrozen(ui.a[field])); throws(() => ui.a[field].unknown = 2);
            }
            throws(() => ui.a.border.color[0] = 9);
            throws(() => Object.defineProperty(ui.a, 'text', { value: 'bypass' }));
            throws(() => Object.setPrototypeOf(ui, {}));
            throws(() => Object.defineProperty(ui, 'a', { value: {} }));
            const old = ui.a; delete ui.a; throws(() => old.text = 'late');
        "#);
    }

    #[test]
    fn ui_registry_same_value_assignment_is_noop() {
        check(r#"
            const handler = () => {};
            ui.a = { text: 'x', color: [1,2,3], corner: { radius: 5, topLeft: 0 }, onClick: handler };
            const count = snapshots.length, first = snapshots.at(-1)[0];
            ui.a.text = 'x'; ui.a.color = [1,2,3]; ui.a.corner = { topLeft: 0, radius: 5 }; ui.a.onClick = handler;
            delete ui.absent; delete ui.a.padding;
            assert(snapshots.length === count);
            ui.a.text = 'y'; ui.a.text = 'y'; assert(snapshots.length === count + 1);
            assert(snapshots.at(-1)[0].eventsVersion === first.eventsVersion);
            ui.a.onClick = () => {}; assert(snapshots.at(-1)[0].eventsVersion > first.eventsVersion);
            const old = ui.a; ui.a = { text: 'y' }; assert(ui.a !== old);
        "#);
    }

    #[test]
    fn ui_registry_border_corner_padding_defaults_and_overrides() {
        check(r#"
            ui.a = {};
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
    fn ui_callback_does_not_expose_registry_storage_as_this() {
        check(r#"
            let receiver;
            ui.a = { text: 'safe', onClick() { 'use strict'; receiver = this; } };
            const state = snapshots.at(-1)[0];
            api.dispatch({ sessionId: 'session', elementId: 'a', instanceId: state.instanceId, eventsVersion: state.eventsVersion, event: 'click' });
            assert(receiver === undefined);
        "#);
    }

    #[test]
    fn ui_event_rejects_old_session_instance_and_handler() {
        check(r#"
            let calls = 0;
            ui.a = { onClick() { calls++; } };
            const state = snapshots.at(-1)[0];
            const event = { sessionId: 'session', elementId: 'a', instanceId: state.instanceId, eventsVersion: state.eventsVersion, event: 'click' };
            api.dispatch(event); assert(calls === 1);
            ui.a.text = 'visual'; api.dispatch(event); assert(calls === 2);
            ui.a.onClick = () => { calls += 100; }; api.dispatch(event); assert(calls === 2);
            api.dispatch({ ...event, sessionId: 'old' }); assert(calls === 2);
            delete ui.a; api.dispatch(event); assert(calls === 2);
            ui.a = { onClick() { calls += 1000; } };
            api.dispatch(event); assert(calls === 2);
            const replacement = snapshots.at(-1)[0];
            api.dispatch({ ...event, instanceId: replacement.instanceId, eventsVersion: replacement.eventsVersion });
            assert(calls === 1002);
        "#);
    }
}
