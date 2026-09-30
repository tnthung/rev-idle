use super::{
    bindings::{BridgeMouseInput, HostControls, SharedMouse},
    history,
    session::{ScriptInvocationError, ScriptSession},
};
#[cfg(test)]
use super::State;
use crate::{
    app::{ActionGate, PauseUpdate, ScriptCommand, StateUpdate},
    bridge::{ConnectionEvent, QueuedScriptUiEvent, ScriptUiPublisher, WsConnection},
    capture::{CaptureState, LockState},
    script::ScriptControl,
    window::Win32WindowControl,
};
use futures_util::{stream::FuturesUnordered, StreamExt};
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    path::Path,
    pin::Pin,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};

async fn load_path(
    path: &Path,
    connection: WsConnection,
    script_control: &ScriptControl,
    ui_publisher: &ScriptUiPublisher,
) -> Result<(Rc<ScriptSession>, std::path::PathBuf), String> {
    let absolute_path = std::path::absolute(path)
        .map_err(|error| format!("failed to resolve {}: {error}", path.display()))?;
    let session_control = script_control.register_session();
    let mut stopped = session_control.subscribe_stopped();
    if *stopped.borrow_and_update() {
        return Err(crate::script::SessionControl::error_message().to_owned());
    }
    let source = tokio::select! {
        source = tokio::fs::read_to_string(&absolute_path) => source
            .map_err(|error| format!("failed to read {}: {error}", absolute_path.display()))?,
        _ = stopped.wait_for(|stopped| *stopped) => {
            return Err(crate::script::SessionControl::error_message().to_owned());
        }
    };
    if session_control.is_stopped() {
        return Err(crate::script::SessionControl::error_message().to_owned());
    }

    ScriptSession::new_with_connection_and_control(
        &source,
        &absolute_path.to_string_lossy(),
        connection,
        session_control,
        ui_publisher.clone(),
    )
        .await
        .map(|session| (Rc::new(session), absolute_path.clone()))
        .map_err(|error| format!("failed to load {}: {error}", absolute_path.display()))
}

fn terminal_generation(control: &ScriptControl) -> u64 {
    if control.terminal_pending() {
        control.requested_generation()
    } else {
        control.request_internal_terminal()
    }
}
fn unpack_command(command: ScriptCommand) -> (ScriptCommand, Option<u64>) {
    match command {
        ScriptCommand::Terminal { command, generation } => (*command, Some(generation)),
        command => (command, None),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_with_control(
    commands: mpsc::Receiver<ScriptCommand>,
    ui_events: mpsc::Receiver<QueuedScriptUiEvent>,
    connection: WsConnection,
    hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    actions_paused: ActionGate,
    shutdown: watch::Receiver<bool>,
    script_running: Arc<AtomicBool>,
    console_locked: Arc<AtomicBool>,
    capture_state: CaptureState,
    lock_state: LockState,
    state_updates: watch::Sender<StateUpdate>,
    script_control: ScriptControl,
    ui_publisher: ScriptUiPublisher,
) -> Result<(), String> {
    let mouse: SharedMouse = Rc::new(RefCell::new(BridgeMouseInput {
        connection: connection.clone(),
    }));

    run_with_controls_and_lifecycle_with_control(
        commands,
        ui_events,
        connection,
        hotkey_pauses,
        initial_path,
        HostControls { mouse, window: Rc::new(Win32WindowControl), actions_paused },
        Duration::from_millis(50),
        shutdown,
        script_running,
        console_locked,
        capture_state,
        lock_state,
        Some(history::history_path()),
        state_updates,
        script_control,
        ui_publisher,
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
    let lock_state = LockState::default();
    let (state_updates, _) = watch::channel(StateUpdate::new(false, false, false, false, false));
    let connection = WsConnection::disconnected_for_test();
    run_with_controls_and_lifecycle(
        commands,
        connection,
        hotkey_pauses,
        initial_path,
        controls,
        loop_delay,
        shutdown,
        script_running,
        console_locked,
        capture_state,
        lock_state,
        None,
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
    lock_state: LockState,
    loop_delay: Duration,
) -> Result<(), String> {
    let (_shutdown_tx, shutdown) = watch::channel(false);
    let script_running = Arc::new(AtomicBool::new(false));
    let console_locked = Arc::new(AtomicBool::new(false));
    let capture_state = CaptureState::default();
    let connection = WsConnection::disconnected_for_test();
    run_with_controls_and_lifecycle(
        commands,
        connection,
        hotkey_pauses,
        initial_path,
        controls,
        loop_delay,
        shutdown,
        script_running,
        console_locked,
        capture_state,
        lock_state,
        None,
        state_updates,
    )
    .await
}

#[cfg(test)]
pub(super) async fn run_with_controls_and_connection(
    commands: mpsc::Receiver<ScriptCommand>,
    connection: WsConnection,
    pause_rx: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    controls: HostControls,
    loop_delay: Duration,
) -> Result<(), String> {
    let (_shutdown_tx, shutdown) = watch::channel(false);
    let script_running = Arc::new(AtomicBool::new(false));
    let console_locked = Arc::new(AtomicBool::new(false));
    let capture_state = CaptureState::default();
    let lock_state = LockState::default();
    let (state_updates, _) = watch::channel(StateUpdate::new(false, false, false, false, false));
    run_with_controls_and_lifecycle(
        commands,
        connection,
        pause_rx,
        initial_path,
        controls,
        loop_delay,
        shutdown,
        script_running,
        console_locked,
        capture_state,
        lock_state,
        None,
        state_updates,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(super) async fn run_with_controls_and_lifecycle(
    commands: mpsc::Receiver<ScriptCommand>,
    connection: WsConnection,
    hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    controls: HostControls,
    loop_delay: Duration,
    shutdown: watch::Receiver<bool>,
    script_running: Arc<AtomicBool>,
    console_locked: Arc<AtomicBool>,
    capture_state: CaptureState,
    lock_state: LockState,
    script_history_path: Option<std::path::PathBuf>,
    state_updates: watch::Sender<StateUpdate>,
) -> Result<(), String> {
    let (_ui_events_tx, ui_events) = mpsc::channel(1);
    run_with_controls_and_lifecycle_with_control(
        commands,
        ui_events,
        connection,
        hotkey_pauses,
        initial_path,
        controls,
        loop_delay,
        shutdown,
        script_running,
        console_locked,
        capture_state,
        lock_state,
        script_history_path,
        state_updates,
        ScriptControl::default(),
        ScriptUiPublisher::default(),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_with_controls_and_lifecycle_with_control(
    mut commands: mpsc::Receiver<ScriptCommand>,
    mut ui_events: mpsc::Receiver<QueuedScriptUiEvent>,
    connection: WsConnection,
    mut hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    controls: HostControls,
    loop_delay: Duration,
    mut shutdown: watch::Receiver<bool>,
    script_running: Arc<AtomicBool>,
    console_locked: Arc<AtomicBool>,
    capture_state: CaptureState,
    lock_state: LockState,
    script_history_path: Option<std::path::PathBuf>,
    state_updates: watch::Sender<StateUpdate>,
    script_control: ScriptControl,
    ui_publisher: ScriptUiPublisher,
) -> Result<(), String> {
    let mut current_path = initial_path;
    let (mut script_history, script_history_writable) = match script_history_path.as_deref() {
        Some(path) => match history::load_history(path) {
            Ok(history) => (history, true),
            Err(error) => {
                eprintln!("failed to read script history {}: {error}", path.display());
                (Vec::new(), false)
            }
        },
        None => (Vec::new(), true),
    };
    let connection_generation = connection.connection_generation();
    let mut connected = false;
    let mut connection_events = None;
    let mut session = match current_path.clone() {
        Some(path) => {
            script_running.store(true, Ordering::Release);
            match load_path(&path, connection.clone(), &script_control, &ui_publisher).await {
            Ok((session, absolute_path)) => {
                session.screen_ownership.attach(lock_state, state_updates.clone());
                connection_events = Some(connection.connection_events());
                connected = *connection_generation.borrow() != 0;
                current_path = Some(absolute_path.clone());
                history::record(&mut script_history, absolute_path.clone());
                if script_history_writable
                    && let Some(path) = script_history_path.as_deref()
                {
                    history::save_history(path, &script_history);
                }
                println!("running {}", absolute_path.display());
                Some(session)
            }
            Err(error) => {
                eprintln!("{error}");
                script_history.retain(|entry| {
                    entry != &std::path::absolute(&path).unwrap_or_else(|_| path.clone())
                });
                if script_history_writable
                    && let Some(path) = script_history_path.as_deref()
                {
                    history::save_history(path, &script_history);
                }
                None
            }
            }
        }
        None => None,
    };
    script_running.store(session.is_some(), Ordering::Release);
    let initial_hotkey_update = *hotkey_pauses.borrow_and_update();
    let mut paused = false;
    if session.is_none() {
        controls.actions_paused.reset_if_current(initial_hotkey_update);
    }
    let mut hotkey_channel_open = true;
    let mut terminal_updates = script_control.subscribe_terminal();
    terminal_updates.borrow_and_update();
    let mut script_history_strings = script_history
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let mut script_history_changed = !script_history_strings.is_empty();
    let mut script_history_checked_at = Instant::now();
    let mut invocation: Option<Pin<Box<dyn Future<Output = Result<bool, ScriptInvocationError>>>>> = None;
    let mut next_invocation_at = None;
    let mut ui_events_open = true;
    let mut pending_ui_events: VecDeque<QueuedScriptUiEvent> = VecDeque::new();
    let mut callbacks: FuturesUnordered<Pin<Box<dyn Future<Output = Result<(), String>>>>> = FuturesUnordered::new();
    let mut background: Option<Pin<Box<dyn Future<Output = ()>>>> = session.as_ref().map(|active| {
        let active = active.clone();
        let controls = controls.clone();
        Box::pin(async move { active.drive_background(controls).await }) as Pin<Box<dyn Future<Output = ()>>>
    });
    let mut after_load_pending = session.is_some();
    let mut main_hook: Option<Pin<Box<dyn Future<Output = Result<(), String>>>>> = None;
    let mut main_hook_name = "";
    let mut connection_hooks = VecDeque::new();
    let mut before_pause: Option<(PauseUpdate, Pin<Box<dyn Future<Output = Result<(), String>>>>)> = None;
    let mut after_resume: Option<Pin<Box<dyn Future<Output = Result<(), String>>>>> = None;
    let mut lock_after_resume = false;

    loop {
        if *shutdown.borrow() {
            let generation = terminal_generation(&script_control);
            drop(invocation.take());
            drop(background.take());
            drop(main_hook.take());
            drop(before_pause.take());
            drop(after_resume.take());
            connection_hooks.clear();
            callbacks.clear();
            pending_ui_events.clear();
            drop(connection_events.take());
            if let Some(active) = session.as_ref() {
                active.terminate();
            }
            drop(session.take());
            script_control.acknowledge_terminal(generation);
            script_running.store(false, Ordering::Release);
            capture_state.set_enabled(false);
            lock_state.set_enabled(false);
            controls.actions_paused.set_paused(false);
            console_locked.store(false, Ordering::Release);
            return Ok(());
        }

        if session.is_none() {
            script_control.acknowledge_unpaired_terminal();
        } else if session.as_ref().is_some_and(|active| active.is_stopped()) {
            drop(invocation.take());
            next_invocation_at = None;
            drop(background.take());
            drop(main_hook.take());
            drop(before_pause.take());
            drop(after_resume.take());
            connection_hooks.clear();
            callbacks.clear();
            pending_ui_events.clear();
            drop(connection_events.take());
            if let Some(active) = session.as_ref() {
                active.terminate();
            }
            lock_state.set_enabled(false);
            controls.actions_paused.set_paused(false);
            drop(session.take());
            script_running.store(false, Ordering::Release);
            console_locked.store(false, Ordering::Release);
            paused = false;
            println!("script stopped");
            script_control.acknowledge_unpaired_terminal();
            continue;
        }

        if script_history_checked_at.elapsed() >= Duration::from_secs(1) {
            script_history_checked_at = Instant::now();
            let history_len = script_history.len();
            script_history.retain(|path| path.is_file());
            if script_history.len() != history_len {
                script_history_strings = script_history
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect();
                script_history_changed = true;
                if script_history_writable
                    && let Some(path) = script_history_path.as_deref()
                {
                    history::save_history(path, &script_history);
                }
            }
        }
        let mut state = StateUpdate::new(
            current_path.is_some(),
            session.is_some(),
            paused,
            capture_state.is_enabled(),
            lock_state.is_enabled(),
        );
        let state_changed = {
            let current = state_updates.borrow();
            if state.locked { state.lock_label = current.lock_label.clone(); }
            current.phase != state.phase
                || current.capture != state.capture
                || current.locked != state.locked
                || script_history_changed
        };
        if state_changed {
            state.scripts = script_history_strings.clone();
            state_updates.send_replace(state);
            script_history_changed = false;
        }

        // Recomputed every iteration (rather than at each of the many places
        // `session`/`paused` change) so the console's lock state can never
        // drift out of sync with them.
        console_locked.store(session.is_some() && !paused, Ordering::Release);

        if let Some(active) = session.as_ref() {
            paused = active.is_paused();
            let requested = controls.actions_paused.current_update();
            if paused && !requested.paused() {
                active.acknowledge_pause(false);
                paused = false;
                capture_state.set_enabled(false);
                if lock_after_resume {
                    active.screen_ownership.set_manual_lock();
                    lock_after_resume = false;
                }
                active.screen_ownership.set_paused(false);
                let active = active.clone();
                let controls = controls.clone();
                after_resume = Some(Box::pin(async move { active.run_after_resume(controls).await }));
                println!("script resumed");
            } else if !paused
                && requested.paused()
                && before_pause.is_none()
                && after_resume.is_none()
                && !after_load_pending
                && main_hook_name != "afterLoad"
            {
                let active = active.clone();
                let controls = controls.clone();
                before_pause = Some((requested, Box::pin(async move { active.run_before_pause(controls).await })));
            }

            if !paused
                && before_pause.is_none()
                && after_resume.is_none()
                && main_hook.is_none()
            {
                if after_load_pending {
                    let active = active.clone();
                    let controls = controls.clone();
                    main_hook = Some(Box::pin(async move { active.run_after_load(controls).await }));
                    main_hook_name = "afterLoad";
                    after_load_pending = false;
                } else if !requested.paused()
                    && let Some(event) = connection_hooks.pop_front()
                {
                    let active = active.clone();
                    let controls = controls.clone();
                    match event {
                        ConnectionEvent::Connected => {
                            main_hook = Some(Box::pin(async move { active.run_on_connect(controls).await }));
                            main_hook_name = "onConnect";
                        }
                        ConnectionEvent::Disconnected => {
                            main_hook = Some(Box::pin(async move { active.run_on_disconnect(controls).await }));
                            main_hook_name = "onDisconnect";
                        }
                    }
                }
            }
        }

        while let Some(event) = pending_ui_events.pop_front() {
            let Some(active) = session.as_ref().cloned() else { break };
            let generations = connection.connection_generation();
            let capture_state = capture_state;
            let capture_epoch = event.capture_epoch;
            let generation = event.generation;
            let controls = controls.clone();
            callbacks.push(Box::pin(async move {
                active
                    .dispatch_ui_event_if_current(
                        event.event,
                        controls,
                        move || {
                            capture_state.inactive_epoch() == Some(capture_epoch)
                                && *generations.borrow() == generation
                        },
                    )
                    .await
            }));
        }

        if invocation.is_none()
            && main_hook.is_none()
            && before_pause.is_none()
            && after_resume.is_none()
            && !after_load_pending
            && connection_hooks.is_empty()
            && next_invocation_at.is_none_or(|deadline| deadline <= tokio::time::Instant::now())
            && session
                .as_ref()
                .is_some_and(|active| {
                    !active.is_stopped()
                        && !paused
                        && !controls.actions_paused.is_paused()
                        && (connected || !active.has_connection_hooks())
                })
        {
            let active = session.as_ref().expect("eligible script session must exist").clone();
            let controls = controls.clone();
            invocation = Some(Box::pin(async move { active.invoke_classified((), controls).await }));
            next_invocation_at = None;
        }

        let mut completed_invocation = None;
        let mut completed_before_pause = None;
        let mut completed_after_resume = None;
        let mut completed_main_hook = None;
        let command = tokio::select! {
                biased;
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        let generation = terminal_generation(&script_control);
                        drop(invocation.take());
                        drop(background.take());
                        drop(main_hook.take());
                        drop(before_pause.take());
                        drop(after_resume.take());
                        connection_hooks.clear();
                        callbacks.clear();
                        pending_ui_events.clear();
                        drop(connection_events.take());
                        if let Some(active) = session.as_ref() {
                            active.terminate();
                        }
                        drop(session.take());
                        script_control.acknowledge_terminal(generation);
                        script_running.store(false, Ordering::Release);
                        capture_state.set_enabled(false);
                        lock_state.set_enabled(false);
                        controls.actions_paused.set_paused(false);
                        console_locked.store(false, Ordering::Release);
                        return Ok(());
                    }
                    continue;
                }
                changed = terminal_updates.changed() => {
                    let _ = changed;
                    None
                }
                command = commands.recv() => match command {
                    Some(command) => {
                        Some(unpack_command(command))
                    },
                    None => {
                        let generation = terminal_generation(&script_control);
                        drop(invocation.take());
                        drop(background.take());
                        drop(main_hook.take());
                        drop(before_pause.take());
                        drop(after_resume.take());
                        connection_hooks.clear();
                        callbacks.clear();
                        pending_ui_events.clear();
                        drop(connection_events.take());
                        if let Some(active) = session.as_ref() {
                            active.terminate();
                        }
                        drop(session.take());
                        script_control.acknowledge_terminal(generation);
                        script_running.store(false, Ordering::Release);
                        capture_state.set_enabled(false);
                        lock_state.set_enabled(false);
                        controls.actions_paused.set_paused(false);
                        console_locked.store(false, Ordering::Release);
                        return Ok(());
                    }
                },
                event = ui_events.recv(), if ui_events_open => {
                    match event {
                        Some(event) => pending_ui_events.push_back(event),
                        None => ui_events_open = false,
                    }
                    None
                }
                callback = callbacks.next(), if !callbacks.is_empty() => {
                    if let Some(Err(error)) = callback
                        && !session.as_ref().is_some_and(|active| active.is_stopped())
                    {
                        eprintln!("[Script UI] callback failed: {error}");
                    }
                    None
                }
                result = async {
                    match before_pause.as_mut() {
                        Some((update, future)) => Some((*update, future.await)),
                        None => std::future::pending().await,
                    }
                }, if !paused && after_resume.is_none() => {
                    before_pause = None;
                    completed_before_pause = result;
                    None
                }
                result = async {
                    match after_resume.as_mut() {
                        Some(future) => Some(future.await),
                        None => std::future::pending().await,
                    }
                }, if !paused => {
                    after_resume = None;
                    completed_after_resume = result;
                    None
                }
                result = async {
                    match main_hook.as_mut() {
                        Some(future) => Some(future.await),
                        None => std::future::pending().await,
                    }
                }, if !paused && before_pause.is_none() && after_resume.is_none() => {
                    main_hook = None;
                    completed_main_hook = result;
                    None
                }
                changed = hotkey_pauses.changed(), if hotkey_channel_open => {
                    match changed {
                        Ok(()) => {
                            let update = *hotkey_pauses.borrow_and_update();
                            if session.is_none() {
                                controls.actions_paused.reset_if_current(update);
                            }
                        }
                        Err(_) => hotkey_channel_open = false,
                    }
                    continue;
                }
                event = async {
                    match connection_events.as_mut() {
                        Some(events) => events.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match event {
                        Some(ConnectionEvent::Connected) => {
                            connected = true;
                            connection_hooks.push_back(ConnectionEvent::Connected);
                        }
                        Some(ConnectionEvent::Disconnected) => {
                            connected = false;
                            connection_hooks.push_back(ConnectionEvent::Disconnected);
                        }
                        None => {
                            connected = false;
                            connection_events = None;
                        }
                    }
                    continue;
                }
                result = async {
                    match invocation.as_mut() {
                        Some(future) => Some(future.await),
                        None => std::future::pending().await,
                    }
                }, if !paused && before_pause.is_none() && after_resume.is_none() && main_hook.is_none() => {
                    invocation = None;
                    completed_invocation = result;
                    None
                }
                _ = async {
                    match next_invocation_at {
                        Some(deadline) => tokio::time::sleep_until(deadline).await,
                        None => std::future::pending().await,
                    }
                }, if invocation.is_none() && next_invocation_at.is_some() => {
                    next_invocation_at = None;
                    None
                }
                _ = async {
                    match session.as_ref() {
                        Some(active) => active.drive().await,
                        None => std::future::pending().await,
                    }
                }, if !paused
                    && !controls.actions_paused.is_paused()
                    && before_pause.is_none()
                    && after_resume.is_none()
                    && main_hook.is_none()
                    && invocation.is_none() => {
                    tokio::task::yield_now().await;
                    None
                }
                _ = async {
                    match background.as_mut() {
                        Some(future) => future.await,
                        None => std::future::pending().await,
                    }
                } => {
                    background = None;
                    None
                }
        };

        if let Some((update, result)) = completed_before_pause {
            if let Err(error) = result {
                eprintln!("beforePause hook failed: {error}");
            }
            if controls.actions_paused.is_current(update) && update.paused()
                && let Some(active) = session.as_ref()
            {
                active.acknowledge_pause(true);
                active.screen_ownership.set_paused(true);
                paused = true;
                lock_state.set_enabled(false);
                println!("script paused");
            }
            continue;
        }

        if let Some(result) = completed_after_resume {
            if let Err(error) = result {
                eprintln!("afterResume hook failed: {error}");
            }
            continue;
        }

        if let Some(result) = completed_main_hook {
            if let Err(error) = result {
                eprintln!("{main_hook_name} hook failed: {error}");
            }
            main_hook_name = "";
            continue;
        }

        if let Some(result) = completed_invocation {
            let terminal = match result {
                Ok(stop_requested) => stop_requested,
                Err(error) => {
                    if *connection.connection_generation().borrow() == 0 {
                        connected = false;
                    }
                    if error.disconnected
                        && session.as_ref().is_some_and(|active| active.has_connection_hooks())
                    {
                        false
                    } else {
                        eprintln!("script invocation failed: {error}");
                        true
                    }
                }
            };
            if terminal {
                next_invocation_at = None;
                background = None;
                main_hook = None;
                before_pause = None;
                after_resume = None;
                connection_hooks.clear();
                callbacks.clear();
                pending_ui_events.clear();
                connection_events = None;
                if let Some(active) = session.as_ref() {
                    active.terminate();
                }
                lock_state.set_enabled(false);
                controls.actions_paused.set_paused(false);
                session = None;
                script_running.store(false, Ordering::Release);
                console_locked.store(false, Ordering::Release);
                paused = false;
                println!("script stopped");
                script_control.acknowledge_unpaired_terminal();
            } else {
                next_invocation_at = Some(tokio::time::Instant::now() + loop_delay);
            }
            continue;
        }

        if let Some((command, command_generation)) = command {
            if command_generation.is_some_and(|generation| {
                generation <= script_control.acknowledged_generation()
                    || generation < script_control.requested_generation()
            }) {
                continue;
            }
            match command {
                ScriptCommand::RemoveFromHistory(path) => {
                    let history_len = script_history.len();
                    script_history.retain(|entry| entry != &path);
                    if script_history.len() != history_len {
                        script_history_strings = script_history
                            .iter()
                            .map(|path| path.to_string_lossy().into_owned())
                            .collect();
                        script_history_changed = true;
                        if script_history_writable
                            && let Some(path) = script_history_path.as_deref()
                        {
                            history::save_history(path, &script_history);
                        }
                    }
                }
                load @ (ScriptCommand::Load(_) | ScriptCommand::LoadLocked(_)) => {
                    let generation = command_generation.unwrap_or_else(|| terminal_generation(&script_control));
                    invocation = None;
                    next_invocation_at = None;
                    background = None;
                    main_hook = None;
                    before_pause = None;
                    after_resume = None;
                    connection_hooks.clear();
                    callbacks.clear();
                    pending_ui_events.clear();
                    let (path, lock_after_load) = match load {
                        ScriptCommand::Load(path) => (path, false),
                        ScriptCommand::LoadLocked(path) => (path, true),
                        _ => unreachable!(),
                    };
                    drop(connection_events.take());
                    if let Some(active) = session.as_ref() {
                        active.terminate();
                    }
                    capture_state.set_enabled(false);
                    lock_state.set_enabled(false);
                    controls.actions_paused.set_paused(false);
                    session = None;
                    script_control.acknowledge_terminal(generation);
                    script_running.store(false, Ordering::Release);
                    paused = false;
                    current_path = Some(path);
                    script_running.store(true, Ordering::Release);
                    if let Some(path) = current_path.clone() {
                        match load_path(&path, connection.clone(), &script_control, &ui_publisher).await {
                            Ok((loaded, absolute_path)) => {
                                loaded.screen_ownership.attach(lock_state, state_updates.clone());
                                connection_events = Some(connection.connection_events());
                                connected = *connection_generation.borrow() != 0;
                                current_path = Some(absolute_path.clone());
                                history::record(&mut script_history, absolute_path.clone());
                                script_history_strings = script_history
                                    .iter()
                                    .map(|path| path.to_string_lossy().into_owned())
                                    .collect();
                                script_history_changed = true;
                                if script_history_writable
                                    && let Some(path) = script_history_path.as_deref()
                                {
                                    history::save_history(path, &script_history);
                                }
                                if lock_after_load {
                                    loaded.screen_ownership.set_manual_lock();
                                }
                                background = Some({
                                    let loaded = loaded.clone();
                                    let controls = controls.clone();
                                    Box::pin(async move { loaded.drive_background(controls).await })
                                });
                                session = Some(loaded);
                                after_load_pending = true;
                                script_running.store(true, Ordering::Release);
                                println!("running {}", absolute_path.display());
                            }
                            Err(error) => {
                                script_running.store(false, Ordering::Release);
                                eprintln!("{error}");
                                let history_len = script_history.len();
                                script_history.retain(|entry| {
                                    entry != &std::path::absolute(&path).unwrap_or_else(|_| path.clone())
                                });
                                if script_history.len() != history_len {
                                    script_history_strings = script_history
                                        .iter()
                                        .map(|path| path.to_string_lossy().into_owned())
                                        .collect();
                                    script_history_changed = true;
                                    if script_history_writable
                                        && let Some(path) = script_history_path.as_deref()
                                    {
                                        history::save_history(path, &script_history);
                                    }
                                }
                            }
                        }
                    }
                }
                ScriptCommand::Reload | ScriptCommand::ReloadLocked => {
                    let generation = command_generation.unwrap_or_else(|| terminal_generation(&script_control));
                    invocation = None;
                    next_invocation_at = None;
                    background = None;
                    main_hook = None;
                    before_pause = None;
                    after_resume = None;
                    connection_hooks.clear();
                    callbacks.clear();
                    pending_ui_events.clear();
                    let lock_after_load = matches!(command, ScriptCommand::ReloadLocked);
                    connection_events = None;
                    if let Some(active) = session.as_ref() {
                        active.terminate();
                    }
                    capture_state.set_enabled(false);
                    lock_state.set_enabled(false);
                    controls.actions_paused.set_paused(false);
                    session = None;
                    script_control.acknowledge_terminal(generation);
                    script_running.store(false, Ordering::Release);
                    paused = false;
                    script_running.store(true, Ordering::Release);
                    if let Some(path) = current_path.clone() {
                        match load_path(&path, connection.clone(), &script_control, &ui_publisher).await {
                            Ok((loaded, absolute_path)) => {
                                loaded.screen_ownership.attach(lock_state, state_updates.clone());
                                connection_events = Some(connection.connection_events());
                                connected = *connection_generation.borrow() != 0;
                                current_path = Some(absolute_path.clone());
                                history::record(&mut script_history, absolute_path.clone());
                                script_history_strings = script_history
                                    .iter()
                                    .map(|path| path.to_string_lossy().into_owned())
                                    .collect();
                                script_history_changed = true;
                                if script_history_writable
                                    && let Some(path) = script_history_path.as_deref()
                                {
                                    history::save_history(path, &script_history);
                                }
                                if lock_after_load {
                                    loaded.screen_ownership.set_manual_lock();
                                }
                                background = Some({
                                    let loaded = loaded.clone();
                                    let controls = controls.clone();
                                    Box::pin(async move { loaded.drive_background(controls).await })
                                });
                                session = Some(loaded);
                                after_load_pending = true;
                                script_running.store(true, Ordering::Release);
                                println!("reloaded {}", absolute_path.display());
                            }
                            Err(error) => {
                                script_running.store(false, Ordering::Release);
                                eprintln!("{error}");
                                let history_len = script_history.len();
                                script_history.retain(|entry| {
                                    entry != &std::path::absolute(&path).unwrap_or_else(|_| path.clone())
                                });
                                if script_history.len() != history_len {
                                    script_history_strings = script_history
                                        .iter()
                                        .map(|path| path.to_string_lossy().into_owned())
                                        .collect();
                                    script_history_changed = true;
                                    if script_history_writable
                                        && let Some(path) = script_history_path.as_deref()
                                    {
                                        history::save_history(path, &script_history);
                                    }
                                }
                            }
                        }
                    } else {
                        println!("no script is loaded; use load <script-path>");
                    }
                }
                ScriptCommand::Pause => {
                    if session.is_none() {
                        controls.actions_paused.set_paused(false);
                        println!("no script is running");
                    } else if controls.actions_paused.is_paused() {
                        println!("script is already paused");
                    } else {
                        controls.actions_paused.set_paused(true);
                    }
                }
                ScriptCommand::Resume | ScriptCommand::ResumeLocked => {
                    lock_after_resume |= matches!(command, ScriptCommand::ResumeLocked);
                    if session.is_none() {
                        lock_after_resume = false;
                        lock_state.set_enabled(false);
                        controls.actions_paused.set_paused(false);
                        println!("script is stopped; use reload or load");
                    } else if controls.actions_paused.is_paused() {
                        controls.actions_paused.set_paused(false);
                        if session.as_ref().is_some_and(|active| !active.is_paused())
                            && lock_after_resume
                            && let Some(active) = session.as_ref()
                        {
                            active.screen_ownership.set_manual_lock();
                            lock_after_resume = false;
                        }
                    } else {
                        if lock_after_resume
                            && let Some(active) = session.as_ref()
                        {
                            active.screen_ownership.set_manual_lock();
                            lock_after_resume = false;
                        }
                        println!("script is already running");
                    }
                }
                ScriptCommand::Stop => {
                    let generation = command_generation.unwrap_or_else(|| terminal_generation(&script_control));
                    invocation = None;
                    next_invocation_at = None;
                    background = None;
                    main_hook = None;
                    before_pause = None;
                    after_resume = None;
                    connection_hooks.clear();
                    callbacks.clear();
                    pending_ui_events.clear();
                    connection_events = None;
                    lock_state.set_enabled(false);
                    controls.actions_paused.set_paused(false);
                    if let Some(active) = session.as_ref() {
                        active.terminate();
                        session = None;
                        script_control.acknowledge_terminal(generation);
                        script_running.store(false, Ordering::Release);
                        console_locked.store(false, Ordering::Release);
                        paused = false;
                        println!("script stopped");
                    } else {
                        script_control.acknowledge_terminal(generation);
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
                ScriptCommand::Lock => {
                    if let Some(active) = session.as_ref() {
                        active.screen_ownership.set_manual_lock();
                    } else {
                        lock_state.set_enabled(true);
                    }
                }
                ScriptCommand::Exit => {
                    let generation = command_generation.unwrap_or_else(|| terminal_generation(&script_control));
                    drop(invocation.take());
                    drop(background.take());
                    drop(main_hook.take());
                    drop(before_pause.take());
                    drop(after_resume.take());
                    connection_hooks.clear();
                    callbacks.clear();
                    pending_ui_events.clear();
                    drop(connection_events.take());
                    if let Some(active) = session.as_ref() {
                        active.terminate();
                    }
                    capture_state.set_enabled(false);
                    lock_state.set_enabled(false);
                    controls.actions_paused.set_paused(false);
                    drop(session.take());
                    script_control.acknowledge_terminal(generation);
                    script_running.store(false, Ordering::Release);
                    console_locked.store(false, Ordering::Release);
                    return Ok(());
                }
                ScriptCommand::SetPaused(requested_paused) => {
                    let update = controls.actions_paused.current_update();
                    if session.is_none() && update.paused() == requested_paused {
                        controls.actions_paused.reset_if_current(update);
                    }
                }
                ScriptCommand::Terminal { .. } => unreachable!("terminal command must be unpacked before dispatch"),
            }

            continue;
        }

        continue;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CaptureAction {
    Enable,
    Disable,
    Reject,
}

pub(super) fn capture_action(
    enabled: bool,
    script_loaded: bool,
    paused: bool,
) -> CaptureAction {
    if enabled {
        CaptureAction::Disable
    } else if !script_loaded || paused {
        CaptureAction::Enable
    } else {
        CaptureAction::Reject
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
