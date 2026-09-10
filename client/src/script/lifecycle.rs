use super::{
    bindings::{BridgeMouseInput, HostControls, SharedMouse},
    session::ScriptSession,
};
#[cfg(test)]
use super::State;
use crate::{
    app::{ActionGate, PauseUpdate, ScriptCommand, StateUpdate},
    bridge::WsConnection,
    capture::CaptureState,
    window::Win32WindowControl,
};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{mpsc, watch};

async fn load_path(path: &Path, connection: WsConnection) -> Result<ScriptSession, String> {
    let source = tokio::fs::read_to_string(path)
        .await
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    ScriptSession::new_with_connection(&source, &path.to_string_lossy(), connection)
        .await
        .map_err(|error| format!("failed to load {}: {error}", path.display()))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run(
    commands: mpsc::Receiver<ScriptCommand>,
    connection: WsConnection,
    hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    actions_paused: ActionGate,
    shutdown: watch::Receiver<bool>,
    script_running: Arc<AtomicBool>,
    console_locked: Arc<AtomicBool>,
    capture_state: CaptureState,
    state_updates: watch::Sender<StateUpdate>,
) -> Result<(), String> {
    let mouse: SharedMouse = Rc::new(RefCell::new(BridgeMouseInput {
        connection: connection.clone(),
    }));

    run_with_controls_and_lifecycle(
        commands,
        connection,
        hotkey_pauses,
        initial_path,
        HostControls { mouse, window: Rc::new(Win32WindowControl), actions_paused },
        Duration::from_millis(50),
        shutdown,
        script_running,
        console_locked,
        capture_state,
        state_updates,
    )
    .await
}

#[cfg(test)]
pub(super) async fn run_with_controls(
    commands: mpsc::Receiver<ScriptCommand>,
    _states: watch::Receiver<State>,
    hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    controls: HostControls,
    loop_delay: Duration,
) -> Result<(), String> {
    let (_shutdown_tx, shutdown) = watch::channel(false);
    let script_running = Arc::new(AtomicBool::new(false));
    let console_locked = Arc::new(AtomicBool::new(false));
    let capture_state = CaptureState::default();
    let (state_updates, _) = watch::channel(StateUpdate::new(false, false, false, false));
    run_with_controls_and_lifecycle(
        commands,
        WsConnection::disconnected_for_test(),
        hotkey_pauses,
        initial_path,
        controls,
        loop_delay,
        shutdown,
        script_running,
        console_locked,
        capture_state,
        state_updates,
    )
    .await
}

#[cfg(test)]
pub(super) async fn run_with_controls_and_state(
    commands: mpsc::Receiver<ScriptCommand>,
    state_updates: watch::Sender<StateUpdate>,
    hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    controls: HostControls,
    loop_delay: Duration,
) -> Result<(), String> {
    let (_shutdown_tx, shutdown) = watch::channel(false);
    let script_running = Arc::new(AtomicBool::new(false));
    let console_locked = Arc::new(AtomicBool::new(false));
    let capture_state = CaptureState::default();
    run_with_controls_and_lifecycle(
        commands,
        WsConnection::disconnected_for_test(),
        hotkey_pauses,
        initial_path,
        controls,
        loop_delay,
        shutdown,
        script_running,
        console_locked,
        capture_state,
        state_updates,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn run_with_controls_and_lifecycle(
    mut commands: mpsc::Receiver<ScriptCommand>,
    connection: WsConnection,
    mut hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    controls: HostControls,
    loop_delay: Duration,
    mut shutdown: watch::Receiver<bool>,
    script_running: Arc<AtomicBool>,
    console_locked: Arc<AtomicBool>,
    capture_state: CaptureState,
    state_updates: watch::Sender<StateUpdate>,
) -> Result<(), String> {
    let mut current_path = initial_path;
    let mut session = match current_path.as_deref() {
        Some(path) => match load_path(path, connection.clone()).await {
            Ok(session) => {
                println!("running {}", path.display());
                Some(session)
            }
            Err(error) => {
                eprintln!("{error}");
                None
            }
        },
        None => None,
    };
    script_running.store(session.is_some(), Ordering::Release);
    let initial_hotkey_update = *hotkey_pauses.borrow_and_update();
    let mut paused = false;
    apply_hotkey_update(
        initial_hotkey_update,
        &controls.actions_paused,
        session.is_some(),
        &mut paused,
    );
    let mut hotkey_channel_open = true;
    // A non-stop command pulled off `commands` while an invocation was in
    // flight (see below) can't be pushed back onto the mpsc channel, so it
    // waits here and takes priority over the channel on the next iteration.
    let mut pending_command: Option<ScriptCommand> = None;

    loop {
        if *shutdown.borrow() {
            script_running.store(false, Ordering::Release);
            return Ok(());
        }

        let state = StateUpdate::new(
            current_path.is_some(),
            session.is_some(),
            paused,
            capture_state.is_enabled(),
        );
        if *state_updates.borrow() != state {
            state_updates.send_replace(state);
        }

        // Recomputed every iteration (rather than at each of the many places
        // `session`/`paused` change) so the console's lock state can never
        // drift out of sync with them.
        console_locked.store(session.is_some() && !paused, Ordering::Release);

        if hotkey_channel_open {
            match hotkey_pauses.has_changed() {
                Ok(true) => {
                    let update = *hotkey_pauses.borrow_and_update();
                    apply_hotkey_update_and_report(
                        update,
                        &controls.actions_paused,
                        session.as_ref(),
                        &mut paused,
                    )
                    .await;
                    disable_capture_if_running(&capture_state, session.is_some(), paused);
                    continue;
                }
                Ok(false) => {}
                Err(_) => hotkey_channel_open = false,
            }
        }

        let command = if let Some(command) = pending_command.take() {
            Some(command)
        } else if session.is_some() && !paused {
            match commands.try_recv() {
                Ok(command) => Some(command),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    script_running.store(false, Ordering::Release);
                    return Ok(());
                }
            }
        } else {
            tokio::select! {
                biased;
                changed = hotkey_pauses.changed(), if hotkey_channel_open => {
                    match changed {
                        Ok(()) => {
                            let update = *hotkey_pauses.borrow_and_update();
                            apply_hotkey_update_and_report(
                                update,
                                &controls.actions_paused,
                                session.as_ref(),
                                &mut paused,
                            )
                            .await;
                            disable_capture_if_running(&capture_state, session.is_some(), paused);
                        }
                        Err(_) => hotkey_channel_open = false,
                    }
                    continue;
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        script_running.store(false, Ordering::Release);
                        return Ok(());
                    }
                    continue;
                }
                command = commands.recv() => match command {
                    Some(command) => Some(command),
                    None => {
                        script_running.store(false, Ordering::Release);
                        return Ok(());
                    }
                },
            }
        };

        if let Some(command) = command {
            match command {
                ScriptCommand::Load(path) => {
                    capture_state.set_enabled(false);
                    controls.actions_paused.set_paused(false);
                    session = None;
                    script_running.store(false, Ordering::Release);
                    paused = false;
                    current_path = Some(path);
                    if let Some(path) = current_path.as_deref() {
                        match load_path(path, connection.clone()).await {
                            Ok(loaded) => {
                                session = Some(loaded);
                                script_running.store(true, Ordering::Release);
                                println!("running {}", path.display());
                            }
                            Err(error) => eprintln!("{error}"),
                        }
                    }
                }
                ScriptCommand::Reload => {
                    capture_state.set_enabled(false);
                    controls.actions_paused.set_paused(false);
                    session = None;
                    script_running.store(false, Ordering::Release);
                    paused = false;
                    if let Some(path) = current_path.as_deref() {
                        match load_path(path, connection.clone()).await {
                            Ok(loaded) => {
                                session = Some(loaded);
                                script_running.store(true, Ordering::Release);
                                println!("reloaded {}", path.display());
                            }
                            Err(error) => eprintln!("{error}"),
                        }
                    } else {
                        println!("no script is loaded; use load <script-path>");
                    }
                }
                ScriptCommand::Pause => {
                    if session.is_none() {
                        controls.actions_paused.set_paused(false);
                        println!("no script is running");
                    } else if paused {
                        controls.actions_paused.set_paused(true);
                        println!("script is already paused");
                    } else {
                        if let Some(active) = session.as_ref()
                            && let Err(error) = active.run_before_pause().await
                        {
                            eprintln!("beforePause hook failed: {error}");
                        }
                        controls.actions_paused.set_paused(true);
                        paused = true;
                        println!("script paused");
                    }
                }
                ScriptCommand::Resume => {
                    if session.is_none() {
                        controls.actions_paused.set_paused(false);
                        println!("script is stopped; use reload or load");
                    } else if paused {
                        capture_state.set_enabled(false);
                        controls.actions_paused.set_paused(false);
                        paused = false;
                        println!("script resumed");
                        if let Some(active) = session.as_ref()
                            && let Err(error) = active.run_after_resume().await
                        {
                            eprintln!("afterResume hook failed: {error}");
                        }
                    } else {
                        controls.actions_paused.set_paused(false);
                        println!("script is already running");
                    }
                }
                ScriptCommand::Stop => {
                    controls.actions_paused.set_paused(false);
                    if session.take().is_some() {
                        script_running.store(false, Ordering::Release);
                        paused = false;
                        println!("script stopped");
                    } else {
                        println!("script is already stopped");
                    }
                }
                ScriptCommand::Capture => match capture_action(
                    capture_state.is_enabled(),
                    session.is_some(),
                    paused,
                ) {
                    CaptureAction::Enable => {
                        capture_state.set_enabled(true);
                        println!("mouse capture started");
                    }
                    CaptureAction::Disable => {
                        capture_state.set_enabled(false);
                        println!("mouse capture stopped");
                    }
                    CaptureAction::Reject => {
                        eprintln!("cannot capture while script is running");
                    }
                },
                ScriptCommand::StartCapture => {
                    if session.is_none() || paused {
                        capture_state.set_enabled(true);
                    }
                }
                ScriptCommand::StopCapture | ScriptCommand::CaptureConsumed => {
                    capture_state.set_enabled(false);
                }
                ScriptCommand::Exit => {
                    capture_state.set_enabled(false);
                    controls.actions_paused.set_paused(false);
                    script_running.store(false, Ordering::Release);
                    return Ok(());
                }
                ScriptCommand::SetPaused(requested_paused) => {
                    let update = controls.actions_paused.current_update();
                    if update.paused() == requested_paused {
                        apply_hotkey_update_and_report(
                            update,
                            &controls.actions_paused,
                            session.as_ref(),
                            &mut paused,
                        )
                        .await;
                        disable_capture_if_running(&capture_state, session.is_some(), paused);
                    }
                }
            }

            continue;
        }

        let stop_requested = {
            let Some(active) = session.as_ref() else {
                continue;
            };
            let invocation = active.invoke((), controls.clone());
            tokio::pin!(invocation);
            tokio::select! {
                result = &mut invocation => {
                    match result {
                        Ok(stop_requested) => stop_requested,
                        Err(_) if controls.actions_paused.is_paused() => {
                            // A host action (rev.click/scroll/drag/...) can still
                            // observe the gate flipping to paused in the narrow
                            // window between the hotkey thread setting it and this
                            // select noticing (see the hotkey_pauses branch below);
                            // those calls no-op rather than throw, but treat any
                            // stray rejection here the same way: an expected
                            // interruption, not a script failure, so the session
                            // stays alive instead of being stopped.
                            false
                        }
                        Err(error) => {
                            eprintln!("script invocation failed: {error}");
                            true
                        }
                    }
                }
                // F8 flips the shared ActionGate immediately, from a separate
                // thread, before this update is even published here — so by
                // the time we observe it, the pause has already taken effect.
                // Cancel the in-flight invocation right away (same trick as
                // Stop/Exit below) instead of letting it run until its next
                // rev.* call or its own completion. `changed()` marks the
                // update as seen, so (unlike the plain command case) this
                // arm must finish processing it itself rather than leaving
                // it for the top of the loop to pick up.
                changed = hotkey_pauses.changed(), if hotkey_channel_open => {
                    if changed.is_err() {
                        hotkey_channel_open = false;
                    } else {
                        let update = *hotkey_pauses.borrow_and_update();
                        apply_hotkey_update_and_report(
                            update,
                            &controls.actions_paused,
                            Some(active),
                            &mut paused,
                        )
                        .await;
                        disable_capture_if_running(&capture_state, true, paused);
                    }
                    false
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        script_running.store(false, Ordering::Release);
                        return Ok(());
                    }
                    false
                }
                // Dropping `invocation` here (by not polling it again) cancels
                // whatever the script is awaiting, including rev.sleep, so
                // Stop/Exit take effect immediately instead of waiting for the
                // current invocation (and its sleep) to finish on its own.
                command = commands.recv() => match command {
                    Some(ScriptCommand::Stop) => true,
                    Some(ScriptCommand::Exit) => {
                        capture_state.set_enabled(false);
                        controls.actions_paused.set_paused(false);
                        script_running.store(false, Ordering::Release);
                        return Ok(());
                    }
                    Some(other) => {
                        pending_command = Some(other);
                        false
                    }
                    None => {
                        script_running.store(false, Ordering::Release);
                        return Ok(());
                    }
                },
            }
        };

        if stop_requested {
            controls.actions_paused.set_paused(false);
            session = None;
            script_running.store(false, Ordering::Release);
            paused = false;
            println!("script stopped");
            continue;
        }

        tokio::time::sleep(loop_delay).await;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CaptureAction {
    Enable,
    Disable,
    Reject,
}

pub(super) fn capture_action(enabled: bool, script_loaded: bool, paused: bool) -> CaptureAction {
    if enabled {
        CaptureAction::Disable
    } else if script_loaded && !paused {
        CaptureAction::Reject
    } else {
        CaptureAction::Enable
    }
}

pub(super) fn disable_capture_if_running(
    capture_state: &CaptureState,
    script_loaded: bool,
    paused: bool,
) {
    if script_loaded && !paused {
        capture_state.set_enabled(false);
    }
}

pub(super) fn apply_hotkey_update(
    update: PauseUpdate,
    gate: &ActionGate,
    session_running: bool,
    lifecycle_paused: &mut bool,
) -> bool {
    if session_running {
        if !gate.is_current(update) {
            return false;
        }
        *lifecycle_paused = update.paused();
        true
    } else if gate.reset_if_current(update) {
        *lifecycle_paused = false;
        true
    } else {
        false
    }
}

/// Applies a hotkey-driven pause/resume update and prints its result. The
/// `ActionGate` itself was already flipped by the hotkey handler before this
/// update was published, so unlike the `Pause`/`Resume` commands there is no
/// window to run `beforePause` before actions actually stop; the hooks still
/// run here so scripts see hotkey-triggered pauses the same as command ones.
async fn apply_hotkey_update_and_report(
    update: PauseUpdate,
    gate: &ActionGate,
    session: Option<&ScriptSession>,
    lifecycle_paused: &mut bool,
) {
    let was_paused = *lifecycle_paused;
    if !apply_hotkey_update(
        update,
        gate,
        session.is_some(),
        lifecycle_paused,
    ) {
        return;
    }

    if session.is_none() {
        println!("no script is running");
    } else if *lifecycle_paused == was_paused {
        if *lifecycle_paused {
            println!("script is already paused");
        } else {
            println!("script is already running");
        }
    } else if *lifecycle_paused {
        if let Some(active) = session
            && let Err(error) = active.run_before_pause().await
        {
            eprintln!("beforePause hook failed: {error}");
        }
        println!("script paused");
    } else {
        println!("script resumed");
        if let Some(active) = session
            && let Err(error) = active.run_after_resume().await
        {
            eprintln!("afterResume hook failed: {error}");
        }
    }
}
