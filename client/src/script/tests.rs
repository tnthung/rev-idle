use super::State;
use super::bindings::{click_at_with, Button, HostControls, MouseInput, SharedMouse};
use crate::window::{Axis, WindowControl};
use super::lifecycle::{
    apply_hotkey_update,
    capture_action,
    disable_capture_if_running,
    run_with_controls,
    run_with_controls_and_state,
    CaptureAction,
};
use super::session::ScriptSession;
use super::session::format_console_message;
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
    fn click_at(&mut self, x: i32, y: i32, button: Button) -> Result<(), String> {
        self.events.borrow_mut().push(HostEvent::Click(x, y, button)); Ok(())
    }

    fn scroll(&mut self, x: i32, y: i32, length: i32, axis: Axis) -> Result<(), String> {
        self.events.borrow_mut().push(HostEvent::Scroll(x, y, length, axis)); Ok(())
    }

    fn press(&mut self, key: String) -> Result<(), String> {
        self.events.borrow_mut().push(HostEvent::Press(key)); Ok(())
    }

}
fn recording_controls() -> (HostControls, Rc<RefCell<Vec<HostEvent>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    (HostControls { mouse: Rc::new(RefCell::new(RecordingMouse { events: events.clone() })), window: Rc::new(RecordingWindow { events: events.clone(), clipboard: RefCell::new(String::new()) }), actions_paused: ActionGate::default() }, events)
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
async fn rev_press_skips_input_while_actions_are_paused() {
    let session = ScriptSession::new(r#"export default (() => rev.press("enter"))"#).await.unwrap();
    let (controls, events) = recording_controls();
    controls.actions_paused.set_paused(true);

    session.invoke(State::default(), controls).await.unwrap();

    assert!(events.borrow().is_empty());
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
    for (response_type, payload, expect_error) in [
        ("InvokeRes", json!({}), false),
        ("RemoteError", json!({ "message": remote_error }), true),
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
async fn rev_invoke_skips_paused_actions_and_rejects_empty_paths() {
    let session = ScriptSession::new(r#"export default (async () => await rev.invoke("Buy"))"#).await.unwrap();
    let (controls, _) = recording_controls();
    controls.actions_paused.set_paused(true);
    session.invoke(State::default(), controls).await.unwrap();

    let session = ScriptSession::new(r#"export default (async () => await rev.invoke(" "))"#).await.unwrap();
    let (controls, _) = recording_controls();
    assert!(session.invoke(State::default(), controls).await.unwrap_err().contains("button path"));
}

#[tokio::test(flavor = "current_thread")]
async fn rev_transfer_sends_source_and_destination_paths() {
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
async fn rev_transfer_reports_errors_and_skips_when_paused_or_given_blank_paths() {
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

    let session = ScriptSession::new(
        r#"export default (async () => await rev.transfer("scene:1/Canvas[0]/Inventory/3", "scene:1/Canvas[0]/Combine/0"))"#,
    ).await.unwrap();
    let (controls, _) = recording_controls();
    controls.actions_paused.set_paused(true);
    session.invoke(State::default(), controls).await.unwrap();
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
    ) -> Result<(), String> {
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
        ) -> Result<(), String> {
            self.0.send((x, y, button)).map_err(|error| error.to_string())
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
async fn pause_error_does_not_stop_the_active_script() {
    use crate::app::ScriptCommand;
    use std::fs;
    use tokio::sync::{mpsc, watch};

    let path = std::env::temp_dir().join(format!(
        "rev-idle-requested-pause-test-{}.js",
        std::process::id(),
    ));
    fs::write(
        &path,
        r#"export default (async () => { await rev.sleep(30); rev.click(1, 1); })"#,
    )
    .unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct GateMouse(mpsc::UnboundedSender<()>);
    impl MouseInput for GateMouse {
        fn click_at(
            &mut self,
            _x: i32,
            _y: i32,
            _button: Button,
        ) -> Result<(), String> {
            self.0.send(()).map_err(|error| error.to_string())
        }
    }

    let controls = HostControls {
        mouse: Rc::new(RefCell::new(GateMouse(event_tx))),
        window: Rc::new(FakeWindow),
        actions_paused: ActionGate::default(),
    };
    let gate = controls.actions_paused.clone();
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let runner_path = path.clone();
    let local = tokio::task::LocalSet::new();

    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_controls(
                command_rx,
                state_rx,
                pause_rx,
                Some(runner_path),
                controls,
                Duration::from_millis(1),
            ));

            tokio::time::sleep(Duration::from_millis(5)).await;
            gate.set_paused(true);
            command_tx
                .send(ScriptCommand::SetPaused(true))
                .await
                .unwrap();

            assert!(tokio::time::timeout(
                Duration::from_millis(80),
                event_rx.recv(),
            )
            .await
            .is_err());
            // Pausing mid-invocation must not tear down the session: the
            // gate stays paused and the script keeps waiting to be resumed
            // instead of being force-stopped.
            assert!(gate.is_paused());

            command_tx.send(ScriptCommand::Resume).await.unwrap();
            assert!(tokio::time::timeout(
                Duration::from_millis(200),
                event_rx.recv(),
            )
                .await
                .is_ok());

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
        ) -> Result<(), String> {
            self.0.send((x, y)).map_err(|error| error.to_string())
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
async fn hotkey_pause_cancels_the_in_flight_invocation_immediately() {
    use crate::app::ScriptCommand;
    use crate::global_state::GlobalState;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
    use tokio::sync::{mpsc, watch};

    // F8 flips the shared `ActionGate` directly from a separate OS
    // thread (see hotkey.rs), independently of the script runner's own
    // loop, so this simulates a hotkey press exactly the way that
    // worker does: mutate the gate, then publish the resulting update.
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let hook_key = format!(
        "hotkey-cancel-test-marker-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, AtomicOrdering::Relaxed),
    );
    let path = std::env::temp_dir().join(format!(
        "rev-idle-hotkey-cancel-test-{}.js",
        std::process::id(),
    ));
    // beforePause signals through rev.global rather than a click: the
    // ActionGate is already closed by the time this hook runs (that's
    // the whole point of F8 taking effect immediately), so its own
    // rev.click would be silently skipped same as the cancelled
    // invocation's — rev.global isn't gated, so it still proves the
    // hook ran, and ran promptly rather than after the cancelled sleep.
    fs::write(
        &path,
        format!(
            r#"
            export default (async () => {{ rev.click(1, 1); await rev.sleep(2000); rev.click(2, 2); }});
            export function beforePause() {{ rev.global['{hook_key}'] = true; }}
            "#
        ),
    )
    .unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct HotkeyMouse(mpsc::UnboundedSender<(i32, i32)>);
    impl MouseInput for HotkeyMouse {
        fn click_at(
            &mut self,
            x: i32,
            y: i32,
            _button: Button,
        ) -> Result<(), String> {
            self.0.send((x, y)).map_err(|error| error.to_string())
        }
    }

    let gate = ActionGate::default();
    let controls = HostControls {
        mouse: Rc::new(RefCell::new(HotkeyMouse(event_tx))),
        window: Rc::new(FakeWindow),
        actions_paused: gate.clone(),
    };
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let runner_path = path.clone();
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

            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap(),
                (1, 1),
                "script should click before its long sleep"
            );

            // Simulate F8 partway through the 2s sleep: hotkey.rs itself
            // flips the gate via `toggle()`, private to that module, but
            // `set_paused` produces the same update from an unpaused gate.
            tokio::time::sleep(Duration::from_millis(20)).await;
            pause_tx.send_replace(gate.set_paused(true));
            assert!(gate.is_paused());

            // If the invocation merely ran to the end of its sleep and
            // only then got interrupted, this marker wouldn't appear
            // for ~2s. Seeing it almost immediately proves the sleep
            // itself was cancelled, not just skipped over.
            let saw_hook_marker = tokio::time::timeout(Duration::from_millis(300), async {
                while GlobalState.get(&hook_key).is_none() {
                    tokio::task::yield_now().await;
                }
            })
            .await;
            assert!(
                saw_hook_marker.is_ok(),
                "beforePause should run right after the hotkey pause, not after the cancelled sleep"
            );

            // The cancelled invocation's remaining click must never
            // arrive, even after its original sleep would have elapsed.
            assert!(
                tokio::time::timeout(Duration::from_millis(2200), event_rx.recv())
                    .await
                    .is_err(),
                "the cancelled invocation's remaining click must not fire"
            );

            GlobalState.delete(&hook_key);
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
        ) -> Result<(), String> {
            self.0.send(()).map_err(|error| error.to_string())
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
async fn rev_stop_unloads_script_after_current_invocation() {
    use std::fs;
    use tokio::sync::{mpsc, watch};

    let path = std::env::temp_dir().join(format!(
        "rev-idle-stop-test-{}.js",
        std::process::id(),
    ));
    fs::write(&path, r#"export default (() => { rev.stop(); rev.click(1, 1); })"#).unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct StopMouse(mpsc::UnboundedSender<()>);
    impl MouseInput for StopMouse {
        fn click_at(
            &mut self,
            _x: i32,
            _y: i32,
            _button: Button,
        ) -> Result<(), String> {
            self.0.send(()).map_err(|error| error.to_string())
        }
    }

    let mouse: SharedMouse = Rc::new(RefCell::new(StopMouse(event_tx)));
    let (command_tx, command_rx) = mpsc::channel(32);
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
                Duration::from_millis(5),
            ));

            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(30), event_rx.recv())
                    .await
                    .is_err()
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
        ) -> Result<(), String> {
            self.0.send((x, y)).map_err(|error| error.to_string())
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
        ) -> Result<(), String> {
            self.0.send(()).map_err(|error| error.to_string())
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
        ) -> Result<(), String> {
            self.0.send(()).map_err(|error| error.to_string())
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
fn capture_is_allowed_only_when_the_script_is_paused_or_stopped() {
    assert_eq!(capture_action(false, false, false, false), CaptureAction::Reject);
    assert_eq!(capture_action(false, true, false, false), CaptureAction::Enable);
    assert_eq!(capture_action(false, true, true, true), CaptureAction::Enable);
    assert_eq!(capture_action(false, true, true, false), CaptureAction::Reject);
    assert_eq!(capture_action(true, true, true, false), CaptureAction::Disable);

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
        events.borrow_mut().clear();
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
async fn capture_commands_stay_disarmed_without_a_script_path() {
    CaptureState.set_enabled(false);
    let (command_tx, command_rx) = mpsc::channel(8);
    let (_pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (state_tx, _state_rx) = watch::channel(StateUpdate {
        phase: ScriptPhase::Unloaded,
        capture: false,
        locked: false,
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

        command_tx.send(ScriptCommand::Capture).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(!CaptureState.is_enabled());

        command_tx.send(ScriptCommand::StartCapture).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(!CaptureState.is_enabled());

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
