use super::State;
use super::bindings::{click_at_with, BridgeMouseInput, Button, HostControls, MouseInput, MouseInputError, SharedMouse};
use crate::window::{Axis, WindowControl};
use super::lifecycle::{
    apply_hotkey_update,
    capture_action,
    disable_capture_if_running,
    run_with_controls,
    run_with_controls_and_connection,
    run_with_controls_and_lifecycle,
    run_with_controls_and_lifecycle_with_control,
    run_with_controls_and_state,
    CaptureAction,
};
use super::session::ScriptSession;
use super::session::format_console_message;
use super::ScriptControl;
use crate::{app::{PauseUpdate, ScriptCommand, ScriptPhase, StateUpdate}, capture::{CaptureState, LockState}};
use rquickjs::{function::Rest, AsyncContext, AsyncRuntime, Value};
use tokio::sync::{mpsc, watch};
use crate::app::ActionGate;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Click {
    x: i32,
    y: i32,
    button: Button,
}

struct FakeMouse {
    clicks: Rc<RefCell<Vec<Click>>>,
}

struct FakeWindow;

#[test]
fn stale_hotkey_delivery_cannot_reopen_or_resume_a_newer_pause() {
    let gate = ActionGate::default();
    let stale_resume = gate.set_paused(false);
    let current_pause = gate.set_paused(true);
    let mut lifecycle_paused = true;

    assert!(!apply_hotkey_update(
        stale_resume,
        &gate,
        true,
        &mut lifecycle_paused,
    ));
    assert!(gate.is_paused());
    assert!(lifecycle_paused);

    assert!(apply_hotkey_update(
        current_pause,
        &gate,
        true,
        &mut lifecycle_paused,
    ));
    assert!(gate.is_paused());
    assert!(lifecycle_paused);
}

#[test]
fn production_mouse_adapter_sends_one_bridge_event() {
    let mut received = None;
    click_at_with(1200, 80, |x, y| {
        received = Some((x, y));
        Ok(())
    })
    .unwrap();

    assert_eq!(received, Some((1200, 80)));
}

impl WindowControl for FakeWindow {
    fn resize_client(&self, _width: i32, _height: i32) -> Result<(), String> { Ok(()) }
}

impl From<SharedMouse> for HostControls {
    fn from(mouse: SharedMouse) -> Self {
        Self { mouse, window: Rc::new(FakeWindow), actions_paused: ActionGate::default() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum HostEvent {
    Resize(i32, i32),
    Click(i32, i32, Button),
    Scroll(i32, i32, i32, Axis),
    Press(String),
    Clipboard(String),
}

struct RecordingWindow { events: Rc<RefCell<Vec<HostEvent>>>, clipboard: RefCell<String> }
impl WindowControl for RecordingWindow {
    fn resize_client(&self, width: i32, height: i32) -> Result<(), String> {
        self.events.borrow_mut().push(HostEvent::Resize(width, height)); Ok(())
    }

    fn write_clipboard(&self, text: &str) -> Result<(), String> {
        self.events.borrow_mut().push(HostEvent::Clipboard(text.to_owned()));
        *self.clipboard.borrow_mut() = text.to_owned();
        Ok(())
    }

    fn read_clipboard(&self) -> Result<String, String> {
        Ok(self.clipboard.borrow().clone())
    }

}
struct RecordingMouse { events: Rc<RefCell<Vec<HostEvent>>> }
impl MouseInput for RecordingMouse {
    fn click_at(&mut self, x: i32, y: i32, button: Button) -> Result<(), MouseInputError> {
        self.events.borrow_mut().push(HostEvent::Click(x, y, button)); Ok(())
    }

    fn scroll(&mut self, x: i32, y: i32, length: i32, axis: Axis) -> Result<(), MouseInputError> {
        self.events.borrow_mut().push(HostEvent::Scroll(x, y, length, axis)); Ok(())
    }

    fn press(&mut self, key: String) -> Result<(), MouseInputError> {
        self.events.borrow_mut().push(HostEvent::Press(key)); Ok(())
    }

}
fn recording_controls() -> (HostControls, Rc<RefCell<Vec<HostEvent>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    (HostControls { mouse: Rc::new(RefCell::new(RecordingMouse { events: events.clone() })), window: Rc::new(RecordingWindow { events: events.clone(), clipboard: RefCell::new(String::new()) }), actions_paused: ActionGate::default() }, events)
}

#[tokio::test(flavor = "current_thread")]
async fn script_pause_and_daemon_resume_preserve_invocation_and_ownership() {
    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    let path = std::env::temp_dir().join(format!("rev-idle-script-pause-resume-{}.js", uuid::Uuid::new_v4()));
    std::fs::write(&path, r#"
        let owner;
        export function afterLoad() {
            rev.daemon('resume', async function() {
                while (!rev.paused) await rev.sleep(1);
                rev.pause();
                while (rev.read_clipboard() !== 'resume') await rev.sleep(1);
                rev.click(3, Number(rev.paused));
                rev.resume();
                rev.resume();
                rev.daemon('resume', null);
            });
        }
        export function beforePause() {
            rev.pause();
            rev.click(2, Number(rev.paused));
        }
        export function afterResume() {
            rev.resume();
            rev.click(4, Number(rev.paused));
        }
        export default async function() {
            owner = await rev.screenOwnership();
            let value = 41;
            rev.click(1, value);
            rev.pause();
            rev.pause();
            await rev.sleep(50);
            rev.click(5, ++value);
            await new Promise(() => {});
        }
    "#).unwrap();
    let (controls, events) = recording_controls();
    let pause_rx = controls.actions_paused.subscribe();
    let window = controls.window.clone();
    let (commands, command_rx) = mpsc::channel(8);
    let (states, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, states, pause_rx, Some(path.clone()), controls, lock_state, Duration::from_millis(2),
        ));
        tokio::time::timeout(Duration::from_secs(2), async {
            while state_rx.borrow().phase != ScriptPhase::Paused { state_rx.changed().await.unwrap(); }
        }).await.unwrap();
        assert!(!lock_state.is_enabled());
        assert!(!state_rx.borrow().locked);
        assert_eq!(*events.borrow(), vec![
            HostEvent::Click(1, 41, Button::Left),
            HostEvent::Click(2, 0, Button::Left),
        ]);
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(events.borrow().len(), 2, "the main invocation must stay suspended");
        window.write_clipboard("resume").unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while events.borrow().len() < 6 || state_rx.borrow().phase != ScriptPhase::Running {
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
        assert!(lock_state.is_enabled());
        assert!(state_rx.borrow().locked);
        assert_eq!(*events.borrow(), vec![
            HostEvent::Click(1, 41, Button::Left),
            HostEvent::Click(2, 0, Button::Left),
            HostEvent::Clipboard("resume".to_owned()),
            HostEvent::Click(3, 1, Button::Left),
            HostEvent::Click(4, 0, Button::Left),
            HostEvent::Click(5, 42, Button::Left),
        ]);
        commands.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), runner).await.unwrap().unwrap().unwrap();
    }).await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn ensure_running_resolves_when_running() {
    let session = ScriptSession::new(r#"export default (async () => { await rev.ensureRunning(); })"#)
        .await
        .unwrap();
    let (controls, _) = recording_controls();
    session.invoke(State::default(), controls).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn synchronous_bridge_control_disconnect_is_classified() {
    let connection = crate::bridge::WsConnection::disconnected_for_test();
    let session = ScriptSession::new_with_connection(
        "export function onDisconnect() {} export default (() => rev.press('a'));",
        "test.js",
        connection.clone(),
    )
    .await
    .unwrap();
    let result = session
        .invoke_classified(
            State::default(),
            HostControls {
                mouse: Rc::new(RefCell::new(BridgeMouseInput { connection })),
                window: Rc::new(FakeWindow),
                actions_paused: ActionGate::default(),
            },
        )
        .await;
    let Err(error) = result else { panic!("disconnected press must fail") };
    assert!(error.disconnected);
}

#[tokio::test(flavor = "current_thread")]
async fn ensure_running_waits_for_resume() {
    let session = ScriptSession::new(r#"export default (async () => { await rev.ensureRunning(); rev.click(1, 1); })"#)
        .await
        .unwrap();
    let (controls, events) = recording_controls();
    session.acknowledge_pause(true);
    let invocation = session.invoke(State::default(), controls.clone());
    tokio::pin!(invocation);
    assert!(tokio::time::timeout(Duration::from_millis(20), &mut invocation).await.is_err());
    assert!(events.borrow().is_empty());
    session.acknowledge_pause(false);
    tokio::time::timeout(Duration::from_secs(1), &mut invocation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(events.borrow().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn ensure_running_rechecks_rapid_transitions() {
    let session = ScriptSession::new(r#"export default async function() {
        await Promise.all([rev.ensureRunning(), rev.ensureRunning(), rev.ensureRunning()]);
        rev.click(3, 3);
    }"#).await.unwrap();
    let (controls, events) = recording_controls();
    session.acknowledge_pause(true);
    let invocation = session.invoke((), controls.clone());
    tokio::pin!(invocation);
    assert!(tokio::time::timeout(Duration::from_millis(10), &mut invocation).await.is_err());
    session.acknowledge_pause(false);
    session.acknowledge_pause(true);
    assert!(tokio::time::timeout(Duration::from_millis(10), &mut invocation).await.is_err());
    assert!(events.borrow().is_empty());
    session.acknowledge_pause(false);
    tokio::time::timeout(Duration::from_secs(1), invocation).await.unwrap().unwrap();
    assert_eq!(*events.borrow(), vec![HostEvent::Click(3, 3, Button::Left)]);
}

#[tokio::test(flavor = "current_thread")]
async fn ensure_running_cannot_wake_in_replacement_session() {
    let control = super::ScriptControl::default();
    let old_control = control.register_session();
    let old = ScriptSession::new_with_connection_and_control(
        "export default async function() { await rev.ensureRunning(); rev.click(1, 1); }", "old-checkpoint.js",
        crate::bridge::WsConnection::disconnected_for_test(), old_control.clone(), crate::bridge::ScriptUiPublisher::default(),
    ).await.unwrap();
    let (controls, events) = recording_controls();
    old_control.acknowledge_pause(true);
    let invocation = old.invoke((), controls.clone());
    tokio::pin!(invocation);
    assert!(tokio::time::timeout(Duration::from_millis(10), &mut invocation).await.is_err());
    old_control.stop();
    let replacement = ScriptSession::new_with_connection_and_control(
        "export default async function() { await rev.ensureRunning(); rev.click(2, 2); }", "new-checkpoint.js",
        crate::bridge::WsConnection::disconnected_for_test(), control.register_session(), crate::bridge::ScriptUiPublisher::default(),
    ).await.unwrap();
    replacement.invoke((), controls).await.unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(1), invocation).await.unwrap().is_err());
    assert!(old.is_stopped());
    assert_eq!(*events.borrow(), vec![HostEvent::Click(2, 2, Button::Left)]);
}

#[tokio::test(flavor = "current_thread")]
async fn screen_ownership_using_serializes_waiters_and_releases_on_exit() {
    let session = ScriptSession::new_with_connection(
        r#"
            export default async function() {
                const order: number[] = [];
                async function work(id: number) {
                    using so = await rev.screenOwnership();
                    order.push(id);
                    await rev.sleep(1);
                    order.push(-id);
                    if (id === 2) throw new Error("expected");
                }
                await Promise.all([work(1), work(2).catch(() => {}), work(3)]);
                if (order.join() !== "1,-1,2,-2,3,-3") throw new Error(order.join());
                using so = await rev.screenOwnership();
                so.release();
                so.release();
                using next = await rev.screenOwnership(undefined);
            }
        "#,
        "screen-ownership.ts",
        crate::bridge::WsConnection::disconnected_for_test(),
    ).await.unwrap();
    let (controls, _) = recording_controls();
    tokio::time::timeout(Duration::from_secs(2), session.invoke((), controls)).await.unwrap().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn screen_ownership_drop_releases_an_unreferenced_token() {
    let session = ScriptSession::new(r#"
        export default async function() {
            await (async () => { const so = await rev.screenOwnership(); })();
            using next = await rev.screenOwnership();
        }
    "#).await.unwrap();
    let (controls, _) = recording_controls();
    tokio::time::timeout(Duration::from_secs(2), session.invoke((), controls)).await.unwrap().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn screen_ownership_using_survives_a_suspended_invocation() {
    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    let (state_updates, _) = watch::channel(StateUpdate::new(true, true, false, false, false));
    let session = ScriptSession::new(r#"
        export default async function() {
            using so = await rev.screenOwnership();
            rev.click(1, 1);
            await rev.sleep(30);
            rev.click(3, 3);
        }
        export async function afterResume() {
            using so = await rev.screenOwnership();
            rev.click(2, 2);
        }
    "#).await.unwrap();
    session.screen_ownership.attach(lock_state, state_updates);
    let (controls, events) = recording_controls();
    {
        let invocation = session.invoke((), controls.clone());
        tokio::pin!(invocation);
        tokio::select! {
            biased;
            result = &mut invocation => panic!("invocation finished before suspension: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }
        assert_eq!(*events.borrow(), vec![HostEvent::Click(1, 1, Button::Left)]);
    }
    session.screen_ownership.set_paused(true);
    assert!(!lock_state.is_enabled());
    session.screen_ownership.set_paused(false);
    assert!(lock_state.is_enabled());
    tokio::time::timeout(Duration::from_secs(2), session.run_after_resume(controls)).await.unwrap().unwrap();
    assert_eq!(*events.borrow(), vec![
        HostEvent::Click(1, 1, Button::Left),
        HostEvent::Click(3, 3, Button::Left),
        HostEvent::Click(2, 2, Button::Left),
    ]);
    assert!(!lock_state.is_enabled());
}

#[tokio::test(flavor = "current_thread")]
async fn screen_ownership_tracks_pause_resume_and_stop_during_an_invocation() {
    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    let path = std::env::temp_dir().join(format!("rev-idle-screen-ownership-{}.js", std::process::id()));
    std::fs::write(&path, r#"
        let owner;
        export default async function() {
            owner ??= await rev.screenOwnership("Unity loop");
            await rev.sleep(5000);
        }
        export async function beforeStop() {
            let rejected = false;
            try { await rev.screenOwnership(); } catch (_) { rejected = true; }
            if (!rejected) throw new Error("stopped session granted ownership");
            rev.click(9, 9);
        }
    "#).unwrap();
    let (command_tx, command_rx) = mpsc::channel(8);
    let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    let (controls, events) = recording_controls();
    let pause_gate = controls.actions_paused.clone();
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, state_tx, pause_rx, Some(path.clone()), controls, lock_state, Duration::from_millis(5),
        ));
        for (phase, locked, command) in [
            (ScriptPhase::Running, true, Some(ScriptCommand::Pause)),
            (ScriptPhase::Paused, false, Some(ScriptCommand::Resume)),
            (ScriptPhase::Running, true, None),
        ] {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if state_rx.borrow().phase == phase && state_rx.borrow().locked == locked { break; }
                    state_rx.changed().await.unwrap();
                }
            }).await.unwrap();
            assert_eq!(lock_state.is_enabled(), locked);
            assert_eq!(serde_json::to_value(&*state_rx.borrow()).unwrap()["lockLabel"],
                if locked { serde_json::json!("Unity loop") } else { serde_json::Value::Null });
            if let Some(command) = command { command_tx.send(command).await.unwrap(); }
        }
        for paused in [true, false] {
            pause_tx.send_replace(pause_gate.set_paused(paused));
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if state_rx.borrow().phase == if paused { ScriptPhase::Paused } else { ScriptPhase::Running }
                        && state_rx.borrow().locked == !paused { break; }
                    state_rx.changed().await.unwrap();
                }
            }).await.unwrap();
            assert_eq!(lock_state.is_enabled(), !paused);
            assert_eq!(serde_json::to_value(&*state_rx.borrow()).unwrap()["lockLabel"],
                if paused { serde_json::Value::Null } else { serde_json::json!("Unity loop") });
        }
        command_tx.send(ScriptCommand::Stop).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Stopped && !state_rx.borrow().locked { break; }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();
        assert!(!lock_state.is_enabled());
        assert!(serde_json::to_value(&*state_rx.borrow()).unwrap()["lockLabel"].is_null());
        assert!(events.borrow().is_empty(), "terminal teardown must skip beforeStop");
        command_tx.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), runner).await.unwrap().unwrap().unwrap();
    }).await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn rev_file_io_reads_missing_files_and_overwrites_text() {
    let path = std::env::temp_dir().join(format!("rev_file_io_{}.txt", std::process::id()));
    let session = ScriptSession::new(&format!(r#"export default (() => {{
        const path = {};
        if (rev.read_file(path) !== null) throw new Error("missing file must be null");
        rev.write_file(path, "hello 世界\n");
        if (rev.read_file(path) !== "hello 世界\n") throw new Error("text did not round trip");
        rev.write_file(path, "");
        if (rev.read_file(path) !== "") throw new Error("file was not truncated");
        let errors = 0;
        try {{ rev.read_file({}); }} catch (_) {{ errors++; }}
        try {{ rev.write_file({}, "text"); }} catch (_) {{ errors++; }}
        if (errors !== 2) throw new Error("file errors must throw");
    }})"#,
        serde_json::to_string(&path).unwrap(),
        serde_json::to_string(&std::env::current_dir().unwrap()).unwrap(),
        serde_json::to_string(&std::env::current_dir().unwrap()).unwrap(),
    )).await.unwrap();
    let (controls, _) = recording_controls();
    let result = session.invoke(State::default(), controls).await;
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    result.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn rev_delete_file_reports_whether_a_file_was_deleted() {
    let path = std::env::temp_dir().join(format!("rev_delete_file_{}.txt", std::process::id()));
    std::fs::write(&path, "delete me").unwrap();
    let session = ScriptSession::new(&format!(r#"export default (() => {{
        const path = {};
        if (rev.delete_file(path) !== true) throw new Error("existing file was not deleted");
        if (rev.delete_file(path) !== false) throw new Error("missing file must return false");
    }})"#,
        serde_json::to_string(&path).unwrap(),
    )).await.unwrap();
    let (controls, _) = recording_controls();
    let result = session.invoke(State::default(), controls).await;
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    result.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn rev_shell_returns_stdout_and_stderr_for_an_unsuccessful_command() {
    let session = ScriptSession::new(r#"export default (() => {
        const result = rev.shell("echo stdout & echo stderr 1>&2 & exit /b 7");
        if (result.stdout.trim() !== "stdout") throw new Error("stdout was not captured");
        if (result.stderr.trim() !== "stderr") throw new Error("stderr was not captured");
    })"#).await.unwrap();
    let (controls, _) = recording_controls();

    session.invoke(State::default(), controls).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn rev_press_forwards_supported_keys_to_the_input_transport() {
    let session = ScriptSession::new(r#"export default (() => {
        rev.press("a");
        rev.press("enter");
        rev.press("f12");
    })"#).await.unwrap();
    let (controls, events) = recording_controls();

    session.invoke(State::default(), controls).await.unwrap();

    assert_eq!(
        *events.borrow(),
        vec![
            HostEvent::Press("a".to_owned()),
            HostEvent::Press("enter".to_owned()),
            HostEvent::Press("f12".to_owned()),
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn rev_press_rejects_unsupported_keys_without_sending_input() {
    let session = ScriptSession::new(r#"export default (() => rev.press("ctrl+c"))"#).await.unwrap();
    let (controls, events) = recording_controls();

    assert!(session.invoke(State::default(), controls).await.is_err());
    assert!(events.borrow().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn rev_press_sends_input_while_actions_are_paused() {
    let session = ScriptSession::new(r#"export default (() => rev.press("enter"))"#).await.unwrap();
    let (controls, events) = recording_controls();
    controls.actions_paused.set_paused(true);

    session.invoke(State::default(), controls).await.unwrap();

    assert_eq!(*events.borrow(), vec![HostEvent::Press("enter".to_owned())]);
}

#[tokio::test(flavor = "current_thread")]
async fn resize_and_click_sends_one_event_without_window_work() {
    let session = ScriptSession::new(r#"export default (() => { rev.resize(1280, 720); rev.click(10, 20, "right"); })"#).await.unwrap();
    let (controls, events) = recording_controls();
    session.invoke(State::default(), controls).await.unwrap();
    assert_eq!(*events.borrow(), vec![HostEvent::Resize(1280, 720), HostEvent::Click(10, 20, Button::Right)]);
}

#[tokio::test(flavor = "current_thread")]
async fn rev_state_parses_mixed_json_and_freezes_only_top_level() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    let (address, peer_rx) = raw_server().await;
    let connection = WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_secs(1),
    );
    let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
        .await
        .unwrap()
        .unwrap();
    let session = ScriptSession::new_with_connection(
        r#"export default (async () => {
            const state = await rev.state("score", "items");
            if (!Object.isFrozen(state)) throw new Error("state is not frozen");
            if (state.score !== 42 || state.items[1].ok !== true || state.items[2] !== null) {
                throw new Error("mixed state was not preserved");
            }
            if (Object.keys(state).length !== 2) throw new Error("selected keys were lost");
            if (Object.isFrozen(state.items) || Object.isFrozen(state.items[1])) throw new Error("nested state was frozen");
        })"#,
        "state-test.js",
        connection.clone(),
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();
    let invocation = session.invoke(State::default(), controls);
    tokio::pin!(invocation);
    let message = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::select! {
                result = &mut invocation => panic!("state invocation completed before bridge response: {result:?}"),
                message = peer.next() => break message.unwrap().unwrap(),
            }
        }
    })
    .await
    .unwrap();
    let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
    assert_eq!(request.get("type"), Some(&json!("StateReq")));
    assert_eq!(
        request.get("payload"),
        Some(&json!({ "keys": ["score", "items"] })),
    );
    peer.send(Message::Text(
        json!({
            "uuid": request["uuid"],
            "type": "StateRes",
            "payload": { "value": { "score": 42, "items": [1, { "ok": true }, null] } },
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    invocation.await.unwrap();
    connection.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn rev_invoke_sends_button_path_and_reports_plugin_errors() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    let remote_error = "invoke target is not interactable: 'scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_content[0]/scroll_view[0]/viewport[0]/content[0]/zodiac_upgrade_row[12]/buy_button[0]' -- complete remote error";
    for (response_type, payload, expect_error, paused) in [
        ("InvokeRes", json!({}), false, false),
        ("InvokeRes", json!({}), false, true),
        ("RemoteError", json!({ "message": remote_error }), true, false),
        ("RemoteError", json!({ "message": remote_error }), true, true),
    ] {
        let (address, peer_rx) = raw_server().await;
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let session = ScriptSession::new_with_connection(
            r#"export default (async () => await rev.invoke("Canvas/Buy DTP & More"))"#,
            "invoke-test.js",
            connection.clone(),
        ).await.unwrap();
        let (controls, _) = recording_controls();
        controls.actions_paused.set_paused(paused);
        let invocation = session.invoke(State::default(), controls);
        tokio::pin!(invocation);
        let message = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                tokio::select! {
                    result = &mut invocation => panic!("invoke completed before bridge response: {result:?}"),
                    message = peer.next() => break message.unwrap().unwrap(),
                }
            }
        })
        .await
        .unwrap();
        let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(request.get("type"), Some(&json!("InvokeReq")));
        assert_eq!(
            request.get("payload"),
            Some(&json!({ "path": "Canvas/Buy DTP & More" })),
        );
        peer.send(Message::Text(
            json!({ "uuid": request["uuid"], "type": response_type, "payload": payload })
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
        let result = invocation.await;
        if expect_error {
            assert!(result.unwrap_err().contains(remote_error));
        } else {
            result.unwrap();
        }
        connection.shutdown().await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn rev_invoke_rejects_empty_paths() {
    let session = ScriptSession::new(r#"export default (async () => await rev.invoke(" "))"#).await.unwrap();
    let (controls, _) = recording_controls();
    assert!(session.invoke(State::default(), controls).await.unwrap_err().contains("UI path"));
}

#[tokio::test(flavor = "current_thread")]
async fn rev_input_sends_text_and_awaits_success_or_remote_error() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    for (text, response_type, payload, paused) in [
        ("123", "InputRes", json!({}), false),
        ("123", "InputRes", json!({}), true),
        ("", "InputRes", json!({}), false),
        ("文字 \"value\"\n", "RemoteError", json!({ "message": "input target is read-only" }), true),
    ] {
        let (address, peer_rx) = raw_server().await;
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx).await.unwrap().unwrap();
        let session = ScriptSession::new_with_connection(
            &format!(r#"export default async () => await rev.input("Canvas/Input", {})"#, json!(text)),
            "input-test.js",
            connection.clone(),
        ).await.unwrap();
        let (controls, _) = recording_controls();
        controls.actions_paused.set_paused(paused);
        let invocation = session.invoke(State::default(), controls);
        tokio::pin!(invocation);
        let message = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                result = &mut invocation => panic!("input completed before bridge response: {result:?}"),
                message = peer.next() => message.unwrap().unwrap(),
            }
        }).await.unwrap();
        let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(request["type"], "InputReq");
        assert_eq!(request["payload"], json!({ "path": "Canvas/Input", "text": text }));
        peer.send(Message::Text(
            json!({ "uuid": request["uuid"], "type": response_type, "payload": payload }).to_string().into(),
        )).await.unwrap();
        if response_type == "RemoteError" {
            assert!(invocation.await.unwrap_err().contains("input target is read-only"));
        } else {
            invocation.await.unwrap();
        }
        connection.shutdown().await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn rev_input_rejects_blank_paths() {
    let session = ScriptSession::new(r#"export default async () => await rev.input(" ", "123")"#).await.unwrap();
    let (controls, _) = recording_controls();
    assert!(session.invoke(State::default(), controls).await.unwrap_err().contains("UI path"));
}

#[tokio::test(flavor = "current_thread")]
async fn rev_scroll_into_view_works_while_paused_and_rejects_empty_paths() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    let (address, peer_rx) = raw_server().await;
    let connection = WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_secs(1),
    );
    let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
        .await
        .unwrap()
        .unwrap();
    let session = ScriptSession::new_with_connection(
        r#"export default (async () => await rev.scrollIntoView("Canvas/Relic"))"#,
        "scroll-into-view-test.js",
        connection.clone(),
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();
    controls.actions_paused.set_paused(true);
    let invocation = session.invoke(State::default(), controls);
    tokio::pin!(invocation);
    let message = tokio::time::timeout(Duration::from_secs(1), async {
        tokio::select! {
            result = &mut invocation => panic!("scrollIntoView completed before bridge response: {result:?}"),
            message = peer.next() => message.unwrap().unwrap(),
        }
    })
    .await
    .unwrap();
    let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
    assert_eq!(request["type"], "ScrollIntoViewReq");
    assert_eq!(request["payload"], json!({ "path": "Canvas/Relic" }));
    peer.send(Message::Text(
        json!({ "uuid": request["uuid"], "type": "ScrollIntoViewRes", "payload": {} })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    invocation.await.unwrap();
    connection.shutdown().await;

    let session = ScriptSession::new(r#"export default (async () => await rev.scrollIntoView(" "))"#).await.unwrap();
    let (controls, _) = recording_controls();
    assert!(session.invoke(State::default(), controls).await.unwrap_err().contains("UI path"));
}

#[tokio::test(flavor = "current_thread")]
async fn rev_slot_returns_arbitrary_data_null_and_remote_errors() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    for (response_type, payload, expected) in [
        ("SlotRes", json!({ "value": { "level": 12, "effects": ["speed", null] } }), json!({ "level": 12, "effects": ["speed", null] })),
        ("SlotRes", json!({ "value": null }), Value::Null),
        ("RemoteError", json!({ "message": "slot target not found" }), Value::Null),
    ] {
        let (address, peer_rx) = raw_server().await;
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx).await.unwrap().unwrap();
        let session = ScriptSession::new_with_connection(
            &format!(r#"export default async () => {{
                const value = await rev.slot("scene:1/Canvas[0]/Slot[3]");
                if (JSON.stringify(value) !== JSON.stringify({expected})) throw new Error("unexpected slot value");
            }}"#),
            "slot-test.js",
            connection.clone(),
        ).await.unwrap();
        let (controls, _) = recording_controls();
        controls.actions_paused.set_paused(true);
        let invocation = session.invoke(State::default(), controls);
        tokio::pin!(invocation);
        let message = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                result = &mut invocation => panic!("slot completed before bridge response: {result:?}"),
                message = peer.next() => message.unwrap().unwrap(),
            }
        }).await.unwrap();
        let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(request["type"], "SlotReq");
        assert_eq!(request["payload"], json!({ "path": "scene:1/Canvas[0]/Slot[3]" }));
        peer.send(Message::Text(
            json!({ "uuid": request["uuid"], "type": response_type, "payload": payload }).to_string().into(),
        )).await.unwrap();
        if response_type == "RemoteError" {
            assert!(invocation.await.unwrap_err().contains("slot target not found"));
        } else {
            invocation.await.unwrap();
        }
        connection.shutdown().await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn rev_slot_rejects_blank_paths() {
    let session = ScriptSession::new(r#"export default async () => await rev.slot(" ")"#).await.unwrap();
    let (controls, _) = recording_controls();
    assert!(session.invoke(State::default(), controls).await.unwrap_err().contains("slot path must not be empty"));
}

#[tokio::test(flavor = "current_thread")]
async fn rev_transfer_sends_source_and_destination_paths_while_paused() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    let (address, peer_rx) = raw_server().await;
    let connection = WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_secs(1),
    );
    let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
        .await
        .unwrap()
        .unwrap();
    let session = ScriptSession::new_with_connection(
        r#"export default (async () => await rev.transfer("scene:1/Canvas[0]/Inventory/3", "scene:1/Canvas[0]/Combine/0"))"#,
        "transfer-test.js",
        connection.clone(),
    ).await.unwrap();
    let (controls, _) = recording_controls();
    controls.actions_paused.set_paused(true);
    let invocation = session.invoke(State::default(), controls);
    tokio::pin!(invocation);
    let message = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::select! {
                result = &mut invocation => panic!("transfer completed before bridge response: {result:?}"),
                message = peer.next() => break message.unwrap().unwrap(),
            }
        }
    })
    .await
    .unwrap();
    let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
    assert_eq!(request.get("type"), Some(&json!("TransferReq")));
    assert_eq!(
        request.get("payload"),
        Some(&json!({
            "source": "scene:1/Canvas[0]/Inventory/3",
            "destination": "scene:1/Canvas[0]/Combine/0",
        })),
    );
    peer.send(Message::Text(
        json!({ "uuid": request["uuid"], "type": "TransferRes", "payload": {} })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    invocation.await.unwrap();
    connection.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn rev_transfer_reports_errors_and_rejects_blank_paths() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    let (address, peer_rx) = raw_server().await;
    let connection = WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_secs(1),
    );
    let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
        .await
        .unwrap()
        .unwrap();
    let session = ScriptSession::new_with_connection(
        r#"export default (async () => await rev.transfer("scene:1/Canvas[0]/Inventory/3", "scene:1/Canvas[0]/Combine/0"))"#,
        "transfer-error-test.js",
        connection.clone(),
    ).await.unwrap();
    let (controls, _) = recording_controls();
    let invocation = session.invoke(State::default(), controls);
    tokio::pin!(invocation);
    let message = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::select! {
                result = &mut invocation => panic!("transfer completed before bridge response: {result:?}"),
                message = peer.next() => break message.unwrap().unwrap(),
            }
        }
    })
    .await
    .unwrap();
    let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
    assert_eq!(request.get("type"), Some(&json!("TransferReq")));
    peer.send(Message::Text(
        json!({
            "uuid": request["uuid"],
            "type": "RemoteError",
            "payload": { "message": "path lookup failed" },
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let error = invocation.await.unwrap_err();
    assert!(error.contains("path lookup failed"));
    connection.shutdown().await;

    for script in [
        r#"export default (async () => await rev.transfer(" ", "scene:1/Canvas[0]/Combine/0"))"#,
        r#"export default (async () => await rev.transfer("scene:1/Canvas[0]/Inventory/3", " "))"#,
    ] {
        let session = ScriptSession::new(script).await.unwrap();
        let (controls, _) = recording_controls();
        assert!(session.invoke(State::default(), controls).await.unwrap_err().contains("slot paths"));
    }

}

#[tokio::test(flavor = "current_thread")]
async fn rev_state_unwraps_single_key_request() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    let (address, peer_rx) = raw_server().await;
    let connection = WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_secs(1),
    );
    let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
        .await
        .unwrap()
        .unwrap();
    let session = ScriptSession::new_with_connection(
        r#"export default (async () => {
            const state = await rev.state("EP");
            if (state !== "0e0") throw new Error("single-key state was not unwrapped: " + JSON.stringify(state));
        })"#,
        "state-single-key-test.js",
        connection.clone(),
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();
    let invocation = session.invoke(State::default(), controls);
    tokio::pin!(invocation);
    let message = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::select! {
                result = &mut invocation => panic!("single-key invocation completed before bridge response: {result:?}"),
                message = peer.next() => break message.unwrap().unwrap(),
            }
        }
    })
    .await
    .unwrap();
    let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
    assert_eq!(request.get("type"), Some(&json!("StateReq")));
    assert_eq!(request.get("payload"), Some(&json!({ "keys": ["EP"] })));
    peer.send(Message::Text(
        json!({
            "uuid": request["uuid"],
            "type": "StateRes",
            "payload": { "value": { "EP": "0e0" } },
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    invocation.await.unwrap();
    connection.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn rev_state_uses_native_freeze_after_global_freeze_is_replaced() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    let (address, peer_rx) = raw_server().await;
    let connection = WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_secs(1),
    );
    let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
        .await
        .unwrap()
        .unwrap();
    let session = ScriptSession::new_with_connection(
        r#"export default (async () => {
            Object.freeze = (value) => value;
            const state = await rev.state();
            if (!Object.isFrozen(state)) throw new Error("state is not natively frozen");
        })"#,
        "state-native-freeze-test.js",
        connection.clone(),
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();
    let invocation = session.invoke(State::default(), controls);
    tokio::pin!(invocation);
    let message = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::select! {
                result = &mut invocation => panic!("native-freeze invocation completed before bridge response: {result:?}"),
                message = peer.next() => break message.unwrap().unwrap(),
            }
        }
    })
    .await
    .unwrap();
    let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
    peer.send(Message::Text(
        json!({
            "uuid": request["uuid"],
            "type": "StateRes",
            "payload": { "value": { "score": 42 } },
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    invocation.await.unwrap();
    connection.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn rev_state_uses_native_parse_after_global_parse_is_replaced() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    let (address, peer_rx) = raw_server().await;
    let connection = WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_secs(1),
    );
    let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
        .await
        .unwrap()
        .unwrap();
    let session = ScriptSession::new_with_connection(
        r#"export default (async () => {
            JSON.parse = () => ({ score: -1 });
            const state = await rev.state();
            if (state.score !== 42) throw new Error("state did not use native parse");
        })"#,
        "state-native-parse-test.js",
        connection.clone(),
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();
    let invocation = session.invoke(State::default(), controls);
    tokio::pin!(invocation);
    let message = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::select! {
                result = &mut invocation => panic!("native-parse invocation completed before bridge response: {result:?}"),
                message = peer.next() => break message.unwrap().unwrap(),
            }
        }
    })
    .await
    .unwrap();
    let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
    peer.send(Message::Text(
        json!({
            "uuid": request["uuid"],
            "type": "StateRes",
            "payload": { "value": { "score": 42 } },
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    invocation.await.unwrap();
    connection.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn rev_state_rejects_malformed_json_as_promise_error() {
    use crate::bridge::{test_support::raw_server, WsConnection};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

    let (address, peer_rx) = raw_server().await;
    let connection = WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_secs(1),
    );
    let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
        .await
        .unwrap()
        .unwrap();
    let session = ScriptSession::new_with_connection(
        r#"export default (async () => await rev.state())"#,
        "state-invalid-test.js",
        connection.clone(),
    )
        .await
        .unwrap();
    let (controls, _) = recording_controls();
    let invocation = session.invoke(State::default(), controls);
    tokio::pin!(invocation);
    let message = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::select! {
                result = &mut invocation => panic!("invalid-state invocation completed before bridge response: {result:?}"),
                message = peer.next() => break message.unwrap().unwrap(),
            }
        }
    })
    .await
    .unwrap();
    let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
    peer.send(Message::Text(
        json!({
            "uuid": request["uuid"],
            "type": "StateRes",
            "payload": { "value": "not an object" },
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let error = invocation.await.unwrap_err();
    assert!(error.contains("state"), "missing state rejection: {error}");
    connection.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn resize_rejects_invalid_dimensions_without_window_work() {
    for source in [r#"export default (() => rev.resize(0, 720))"#, r#"export default (() => rev.resize(-1, 720))"#, r#"export default (() => rev.resize(1.5, 720))"#, r#"export default (() => rev.resize(Infinity, 720))"#] {
        let (controls, events) = recording_controls();
        let session = ScriptSession::new(source).await.unwrap();
        assert!(session.invoke(State::default(), controls).await.is_err());
        assert!(events.borrow().is_empty());
    }
    let (controls, events) = recording_controls();
    let session = ScriptSession::new(r#"export default (() => rev.resize(1280, 720))"#).await.unwrap();
    session.invoke(State::default(), controls).await.unwrap();
    assert_eq!(*events.borrow(), vec![HostEvent::Resize(1280, 720)]);
}

#[tokio::test(flavor = "current_thread")]
async fn script_errors_include_message_and_stack() {
    let session = ScriptSession::new(
        r#"export default (() => { function fail() { throw new Error("boom"); } fail(); })"#,
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();

    let error = session
        .invoke(State::default(), controls)
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("boom"), "missing exception message: {error}");
    assert!(error.contains("fail"), "missing exception stack: {error}");
}

#[tokio::test(flavor = "current_thread")]
async fn async_script_errors_include_message_and_stack() {
    let session = ScriptSession::new(
        r#"export default (async () => { await rev.sleep(0); function fail() { throw new Error("async boom"); } fail(); })"#,
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();

    let error = session
        .invoke(State::default(), controls)
        .await
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("async boom"),
        "missing async exception message: {error}"
    );
    assert!(
        error.contains("fail"),
        "missing async exception stack: {error}"
    );
}

async fn run_with_mouse(
    commands: mpsc::Receiver<ScriptCommand>, states: watch::Receiver<State>,
    initial_path: std::path::PathBuf, mouse: SharedMouse, loop_delay: Duration,
) -> Result<(), String> {
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    run_with_controls(commands, states, pause_rx, Some(initial_path), mouse.into(), loop_delay).await
}

async fn run_with_optional_mouse(
    commands: mpsc::Receiver<ScriptCommand>, states: watch::Receiver<State>,
    initial_path: Option<std::path::PathBuf>, mouse: SharedMouse, loop_delay: Duration,
) -> Result<(), String> {
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    run_with_controls(commands, states, pause_rx, initial_path, mouse.into(), loop_delay).await
}

impl MouseInput for FakeMouse {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), MouseInputError> {
        self.clicks.borrow_mut().push(Click { x, y, button });
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn no_initial_script_starts_stopped_until_loaded() {
    use std::fs;

    let path = std::env::temp_dir().join(format!(
        "rev-idle-no-initial-script-test-{}.js",
        std::process::id(),
    ));
    fs::write(&path, r#"export default (() => rev.click(1, 1, "left"))"#).unwrap();

    let clicks = Rc::new(RefCell::new(Vec::new()));
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: clicks.clone(),
    }));
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let local = tokio::task::LocalSet::new();
    let cleanup_path = path.clone();

    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_optional_mouse(
                command_rx,
                state_rx,
                None,
                mouse,
                Duration::from_millis(5),
            ));

            tokio::time::sleep(Duration::from_millis(15)).await;
            assert!(clicks.borrow().is_empty());

            command_tx.send(ScriptCommand::Load(path.clone())).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if !clicks.borrow().is_empty() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();

            command_tx.send(ScriptCommand::Exit).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        })
        .await;

    fs::remove_file(cleanup_path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn script_can_import_a_sibling_module_via_relative_path() {
    use std::fs;

    let root = std::env::temp_dir().join(format!(
        "rev-idle-import-test-{}",
        std::process::id(),
    ));
    fs::create_dir_all(&root).unwrap();
    let helper_path = root.join("helper.js");
    let entry_path = root.join("entry.js");

    fs::write(
        &helper_path,
        r#"export function clickHelper() { rev.click(7, 8, "right"); }"#,
    )
    .unwrap();
    fs::write(
        &entry_path,
        r#"
            import { clickHelper } from './helper.js';
            export default (() => clickHelper());
        "#,
    )
    .unwrap();

    let clicks = Rc::new(RefCell::new(Vec::new()));
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: clicks.clone(),
    }));
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let local = tokio::task::LocalSet::new();
    let observed_clicks = clicks.clone();

    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                entry_path.clone(),
                mouse,
                Duration::from_millis(5),
            ));

            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if !observed_clicks.borrow().is_empty() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();

            command_tx.send(ScriptCommand::Exit).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        })
        .await;

    assert_eq!(
        clicks.borrow().as_slice(),
        &[Click { x: 7, y: 8, button: Button::Right }]
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn import_from_a_subdirectory_resolves_without_an_explicit_js_extension() {
    use std::fs;

    let root = std::env::temp_dir().join(format!(
        "rev-idle-import-subdir-test-{}",
        std::process::id(),
    ));
    let lib_dir = root.join("lib");
    fs::create_dir_all(&lib_dir).unwrap();
    let helper_path = lib_dir.join("action.js");
    let entry_path = root.join("entry.js");

    fs::write(
        &helper_path,
        r#"export function clickHelper() { rev.click(9, 10, "middle"); }"#,
    )
    .unwrap();
    fs::write(
        &entry_path,
        r#"
            import { clickHelper } from './lib/action';
            export default (() => clickHelper());
        "#,
    )
    .unwrap();

    let clicks = Rc::new(RefCell::new(Vec::new()));
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: clicks.clone(),
    }));
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let local = tokio::task::LocalSet::new();
    let observed_clicks = clicks.clone();

    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                entry_path.clone(),
                mouse,
                Duration::from_millis(5),
            ));

            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if !observed_clicks.borrow().is_empty() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();

            command_tx.send(ScriptCommand::Exit).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        })
        .await;

    assert_eq!(
        clicks.borrow().as_slice(),
        &[Click { x: 9, y: 10, button: Button::Middle }]
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn dynamic_import_observes_updated_module_source() {
    use std::fs;

    let root = std::env::temp_dir().join(format!(
        "rev-idle-dynamic-import-refresh-test-{}",
        std::process::id(),
    ));
    fs::create_dir_all(&root).unwrap();
    let config_path = root.join("config.js");
    let entry_path = root.join("entry.js");
    fs::write(&config_path, "export default { x: 1 };").unwrap();
    let source = r#"
        export default (async () => {
            const config = (await import("./config.js")).default;
            rev.click(config.x, 0);
        });
    "#;

    let session = ScriptSession::new_with_connection(
        source,
        &entry_path.to_string_lossy(),
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .unwrap();
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: clicks.clone(),
    }));

    session
        .invoke(State::default(), mouse.clone())
        .await
        .unwrap();
    fs::write(&config_path, "export default { x: 2 };").unwrap();
    session.invoke(State::default(), mouse).await.unwrap();

    assert_eq!(
        clicks.borrow().as_slice(),
        &[
            Click {
                x: 1,
                y: 0,
                button: Button::Left,
            },
            Click {
                x: 2,
                y: 0,
                button: Button::Left,
            },
        ]
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn passes_fresh_state_and_preserves_globals() {
    let source = r#"
        export default (async () => {
            if (!Object.isFrozen(rev)) {
                throw new Error("rev and state must be frozen");
            }
            globalThis.count = (globalThis.count ?? 0) + 1;
            if (globalThis.count === 2) {
                rev.click(-12, 34, "right");
            }
            await rev.sleep(1);
        })
    "#;

    let session = ScriptSession::new(source).await.unwrap();
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: clicks.clone(),
    }));

    session
        .invoke(
            State {
                score: Some("1e1".to_owned()),
                sequence: 1,
                received_at_ms: Some(10),
            },
            mouse.clone(),
        )
        .await
        .unwrap();
    session
        .invoke(
            State {
                score: Some("2e2".to_owned()),
                sequence: 2,
                received_at_ms: Some(20),
            },
            mouse,
        )
        .await
        .unwrap();

    assert_eq!(
        clicks.borrow().as_slice(),
        &[Click {
            x: -12,
            y: 34,
            button: Button::Right,
        }]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn source_replacing_freeze_still_receives_frozen_host_objects() {
    let session = ScriptSession::new(
        r#"
            export default (() => {
                Object.freeze = (value) => value;
                return (() => {
                    if (!Object.isFrozen(rev)) {
                        throw new Error("rev and state must be frozen");
                    }
                    globalThis.calls = (globalThis.calls ?? 0) + 1;
                });
            })()
        "#,
    )
    .await
    .unwrap();
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: Rc::new(RefCell::new(Vec::new())),
    }));

    session
        .invoke(State::default(), mouse.clone())
        .await
        .unwrap();
    session.invoke(State::default(), mouse).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn invocation_replacing_freeze_does_not_affect_later_host_objects() {
    let session = ScriptSession::new(
        r#"
            export default (() => {
                if (!Object.isFrozen(rev)) {
                    throw new Error("rev and state must be frozen");
                }
                globalThis.calls = (globalThis.calls ?? 0) + 1;
                if (globalThis.calls === 1) {
                    Object.freeze = (value) => value;
                }
            })
        "#,
    )
    .await
    .unwrap();
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: Rc::new(RefCell::new(Vec::new())),
    }));

    session
        .invoke(State::default(), mouse.clone())
        .await
        .unwrap();
    session.invoke(State::default(), mouse).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn invocation_error_does_not_reset_globals() {
    let session = ScriptSession::new(
        r#"
            export default (() => {
                globalThis.count = (globalThis.count ?? 0) + 1;
                if (globalThis.count === 1) {
                    throw new Error("first call");
                }
                if (globalThis.count !== 2) {
                    throw new Error("globals were reset");
                }
            })
        "#,
    )
    .await
    .unwrap();
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: Rc::new(RefCell::new(Vec::new())),
    }));

    assert!(
        session
            .invoke(State::default(), mouse.clone())
            .await
            .is_err()
    );
    session.invoke(State::default(), mouse).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn global_round_trips_values_and_lists_keys() {
    let session = ScriptSession::new(
        r#"export default (() => {
            rev.global.count = 1;
            rev.global.nested = { a: [1, 2, 3] };
            if (rev.global.count !== 1) throw new Error("number did not round-trip");
            if (JSON.stringify(rev.global.nested) !== JSON.stringify({ a: [1, 2, 3] })) {
                throw new Error("object did not round-trip");
            }
            if (rev.global.missing !== undefined) throw new Error("missing key must be undefined");
            if (!("count" in rev.global) || !("nested" in rev.global)) {
                throw new Error("has() must see stored keys");
            }
            const keys = Object.keys(rev.global);
            if (!keys.includes("count") || !keys.includes("nested")) {
                throw new Error("Object.keys() must list stored keys");
            }
            delete rev.global.count;
            if ("count" in rev.global) throw new Error("delete must remove the key");
            delete rev.global.nested;
        })"#,
    )
    .await
    .unwrap();
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: Rc::new(RefCell::new(Vec::new())),
    }));

    session.invoke(State::default(), mouse).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn global_survives_a_new_script_session_simulating_reload() {
    let first = ScriptSession::new(r#"export default (() => { rev.global.reload_probe = 42; })"#)
        .await
        .unwrap();
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: Rc::new(RefCell::new(Vec::new())),
    }));
    first.invoke(State::default(), mouse.clone()).await.unwrap();

    // A fresh ScriptSession is exactly what `load`/`reload` builds: a new
    // AsyncRuntime/AsyncContext, so a plain JS global would not survive it.
    let second = ScriptSession::new(
        r#"export default (() => {
            if (rev.global.reload_probe !== 42) {
                throw new Error("global did not survive a new script session");
            }
            delete rev.global.reload_probe;
        })"#,
    )
    .await
    .unwrap();
    second.invoke(State::default(), mouse).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn missing_state_fields_are_null() {
    let session = ScriptSession::new(
        r#"export default (() => {
            if (typeof rev.state !== "function") {
                throw new Error("state must be callable");
            }
        })"#,
    )
    .await
    .unwrap();
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: Rc::new(RefCell::new(Vec::new())),
    }));

    session.invoke(State::default(), mouse).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn console_log_and_error_accept_multiple_values() {
    let session = ScriptSession::new(
        r#"export default (() => {
            console.log("hello", { answer: 42 });
            console.error("problem", 7);
        })"#,
    )
    .await
    .unwrap();
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: Rc::new(RefCell::new(Vec::new())),
    }));

    session.invoke(State::default(), mouse).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn console_message_formats_errors_by_message_instead_of_json_stringify() {
    let runtime = AsyncRuntime::new().unwrap();
    let context = AsyncContext::full(&runtime).await.unwrap();

    let message = context
        .async_with(async move |ctx| {
            let error: Value = ctx.eval("new Error('boom')").unwrap();
            format_console_message(Rest(vec![error])).unwrap()
        })
        .await;

    // `JSON.stringify(new Error(...))` is "{}" because Error properties
    // aren't enumerable, so a naive object formatter loses the message.
    assert_ne!(message, "{}", "error should not be formatted as an empty object");
    assert!(message.contains("boom"), "expected the error message in: {message}");
}

#[tokio::test(flavor = "current_thread")]
async fn sleep_is_awaited_and_non_callable_source_is_rejected() {
    let session = ScriptSession::new(
        "export default (async () => { await rev.sleep(20); })",
    )
    .await
    .unwrap();
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: Rc::new(RefCell::new(Vec::new())),
    }));
    let started = Instant::now();

    let invocation = session.invoke(State::default(), mouse);
    tokio::pin!(invocation);
    tokio::select! {
        result = &mut invocation => {
            panic!("sleep completed before yielding: {result:?}")
        }
        _ = tokio::time::sleep(Duration::from_millis(1)) => {}
    }
    invocation.await.unwrap();

    assert!(started.elapsed() >= Duration::from_millis(20));
    assert!(ScriptSession::new("42").await.is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_host_arguments_throw_without_clicking() {
    let clicks = Rc::new(RefCell::new(Vec::new()));

    for source in [
        r#"export default (() => rev.click(1, 2, "side"))"#,
        r#"export default (() => rev.click(1.5, 2, "left"))"#,
        r#"export default (() => rev.sleep(-1))"#,
        r#"export default (() => rev.sleep(1.5))"#,
    ] {
        let session = ScriptSession::new(source).await.unwrap();
        let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
            clicks: clicks.clone(),
        }));
        assert!(session.invoke(State::default(), mouse).await.is_err());
    }

    assert!(clicks.borrow().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn clickn_repeats_clicks_and_allows_zero_clicks() {
    let session = ScriptSession::new(
        r#"
            export default (async () => {
                await rev.clickn(10, 20, 3, "right");
                await rev.clickn(30, 40, 0);
            })
        "#,
    )
    .await
    .unwrap();
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: clicks.clone(),
    }));

    let started = Instant::now();
    session.invoke(State::default(), mouse).await.unwrap();

    assert!(started.elapsed() >= Duration::from_millis(20));
    assert_eq!(
        clicks.borrow().as_slice(),
        &[
            Click { x: 10, y: 20, button: Button::Right },
            Click { x: 10, y: 20, button: Button::Right },
            Click { x: 10, y: 20, button: Button::Right },
        ]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn scroll_posts_through_window_control_for_named_axes_and_short_aliases() {
    let session = ScriptSession::new(
        r#"
            export default (async () => {
                await rev.scroll(10, 20, 2, "vertical");
                await rev.scroll(-30, 40, -1, "h");
                await rev.scroll(50, -60, 3, "v");
                await rev.scroll(-70, -80, -4, "horizontal");
            })
        "#,
    )
    .await
    .unwrap();
    let (controls, events) = recording_controls();

    session.invoke(State::default(), controls).await.unwrap();

    assert_eq!(
        *events.borrow(),
        vec![
            HostEvent::Scroll(10, 20, 2, Axis::Vertical),
            HostEvent::Scroll(-30, 40, -1, Axis::Horizontal),
            HostEvent::Scroll(50, -60, 3, Axis::Vertical),
            HostEvent::Scroll(-70, -80, -4, Axis::Horizontal),
        ]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn repeated_click_and_scroll_arguments_are_validated() {
    for source in [
        r#"export default (() => rev.clickn(1, 2, -1))"#,
        r#"export default (() => rev.clickn(1, 2, 1.5))"#,
        r#"export default (() => rev.clickn(1, 2, Infinity))"#,
        r#"export default (() => rev.scroll(1.5, 2, 1))"#,
        r#"export default (() => rev.scroll(1, Infinity, 1))"#,
        r#"export default (() => rev.scroll(1, 2, 1.5))"#,
        r#"export default (() => rev.scroll(1, 2, 1, "diagonal"))"#,
        r#"export default (() => rev.scroll(1, 2, 1, null))"#,
    ] {
        let session = ScriptSession::new(source).await.unwrap();
        let (controls, events) = recording_controls();
        assert!(session.invoke(State::default(), controls).await.is_err());
        assert!(events.borrow().is_empty());
    }
}

#[tokio::test(flavor = "current_thread")]
async fn click_button_defaults_accepts_valid_and_rejects_invalid_values() {
    let session = ScriptSession::new(
        r#"
            export default (() => {
                for (const button of [null, "side", true, 1, {}, []]) {
                    let threw = false;
                    try {
                        rev.click(100, 200, button);
                    } catch (_) {
                        threw = true;
                    }
                    if (!threw) {
                        throw new Error("invalid button must throw");
                    }
                }

                rev.click(1, 11);
                rev.click(2, 12, undefined);
                rev.click(3, 13, "left");
                rev.click(4, 14, "right");
                rev.click(5, 15, "middle");
            })
        "#,
    )
    .await
    .unwrap();
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
        clicks: clicks.clone(),
    }));

    session.invoke(State::default(), mouse).await.unwrap();

    assert_eq!(
        clicks.borrow().as_slice(),
        &[
            Click {
                x: 1,
                y: 11,
                button: Button::Left,
            },
            Click {
                x: 2,
                y: 12,
                button: Button::Left,
            },
            Click {
                x: 3,
                y: 13,
                button: Button::Left,
            },
            Click {
                x: 4,
                y: 14,
                button: Button::Right,
            },
            Click {
                x: 5,
                y: 15,
                button: Button::Middle,
            },
        ]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn console_commands_control_context_lifetime() {
    use crate::app::ScriptCommand;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    use tokio::sync::{mpsc, watch};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "rev-idle-script-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir_all(&root).unwrap();
    let first = root.join("first.js");
    let second = root.join("second.js");

    fs::write(
        &first,
        r#"
            export default (() => {
                globalThis.count = (globalThis.count ?? 0) + 1;
                rev.click(globalThis.count, 0);
            })
        "#,
    )
    .unwrap();
    fs::write(
        &second,
        r#"
            export default (() => {
                globalThis.count = (globalThis.count ?? 0) + 1;
                rev.click(globalThis.count, 0, "right");
            })
        "#,
    )
    .unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct ChannelMouse(mpsc::UnboundedSender<(i32, i32, Button)>);
    impl MouseInput for ChannelMouse {
        fn click_at(
            &mut self,
            x: i32,
            y: i32,
            button: Button,
        ) -> Result<(), MouseInputError> {
            self.0.send((x, y, button)).map_err(|error| error.to_string().into())
        }
    }

    let mouse: SharedMouse = Rc::new(RefCell::new(ChannelMouse(event_tx)));
    let (command_tx, command_rx) = mpsc::channel(32);
    let (_state_tx, state_rx) = watch::channel(State::default());

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                first.clone(),
                mouse,
                Duration::from_millis(5),
            ));

            assert_eq!(
                event_rx.recv().await,
                Some((1, 0, Button::Left))
            );

            command_tx.send(ScriptCommand::Resume).await.unwrap();
            assert!(event_rx.recv().await.unwrap().0 >= 2);

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            while event_rx.try_recv().is_ok() {}
            assert!(
                tokio::time::timeout(
                    Duration::from_millis(15),
                    event_rx.recv(),
                )
                .await
                .is_err()
            );

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            command_tx.send(ScriptCommand::Resume).await.unwrap();
            let resumed = event_rx.recv().await.unwrap();
            assert!(resumed.0 >= 2);

            command_tx.send(ScriptCommand::Reload).await.unwrap();
            loop {
                if event_rx.recv().await == Some((1, 0, Button::Left)) {
                    break;
                }
            }

            command_tx
                .send(ScriptCommand::Load(second.clone()))
                .await
                .unwrap();
            loop {
                if event_rx.recv().await == Some((1, 0, Button::Right)) {
                    break;
                }
            }

            command_tx.send(ScriptCommand::Stop).await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            while event_rx.try_recv().is_ok() {}
            assert!(
                tokio::time::timeout(
                    Duration::from_millis(15),
                    event_rx.recv(),
                )
                .await
                .is_err()
            );

            command_tx.send(ScriptCommand::Stop).await.unwrap();
            command_tx.send(ScriptCommand::Pause).await.unwrap();
            command_tx.send(ScriptCommand::Resume).await.unwrap();
            command_tx.send(ScriptCommand::Reload).await.unwrap();
            loop {
                if event_rx.recv().await == Some((1, 0, Button::Right)) {
                    break;
                }
            }

            runner.abort();
            let _ = runner.await;
        })
        .await;

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn paused_default_failure_stops_after_resume() {
    let path = std::env::temp_dir().join(format!("rev-idle-paused-failure-{}.js", uuid::Uuid::new_v4()));
    std::fs::write(&path, r#"
        let reject;
        export default async function() {
            rev.click(1, 1);
            await new Promise((_, fail) => { reject = fail; });
        }
        export function beforePause() { reject(new Error('paused failure')); }
        export function beforeStop() { rev.click(9, 9); }
    "#).unwrap();
    let (controls, events) = recording_controls();
    let (commands, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (states, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, states, pause_rx, Some(path.clone()), controls, LockState::default(), Duration::from_millis(2),
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            while events.borrow().is_empty() { tokio::task::yield_now().await; }
        }).await.unwrap();
        commands.send(ScriptCommand::Pause).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while state_rx.borrow().phase != ScriptPhase::Paused { state_rx.changed().await.unwrap(); }
        }).await.unwrap();
        assert_eq!(*events.borrow(), vec![HostEvent::Click(1, 1, Button::Left)]);
        commands.send(ScriptCommand::Resume).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while state_rx.borrow().phase != ScriptPhase::Stopped { state_rx.changed().await.unwrap(); }
        }).await.unwrap();
        commands.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
    }).await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn pause_and_resume_hooks_run_around_the_actual_transition() {
    use crate::app::ScriptCommand;
    use std::fs;
    use tokio::sync::{mpsc, watch};

    let path = std::env::temp_dir().join(format!(
        "rev-idle-hooks-test-{}.js",
        std::process::id(),
    ));
    fs::write(
        &path,
        r#"
            export default (async () => { rev.click(0, 0); await rev.sleep(5); });
            export function beforePause() { rev.click(9, 9); }
            export function afterResume() { rev.click(8, 8); }
        "#,
    )
    .unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct HookMouse(mpsc::UnboundedSender<(i32, i32)>);
    impl MouseInput for HookMouse {
        fn click_at(
            &mut self,
            x: i32,
            y: i32,
            _button: Button,
        ) -> Result<(), MouseInputError> {
            self.0.send((x, y)).map_err(|error| error.to_string().into())
        }
    }

    let mouse: SharedMouse = Rc::new(RefCell::new(HookMouse(event_tx)));
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let local = tokio::task::LocalSet::new();
    let runner_path = path.clone();

    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                runner_path,
                mouse,
                Duration::from_millis(2),
            ));

            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap(),
                (0, 0),
                "script should click before its first sleep"
            );

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap(),
                (9, 9),
                "beforePause hook should run when the script actually pauses"
            );

            // Drain any trailing click from an invocation already in
            // flight, then confirm the script does not click again while
            // paused (i.e. the pause itself took effect after the hook).
            tokio::time::sleep(Duration::from_millis(20)).await;
            while event_rx.try_recv().is_ok() {}
            assert!(
                tokio::time::timeout(Duration::from_millis(30), event_rx.recv())
                    .await
                    .is_err(),
                "script must not click again while paused"
            );

            command_tx.send(ScriptCommand::Resume).await.unwrap();
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap(),
                (8, 8),
                "afterResume hook should run when the script actually resumes"
            );

            command_tx.send(ScriptCommand::Exit).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        })
        .await;

    fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn two_runtime_pause_hooks_and_connections() {
    use futures_util::StreamExt;
    use std::fs;
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};

    let path = std::env::temp_dir().join(format!(
        "rev-idle-lifecycle-hooks-test-{}.js",
        std::process::id(),
    ));
    fs::write(
        &path,
        r#"
            let invocations = 0;
            export function afterLoad() { rev.click(1, 1); }
            export function onConnect() { rev.click(2, 2); }
            export async function onDisconnect() { rev.click(3, 3); await rev.sleep(100); rev.click(4, 4); }
            export function beforePause() { rev.click(7, Number(rev.paused)); }
            export function afterResume() { rev.click(8, Number(rev.paused)); }
            export function beforeStop() { rev.click(99, 99); }
            export default async function() {
                if (++invocations > 1) rev.click(9, 9);
                await rev.state();
            }
        "#,
    )
    .unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct HookMouse(mpsc::UnboundedSender<(i32, i32)>);
    impl MouseInput for HookMouse {
        fn click_at(
            &mut self,
            x: i32,
            y: i32,
            _button: Button,
        ) -> Result<(), MouseInputError> {
            self.0.send((x, y)).map_err(|error| error.to_string().into())
        }
    }

    let gate = ActionGate::default();
    let controls = HostControls {
        mouse: Rc::new(RefCell::new(HookMouse(event_tx))),
        window: Rc::new(FakeWindow),
        actions_paused: gate.clone(),
    };
    let (command_tx, command_rx) = mpsc::channel(8);
    let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let reserved = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let connection = crate::bridge::WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_millis(100),
    );
    let cleanup_path = path.clone();
    let local = tokio::task::LocalSet::new();

    local
        .run_until(async move {
            let runner_connection = connection.clone();
            let runner = tokio::task::spawn_local(run_with_controls_and_connection(
                command_rx,
                runner_connection,
                pause_rx,
                Some(path),
                controls,
                Duration::from_millis(2),
            ));

            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await.unwrap().unwrap(),
                (1, 1),
                "afterLoad must run before the default function"
            );
            let listener = tokio::net::TcpListener::bind(address).await.unwrap();
            let (stream, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut peer = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
            assert_eq!(
                tokio::time::timeout(Duration::from_millis(300), event_rx.recv()).await.unwrap().unwrap(),
                (2, 2),
                "onConnect must run before the first default call"
            );
            tokio::time::timeout(Duration::from_secs(1), peer.next())
                .await
                .expect("script did not start an in-flight bridge request")
                .expect("peer closed before receiving the bridge request")
                .expect("bridge request was malformed");
            peer.close(None).await.unwrap();
            assert_eq!(
                tokio::time::timeout(Duration::from_millis(300), event_rx.recv()).await.unwrap().unwrap(),
                (3, 3),
                "onDisconnect must win over the in-flight request failure"
            );
            pause_tx.send_replace(gate.set_paused(true));
            assert_eq!(tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await.unwrap().unwrap(), (7, 0), "beforePause must run before acknowledgment");
            assert!(tokio::time::timeout(Duration::from_millis(30), event_rx.recv()).await.is_err(), "acknowledged pause must retain the in-flight disconnect hook");
            let (stream, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept()).await.unwrap().unwrap();
            let mut reconnected = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
            tokio::time::sleep(Duration::from_millis(30)).await;
            assert!(event_rx.try_recv().is_err(), "connection hooks must remain deferred while paused");
            reconnected.close(None).await.unwrap();
            drop(listener);
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert!(event_rx.try_recv().is_err(), "disconnect hooks must remain deferred while paused");
            command_tx.send(ScriptCommand::Resume).await.unwrap();
            let mut resumed = Vec::new();
            while resumed.len() < 5 {
                resumed.push(tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await.unwrap().unwrap());
            }
            assert_eq!(resumed, vec![(8, 0), (4, 4), (2, 2), (3, 3), (4, 4)]);
            let mut deferred_connection_hooks = vec!["disconnect"];
            deferred_connection_hooks.extend(resumed.iter().filter_map(|event| match event.0 { 2 => Some("connect"), 3 => Some("disconnect"), _ => None }));
            assert_eq!(deferred_connection_hooks, vec!["disconnect", "connect", "disconnect"]);

            command_tx.send(ScriptCommand::Exit).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            connection.shutdown().await;
            let unexpected = event_rx.try_recv();
            assert!(unexpected.is_err(), "Exit must skip beforeStop: {unexpected:?}");
        })
        .await;

    fs::remove_file(cleanup_path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn disconnected_script_waits_for_connect_and_survives_disconnect() {
    use futures_util::StreamExt;
    use std::fs;
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};

    let path = std::env::temp_dir().join(format!(
        "rev-idle-disconnected-lifecycle-test-{}.js",
        std::process::id(),
    ));
    fs::write(
        &path,
        r#"
            export function afterLoad() { rev.click(1, 1); }
            export function onConnect() { rev.click(2, 2); }
            export function onDisconnect() { rev.click(3, 3); }
            export function beforeStop() { rev.click(4, 4); }
            export default (async () => {
                rev.click(5, 5);
                await rev.state();
            });
        "#,
    )
    .unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct ConnectionMouse(mpsc::UnboundedSender<(i32, i32)>);
    impl MouseInput for ConnectionMouse {
        fn click_at(
            &mut self,
            x: i32,
            y: i32,
            _button: Button,
        ) -> Result<(), MouseInputError> {
            self.0.send((x, y)).map_err(|error| error.to_string().into())
        }
    }

    let controls = HostControls {
        mouse: Rc::new(RefCell::new(ConnectionMouse(event_tx))),
        window: Rc::new(FakeWindow),
        actions_paused: ActionGate::default(),
    };
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let reserved = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let connection = crate::bridge::WsConnection::connect_for_test(
        address,
        Duration::from_millis(20),
        Duration::from_millis(100),
    );
    let cleanup_path = path.clone();
    let local = tokio::task::LocalSet::new();

    local
        .run_until(async move {
            let runner_connection = connection.clone();
            let runner = tokio::task::spawn_local(run_with_controls_and_connection(
                command_rx,
                runner_connection,
                pause_rx,
                Some(path),
                controls,
                Duration::from_millis(2),
            ));

            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await.unwrap().unwrap(),
                (1, 1),
            );
            assert!(
                tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await.is_err(),
                "the default function must wait until the game connects",
            );

            let listener = tokio::net::TcpListener::bind(address).await.unwrap();
            let (stream, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept())
                .await
                .unwrap()
                .unwrap();
            drop(listener);
            let mut peer = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
            for expected in [(2, 2), (5, 5)] {
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await.unwrap().unwrap(),
                    expected,
                );
            }
            tokio::time::timeout(Duration::from_secs(1), peer.next())
                .await
                .expect("script did not start a bridge request after onConnect")
                .expect("peer closed before receiving the bridge request")
                .expect("bridge request was malformed");
            peer.close(None).await.unwrap();
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await.unwrap().unwrap(),
                (3, 3),
            );
            assert!(
                tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await.is_err(),
                "disconnect must keep the loaded session and its hooks alive",
            );

            let listener = tokio::net::TcpListener::bind(address).await.unwrap();
            let (stream, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut peer = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
            for expected in [(2, 2), (5, 5)] {
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await.unwrap().unwrap(),
                    expected,
                );
            }
            tokio::time::timeout(Duration::from_secs(1), peer.next())
                .await
                .expect("script did not restart its bridge request after reconnect")
                .expect("peer closed before receiving the bridge request")
                .expect("bridge request was malformed");

            command_tx.send(ScriptCommand::Stop).await.unwrap();

            command_tx.send(ScriptCommand::Exit).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(event_rx.try_recv().is_err(), "Stop and Exit must skip beforeStop");
            peer.close(None).await.ok();
            connection.shutdown().await;
        })
        .await;

    fs::remove_file(cleanup_path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn before_stop_is_skipped_for_replacement_self_stop_failure_and_exit() {
    let root = std::env::temp_dir().join(format!("rev-idle-no-before-stop-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let replaced = root.join("replaced.js");
    let self_stopping = root.join("self-stopping.js");
    let failing = root.join("failing.js");
    let exiting = root.join("exiting.js");
    for (path, source) in [
        (&replaced, "export function afterLoad() { rev.click(0, 0); } export function beforeStop() { rev.click(1, 1); } export default async function() { await rev.sleep(5000); }"),
        (&self_stopping, "export function afterLoad() { rev.click(2, 2); } export function beforeStop() { rev.click(3, 3); } export default function() { rev.click(20, 20); rev.stop(); }"),
        (&failing, "export function afterLoad() { rev.click(4, 4); } export function beforeStop() { rev.click(5, 5); } export default function() { rev.click(40, 40); throw new Error('expected failure'); }"),
        (&exiting, "export function afterLoad() { rev.click(6, 6); } export function beforeStop() { rev.click(7, 7); } export default async function() { await rev.sleep(5000); }"),
    ] { std::fs::write(path, source).unwrap(); }
    let (controls, events) = recording_controls();
    let (commands, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (states, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, states, pause_rx, Some(replaced), controls, LockState::default(), Duration::from_millis(2),
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            while events.borrow().is_empty() { tokio::task::yield_now().await; }
        }).await.unwrap();
        for (path, count) in [(self_stopping, 3), (failing, 5)] {
            commands.send(ScriptCommand::Load(path)).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                while events.borrow().len() < count { tokio::task::yield_now().await; }
                while state_rx.borrow().phase != ScriptPhase::Stopped { state_rx.changed().await.unwrap(); }
            }).await.unwrap();
        }
        commands.send(ScriptCommand::Load(exiting)).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while events.borrow().len() < 6 { tokio::task::yield_now().await; }
        }).await.unwrap();
        commands.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
        assert_eq!(*events.borrow(), vec![
            HostEvent::Click(0, 0, Button::Left), HostEvent::Click(2, 2, Button::Left), HostEvent::Click(20, 20, Button::Left),
            HostEvent::Click(4, 4, Button::Left), HostEvent::Click(40, 40, Button::Left), HostEvent::Click(6, 6, Button::Left),
        ]);
    }).await;
    let root = root.canonicalize().unwrap();
    assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn ordinary_javascript_error_after_disconnect_stops_the_session() {
    use futures_util::StreamExt;
    let path = std::env::temp_dir().join(format!("rev-idle-disconnect-error-{}.js", uuid::Uuid::new_v4()));
    std::fs::write(&path, r#"
        export function onDisconnect() { rev.click(2, 2); }
        export default async function() {
            try { await rev.state(); } catch (_) { throw new Error('ordinary JavaScript failure'); }
        }
    "#).unwrap();
    let (address, peer_rx) = crate::bridge::test_support::raw_server().await;
    let connection = crate::bridge::WsConnection::connect_for_test(address, Duration::from_millis(20), Duration::from_secs(1));
    let mut peer = peer_rx.await.unwrap();
    let mut generations = connection.connection_generation();
    while *generations.borrow_and_update() == 0 { generations.changed().await.unwrap(); }
    let (controls, events) = recording_controls();
    let (commands, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let (states, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_lifecycle(
            command_rx, connection.clone(), pause_rx, Some(path.clone()), controls, Duration::from_millis(2), shutdown_rx,
            std::sync::Arc::new(AtomicBool::new(false)), std::sync::Arc::new(AtomicBool::new(false)),
            CaptureState, LockState::default(), None, states,
        ));
        tokio::time::timeout(Duration::from_secs(1), peer.next()).await.unwrap().unwrap().unwrap();
        peer.close(None).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while state_rx.borrow().phase != ScriptPhase::Stopped { state_rx.changed().await.unwrap(); }
        }).await.unwrap();
        assert!(events.borrow().contains(&HostEvent::Click(2, 2, Button::Left)));
        commands.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
    }).await;
    connection.shutdown().await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn typescript_entry_transpiles_annotations_interfaces_and_enums_in_memory() {
    let session = ScriptSession::new_with_connection(
        r#"
            interface Config { x: number }
            enum ButtonValue { Left = 3 }
            const config: Config = { x: ButtonValue.Left };
            export default (() => rev.click(config.x, 0));
        "#,
        "typescript-entry-test.ts",
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .unwrap();
    let (controls, events) = recording_controls();

    session.invoke(State::default(), controls).await.unwrap();

    assert_eq!(events.borrow()[0], HostEvent::Click(3, 0, Button::Left));
}

#[tokio::test(flavor = "current_thread")]
async fn typescript_static_import_loads_alongside_javascript_without_emitting_files() {
    use std::fs;

    let root = std::env::temp_dir().join(format!(
        "rev-idle-typescript-static-import-test-{}",
        std::process::id(),
    ));
    fs::create_dir_all(&root).unwrap();
    let config_path = root.join("config.ts");
    let js_path = root.join("offset.js");
    fs::write(&config_path, "export const value: number = 4;").unwrap();
    fs::write(&js_path, "export const value = 5;").unwrap();
    let entry_path = root.join("entry.ts");
    let session = ScriptSession::new_with_connection(
        r#"
            import { value as typescriptValue } from "./config.ts";
            import { value as javascriptValue } from "./offset.js";
            export default (() => rev.click(typescriptValue + javascriptValue, 0));
        "#,
        &entry_path.to_string_lossy(),
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .unwrap();
    let (controls, events) = recording_controls();

    session.invoke(State::default(), controls).await.unwrap();

    assert_eq!(events.borrow()[0], HostEvent::Click(9, 0, Button::Left));
    let mut files = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files, vec!["config.ts", "offset.js"]);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn dynamic_typescript_import_refresh_keeps_old_function_and_loads_new_function() {
    use std::fs;

    let root = std::env::temp_dir().join(format!(
        "rev-idle-typescript-dynamic-import-test-{}",
        std::process::id(),
    ));
    fs::create_dir_all(&root).unwrap();
    let config_path = root.join("config.ts");
    let entry_path = root.join("entry.js");
    fs::write(
        &config_path,
        "export default async (x: number) => {\n    throw new Error(\"old typescript function\");\n};\n",
    )
    .unwrap();
    let session = ScriptSession::new_with_connection(
        r#"
            let old;
            let refreshed;
            export default (async () => {
                const current = (await import("./config.ts")).default;
                if (!old) {
                    old = current;
                    return;
                }
                if (refreshed) {
                    if (refreshed !== current) throw new Error("unchanged source was reloaded");
                    return;
                }
                let oldStack;
                try {
                    await old(1);
                } catch (error) {
                    oldStack = error.stack;
                }
                let currentStack;
                try {
                    await current(2);
                } catch (error) {
                    currentStack = error.stack;
                }
                if (!oldStack.includes("config.ts:2:15")) throw new Error(oldStack);
                if (!currentStack.includes("config.ts:5:15")) throw new Error(currentStack);
                rev.click(1, 0);
                rev.click(2, 0);
                refreshed = current;
            });
        "#,
        &entry_path.to_string_lossy(),
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .unwrap();
    let (controls, events) = recording_controls();

    session.invoke(State::default(), controls.clone()).await.unwrap();
    fs::write(
        &config_path,
        "\n\n\nexport default async (x: number) => {\n    throw new Error(\"new typescript function\");\n};\n",
    )
    .unwrap();
    session.invoke(State::default(), controls).await.unwrap();
    session.invoke(State::default(), recording_controls().0).await.unwrap();

    assert_eq!(
        events.borrow().as_slice(),
        &[
            HostEvent::Click(1, 0, Button::Left),
            HostEvent::Click(2, 0, Button::Left),
        ],
    );
    assert!(!root.join("config.js").exists());
    assert!(!root.join("config.js.map").exists());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn typescript_runtime_errors_report_original_source_line() {
    let session = ScriptSession::new_with_connection(
        "interface Marker { value: number }\n\n\nenum Kind { Test }\n\nconst marker: Marker = { value: Kind.Test };\nexport default (() => { throw new Error(\"typescript runtime\"); });\n",
        "typescript-errors-test.ts",
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();

    let error = session.invoke(State::default(), controls).await.unwrap_err();

    assert!(error.contains("typescript-errors-test.ts:7"), "unexpected error: {error}");
}

#[tokio::test(flavor = "current_thread")]
async fn typescript_top_level_error_maps_unicode_utf16_column() {
    let error = ScriptSession::new_with_connection(
        "    const label: string = \"😀\"; throw new Error(label);\nexport default (() => {});\n",
        "typescript-top-level-test.ts",
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .err()
    .unwrap();

    assert!(error.contains("typescript-top-level-test.ts:1:43"), "unexpected error: {error}");
}

#[tokio::test(flavor = "current_thread")]
async fn typescript_error_after_unicode_uses_utf16_source_map_columns() {
    let error = ScriptSession::new_with_connection(
        "const values: unknown[] = [\"\u{1f600}\u{1f600}\u{1f600}\u{1f600}\", JSON.parse(\"invalid\")];\nexport default (() => {});\n",
        "typescript-unicode-test.ts",
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .err()
    .unwrap();

    // The mapped call expression starts at JSON, after eight UTF-16 units of emoji.
    assert!(error.contains("typescript-unicode-test.ts:1:40"), "unexpected error: {error}");
}

#[tokio::test(flavor = "current_thread")]
async fn typescript_async_and_hook_errors_report_original_source_lines() {
    let hook_session = ScriptSession::new_with_connection(
        "interface HookConfig { value: number }\n\nexport async function afterLoad() {\n    await Promise.resolve();\n    throw new Error(\"typescript hook\");\n}\nexport default (() => {});\n",
        "typescript-hook-test.ts",
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();
    let hook_error = hook_session.run_after_load(controls).await.unwrap_err();
    assert!(hook_error.contains("typescript-hook-test.ts:5:15"), "unexpected error: {hook_error}");

    let async_session = ScriptSession::new_with_connection(
        "interface AsyncConfig { value: number }\n\nexport default (async () => {\n    await Promise.resolve();\n    throw new Error(\"typescript async\");\n});\n",
        "typescript-async-test.ts",
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();
    let async_error = async_session.invoke(State::default(), controls).await.unwrap_err();
    assert!(async_error.contains("typescript-async-test.ts:5:15"), "unexpected error: {async_error}");
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_typescript_reports_source_name_and_parser_detail() {
    let error = ScriptSession::new_with_connection(
        "interface Config { value: number }\nconst value: = 1;\nexport default (() => value);\n",
        "invalid-typescript-test.ts",
        crate::bridge::WsConnection::disconnected_for_test(),
    )
    .await
    .err()
    .unwrap();

    assert!(error.contains("invalid-typescript-test.ts"), "unexpected error: {error}");
    assert!(error.contains("invalid-typescript-test.ts:2"), "unexpected error: {error}");
    assert_ne!(error, "Exception generated by QuickJS");
}

#[tokio::test(flavor = "current_thread")]
async fn pause_preserves_in_flight_default_and_blocks_next_call() {
    let path = std::env::temp_dir().join(format!("rev-idle-pause-preserved-{}.js", uuid::Uuid::new_v4()));
    std::fs::write(&path, r#"
        let release, count = 0;
        export default async function() {
            const id = ++count;
            rev.click(id, 0);
            await new Promise(resolve => { release = resolve; });
            rev.click(id, 1);
        }
        export function beforePause() { rev.click(8, 8); release(); }
        export async function afterResume() { await rev.ensureRunning(); rev.click(9, 9); }
    "#).unwrap();
    let (controls, events) = recording_controls();
    let gate = controls.actions_paused.clone();
    let (commands, command_rx) = mpsc::channel(8);
    let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (states, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, states, pause_rx, Some(path.clone()), controls, LockState::default(), Duration::from_millis(2),
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            while events.borrow().is_empty() { tokio::task::yield_now().await; }
        }).await.unwrap();
        assert_eq!(*events.borrow(), vec![HostEvent::Click(1, 0, Button::Left)]);
        pause_tx.send_replace(gate.set_paused(true));
        tokio::time::timeout(Duration::from_secs(1), async {
            while events.borrow().len() < 2 { tokio::task::yield_now().await; }
        }).await.unwrap();
        assert_eq!(*events.borrow(), vec![HostEvent::Click(1, 0, Button::Left), HostEvent::Click(8, 8, Button::Left)]);
        while state_rx.borrow().phase != ScriptPhase::Paused { state_rx.changed().await.unwrap(); }
        let main_steps_at_ack = events.borrow().len();
        tokio::time::sleep(Duration::from_millis(30)).await;
        let main_steps_after_ack = events.borrow().len();
        assert_eq!(main_steps_after_ack, main_steps_at_ack, "pause must withhold the next default call");
        commands.send(ScriptCommand::Resume).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while events.borrow().len() < 5 { tokio::task::yield_now().await; }
        }).await.unwrap();
        assert_eq!(&events.borrow()[2..], &[
            HostEvent::Click(1, 1, Button::Left),
            HostEvent::Click(9, 9, Button::Left),
            HostEvent::Click(2, 0, Button::Left),
        ]);
        commands.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
    }).await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn two_runtime_pause_detached_timer_and_promise() {
    let path = std::env::temp_dir().join(format!("rev-idle-two-runtime-pause-{}.js", uuid::Uuid::new_v4()));
    std::fs::write(&path, r#"
        export function afterLoad() {
            rev.ui('main', { text: "idle" });
            rev.ui('detached', { text: "waiting" });
            rev.ui('background', { text: "0" });
            rev.daemon('monitor', async function() {
                let count = 0;
                while (true) {
                    rev.ui.background.text = String(++count);
                    await rev.sleep(2);
                }
            });
        }
        let starts = 0;
        export default async function() {
            const start = ++starts;
            let value = 41;
            rev.ui.main.text = `started-${start}`;
            (async () => {
                await rev.sleep(20);
                rev.ui.detached.text = "done";
            })();
            await rev.sleep(40);
            value++;
            rev.ui.main.text = `resumed-${start}-${value}`;
            await new Promise(() => {});
        }
    "#).unwrap();
    let publisher = crate::bridge::ScriptUiPublisher::default();
    let mut snapshots = publisher.subscribe();
    let (controls, _) = recording_controls();
    let gate = controls.actions_paused.clone();
    let (commands, command_rx) = mpsc::channel(8);
    let (_ui_events_tx, ui_events) = mpsc::channel(1);
    let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (_shutdown_tx, shutdown) = watch::channel(false);
    let (states, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_lifecycle_with_control(
            command_rx,
            ui_events,
            crate::bridge::WsConnection::disconnected_for_test(),
            pause_rx,
            Some(path.clone()),
            controls,
            Duration::from_millis(2),
            shutdown,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            CaptureState::default(),
            LockState::default(),
            None,
            states,
            ScriptControl::default(),
            publisher,
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if snapshots.borrow().elements.iter().any(|element| element.id == "main" && element.text == "started-1")
                    && snapshots.borrow().elements.iter().any(|element| element.id == "background" && element.text != "0")
                {
                    break;
                }
                snapshots.changed().await.unwrap();
            }
        }).await.unwrap();
        pause_tx.send_replace(gate.set_paused(true));
        tokio::time::timeout(Duration::from_secs(1), async {
            while state_rx.borrow().phase != ScriptPhase::Paused { state_rx.changed().await.unwrap(); }
        }).await.unwrap();
        let main_steps_at_ack = snapshots.borrow().elements.iter().find(|element| element.id == "main").unwrap().text.clone();
        let detached_at_ack = snapshots.borrow().elements.iter().find(|element| element.id == "detached").unwrap().text.clone();
        let background_steps_at_ack = snapshots.borrow().elements.iter().find(|element| element.id == "background").unwrap().text.parse::<usize>().unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        let main_steps_after_ack = snapshots.borrow().elements.iter().find(|element| element.id == "main").unwrap().text.clone();
        let detached_after_ack = snapshots.borrow().elements.iter().find(|element| element.id == "detached").unwrap().text.clone();
        let background_steps_after_ack = snapshots.borrow().elements.iter().find(|element| element.id == "background").unwrap().text.parse::<usize>().unwrap();
        assert_eq!(main_steps_after_ack, main_steps_at_ack);
        assert_eq!(detached_after_ack, detached_at_ack);
        assert!(background_steps_after_ack > background_steps_at_ack);

        commands.send(ScriptCommand::Resume).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if snapshots.borrow().elements.iter().any(|element| element.id == "main" && element.text == "resumed-1-42")
                    && snapshots.borrow().elements.iter().any(|element| element.id == "detached" && element.text == "done")
                {
                    break;
                }
                snapshots.changed().await.unwrap();
            }
        }).await.unwrap();
        assert!(!snapshots.borrow().elements.iter().any(|element| element.id == "main" && element.text.contains("started-2")));
        commands.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
    }).await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn two_runtime_pause_stale_request_is_not_acknowledged() {
    let path = std::env::temp_dir().join(format!("rev-idle-two-runtime-stale-pause-{}.js", uuid::Uuid::new_v4()));
    std::fs::write(&path, r#"
        export async function beforePause() {
            rev.click(1, 0);
            await rev.sleep(40);
            rev.click(2, 0);
        }
        export default async function() { await new Promise(() => {}); }
    "#).unwrap();
    let (controls, events) = recording_controls();
    let gate = controls.actions_paused.clone();
    let (commands, command_rx) = mpsc::channel(8);
    let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (states, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, states, pause_rx, Some(path.clone()), controls, LockState::default(), Duration::from_millis(2),
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            while state_rx.borrow().phase != ScriptPhase::Running { state_rx.changed().await.unwrap(); }
        }).await.unwrap();
        pause_tx.send_replace(gate.set_paused(true));
        tokio::time::timeout(Duration::from_secs(1), async {
            while events.borrow().len() < 1 { tokio::task::yield_now().await; }
        }).await.unwrap();
        pause_tx.send_replace(gate.set_paused(false));
        pause_tx.send_replace(gate.set_paused(true));
        tokio::time::timeout(Duration::from_secs(1), async {
            while events.borrow().len() < 3 { tokio::task::yield_now().await; }
        }).await.unwrap();
        assert_eq!(&events.borrow()[..3], &[
            HostEvent::Click(1, 0, Button::Left),
            HostEvent::Click(2, 0, Button::Left),
            HostEvent::Click(1, 0, Button::Left),
        ]);
        assert_eq!(state_rx.borrow().phase, ScriptPhase::Running, "stale pause request was acknowledged");
        tokio::time::timeout(Duration::from_secs(1), async {
            while state_rx.borrow().phase != ScriptPhase::Paused { state_rx.changed().await.unwrap(); }
        }).await.unwrap();
        assert_eq!(events.borrow()[3], HostEvent::Click(2, 0, Button::Left));
        commands.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
    }).await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn lifecycle_commands_keep_requested_pause_gate_in_sync() {
    use crate::app::ScriptCommand;
    use std::fs;
    use tokio::sync::{mpsc, watch};

    let root = std::env::temp_dir().join(format!(
        "rev-idle-pause-lifecycle-test-{}",
        std::process::id(),
    ));
    fs::create_dir_all(&root).unwrap();
    let first = root.join("first.js");
    let second = root.join("second.js");
    fs::write(&first, r#"export default (() => rev.click(1, 1))"#).unwrap();
    fs::write(&second, r#"export default (() => rev.click(2, 2, "right"))"#).unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct LifecycleMouse(mpsc::UnboundedSender<()>);
    impl MouseInput for LifecycleMouse {
        fn click_at(
            &mut self,
            _x: i32,
            _y: i32,
            _button: Button,
        ) -> Result<(), MouseInputError> {
            self.0.send(()).map_err(|error| error.to_string().into())
        }
    }

    let controls = HostControls {
        mouse: Rc::new(RefCell::new(LifecycleMouse(event_tx))),
        window: Rc::new(FakeWindow),
        actions_paused: ActionGate::default(),
    };
    let gate = controls.actions_paused.clone();
    let (command_tx, command_rx) = mpsc::channel(16);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let runner_path = first.clone();
    let local = tokio::task::LocalSet::new();

    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_controls(
                command_rx,
                state_rx,
                pause_rx,
                Some(runner_path),
                controls,
                Duration::from_millis(2),
            ));

            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(gate.is_paused());

            gate.set_paused(false);
            command_tx
                .send(ScriptCommand::SetPaused(false))
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if !gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(!gate.is_paused());

            command_tx.send(ScriptCommand::Resume).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(!gate.is_paused());

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(gate.is_paused());

            command_tx
                .send(ScriptCommand::Load(second.clone()))
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(!gate.is_paused());

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(gate.is_paused());

            command_tx.send(ScriptCommand::Reload).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(!gate.is_paused());

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            while event_rx.try_recv().is_ok() {}

            command_tx.send(ScriptCommand::Stop).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if !gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(!gate.is_paused());
            assert!(tokio::time::timeout(
                Duration::from_millis(15),
                event_rx.recv(),
            )
            .await
            .is_err());

            command_tx
                .send(ScriptCommand::Load(second.clone()))
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(!gate.is_paused());

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            while event_rx.try_recv().is_ok() {}
            fs::remove_file(&second).unwrap();

            command_tx.send(ScriptCommand::Reload).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if !gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(!gate.is_paused());
            assert!(tokio::time::timeout(
                Duration::from_millis(15),
                event_rx.recv(),
            )
            .await
            .is_err());

            gate.set_paused(true);
            command_tx
                .send(ScriptCommand::SetPaused(true))
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if !gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(!gate.is_paused());

            gate.set_paused(false);
            command_tx
                .send(ScriptCommand::SetPaused(false))
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if !gate.is_paused() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(!gate.is_paused());

            runner.abort();
            let _ = runner.await;
        })
        .await;

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn rev_stop_interrupts_the_session() {
    let path = std::env::temp_dir().join(format!("rev-idle-self-stop-{}.js", uuid::Uuid::new_v4()));
    std::fs::write(&path, r#"export default function() { rev.click(0, 0); rev.stop(); rev.click(1, 1); }"#).unwrap();
    let (controls, events) = recording_controls();
    let (commands, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (states, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    tokio::task::LocalSet::new().run_until(async {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, states, pause_rx, Some(path.clone()), controls, LockState::default(), Duration::from_millis(2),
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            while state_rx.borrow().phase != ScriptPhase::Stopped { state_rx.changed().await.unwrap(); }
        }).await.unwrap();
        assert_eq!(*events.borrow(), vec![HostEvent::Click(0, 0, Button::Left)]);
        commands.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
    }).await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn stop_command_cancels_an_in_flight_sleep_immediately() {
    use std::fs;
    use tokio::sync::{mpsc, watch};

    let sleeping_path = std::env::temp_dir().join(format!(
        "rev-idle-stop-sleep-test-{}.js",
        std::process::id(),
    ));
    fs::write(
        &sleeping_path,
        r#"export default (async () => {
            rev.click(1, 1);
            await rev.sleep(5000);
            rev.click(2, 2);
        })"#,
    )
    .unwrap();
    let followup_path = std::env::temp_dir().join(format!(
        "rev-idle-stop-sleep-followup-{}.js",
        std::process::id(),
    ));
    fs::write(&followup_path, r#"export default (() => rev.click(3, 3))"#).unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct ClickMouse(mpsc::UnboundedSender<(i32, i32)>);
    impl MouseInput for ClickMouse {
        fn click_at(
            &mut self,
            x: i32,
            y: i32,
            _button: Button,
        ) -> Result<(), MouseInputError> {
            self.0.send((x, y)).map_err(|error| error.to_string().into())
        }
    }

    let mouse: SharedMouse = Rc::new(RefCell::new(ClickMouse(event_tx)));
    let (command_tx, command_rx) = mpsc::channel(32);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let local = tokio::task::LocalSet::new();

    let runner_path = sleeping_path.clone();
    let load_path = followup_path.clone();
    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                runner_path,
                mouse,
                Duration::from_millis(5),
            ));

            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap(),
                (1, 1),
                "script did not start before sleeping"
            );

            command_tx.send(ScriptCommand::Stop).await.unwrap();
            // Queued right behind Stop: if Stop had to wait for the 5s
            // sleep to finish before being processed, this Load (and the
            // click its script makes) would too, and the timeout below
            // would fire well before either could complete.
            command_tx
                .send(ScriptCommand::Load(load_path))
                .await
                .unwrap();

            assert_eq!(
                tokio::time::timeout(Duration::from_millis(200), event_rx.recv())
                    .await
                    .expect("Stop did not cancel the in-flight sleep promptly")
                    .unwrap(),
                (3, 3),
                "follow-up script did not run after Stop"
            );

            command_tx.send(ScriptCommand::Exit).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        })
        .await;

    fs::remove_file(sleeping_path).unwrap();
    fs::remove_file(followup_path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn write_clipboard_forwards_text_to_host() {
    let session = ScriptSession::new(
        r#"export default (() => rev.write_clipboard("hello 世界"))"#,
    )
    .await
    .unwrap();
    let (controls, events) = recording_controls();

    session.invoke(State::default(), controls).await.unwrap();

    assert_eq!(
        *events.borrow(),
        vec![HostEvent::Clipboard("hello 世界".to_string())]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn read_clipboard_returns_host_text() {
    let session = ScriptSession::new(
        r#"export default (async () => {
            rev.write_clipboard("round trip");
            const text = await rev.read_clipboard();
            if (text !== "round trip") throw new Error("unexpected clipboard text: " + text);
        })"#,
    )
    .await
    .unwrap();
    let (controls, _) = recording_controls();

    session.invoke(State::default(), controls).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn unhandled_exception_stops_script_after_first_invocation() {
    use std::fs;
    use tokio::sync::{mpsc, watch};

    let path = std::env::temp_dir().join(format!(
        "rev-idle-delay-test-{}.js",
        std::process::id(),
    ));
    fs::write(
        &path,
        r#"
            export default (() => {
                rev.click(1, 0, "left");
                throw new Error("stop after this invocation");
            })
        "#,
    )
    .unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct ErrorMouse(mpsc::UnboundedSender<()>);
    impl MouseInput for ErrorMouse {
        fn click_at(
            &mut self,
            _x: i32,
            _y: i32,
            _button: Button,
        ) -> Result<(), MouseInputError> {
            self.0.send(()).map_err(|error| error.to_string().into())
        }
    }

    let mouse: SharedMouse = Rc::new(RefCell::new(ErrorMouse(event_tx)));
    let (command_tx, command_rx) = mpsc::channel(32);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let local = tokio::task::LocalSet::new();

    let runner_path = path.clone();
    let invoked_again = local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                runner_path,
                mouse,
                Duration::from_millis(5),
            ));

            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            let invoked_again = tokio::time::timeout(
                Duration::from_millis(30),
                event_rx.recv(),
            )
            .await
            .is_ok();

            command_tx.send(ScriptCommand::Exit).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            invoked_again
        })
        .await;

    fs::remove_file(path).unwrap();
    assert!(!invoked_again, "script ran again after an unhandled exception");
}

#[tokio::test(flavor = "current_thread")]
async fn failed_load_stays_stopped_and_reload_retries_that_path() {
    use crate::app::ScriptCommand;
    use std::fs;
    use tokio::sync::{mpsc, watch};

    let missing = std::env::temp_dir().join(format!(
        "rev-idle-retry-test-{}.js",
        std::process::id(),
    ));
    let _ = fs::remove_file(&missing);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct RetryMouse(mpsc::UnboundedSender<()>);
    impl MouseInput for RetryMouse {
        fn click_at(
            &mut self,
            _x: i32,
            _y: i32,
            _button: Button,
        ) -> Result<(), MouseInputError> {
            self.0.send(()).map_err(|error| error.to_string().into())
        }
    }

    let mouse: SharedMouse = Rc::new(RefCell::new(RetryMouse(event_tx)));
    let (command_tx, command_rx) = mpsc::channel(32);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let local = tokio::task::LocalSet::new();

    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                missing.clone(),
                mouse,
                Duration::from_millis(5),
            ));

            assert!(
                tokio::time::timeout(
                    Duration::from_millis(15),
                    event_rx.recv(),
                )
                .await
                .is_err()
            );

            fs::write(
                &missing,
                r#"export default (() => rev.click(1, 1, "left"))"#,
            )
            .unwrap();
            command_tx.send(ScriptCommand::Reload).await.unwrap();
            tokio::time::timeout(
                Duration::from_secs(1),
                event_rx.recv(),
            )
            .await
            .unwrap()
            .unwrap();

            drop(command_tx);
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            fs::remove_file(missing).unwrap();
        })
        .await;
}

#[test]
fn capture_is_allowed_when_the_script_is_unloaded_paused_or_stopped() {
    assert_eq!(capture_action(false, false, false), CaptureAction::Enable);
    assert_eq!(capture_action(false, true, true), CaptureAction::Enable);
    assert_eq!(capture_action(false, true, false), CaptureAction::Reject);
    assert_eq!(capture_action(true, true, false), CaptureAction::Disable);

let capture_state = CaptureState;
    capture_state.set_enabled(true);
    disable_capture_if_running(&capture_state, true, false);
    assert!(!capture_state.is_enabled());
}

#[tokio::test(flavor = "current_thread")]
async fn load_time_failures_report_a_detailed_message_not_a_bare_exception() {
    let missing_import = ScriptSession::new(
        r#"
            import { x } from './missing.js';
            export default (() => {});
        "#,
    )
    .await
    .err()
    .unwrap();
    assert!(
        missing_import.contains("missing.js"),
        "missing import error lacked detail: {missing_import}"
    );

    let throwing_top_level = ScriptSession::new(
        r#"
            throw new Error("boom at module top level");
        "#,
    )
    .await
    .err()
    .unwrap();
    assert!(
        throwing_top_level.contains("boom at module top level"),
        "top-level throw error lacked detail: {throwing_top_level}"
    );

    let syntax_error = ScriptSession::new(r#"export default (() => { const }"#)
        .await
        .err()
        .unwrap();
    assert_ne!(
        syntax_error, "Exception generated by QuickJS",
        "syntax error should include the parser's own message"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lifecycle_publishes_actual_phase_and_one_shot_capture_state() {
    use std::fs;

    let path = std::env::temp_dir().join(format!(
        "rev-idle-state-update-test-{}.js",
        std::process::id(),
    ));
    let cleanup_path = path.clone();
    fs::write(&path, r#"export default (() => {})"#).unwrap();
    let absolute_path = std::path::absolute(&path).unwrap();
    CaptureState.set_enabled(false);
    let (command_tx, command_rx) = mpsc::channel(16);
    let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, mut state_rx) = watch::channel(StateUpdate {
        phase: ScriptPhase::Unloaded,
        capture: false,
        locked: false,
        lock_label: None,
        scripts: Vec::new(),
    });
    let (controls, _) = recording_controls();
    let pause_gate = controls.actions_paused.clone();
    let local = tokio::task::LocalSet::new();
    let runner = local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx,
            state_tx,
            pause_rx,
            None,
            controls,
            LockState::default(),
            Duration::from_millis(5),
        ));

        command_tx.send(ScriptCommand::Load(path.clone())).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if *state_rx.borrow() == (StateUpdate {
                    phase: ScriptPhase::Running,
                    capture: false,
                    locked: false,
                    lock_label: None,
                    scripts: vec![absolute_path.to_string_lossy().into_owned()],
                }) {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        command_tx.send(ScriptCommand::Pause).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Paused {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        command_tx.send(ScriptCommand::Resume).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        pause_tx.send(pause_gate.set_paused(true)).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Paused {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        pause_tx.send(pause_gate.set_paused(false)).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        command_tx.send(ScriptCommand::Stop).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Stopped {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        command_tx.send(ScriptCommand::StartCapture).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().capture {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        command_tx.send(ScriptCommand::CaptureConsumed).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if !state_rx.borrow().capture {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        fs::write(&path, r#"export default (() => {})"#).unwrap();
        command_tx.send(ScriptCommand::Reload).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        fs::write(&path, r#"export default (() => { throw new Error("reload failed") })"#)
            .unwrap();
        command_tx.send(ScriptCommand::Reload).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Stopped {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        fs::write(
            &path,
            r#"export default (async () => { await rev.sleep(20); rev.stop(); })"#,
        )
        .unwrap();
        command_tx.send(ScriptCommand::Reload).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Stopped {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        command_tx.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    });
    runner.await;
    CaptureState.set_enabled(false);
    fs::remove_file(cleanup_path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn lock_command_round_trip_sets_and_clears_reported_state() {
    use std::fs;

    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    lock_state.set_enabled(false);
    let path = std::env::temp_dir().join(format!(
        "rev-idle-lock-test-{}.js",
        std::process::id(),
    ));
    let cleanup_path = path.clone();
    fs::write(&path, r#"export default (() => {})"#).unwrap();
    CaptureState.set_enabled(false);
    let (command_tx, command_rx) = mpsc::channel(16);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, mut state_rx) = watch::channel(StateUpdate {
        phase: ScriptPhase::Unloaded,
        capture: false,
        locked: false,
        lock_label: None,
        scripts: Vec::new(),
    });
    let (controls, _) = recording_controls();
    let local = tokio::task::LocalSet::new();
    let runner = local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx,
            state_tx,
            pause_rx,
            None,
            controls,
            lock_state,
            Duration::from_millis(5),
        ));

        command_tx.send(ScriptCommand::Load(path.clone())).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        command_tx.send(ScriptCommand::Lock).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().locked {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        command_tx.send(ScriptCommand::Pause).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if !state_rx.borrow().locked {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        command_tx.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    });
    runner.await;
    CaptureState.set_enabled(false);
    fs::remove_file(cleanup_path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn load_locked_replaces_the_running_script_and_reports_it_most_recent() {
    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    lock_state.set_enabled(false);
    let first = std::env::temp_dir().join(format!("rev-idle-load-first-{}.js", std::process::id()));
    let second = std::env::temp_dir().join(format!("rev-idle-load-second-{}.js", std::process::id()));
    std::fs::write(&first, r#"export default (() => {})"#).unwrap();
    std::fs::write(&second, r#"export default (() => {})"#).unwrap();
    let first_absolute = std::path::absolute(&first).unwrap();
    let second_absolute = std::path::absolute(&second).unwrap();
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    let (controls, _) = recording_controls();
    let local = tokio::task::LocalSet::new();
    local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, state_tx, pause_rx, Some(first.clone()), controls, lock_state, Duration::from_millis(5),
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running && !state_rx.borrow().locked { break; }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();
        command_tx.send(ScriptCommand::LoadLocked(second.clone())).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let state = state_rx.borrow();
                if state.phase == ScriptPhase::Running && state.locked && state.scripts == vec![
                    first_absolute.to_string_lossy().into_owned(),
                    second_absolute.to_string_lossy().into_owned(),
                ] { break; }
                drop(state);
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();
        command_tx.send(ScriptCommand::Exit).await.unwrap();
        runner.await.unwrap().unwrap();
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn failed_history_load_removes_the_script_from_reported_and_saved_history() {
    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    lock_state.set_enabled(false);
    let dir = std::env::temp_dir().join(format!("rev-idle-failed-history-load-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("script.js");
    let history_path = dir.join("script_history.txt");
    std::fs::write(&path, r#"export default (() => {})"#).unwrap();
    let absolute_path = std::path::absolute(&path).unwrap();
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (_shutdown_tx, shutdown) = watch::channel(false);
    let (state_tx, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    let (controls, _) = recording_controls();
    let local = tokio::task::LocalSet::new();
    local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_lifecycle(
            command_rx,
            crate::bridge::WsConnection::disconnected_for_test(),
            pause_rx,
            Some(path.clone()),
            controls,
            Duration::from_millis(5),
            shutdown,
            std::sync::Arc::new(AtomicBool::new(false)),
            std::sync::Arc::new(AtomicBool::new(false)),
            CaptureState::default(),
            lock_state,
            Some(history_path.clone()),
            state_tx,
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running && state_rx.borrow().scripts == vec![absolute_path.to_string_lossy().into_owned()] {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();
        std::fs::write(&path, r#"export default (() => {"#).unwrap();
        command_tx.send(ScriptCommand::Load(path.clone())).await.unwrap();
        let removed = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Stopped && state_rx.borrow().scripts.is_empty() {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        }).await.is_ok();
        command_tx.send(ScriptCommand::Exit).await.unwrap();
        runner.await.unwrap().unwrap();
        assert!(removed);
        assert_eq!(std::fs::read_to_string(&history_path).unwrap(), "");
        std::fs::remove_dir_all(dir).unwrap();
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn removing_history_keeps_the_current_script_running_and_updates_saved_history() {
    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    lock_state.set_enabled(false);
    let dir = std::env::temp_dir().join(format!("rev-idle-remove-script-history-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("script.js");
    let history_path = dir.join("script_history.txt");
    std::fs::write(&path, r#"export default (() => {})"#).unwrap();
    let absolute_path = std::path::absolute(&path).unwrap();
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (_shutdown_tx, shutdown) = watch::channel(false);
    let (state_tx, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    let (controls, _) = recording_controls();
    let local = tokio::task::LocalSet::new();
    local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_lifecycle(
            command_rx,
            crate::bridge::WsConnection::disconnected_for_test(),
            pause_rx,
            Some(path.clone()),
            controls,
            Duration::from_millis(5),
            shutdown,
            std::sync::Arc::new(AtomicBool::new(false)),
            std::sync::Arc::new(AtomicBool::new(false)),
            CaptureState::default(),
            lock_state,
            Some(history_path.clone()),
            state_tx,
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running &&
                    state_rx.borrow().scripts == vec![absolute_path.to_string_lossy().into_owned()]
                {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();

        command_tx.send(ScriptCommand::RemoveFromHistory(absolute_path)).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running && state_rx.borrow().scripts.is_empty() {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();

        assert_eq!(std::fs::read_to_string(&history_path).unwrap(), "");
        command_tx.send(ScriptCommand::Exit).await.unwrap();
        runner.await.unwrap().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn reload_locked_load_failure_stays_stopped_and_unlocked() {
    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    lock_state.set_enabled(false);
    let path = std::env::temp_dir().join(format!("rev-idle-reload-locked-failure-{}.js", std::process::id()));
    std::fs::write(&path, r#"export default (() => {})"#).unwrap();
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    let (controls, _) = recording_controls();
    let local = tokio::task::LocalSet::new();
    local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, state_tx, pause_rx, Some(path.clone()), controls, lock_state, Duration::from_millis(5),
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running && !state_rx.borrow().locked { break; }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();
        command_tx.send(ScriptCommand::Lock).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().locked { break; }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();
        std::fs::write(&path, r#"export default (() => {"#).unwrap();
        command_tx.send(ScriptCommand::ReloadLocked).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Stopped && !state_rx.borrow().locked { break; }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();
        command_tx.send(ScriptCommand::Exit).await.unwrap();
        runner.await.unwrap().unwrap();
        std::fs::remove_file(path).unwrap();
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn resume_locked_from_paused_starts_running_and_locked() {
    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    lock_state.set_enabled(false);
    let path = std::env::temp_dir().join(format!("rev-idle-resume-locked-{}.js", std::process::id()));
    std::fs::write(&path, r#"export default (() => {})"#).unwrap();
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, mut state_rx) = watch::channel(StateUpdate::new(false, false, false, false, false));
    let (controls, _) = recording_controls();
    let local = tokio::task::LocalSet::new();
    local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx, state_tx, pause_rx, Some(path.clone()), controls, lock_state, Duration::from_millis(5),
        ));
        command_tx.send(ScriptCommand::Pause).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Paused && !state_rx.borrow().locked { break; }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();
        command_tx.send(ScriptCommand::ResumeLocked).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running && state_rx.borrow().locked { break; }
                state_rx.changed().await.unwrap();
            }
        }).await.unwrap();
        command_tx.send(ScriptCommand::Exit).await.unwrap();
        runner.await.unwrap().unwrap();
        std::fs::remove_file(path).unwrap();
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn hotkey_pause_clears_reported_lock_state() {
    use std::fs;

    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    lock_state.set_enabled(false);
    let path = std::env::temp_dir().join(format!(
        "rev-idle-hotkey-unlock-test-{}.js",
        std::process::id(),
    ));
    let cleanup_path = path.clone();
    fs::write(&path, r#"export default (() => {})"#).unwrap();
    CaptureState.set_enabled(false);
    let (command_tx, command_rx) = mpsc::channel(16);
    let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, mut state_rx) = watch::channel(StateUpdate {
        phase: ScriptPhase::Unloaded,
        capture: false,
        locked: false,
        lock_label: None,
        scripts: Vec::new(),
    });
    let (controls, _) = recording_controls();
    let pause_gate = controls.actions_paused.clone();
    let local = tokio::task::LocalSet::new();
    let unlocked = local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx,
            state_tx,
            pause_rx,
            None,
            controls,
            lock_state,
            Duration::from_millis(5),
        ));

        command_tx.send(ScriptCommand::Load(path)).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        command_tx.send(ScriptCommand::Lock).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().locked {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();

        pause_tx.send_replace(pause_gate.set_paused(true));
        let unlocked = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Paused && !state_rx.borrow().locked {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .is_ok();

        command_tx.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        unlocked
    }).await;
    fs::remove_file(cleanup_path).unwrap();
    assert!(unlocked, "hotkey pause must unlock the game window");
}

#[tokio::test(flavor = "current_thread")]
async fn stop_command_during_an_invocation_clears_lock_state() {
    use std::fs;

    static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);
    let lock_state = LockState { enabled: &LOCK_ENABLED };
    lock_state.set_enabled(false);
    let path = std::env::temp_dir().join(format!(
        "rev-idle-stop-unlock-test-{}.js",
        std::process::id(),
    ));
    let cleanup_path = path.clone();
    fs::write(&path, r#"export default (async () => { rev.click(1, 1); await rev.sleep(5000); })"#).unwrap();
    CaptureState.set_enabled(false);
    let (command_tx, command_rx) = mpsc::channel(16);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, mut state_rx) = watch::channel(StateUpdate {
        phase: ScriptPhase::Unloaded,
        capture: false,
        locked: false,
        lock_label: None,
        scripts: Vec::new(),
    });
    let (controls, events) = recording_controls();
    let local = tokio::task::LocalSet::new();
    let unlocked = local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx,
            state_tx,
            pause_rx,
            None,
            controls,
            lock_state,
            Duration::from_millis(5),
        ));

        command_tx.send(ScriptCommand::Load(path)).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Running {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        command_tx.send(ScriptCommand::Lock).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().locked {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if !events.borrow().is_empty() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        command_tx.send(ScriptCommand::Stop).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if state_rx.borrow().phase == ScriptPhase::Stopped {
                    break;
                }
                state_rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        let unlocked = !lock_state.is_enabled();

        command_tx.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        unlocked
    }).await;
    fs::remove_file(cleanup_path).unwrap();
    assert!(unlocked, "client-side stop must unlock the game window");
}

#[tokio::test(flavor = "current_thread")]
async fn capture_commands_publish_one_shot_state_without_a_script_path() {
    CaptureState.set_enabled(false);
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, mut state_rx) = watch::channel(StateUpdate {
        phase: ScriptPhase::Unloaded,
        capture: false,
        locked: false,
        lock_label: None,
        scripts: Vec::new(),
    });
    let (controls, _) = recording_controls();
    let local = tokio::task::LocalSet::new();
    let runner = local.run_until(async move {
        let runner = tokio::task::spawn_local(run_with_controls_and_state(
            command_rx,
            state_tx,
            pause_rx,
            None,
            controls,
            LockState::default(),
            Duration::from_millis(5),
        ));
        tokio::task::yield_now().await;

        for (command, capturing) in [
            (ScriptCommand::Capture, true),
            (ScriptCommand::Capture, false),
            (ScriptCommand::StartCapture, true),
            (ScriptCommand::CaptureConsumed, false),
            (ScriptCommand::StartCapture, true),
            (ScriptCommand::StopCapture, false),
        ] {
            command_tx.send(command).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                while state_rx.borrow().capture != capturing {
                    state_rx.changed().await.unwrap();
                }
            })
            .await
            .unwrap();
            assert_eq!(CaptureState.is_enabled(), capturing);
            assert_eq!(state_rx.borrow().phase, ScriptPhase::Unloaded);
        }

        command_tx.send(ScriptCommand::Exit).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), runner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    });
    runner.await;
    CaptureState.set_enabled(false);
}
