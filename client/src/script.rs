use crate::{
    capture::CaptureState,
    console::ScriptCommand,
    hotkey::{ActionGate, PauseUpdate},
    udp::State,
    window::{post_click_to_game, Axis, Win32WindowControl, WindowControl},
};
use rquickjs::{
    convert::Coerced,
    function::Rest,
    function::Async,
    function::Opt,
    promise::MaybePromise,
    AsyncContext,
    AsyncRuntime,
    CaughtError,
    Error,
    FromJs,
    Function,
    Object,
    Persistent,
    Value,
};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{mpsc, watch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Left,
    Right,
    Middle,
}

struct BridgeMouseInput;

trait MouseInput {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), String>;
}

fn click_at_with<C>(
    x: i32,
    y: i32,
    click: C,
) -> Result<(), String>
where
    C: FnOnce(i32, i32) -> Result<(), String>,
{
    click(x, y)
}

impl MouseInput for BridgeMouseInput {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        _button: Button,
    ) -> Result<(), String> {
        click_at_with(x, y, post_click_to_game)
    }
}

type SharedMouse = Rc<RefCell<dyn MouseInput>>;
type SharedWindow = Rc<dyn WindowControl>;

#[derive(Clone)]
struct HostControls {
    mouse: SharedMouse,
    window: SharedWindow,
    actions_paused: ActionGate,
}

fn host_error(message: String) -> Error {
    Error::new_from_js_message("host control", "JavaScript", message)
}

fn ensure_actions_running(paused: &ActionGate) -> Result<(), Error> {
    if paused.is_paused() {
        Err(Error::new_from_js_message("actions", "JavaScript", "actions are paused"))
    } else { Ok(()) }
}

fn validate_coordinate(value: f64) -> Result<i32, Error> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < i32::MIN as f64
        || value > i32::MAX as f64
    {
        return Err(Error::new_from_js_message(
            "number",
            "finite 32-bit integer coordinates",
            "invalid mouse coordinates",
        ));
    }

    Ok(value as i32)
}

fn parse_button(button: Opt<Value>) -> Result<Button, Error> {
    match button.0 {
        None => Ok(Button::Left),
        Some(value) if value.is_undefined() => Ok(Button::Left),
        Some(value) => {
            let type_name = value.type_name();
            let Some(value) = value.as_string() else {
                return Err(Error::new_from_js(type_name, "mouse button"));
            };
            let value = value.to_string()?;
            match value.as_str() {
                "left" => Ok(Button::Left),
                "right" => Ok(Button::Right),
                "middle" => Ok(Button::Middle),
                value => Err(Error::new_from_js_message(
                    "string",
                    "mouse button",
                    format!("unsupported button: {value}"),
                )),
            }
        }
    }
}

fn click_with_controls(
    controls: &HostControls,
    x: i32,
    y: i32,
    button: Button,
) -> Result<(), Error> {
    ensure_actions_running(&controls.actions_paused)?;
    controls
        .mouse
        .borrow_mut()
        .click_at(x, y, button)
        .map_err(|message| Error::new_from_js_message("mouse input", "JavaScript", message))
}

fn validate_scroll_length(value: f64) -> Result<i32, Error> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < i32::MIN as f64
        || value > i32::MAX as f64
    {
        return Err(Error::new_from_js_message(
            "number",
            "finite 32-bit integer scroll length",
            "invalid scroll length",
        ));
    }

    Ok(value as i32)
}

fn parse_scroll_axis(axis: Opt<Value>) -> Result<Axis, Error> {
    match axis.0 {
        None => Ok(Axis::Vertical),
        Some(value) if value.is_undefined() => Ok(Axis::Vertical),
        Some(value) => {
            let type_name = value.type_name();
            let Some(value) = value.as_string() else {
                return Err(Error::new_from_js(type_name, "scroll axis"));
            };
            let value = value.to_string()?;
            match value.as_str() {
                "vertical" | "v" => Ok(Axis::Vertical),
                "horizontal" | "h" => Ok(Axis::Horizontal),
                value => Err(Error::new_from_js_message(
                    "string",
                    "scroll axis",
                    format!("unsupported scroll axis: {value}"),
                )),
            }
        }
    }
}

fn format_console_message(args: Rest<Value>) -> rquickjs::Result<String> {
    args.0
        .into_iter()
        .map(|value| {
            if value.is_string() {
                value
                    .as_string()
                    .expect("string value must have a string representation")
                    .to_string()
            } else if value.is_object() {
                let ctx = value.ctx().clone();
                match ctx.json_stringify(value.clone())? {
                    Some(json) => json.to_string(),
                    None => Ok(Coerced::<std::string::String>::from_js(&ctx, value)?.0),
                }
            } else {
                let ctx = value.ctx().clone();
                Ok(Coerced::<std::string::String>::from_js(&ctx, value)?.0)
            }
        })
        .collect::<rquickjs::Result<Vec<_>>>()
        .map(|values| values.join(" "))
}

struct ScriptSession {
    // Rust drops fields in declaration order. Persistent roots must be gone
    // before their context and runtime.
    script: Persistent<Function<'static>>,
    memory: Persistent<Object<'static>>,
    freeze: Persistent<Function<'static>>,
    context: AsyncContext,
    _runtime: AsyncRuntime,
}

impl ScriptSession {
    async fn new(source: &str) -> rquickjs::Result<Self> {
        let runtime = AsyncRuntime::new()?;
        let context = AsyncContext::full(&runtime).await?;
        let source = source.to_owned();

        let (script, memory, freeze) = context
            .async_with(async move |ctx| {
                let console = Object::new(ctx.clone())?;
                console.set(
                    "log",
                    Function::new(ctx.clone(), move |args: Rest<Value>| {
                        println!("{}", format_console_message(args)?);
                        Ok::<(), rquickjs::Error>(())
                    })?,
                )?;
                console.set(
                    "error",
                    Function::new(ctx.clone(), move |args: Rest<Value>| {
                        eprintln!("{}", format_console_message(args)?);
                        Ok::<(), rquickjs::Error>(())
                    })?,
                )?;
                ctx.globals().set("console", console)?;

                let freeze: Function = ctx.eval("Object.freeze")?;
                let script: Function = ctx.eval(source)?;
                let memory = Object::new(ctx.clone())?;

                Ok::<_, rquickjs::Error>((
                    Persistent::save(&ctx, script),
                    Persistent::save(&ctx, memory),
                    Persistent::save(&ctx, freeze),
                ))
            })
            .await?;

        Ok(Self {
            script,
            memory,
            freeze,
            context,
            _runtime: runtime,
        })
    }

    async fn invoke<C: Into<HostControls>>(
        &self,
        state: State,
        controls: C,
    ) -> Result<bool, String> {
        let controls = controls.into();
        let script = self.script.clone();
        let memory = self.memory.clone();
        let freeze = self.freeze.clone();
        let stop_requested = Rc::new(Cell::new(false));
        let stop_request = stop_requested.clone();

        let result = self.context
            .async_with(async move |ctx| {
                let result: rquickjs::Result<()> = async {
                    let script: Function = script.restore(&ctx)?;
                    let memory: Object = memory.restore(&ctx)?;
                    let freeze: Function = freeze.restore(&ctx)?;

                    let state_object = Object::new(ctx.clone())?;
                    match state.score {
                        Some(score) => state_object.set("score", score)?,
                        None => state_object.set("score", Value::new_null(ctx.clone()))?,
                    }
                    state_object.set("sequence", state.sequence)?;
                    match state.received_at_ms {
                        Some(received_at_ms) => {
                            state_object.set("receivedAtMs", received_at_ms)?
                        }
                        None => state_object
                            .set("receivedAtMs", Value::new_null(ctx.clone()))?,
                    }

                    let rev = Object::new(ctx.clone())?;
                    rev.set("state", state_object.clone())?;
                    rev.set(
                        "stop",
                        Function::new(ctx.clone(), move || stop_request.set(true))?,
                    )?;

                    let click_controls = controls.clone();
                    rev.set(
                        "click",
                        Function::new(
                            ctx.clone(),
                            move |x: f64, y: f64, button: Opt<Value>| {
                                let x = validate_coordinate(x)?;
                                let y = validate_coordinate(y)?;
                                let button = parse_button(button)?;
                                click_with_controls(&click_controls, x, y, button)
                            },
                        )?,
                    )?;

                    let clickn_controls = controls.clone();
                    rev.set(
                        "clickn",
                        Function::new(
                            ctx.clone(),
                            Async(move |x: f64, y: f64, n: f64, button: Opt<Value>| {
                                let arguments = (|| -> Result<_, Error> {
                                    let x = validate_coordinate(x)?;
                                    let y = validate_coordinate(y)?;
                                    if !n.is_finite()
                                        || n.fract() != 0.0
                                        || n < 0.0
                                        || n >= u64::MAX as f64
                                    {
                                        return Err(Error::new_from_js_message(
                                            "number",
                                            "non-negative integer click count",
                                            "invalid click count",
                                        ));
                                    }
                                    let button = parse_button(button)?;
                                    Ok((x, y, n as u64, button))
                                })();
                                let controls = clickn_controls.clone();
                                async move {
                                    let (x, y, count, button) = arguments?;
                                    for index in 0..count {
                                        if index > 0 {
                                            tokio::time::sleep(Duration::from_millis(10)).await;
                                        }
                                        click_with_controls(&controls, x, y, button)?;
                                    }

                                    Ok::<(), Error>(())
                                }
                            }),
                        )?,
                    )?;

                    let scroll_window = controls.window.clone();
                    let scroll_paused = controls.actions_paused.clone();
                    rev.set(
                        "scroll",
                        Function::new(
                            ctx.clone(),
                            move |x: f64, y: f64, length: f64, axis: Opt<Value>| {
                                let x = validate_coordinate(x)?;
                                let y = validate_coordinate(y)?;
                                let length = validate_scroll_length(length)?;
                                let axis = parse_scroll_axis(axis)?;
                                ensure_actions_running(&scroll_paused)?;
                                scroll_window.scroll(x, y, length, axis).map_err(host_error)
                            },
                        )?,
                    )?;

                    let clipboard_window = controls.window.clone();
                    let clipboard_paused = controls.actions_paused.clone();
                    rev.set(
                        "write_clipboard",
                        Function::new(ctx.clone(), move |text: String| {
                            ensure_actions_running(&clipboard_paused)?;
                            clipboard_window.write_clipboard(&text).map_err(host_error)
                        })?,
                    )?;

                    let resize_window = controls.window.clone();
                    let resize_paused = controls.actions_paused.clone();
                    rev.set("resize", Function::new(ctx.clone(), move |width: f64, height: f64| {
                        ensure_actions_running(&resize_paused)?;
                        if !width.is_finite() || width.fract() != 0.0 || width <= 0.0 || width > i32::MAX as f64
                            || !height.is_finite() || height.fract() != 0.0 || height <= 0.0 || height > i32::MAX as f64 {
                            return Err(Error::new_from_js_message("number", "positive finite 32-bit integer dimensions", "invalid window dimensions"));
                        }
                        resize_window.resize_client(width as i32, height as i32).map_err(host_error)
                    })?)?;

                    rev.set(
                        "sleep",
                        Function::new(
                            ctx.clone(),
                            Async(|milliseconds: f64| async move {
                                if !milliseconds.is_finite()
                                    || milliseconds < 0.0
                                    || milliseconds.fract() != 0.0
                                    || milliseconds >= u64::MAX as f64
                                {
                                    return Err(Error::new_from_js_message(
                                        "number",
                                        "non-negative integer milliseconds",
                                        "invalid sleep duration",
                                    ));
                                }

                                tokio::time::sleep(Duration::from_millis(
                                    milliseconds as u64,
                                ))
                                .await;
                                Ok::<(), Error>(())
                            }),
                        )?,
                    )?;

                    let _: Object = freeze.call((state_object.clone(),))?;
                    let _: Object = freeze.call((rev.clone(),))?;

                    let result: MaybePromise = script.call((rev, memory))?;
                    let _: Value = result.into_future().await?;
                    Ok(())
                }
                .await;

                result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
            })
            .await;

        result.map(|()| stop_requested.get())
    }
}

async fn load_path(path: &Path) -> Result<ScriptSession, String> {
    let source = tokio::fs::read_to_string(path)
        .await
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    ScriptSession::new(&source)
        .await
        .map_err(|error| format!("failed to load {}: {error}", path.display()))
}

pub async fn run(
    commands: mpsc::Receiver<ScriptCommand>,
    states: watch::Receiver<State>,
    hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    actions_paused: ActionGate,
    shutdown: watch::Receiver<bool>,
    script_running: Arc<AtomicBool>,
    capture_state: CaptureState,
) -> Result<(), String> {
    let mouse: SharedMouse = Rc::new(RefCell::new(BridgeMouseInput));

    run_with_controls_and_lifecycle(
        commands,
        states,
        hotkey_pauses,
        initial_path,
        HostControls { mouse, window: Rc::new(Win32WindowControl), actions_paused },
        Duration::from_millis(50),
        shutdown,
        script_running,
        capture_state,
    )
    .await
}

#[cfg(test)]
async fn run_with_controls(
    commands: mpsc::Receiver<ScriptCommand>,
    states: watch::Receiver<State>,
    hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    controls: HostControls,
    loop_delay: Duration,
) -> Result<(), String> {
    let (_shutdown_tx, shutdown) = watch::channel(false);
    let script_running = Arc::new(AtomicBool::new(false));
    let capture_state = CaptureState::default();
    run_with_controls_and_lifecycle(
        commands,
        states,
        hotkey_pauses,
        initial_path,
        controls,
        loop_delay,
        shutdown,
        script_running,
        capture_state,
    )
    .await
}

async fn run_with_controls_and_lifecycle(
    mut commands: mpsc::Receiver<ScriptCommand>,
    mut states: watch::Receiver<State>,
    mut hotkey_pauses: watch::Receiver<PauseUpdate>,
    initial_path: Option<std::path::PathBuf>,
    controls: HostControls,
    loop_delay: Duration,
    mut shutdown: watch::Receiver<bool>,
    script_running: Arc<AtomicBool>,
    capture_state: CaptureState,
) -> Result<(), String> {
    let mut current_path = initial_path;
    let mut session = match current_path.as_deref() {
        Some(path) => match load_path(path).await {
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

    loop {
        if *shutdown.borrow() {
            script_running.store(false, Ordering::Release);
            return Ok(());
        }

        if hotkey_channel_open {
            match hotkey_pauses.has_changed() {
                Ok(true) => {
                    let update = *hotkey_pauses.borrow_and_update();
                    apply_hotkey_update_and_report(
                        update,
                        &controls.actions_paused,
                        session.is_some(),
                        &mut paused,
                    );
                    disable_capture_if_running(&capture_state, session.is_some(), paused);
                    continue;
                }
                Ok(false) => {}
                Err(_) => hotkey_channel_open = false,
            }
        }

        let command = if session.is_some() && !paused {
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
                                session.is_some(),
                                &mut paused,
                            );
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
                        match load_path(path).await {
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
                        match load_path(path).await {
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
                            session.is_some(),
                            &mut paused,
                        );
                        disable_capture_if_running(&capture_state, session.is_some(), paused);
                    }
                }
            }

            continue;
        }

        let snapshot = states.borrow_and_update().clone();

        let stop_requested = {
            let Some(active) = session.as_ref() else {
                continue;
            };
            let invocation = active.invoke(snapshot, controls.clone());
            tokio::pin!(invocation);
            tokio::select! {
                result = &mut invocation => {
                    match result {
                        Ok(stop_requested) => stop_requested,
                        Err(error) => {
                            eprintln!("script invocation failed: {error}");
                            false
                        }
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        script_running.store(false, Ordering::Release);
                        return Ok(());
                    }
                    false
                }
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
enum CaptureAction {
    Enable,
    Disable,
    Reject,
}

fn capture_action(enabled: bool, script_loaded: bool, paused: bool) -> CaptureAction {
    if enabled {
        CaptureAction::Disable
    } else if script_loaded && !paused {
        CaptureAction::Reject
    } else {
        CaptureAction::Enable
    }
}

fn disable_capture_if_running(
    capture_state: &CaptureState,
    script_loaded: bool,
    paused: bool,
) {
    if script_loaded && !paused {
        capture_state.set_enabled(false);
    }
}

fn apply_hotkey_update(
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

fn apply_hotkey_update_and_report(
    update: PauseUpdate,
    gate: &ActionGate,
    session_running: bool,
    lifecycle_paused: &mut bool,
) {
    let was_paused = *lifecycle_paused;
    if !apply_hotkey_update(
        update,
        gate,
        session_running,
        lifecycle_paused,
    ) {
        return;
    }

    if !session_running {
        println!("no script is running");
    } else if *lifecycle_paused == was_paused {
        if *lifecycle_paused {
            println!("script is already paused");
        } else {
            println!("script is already running");
        }
    } else if *lifecycle_paused {
        println!("script paused");
    } else {
        println!("script resumed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::ActionGate;
    use crate::udp::State;
    use std::{
        cell::RefCell,
        rc::Rc,
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
        Clipboard(String),
    }

    struct RecordingWindow { events: Rc<RefCell<Vec<HostEvent>>> }
    impl WindowControl for RecordingWindow {
        fn resize_client(&self, width: i32, height: i32) -> Result<(), String> {
            self.events.borrow_mut().push(HostEvent::Resize(width, height)); Ok(())
        }

        fn write_clipboard(&self, text: &str) -> Result<(), String> {
            self.events.borrow_mut().push(HostEvent::Clipboard(text.to_owned())); Ok(())
        }

        fn scroll(&self, x: i32, y: i32, length: i32, axis: Axis) -> Result<(), String> {
            self.events.borrow_mut().push(HostEvent::Scroll(x, y, length, axis)); Ok(())
        }
    }
    struct RecordingMouse { events: Rc<RefCell<Vec<HostEvent>>> }
    impl MouseInput for RecordingMouse {
        fn click_at(&mut self, x: i32, y: i32, button: Button) -> Result<(), String> {
            self.events.borrow_mut().push(HostEvent::Click(x, y, button)); Ok(())
        }

    }
    fn recording_controls() -> (HostControls, Rc<RefCell<Vec<HostEvent>>>) {
        let events = Rc::new(RefCell::new(Vec::new()));
        (HostControls { mouse: Rc::new(RefCell::new(RecordingMouse { events: events.clone() })), window: Rc::new(RecordingWindow { events: events.clone() }), actions_paused: ActionGate::default() }, events)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resize_and_click_sends_one_event_without_window_work() {
        let session = ScriptSession::new(r#"((rev) => { rev.resize(1280, 720); rev.click(10, 20, "right"); })"#).await.unwrap();
        let (controls, events) = recording_controls();
        session.invoke(State::default(), controls).await.unwrap();
        assert_eq!(*events.borrow(), vec![HostEvent::Resize(1280, 720), HostEvent::Click(10, 20, Button::Right)]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resize_rejects_invalid_dimensions_without_window_work() {
        for source in [r#"((rev) => rev.resize(0, 720))"#, r#"((rev) => rev.resize(-1, 720))"#, r#"((rev) => rev.resize(1.5, 720))"#, r#"((rev) => rev.resize(Infinity, 720))"#] {
            let (controls, events) = recording_controls();
            let session = ScriptSession::new(source).await.unwrap();
            assert!(session.invoke(State::default(), controls).await.is_err());
            assert!(events.borrow().is_empty());
        }
        let (controls, events) = recording_controls();
        let session = ScriptSession::new(r#"((rev) => rev.resize(1280, 720))"#).await.unwrap();
        session.invoke(State::default(), controls).await.unwrap();
        assert_eq!(*events.borrow(), vec![HostEvent::Resize(1280, 720)]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn script_errors_include_message_and_stack() {
        let session = ScriptSession::new(
            r#"((rev) => { function fail() { throw new Error("boom"); } fail(); })"#,
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
            r#"(async (rev) => { await rev.sleep(0); function fail() { throw new Error("async boom"); } fail(); })"#,
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
        fs::write(&path, r#"((rev) => rev.click(1, 1, "left"))"#).unwrap();

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
    async fn passes_fresh_state_and_preserves_memory_and_globals() {
        let source = r#"
            (async (rev, memory) => {
                if (!Object.isFrozen(rev) || !Object.isFrozen(rev.state)) {
                    throw new Error("rev and state must be frozen");
                }
                memory.count = (memory.count ?? 0) + 1;
                globalThis.count = (globalThis.count ?? 0) + 1;
                if (memory.count !== globalThis.count) {
                    throw new Error("persistent state mismatch");
                }
                if (rev.state.sequence !== memory.count) {
                    throw new Error("stale invocation state");
                }
                if (memory.count === 2) {
                    rev.click(-12, 34, "right");
                }
                const sequence = rev.state.sequence;
                await rev.sleep(1);
                if (rev.state.sequence !== sequence) {
                    throw new Error("state changed during invocation");
                }
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
                (() => {
                    Object.freeze = (value) => value;
                    return ((rev, memory) => {
                        if (!Object.isFrozen(rev) || !Object.isFrozen(rev.state)) {
                            throw new Error("rev and state must be frozen");
                        }
                        memory.calls = (memory.calls ?? 0) + 1;
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
                ((rev, memory) => {
                    if (!Object.isFrozen(rev) || !Object.isFrozen(rev.state)) {
                        throw new Error("rev and state must be frozen");
                    }
                    memory.calls = (memory.calls ?? 0) + 1;
                    if (memory.calls === 1) {
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
    async fn invocation_error_does_not_reset_memory() {
        let session = ScriptSession::new(
            r#"
                ((rev, memory) => {
                    memory.count = (memory.count ?? 0) + 1;
                    if (memory.count === 1) {
                        throw new Error("first call");
                    }
                    if (memory.count !== 2) {
                        throw new Error("memory was reset");
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
    async fn missing_state_fields_are_null() {
        let session = ScriptSession::new(
            r#"((rev) => {
                if (rev.state.score !== null || rev.state.receivedAtMs !== null) {
                    throw new Error("missing state fields must be null");
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
            r#"((rev) => {
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
    async fn sleep_is_awaited_and_non_callable_source_is_rejected() {
        let session = ScriptSession::new(
            "(async (rev, memory) => { await rev.sleep(20); })",
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
            r#"((rev) => rev.click(1, 2, "side"))"#,
            r#"((rev) => rev.click(1.5, 2, "left"))"#,
            r#"((rev) => rev.sleep(-1))"#,
            r#"((rev) => rev.sleep(1.5))"#,
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
                (async (rev) => {
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
                (async (rev) => {
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
            r#"((rev) => rev.clickn(1, 2, -1))"#,
            r#"((rev) => rev.clickn(1, 2, 1.5))"#,
            r#"((rev) => rev.clickn(1, 2, Infinity))"#,
            r#"((rev) => rev.scroll(1.5, 2, 1))"#,
            r#"((rev) => rev.scroll(1, Infinity, 1))"#,
            r#"((rev) => rev.scroll(1, 2, 1.5))"#,
            r#"((rev) => rev.scroll(1, 2, 1, "diagonal"))"#,
            r#"((rev) => rev.scroll(1, 2, 1, null))"#,
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
                ((rev) => {
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
        use crate::console::ScriptCommand;
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
                ((rev, memory) => {
                    memory.count = (memory.count ?? 0) + 1;
                    rev.click(memory.count, rev.state.sequence);
                })
            "#,
        )
        .unwrap();
        fs::write(
            &second,
            r#"
                ((rev, memory) => {
                    memory.count = (memory.count ?? 0) + 1;
                    rev.click(memory.count, 0, "right");
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
        let (state_tx, state_rx) = watch::channel(State {
            score: Some("1e1".to_owned()),
            sequence: 7,
            received_at_ms: Some(10),
        });

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
                    Some((1, 7, Button::Left))
                );

                state_tx
                    .send(State {
                        score: Some("2e2".to_owned()),
                        sequence: 8,
                        received_at_ms: Some(20),
                    })
                    .unwrap();
                loop {
                    if event_rx.recv().await.unwrap().1 == 8 {
                        break;
                    }
                }

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
                    if event_rx.recv().await == Some((1, 8, Button::Left)) {
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
    async fn requested_pause_gates_the_active_turn_then_pauses_the_loop() {
        use crate::console::ScriptCommand;
        use std::fs;
        use tokio::sync::{mpsc, watch};

        let path = std::env::temp_dir().join(format!(
            "rev-idle-requested-pause-test-{}.js",
            std::process::id(),
        ));
        fs::write(
            &path,
            r#"(async (rev) => { await rev.sleep(30); rev.click(1, 1); })"#,
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

                gate.set_paused(false);
                command_tx
                    .send(ScriptCommand::SetPaused(false))
                    .await
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap();

                runner.abort();
                let _ = runner.await;
            })
            .await;

        fs::remove_file(path).unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lifecycle_commands_keep_requested_pause_gate_in_sync() {
        use crate::console::ScriptCommand;
        use std::fs;
        use tokio::sync::{mpsc, watch};

        let root = std::env::temp_dir().join(format!(
            "rev-idle-pause-lifecycle-test-{}",
            std::process::id(),
        ));
        fs::create_dir_all(&root).unwrap();
        let first = root.join("first.js");
        let second = root.join("second.js");
        fs::write(&first, r#"((rev) => rev.click(1, 1))"#).unwrap();
        fs::write(&second, r#"((rev) => rev.click(2, 2, "right"))"#).unwrap();

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
        fs::write(&path, r#"((rev) => { rev.stop(); rev.click(1, 1); })"#).unwrap();

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
    async fn write_clipboard_forwards_text_to_host() {
        let session = ScriptSession::new(
            r#"((rev) => rev.write_clipboard("hello 世界"))"#,
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
    async fn waits_after_failed_and_successful_invocations() {
        use std::{fs, time::Instant};
        use tokio::sync::{mpsc, watch};

        let path = std::env::temp_dir().join(format!(
            "rev-idle-delay-test-{}.js",
            std::process::id(),
        ));
        fs::write(
            &path,
            r#"
                ((rev) => {
                    globalThis.calls = (globalThis.calls ?? 0) + 1;
                    rev.click(globalThis.calls, 0, "left");
                    if (globalThis.calls === 1) {
                        throw new Error("first call fails");
                    }
                })
            "#,
        )
        .unwrap();

        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        struct TimedMouse(mpsc::UnboundedSender<Instant>);
        impl MouseInput for TimedMouse {
            fn click_at(
                &mut self,
                _x: i32,
                _y: i32,
                _button: Button,
            ) -> Result<(), String> {
                self.0
                    .send(Instant::now())
                    .map_err(|error| error.to_string())
            }
        }

        let mouse: SharedMouse = Rc::new(RefCell::new(TimedMouse(event_tx)));
        let (_command_tx, command_rx) = mpsc::channel(32);
        let (_state_tx, state_rx) = watch::channel(State::default());
        let delay = Duration::from_millis(20);
        let local = tokio::task::LocalSet::new();

        let runner_path = path.clone();
        local
            .run_until(async move {
                let runner = tokio::task::spawn_local(run_with_mouse(
                    command_rx,
                    state_rx,
                    runner_path,
                    mouse,
                    delay,
                ));

                let first = tokio::time::timeout(
                    Duration::from_secs(1),
                    event_rx.recv(),
                )
                .await
                .unwrap()
                .unwrap();
                let second = tokio::time::timeout(
                    Duration::from_secs(1),
                    event_rx.recv(),
                )
                .await
                .unwrap()
                .unwrap();
                let third = tokio::time::timeout(
                    Duration::from_secs(1),
                    event_rx.recv(),
                )
                .await
                .unwrap()
                .unwrap();
                assert!(second.duration_since(first) >= delay);
                assert!(third.duration_since(second) >= delay);
                runner.abort();
                let _ = runner.await;
            })
            .await;

        fs::remove_file(path).unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn failed_load_stays_stopped_and_reload_retries_that_path() {
        use crate::console::ScriptCommand;
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
                    r#"((rev) => rev.click(1, 1, "left"))"#,
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
        assert_eq!(capture_action(false, false, false), CaptureAction::Enable);
        assert_eq!(capture_action(false, true, true), CaptureAction::Enable);
        assert_eq!(capture_action(false, true, false), CaptureAction::Reject);
        assert_eq!(capture_action(true, true, false), CaptureAction::Disable);

        let capture_state = CaptureState::default();
        capture_state.set_enabled(true);
        disable_capture_if_running(&capture_state, true, false);
        assert!(!capture_state.is_enabled());
    }
}
