use crate::{
    app::{ActionGate, ScriptCommand, ScriptCommandSender, ScriptPhase, StateUpdate},
    bridge::{ScriptUiEvent, ScriptUiEventKind, ScriptUiPublisher, WsConnection},
    capture::{CaptureState, LockState},
    script::{ScriptControl, ScriptWorker},
};
use futures_util::SinkExt;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{atomic::{AtomicBool, Ordering}, Arc},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};

#[test]
fn stop_interrupts_infinite_javascript() {
    for case in [
        "module", "import", "default", "hook", "detached", "ui", "self_default", "self_ui", "ui_sync_default",
        "background_dependency_init", "daemon_sync", "daemon_ready", "callback_ready", "background_self_stop", "pending_install_stop",
        "pause_pending_timer", "pause_ready_chain", "pause_sync_segment", "stale_load", "delayed_load", "channel_close", "worker_drop",
        "wait_cancel", "canceled_startup", "channel_close_load",
    ] {
        let directory = std::env::temp_dir().join(format!("rev-idle-stop-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let output = fs::File::create(directory.join("child.log")).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "script::worker_tests::interruption_child", "--nocapture"])
            .env("REV_SCRIPT_STOP_CASE", case)
            .env("REV_SCRIPT_STOP_DIR", &directory)
            .env("APPDATA", directory.join("appdata"))
            .stdout(Stdio::from(output.try_clone().unwrap()))
            .stderr(Stdio::from(output))
            .spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break Some(status); }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let output = fs::read_to_string(directory.join("child.log")).unwrap();
        assert!(status.is_some_and(|status| status.success()), "{case}: child failed or exceeded the 10-second watchdog\n{output}");
        if case == "module" || case == "import" {
            assert!(output.contains("entered_loop"), "{case}: Stop ran before the loop began\n{output}");
        }
        if case == "canceled_startup" {
            assert!(!output.contains("unexpected_module"), "a canceled terminal enqueue must still prevent initial module evaluation\n{output}");
        }
        if case == "channel_close_load" {
            assert!(!output.contains("unexpected_module"), "closing the command channel must prevent a queued Load from evaluating\n{output}");
        }
        if case == "channel_close" || case == "worker_drop" || case == "wait_cancel" || case == "channel_close_load" {
            assert!(output.contains("worker termination cleanup completed"), "{case}: cleanup proof missing\n{output}");
        } else {
            assert!(output.contains("replacement ran on the same worker"), "{case}: replacement proof missing\n{output}");
        }
        let directory = directory.canonicalize().unwrap();
        assert!(directory.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn two_runtime_teardown_invalidates_old_work() {
    let case = "teardown";
    let directory = std::env::temp_dir().join(format!("rev-idle-teardown-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let output = fs::File::create(directory.join("child.log")).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "script::worker_tests::interruption_child", "--nocapture"])
        .env("REV_SCRIPT_STOP_CASE", case)
        .env("REV_SCRIPT_STOP_DIR", &directory)
        .env("APPDATA", directory.join("appdata"))
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(output))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let child_status = loop {
        if let Some(status) = child.try_wait().unwrap() { break Some(status); }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let output = fs::read_to_string(directory.join("child.log")).unwrap();
    assert!(child_status.is_some_and(|status| status.success()), "teardown child failed or exceeded the 10-second watchdog\n{output}");
    assert!(output.contains("two-runtime teardown invalidated old work"), "teardown proof missing\n{output}");
    let directory = directory.canonicalize().unwrap();
    assert!(directory.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    fs::remove_dir_all(directory).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn interruption_child() {
    let Ok(case) = std::env::var("REV_SCRIPT_STOP_CASE") else { return; };
    let directory = PathBuf::from(std::env::var_os("REV_SCRIPT_STOP_DIR").unwrap());
    let entered_path = directory.join("entered.txt");
    let replacement_path = directory.join("replacement.txt");
    let obsolete_started_path = directory.join("obsolete-started.txt");
    let obsolete_finished_path = directory.join("obsolete-finished.txt");
    let daemon_started_path = directory.join("daemon-started.txt");
    let global_before_reload_path = directory.join("global-before-reload.txt");
    let entered = serde_json::to_string(&entered_path.to_string_lossy()).unwrap();
    let replacement = serde_json::to_string(&replacement_path.to_string_lossy()).unwrap();
    let obsolete_started = serde_json::to_string(&obsolete_started_path.to_string_lossy()).unwrap();
    let obsolete_finished = serde_json::to_string(&obsolete_finished_path.to_string_lossy()).unwrap();
    let daemon_started = serde_json::to_string(&daemon_started_path.to_string_lossy()).unwrap();
    let global_before_reload = serde_json::to_string(&global_before_reload_path.to_string_lossy()).unwrap();
    let source = match case.as_str() {
        "module" => "console.log('entered_loop'); while (true) {} export default function() {}".to_owned(),
        "import" => {
            fs::write(directory.join("spin.js"), "console.log('entered_loop'); while (true) {}").unwrap();
            "import './spin.js'; export default function() {}".to_owned()
        }
        "default" | "delayed_load" | "channel_close" | "worker_drop" | "wait_cancel" => format!("export default async function() {{ using owner = await rev.screenOwnership(); rev.write_file({entered}, 'entered'); while (true) {{}} }}"),
        "hook" => format!("export function afterLoad() {{ rev.write_file({entered}, 'entered'); while (true) {{}} }} export default function() {{}}"),
        "detached" => format!("export function afterLoad() {{ (async () => {{ await rev.sleep(1); rev.write_file({entered}, 'entered'); while (true) {{}} }})(); }} export default async function() {{ await rev.sleep(60000); }}"),
        "ui" => format!("export function afterLoad() {{ rev.ui('button', {{}}); rev.ui.button.setOnClick(function() {{ rev.write_file({entered}, 'entered'); while (true) {{}} }}); }} export default async function() {{ await rev.sleep(60000); }}"),
        "ui_sync_default" => format!("export function afterLoad() {{ rev.ui('button', {{}}); rev.ui.button.setOnClick(function() {{ rev.write_file({entered}, 'entered'); while (true) {{}} }}); }} export default function() {{}}"),
        "self_default" => format!("export default function() {{ rev.write_file({entered}, 'entered'); try {{ rev.stop(); }} catch (_) {{}} finally {{ while (true) {{}} }} }}"),
        "self_ui" => format!("export function afterLoad() {{ rev.ui('button', {{}}); rev.ui.button.setOnClick(async function() {{ await rev.sleep(1); rev.write_file({entered}, 'entered'); rev.stop(); while (true) {{}} }}); }} export default async function() {{ await rev.sleep(60000); }}"),
        "background_dependency_init" => {
            fs::write(directory.join("dependency.js"), "globalThis.dependencyInit = (globalThis.dependencyInit ?? 0) + 1; export const marker = `dependency:${globalThis.dependencyInit}`;")
                .unwrap();
            format!("import {{ marker }} from './dependency.js'; export function afterLoad() {{ rev.daemon('dependency', function() {{ rev.write_file({entered}, marker); while (true) {{}} }}); }} export default function() {{}}")
        }
        "daemon_sync" => format!("export function afterLoad() {{ rev.daemon('sync', function() {{ rev.write_file({entered}, 'entered'); while (true) {{}} }}); }} export default function() {{}}"),
        "daemon_ready" => format!("export function afterLoad() {{ rev.daemon('ready', async function() {{ rev.write_file({entered}, 'entered'); while (true) {{ await Promise.resolve().then(() => {{}}); }} }}); }} export default function() {{}}"),
        "callback_ready" => format!("export function afterLoad() {{ rev.ui('button', {{}}); rev.ui.button.setOnClick(function() {{ rev.write_file({entered}, 'entered'); return (async function() {{ while (true) await Promise.resolve(); }})(); }}); }} export default function() {{}}"),
        "background_self_stop" => format!("export function afterLoad() {{ rev.daemon('selfStop', function() {{ rev.write_file({entered}, 'entered'); rev.stop(); while (true) {{}} }}); }} export default function() {{}}"),
        "pending_install_stop" => {
            fs::write(directory.join("pending.js"), "if (typeof rev !== 'undefined') await new Promise(() => {}); export const marker = 'pending';").unwrap();
            format!("import {{ marker }} from './pending.js'; export function afterLoad() {{ rev.ui('button', {{}}); rev.ui.button.setOnClick(function() {{ rev.write_file({entered}, marker); }}); }} export default function() {{}}")
        }
        "pause_pending_timer" => format!("export default async function() {{ rev.write_file({entered}, 'before'); await rev.sleep(200); rev.write_file({entered}, 'after'); await rev.sleep(60000); }}"),
        "pause_ready_chain" => format!("export default async function() {{ rev.write_file({entered}, 'before'); for (let i = 0; i < 1000000; i++) await Promise.resolve(); rev.write_file({entered}, 'after'); await rev.sleep(60000); }}"),
        "pause_sync_segment" => format!("export default async function() {{ rev.write_file({entered}, 'before'); const until = Date.now() + 1000; while (Date.now() < until) {{}} rev.write_file({entered}, 'after'); await rev.sleep(60000); }}"),
        "teardown" => format!("export function afterLoad() {{ rev.global.teardownValue = 'before'; rev.write_file({global_before_reload}, String(rev.global.teardownValue)); rev.ui('button', {{}}); rev.ui.button.setOnClick(async function() {{ rev.write_file({obsolete_started}, 'started'); await this.width(); rev.global.teardownValue = 'obsolete'; rev.write_file({obsolete_finished}, 'finished'); }}); rev.daemon('old', async function() {{ rev.write_file({daemon_started}, 'started'); await rev.sleep(60000); }}); }} export default function() {{}}"),
        "stale_load" => format!("export function afterLoad() {{ rev.write_file({entered}, 'obsolete load ran'); }} export default function() {{}}"),
        "canceled_startup" => "console.log('unexpected_module'); export default function() {}".to_owned(),
        "channel_close_load" => "console.log('unexpected_module'); while (true) {} export default function() {}".to_owned(),
        _ => panic!("unknown interruption case"),
    };
    let source_path = directory.join("first.ts");
    let second_path = directory.join("second.js");
    fs::write(&source_path, source).unwrap();
    fs::write(&second_path, if case == "teardown" {
        format!("export default function() {{ rev.write_file({replacement}, String(rev.global.teardownValue)); }}")
    } else {
        format!("let wrote = false; export default function() {{ if (!wrote) {{ rev.write_file({replacement}, 'replacement'); wrote = true; }} }} export function beforeStop() {{ console.log('unexpected_before_stop'); while (true) {{}} }}")
    }).unwrap();

    let (address, server) = crate::bridge::test_support::raw_server().await;
    let connection = WsConnection::connect_for_test(address, Duration::from_millis(10), Duration::from_secs(1));
    let mut socket = server.await.unwrap();
    let mut generations = connection.connection_generation();
    while *generations.borrow_and_update() == 0 { generations.changed().await.unwrap(); }
    let control = ScriptControl::default();
    let (commands, command_rx) = mpsc::channel(32);
    let commands = ScriptCommandSender::new(commands, control.clone());
    if case != "channel_close" && case != "channel_close_load" { crate::bridge::register_control_handlers(&connection, commands.clone()).unwrap(); }
    let (events, event_rx) = mpsc::channel(256);
    crate::bridge::register_script_ui_handlers(&connection, events).unwrap();
    let gate = ActionGate::default();
    let (shutdown, shutdown_rx) = watch::channel(false);
    let running = Arc::new(AtomicBool::new(false));
    let console_locked = Arc::new(AtomicBool::new(false));
    let (state, states) = watch::channel(StateUpdate::new(true, false, false, false, false));
    let publisher = ScriptUiPublisher::default();
    let mut snapshots = publisher.subscribe();
    if case == "channel_close_load" {
        commands.send(ScriptCommand::Load(source_path)).await.unwrap();
        drop(commands);
        let worker = ScriptWorker::start(
            command_rx, event_rx, connection.clone(), gate.subscribe(), None, gate, shutdown_rx,
            running.clone(), console_locked.clone(), CaptureState, LockState::default(), state, control, publisher,
        ).unwrap();
        worker.wait().await.unwrap();
        assert!(!running.load(Ordering::Acquire));
        assert!(snapshots.borrow().session_id.is_none());
        assert!(states.has_changed().is_err());
        assert!(!LockState::default().is_enabled());
        assert!(!console_locked.load(Ordering::Acquire));
        assert!(!CaptureState.is_enabled());
        println!("worker termination cleanup completed");
        shutdown.send_replace(true);
        connection.shutdown().await;
        return;
    }
    if case == "canceled_startup" {
        for _ in 0..32 { commands.send(ScriptCommand::StopCapture).await.unwrap(); }
        let producer = commands.clone();
        let pending = tokio::spawn(async move { producer.send(ScriptCommand::Stop).await });
        tokio::time::timeout(Duration::from_secs(1), async {
            while control.requested_generation() == 0 { tokio::task::yield_now().await; }
        }).await.unwrap();
        pending.abort();
        assert!(pending.await.unwrap_err().is_cancelled());
    }
    let worker = ScriptWorker::start(
        command_rx, event_rx, connection.clone(), gate.subscribe(), if case == "stale_load" { None } else { Some(source_path.clone()) }, gate, shutdown_rx,
        running.clone(), console_locked.clone(), CaptureState, LockState::default(), state, control.clone(), publisher,
    ).unwrap();

    if case == "stale_load" {
        let old_load = control.request_terminal();
        let newer_stop = control.request_terminal();
        commands.send(ScriptCommand::Terminal { command: Box::new(ScriptCommand::Stop), generation: newer_stop }).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while control.acknowledged_generation() < newer_stop { tokio::time::sleep(Duration::from_millis(1)).await; }
        }).await.unwrap();
        // A producer can be descheduled after signaling but before enqueue.
        commands.send(ScriptCommand::Terminal { command: Box::new(ScriptCommand::Load(source_path)), generation: old_load }).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!entered_path.exists(), "a delayed Load must not overtake a newer handled Stop");
    }

    if case == "ui" || case == "self_ui" || case == "ui_sync_default" || case == "callback_ready" || case == "teardown" {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if snapshots.borrow_and_update().elements.first().is_some_and(|element| element.events.contains(&ScriptUiEventKind::Click)) { break; }
                snapshots.changed().await.unwrap();
            }
        }).await.unwrap();
        let snapshot = snapshots.borrow().clone();
        socket.send(tokio_tungstenite::tungstenite::Message::Text(serde_json::json!({
            "uuid": uuid::Uuid::new_v4(), "type": "ScriptUiEvent", "payload": ScriptUiEvent {
                session_id: snapshot.session_id.unwrap(), element_id: "button".to_owned(),
                instance_id: snapshot.elements[0].instance_id, events_version: snapshot.elements[0].events_version,
                event: ScriptUiEventKind::Click,
            },
        }).to_string().into())).await.unwrap();
    }
    if case == "pending_install_stop" {
        tokio::time::timeout(Duration::from_secs(2), async {
            while snapshots.borrow_and_update().elements.is_empty() { snapshots.changed().await.unwrap(); }
        }).await.unwrap();
    }
    if case == "module" || case == "import" {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if snapshots.borrow_and_update().session_id.is_some() { break; }
                snapshots.changed().await.unwrap();
            }
        }).await.unwrap();
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert!(running.load(Ordering::Acquire), "initial module loading must be visible to Ctrl+C as stoppable");
    } else if case == "teardown" {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !obsolete_started_path.is_file() || !daemon_started_path.is_file() { tokio::time::sleep(Duration::from_millis(5)).await; }
        }).await.unwrap();
    } else if case != "stale_load" && case != "canceled_startup" && case != "pending_install_stop" {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !entered_path.is_file() { tokio::time::sleep(Duration::from_millis(5)).await; }
        }).await.unwrap();
    }
    if case == "background_dependency_init" {
        assert_eq!(fs::read_to_string(&entered_path).unwrap(), "dependency:1");
    }

    if case == "channel_close" || case == "worker_drop" || case == "wait_cancel" {
        if case == "channel_close" {
            drop(commands);
            worker.wait().await.unwrap();
        } else if case == "worker_drop" {
            drop(worker);
        } else {
            let waiter = tokio::spawn(worker.wait());
            tokio::task::yield_now().await;
            waiter.abort();
            assert!(waiter.await.unwrap_err().is_cancelled());
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while running.load(Ordering::Acquire) || snapshots.borrow().session_id.is_some() || states.has_changed().is_ok() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }).await.unwrap();
        assert!(!LockState::default().is_enabled());
        assert!(!console_locked.load(Ordering::Acquire));
        assert!(!CaptureState.is_enabled());
        println!("worker termination cleanup completed");
        shutdown.send_replace(true);
        connection.shutdown().await;
        return;
    }

    if case == "delayed_load" {
        let generation = control.request_terminal();
        tokio::time::timeout(Duration::from_secs(2), async {
            while running.load(Ordering::Acquire) || snapshots.borrow().session_id.is_some() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }).await.unwrap();
        assert!(control.acknowledged_generation() < generation, "interrupt cleanup must not acknowledge an envelope it has not consumed");
        commands.send(ScriptCommand::Terminal { command: Box::new(ScriptCommand::Load(second_path.clone())), generation }).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while fs::read_to_string(&replacement_path).ok().as_deref() != Some("replacement") { tokio::time::sleep(Duration::from_millis(1)).await; }
        }).await.unwrap();
    }

    if case == "pause_pending_timer" || case == "pause_ready_chain" || case == "pause_sync_segment" {
        let requested_at = Instant::now();
        socket.send(tokio_tungstenite::tungstenite::Message::Text(serde_json::json!({
            "uuid": uuid::Uuid::new_v4(), "type": "PauseScript", "payload": {},
        }).to_string().into())).await.unwrap();
        let acknowledged = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if states.borrow().phase == ScriptPhase::Paused { break; }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }).await.is_ok();
        if acknowledged {
            let main_progress_at_ack = fs::read_to_string(&entered_path).unwrap_or_default();
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(fs::read_to_string(&entered_path).unwrap_or_default(), main_progress_at_ack, "main progress continued after Pause acknowledgment");
            println!("pause {case} acknowledged in {}ms with no main progress", requested_at.elapsed().as_millis());
            socket.send(tokio_tungstenite::tungstenite::Message::Text(serde_json::json!({
                "uuid": uuid::Uuid::new_v4(), "type": "ResumeScript", "payload": {},
            }).to_string().into())).await.unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                while states.borrow().phase != ScriptPhase::Running { tokio::time::sleep(Duration::from_millis(1)).await; }
            }).await.unwrap();
        } else {
            println!("pause {case} did not acknowledge before Stop");
        }
    }

    if case == "teardown" {
        socket.send(tokio_tungstenite::tungstenite::Message::Text(serde_json::json!({
            "uuid": uuid::Uuid::new_v4(), "type": "StopScript", "payload": {},
        }).to_string().into())).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if !running.load(Ordering::Acquire) && snapshots.borrow().session_id.is_none() && states.borrow().phase == ScriptPhase::Stopped { break; }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.unwrap();
        let obsolete_session_host_writes = usize::from(obsolete_finished_path.is_file());
        assert_eq!(obsolete_session_host_writes, 0);
        assert!(snapshots.borrow().session_id.is_none());
        let global_value_before_reload = fs::read_to_string(&global_before_reload_path).unwrap();
        if replacement_path.exists() { fs::remove_file(&replacement_path).unwrap(); }
        commands.send(ScriptCommand::Load(second_path)).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while !replacement_path.is_file() { tokio::time::sleep(Duration::from_millis(5)).await; }
        }).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let global_value_after_reload = fs::read_to_string(&replacement_path).unwrap();
        let new_session_states_are_empty = snapshots.borrow().elements.is_empty();
        assert!(new_session_states_are_empty);
        assert_eq!(global_value_after_reload, global_value_before_reload);
        println!("two-runtime teardown invalidated old work");
        commands.send(ScriptCommand::Exit).await.unwrap();
        worker.wait().await.unwrap();
        shutdown.send_replace(true);
        connection.shutdown().await;
        return;
    }

    // This packet is handled by the control runtime while QuickJS spins on
    // its own worker. A timer on the spinning worker cannot prove recovery.
    if case != "self_default" && case != "self_ui" && case != "background_self_stop" && case != "stale_load" && case != "canceled_startup" {
        socket.send(tokio_tungstenite::tungstenite::Message::Text(serde_json::json!({
            "uuid": uuid::Uuid::new_v4(), "type": "StopScript", "payload": {},
        }).to_string().into())).await.unwrap();
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if !running.load(Ordering::Acquire)
                && snapshots.borrow().session_id.is_none()
                && (states.borrow().phase == ScriptPhase::Stopped || (case == "stale_load" && states.borrow().phase == ScriptPhase::Unloaded))
            { break; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    assert!(!LockState::default().is_enabled());
    assert!(!console_locked.load(Ordering::Acquire));
    assert!(!CaptureState.is_enabled());
    if replacement_path.exists() { fs::remove_file(&replacement_path).unwrap(); }
    commands.send(ScriptCommand::Load(second_path)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while fs::read_to_string(&replacement_path).ok().as_deref() != Some("replacement") { tokio::time::sleep(Duration::from_millis(5)).await; }
    }).await.unwrap();
    assert_eq!(fs::read_to_string(replacement_path).unwrap(), "replacement");
    println!("replacement ran on the same worker");
    commands.send(ScriptCommand::Exit).await.unwrap();
    worker.wait().await.unwrap();
    shutdown.send_replace(true);
    connection.shutdown().await;
}
