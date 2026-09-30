use crate::{
    app::ActionGate,
    bridge::{WsConnection, WsError},
    global_state::GlobalState,
    script::{
        background::{BackgroundRegistry, BackgroundTarget},
        control::SessionControl,
        transfer::FunctionTransfer,
        ui::ScriptUiBindings,
    },
    window::{Axis, WindowControl},
};
use rquickjs::{
    function::{Async, Opt, Rest},
    object::Accessor,
    Ctx,
    Error,
    Exception,
    Function,
    Object,
    Value,
};
use std::{
    cell::RefCell,
    io,
    rc::Rc,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Button {
    Left,
    Right,
    Middle,
}

pub(super) struct BridgeMouseInput {
    pub(super) connection: WsConnection,
}

#[derive(Debug)]
pub(super) enum MouseInputError {
    Host(String),
    Bridge(WsError),
}

impl From<String> for MouseInputError {
    fn from(message: String) -> Self {
        Self::Host(message)
    }
}

pub(super) trait MouseInput {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), MouseInputError>;

    fn scroll(&mut self, _x: i32, _y: i32, _length: i32, _axis: Axis) -> Result<(), MouseInputError> {
        Err("mouse scrolling is not supported".to_string().into())
    }

    fn drag(&mut self, _x1: i32, _y1: i32, _x2: i32, _y2: i32) -> Result<(), MouseInputError> {
        Err("mouse dragging is not supported".to_string().into())
    }

    fn press(&mut self, _key: String) -> Result<(), MouseInputError> {
        Err("keyboard input is not supported".to_string().into())
    }
}

pub(super) fn click_at_with<C>(
    x: i32,
    y: i32,
    click: C,
) -> Result<(), MouseInputError>
where
    C: FnOnce(i32, i32) -> Result<(), WsError>,
{
    click(x, y).map_err(MouseInputError::Bridge)
}

impl MouseInput for BridgeMouseInput {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        _button: Button,
    ) -> Result<(), MouseInputError> {
        let (width, height) = crate::window::client_size().map_err(MouseInputError::Host)?;
        click_at_with(x, y, |x, y| crate::bridge::click(&self.connection, x, y, width, height))
    }

    fn scroll(&mut self, x: i32, y: i32, length: i32, axis: Axis) -> Result<(), MouseInputError> {
        let (width, height) = crate::window::client_size().map_err(MouseInputError::Host)?;
        crate::bridge::scroll(&self.connection, x, y, length, axis, width, height)
            .map_err(MouseInputError::Bridge)
    }

    fn drag(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<(), MouseInputError> {
        let (width, height) = crate::window::client_size().map_err(MouseInputError::Host)?;
        crate::bridge::drag(&self.connection, x1, y1, x2, y2, width, height)
            .map_err(MouseInputError::Bridge)
    }

    fn press(&mut self, key: String) -> Result<(), MouseInputError> {
        crate::bridge::press(&self.connection, key).map_err(MouseInputError::Bridge)
    }
}

pub(super) type SharedMouse = Rc<RefCell<dyn MouseInput>>;
pub(super) type SharedWindow = Rc<dyn WindowControl>;

#[derive(Clone)]
pub(super) struct HostControls {
    pub(super) mouse: SharedMouse,
    pub(super) window: SharedWindow,
    pub(super) actions_paused: ActionGate,
}

fn host_error(message: String) -> Error {
    Error::new_from_js_message("host control", "JavaScript", message)
}

pub(super) fn bridge_error(ctx: &Ctx<'_>, message: String) -> Error {
    let disconnected = message == "NotConnected" || message == "Closed";
    match Exception::from_message(ctx.clone(), &message) {
        Ok(exception) => {
            if disconnected {
                let _ = exception
                    .as_object()
                    .set("__revIdleDisconnectedTransport", true);
            }
            exception.throw()
        }
        Err(error) => error,
    }
}

fn mouse_error(ctx: &Ctx<'_>, error: MouseInputError) -> Error {
    match error {
        MouseInputError::Host(message) => host_error(message),
        MouseInputError::Bridge(error) => {
            let disconnected = matches!(error, WsError::NotConnected | WsError::Closed);
            let message = error.to_string();
            match Exception::from_message(ctx.clone(), &message) {
                Ok(exception) => {
                    if disconnected {
                        let _ = exception
                            .as_object()
                            .set("__revIdleDisconnectedTransport", true);
                    }
                    exception.throw()
                }
                Err(error) => error,
            }
        }
    }
}

fn stopped_error(ctx: &Ctx<'_>) -> Error {
    Exception::throw_message(ctx, SessionControl::error_message())
}

pub(super) fn reject_if_stopped<'js>(ctx: &Ctx<'js>, session: &SessionControl) -> rquickjs::Result<()> {
    if session.is_stopped() {
        Err(stopped_error(ctx))
    } else {
        Ok(())
    }
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
    ctx: &Ctx<'_>,
    controls: &HostControls,
    x: i32,
    y: i32,
    button: Button,
) -> Result<(), Error> {
    controls
        .mouse
        .borrow_mut()
        .click_at(x, y, button)
        .map_err(|error| mouse_error(ctx, error))
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


pub(super) fn create_rev<'js>(
    ctx: Ctx<'js>,
    connection: WsConnection,
    controls: HostControls,
    parse: Function<'js>,
    freeze: Function<'js>,
    session: SessionControl,
    screen_ownership: Rc<super::ownership::ScreenOwnershipState>,
    ui: Rc<ScriptUiBindings>,
    transfer: Rc<FunctionTransfer>,
    background: Rc<BackgroundRegistry>,
) -> rquickjs::Result<Object<'js>> {
    let rev = Object::new(ctx.clone())?;
    let paused_session = session.clone();
    rev.prop(
        "paused",
        Accessor::from(move || paused_session.is_paused()).enumerable(),
    )?;
    let pause_session = session.clone();
    let pause_gate = controls.actions_paused.clone();
    rev.set(
        "pause",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>| {
            reject_if_stopped(&ctx, &pause_session)?;
            if !pause_gate.is_paused() {
                pause_gate.set_paused(true);
            }
            Ok::<(), Error>(())
        })?,
    )?;
    let resume_session = session.clone();
    let resume_gate = controls.actions_paused.clone();
    rev.set(
        "resume",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>| {
            reject_if_stopped(&ctx, &resume_session)?;
            if resume_gate.is_paused() {
                resume_gate.set_paused(false);
            }
            Ok::<(), Error>(())
        })?,
    )?;
    let ensure_session = session.clone();
    rev.set(
        "ensureRunning",
        Function::new(ctx.clone(), Async(move |ctx: Ctx<'js>| {
            let session = ensure_session.clone();
            async move {
                let mut pauses = session.subscribe_paused();
                let mut stopped = session.subscribe_stopped();
                loop {
                    if session.is_stopped() {
                        return Err(stopped_error(&ctx));
                    }
                    if !session.is_paused() {
                        return Ok::<(), Error>(());
                    }
                    tokio::select! {
                        changed = pauses.changed() => {
                            if changed.is_err() { return Err(stopped_error(&ctx)); }
                        }
                        changed = stopped.changed() => {
                            if changed.is_err() || *stopped.borrow() { return Err(stopped_error(&ctx)); }
                        }
                    }
                }
            }
        }))?,
    )?;
    rev.set("ui", ui.registry(&ctx)?)?;
    let daemon_transfer = Rc::downgrade(&transfer);
    let daemon_background = background.clone();
    let daemon_session = session.clone();
    let daemon_set = Function::new(ctx.clone(), move |ctx: Ctx<'js>, name: String, function: Function<'js>| {
        reject_if_stopped(&ctx, &daemon_session)?;
        if name.is_empty() {
            return Err(Error::new_from_js_message("string", "nonempty daemon name", "daemon names must be nonempty strings"));
        }
        let transfer = daemon_transfer.upgrade().ok_or_else(|| stopped_error(&ctx))?;
        let descriptor = transfer.capture(&ctx, function)?;
        daemon_background.register(BackgroundTarget::Daemon(name), descriptor);
        Ok::<(), Error>(())
    })?;
    let daemon_background = background.clone();
    let daemon_delete = Function::new(ctx.clone(), move |name: String| {
        daemon_background.unregister(&BackgroundTarget::Daemon(name));
        true
    })?;
    let daemon_background = background.clone();
    let daemon_has = Function::new(ctx.clone(), move |name: String| daemon_background.contains_daemon(&name))?;
    let daemon_names = Function::new(ctx.clone(), move || background.daemon_names())?;
    let daemon_factory: Function = ctx.eval(r#"
        (set, del, has, names) => new Proxy(Object.create(null), {
            get: (_target, key) => {
                if (typeof key !== 'string') return undefined;
                throw new Error(`daemon ${JSON.stringify(key)} cannot be read`);
            },
            set: (_target, key, value) => {
                if (typeof key !== 'string') throw new TypeError('daemon names must be strings');
                if (typeof value !== 'function') throw new TypeError('daemon assignments require a function');
                set(key, value);
                return true;
            },
            has: (_target, key) => typeof key === 'string' && has(key),
            deleteProperty: (_target, key) => typeof key === 'string' && del(key),
            ownKeys: () => names(),
            getOwnPropertyDescriptor: (_target, key) => {
                if (typeof key !== 'string' || !has(key)) return undefined;
                return { enumerable: true, configurable: true };
            },
        })
    "#)?;
    let daemon: Object = daemon_factory.call((daemon_set, daemon_delete, daemon_has, daemon_names))?;
    rev.set("daemon", daemon)?;
    let screen_ownership_session = session.clone();
    rev.set(
        "screenOwnership",
        Function::new(ctx.clone(), Async(move |ctx: Ctx<'js>, label: Opt<Option<String>>| {
            screen_ownership.clone().acquire(ctx, screen_ownership_session.clone(), label.0.flatten())
        }))?,
    )?;
    rev.set(
        "read_file",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, path: String| {
            match std::fs::read_to_string(&path) {
                Ok(content) => rquickjs::String::from_str(ctx, &content).map(|value| value.into_value()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Value::new_null(ctx)),
                Err(error) => Err(host_error(error.to_string())),
            }
        })?,
    )?;
    let write_file_session = session.clone();
    rev.set(
        "write_file",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, path: String, content: String| {
            reject_if_stopped(&ctx, &write_file_session)?;
            std::fs::write(path, content).map_err(|error| host_error(error.to_string()))
        })?,
    )?;
    let delete_file_session = session.clone();
    rev.set(
        "delete_file",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, path: String| {
            reject_if_stopped(&ctx, &delete_file_session)?;
            match std::fs::remove_file(path) {
                Ok(()) => Ok(true),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(host_error(error.to_string())),
            }
        })?,
    )?;
    let shell_session = session.clone();
    rev.set(
        "shell",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, command: String| {
            reject_if_stopped(&ctx, &shell_session)?;
            let output = std::process::Command::new("cmd.exe")
                .args(["/D", "/S", "/C", &command])
                .output()
                .map_err(|error| host_error(error.to_string()))?;
            let result = Object::new(ctx)?;
            result.set("stdout", String::from_utf8_lossy(&output.stdout).into_owned())?;
            result.set("stderr", String::from_utf8_lossy(&output.stderr).into_owned())?;
            Ok::<Object<'js>, Error>(result)
        })?,
    )?;
    let state_connection = connection.clone();
    let state_session = session.clone();
    let state_raw = Function::new(
        ctx.clone(),
        Async(move |ctx: Ctx<'js>, keys: Rest<String>| {
            let connection = state_connection.clone();
            let session = state_session.clone();
            async move {
                reject_if_stopped(&ctx, &session)?;
                crate::bridge::request_state(&connection, &keys.0)
                    .await
                    .map_err(|error| bridge_error(&ctx, error))
                    .and_then(|value| {
                        reject_if_stopped(&ctx, &session)?;
                        Ok(value)
                    })
            }
        }),
    )?;
    let state_wrapper: Function = ctx.eval(
        "(raw, parse, freeze) => async (...keys) => {
            const result = freeze(parse(await raw(...keys)));
            return keys.length === 1 ? result[keys[0]] : result;
        }",
    )?;
    let state: Function = state_wrapper.call((state_raw, parse.clone(), freeze.clone()))?;
    rev.set("state", state)?;
    let slot_connection = connection.clone();
    let slot_session = session.clone();
    rev.set(
        "slot",
        Function::new(ctx.clone(), Async(move |ctx: Ctx<'js>, path: String| {
            let connection = slot_connection.clone();
            let session = slot_session.clone();
            async move {
                if path.trim().is_empty() {
                    return Err(host_error("slot path must not be empty".to_owned()));
                }
                reject_if_stopped(&ctx, &session)?;
                let response = connection.request(crate::bridge::SlotReq { path })
                    .await
                    .map_err(|error| bridge_error(&ctx, error.to_string()))?;
                reject_if_stopped(&ctx, &session)?;
                ctx.json_parse(response.value.to_string())
            }
        }))?,
    )?;
    let invoke_connection = connection.clone();
    let invoke_session = session.clone();
    rev.set(
        "invoke",
        Function::new(ctx.clone(), Async(move |ctx: Ctx<'js>, path: String| {
            let connection = invoke_connection.clone();
            let session = invoke_session.clone();
            async move {
                if path.trim().is_empty() {
                    return Err(host_error("UI path must not be empty".to_owned()));
                }
                reject_if_stopped(&ctx, &session)?;
                crate::bridge::invoke(&connection, path)
                    .await
                    .map_err(|error| bridge_error(&ctx, error))?;
                reject_if_stopped(&ctx, &session)
            }
        }))?,
    )?;
    let input_connection = connection.clone();
    let input_session = session.clone();
    rev.set(
        "input",
        Function::new(ctx.clone(), Async(move |ctx: Ctx<'js>, path: String, text: String| {
            let connection = input_connection.clone();
            let session = input_session.clone();
            async move {
                if path.trim().is_empty() {
                    return Err(host_error("UI path must not be empty".to_owned()));
                }
                reject_if_stopped(&ctx, &session)?;
                connection.request(crate::bridge::InputReq { path, text })
                    .await
                    .map_err(|error| bridge_error(&ctx, error.to_string()))?;
                reject_if_stopped(&ctx, &session)
            }
        }))?,
    )?;
    let scroll_into_view_connection = connection.clone();
    let scroll_into_view_session = session.clone();
    rev.set(
        "scrollIntoView",
        Function::new(ctx.clone(), Async(move |ctx: Ctx<'js>, path: String| {
            let connection = scroll_into_view_connection.clone();
            let session = scroll_into_view_session.clone();
            async move {
                if path.trim().is_empty() {
                    return Err(host_error("UI path must not be empty".to_owned()));
                }
                reject_if_stopped(&ctx, &session)?;
                crate::bridge::scroll_into_view(&connection, path)
                    .await
                    .map_err(|error| bridge_error(&ctx, error))?;
                reject_if_stopped(&ctx, &session)
            }
        }))?,
    )?;
    let transfer_connection = connection.clone();
    let transfer_session = session.clone();
    rev.set(
        "transfer",
        Function::new(ctx.clone(), Async(move |ctx: Ctx<'js>, source: String, destination: String| {
            let connection = transfer_connection.clone();
            let session = transfer_session.clone();
            async move {
                if source.trim().is_empty() || destination.trim().is_empty() {
                    return Err(host_error("slot paths must not be empty".to_owned()));
                }
                reject_if_stopped(&ctx, &session)?;
                crate::bridge::transfer(&connection, source, destination)
                    .await
                    .map_err(|error| bridge_error(&ctx, error))?;
                reject_if_stopped(&ctx, &session)
            }
        }))?,
    )?;
    let stop_session = session.clone();
    rev.set(
        "stop",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>| {
            stop_session.stop();
            Err::<(), Error>(stopped_error(&ctx))
        })?,
    )?;

    let global_get_raw = Function::new(ctx.clone(), |key: String| {
        GlobalState.get(&key).map(|value| value.to_string())
    })?;
    let global_set_session = session.clone();
    let global_set_raw = Function::new(ctx.clone(), move |ctx: Ctx<'js>, key: String, json: String| {
        reject_if_stopped(&ctx, &global_set_session)?;
        let value: serde_json::Value = serde_json::from_str(&json).map_err(|error| {
            Error::new_from_js_message("string", "JSON value", error.to_string())
        })?;
        GlobalState.set(key, value);
        Ok::<(), Error>(())
    })?;
    let global_delete_session = session.clone();
    let global_delete_raw = Function::new(ctx.clone(), move |ctx: Ctx<'js>, key: String| {
        reject_if_stopped(&ctx, &global_delete_session)?;
        Ok::<bool, Error>(GlobalState.delete(&key))
    })?;
    let global_keys_raw = Function::new(ctx.clone(), || GlobalState.keys())?;
    let global_wrapper: Function = ctx.eval(
        "(get, set, del, keys, parse) => new Proxy({}, {
            get: (_target, key) => {
                if (typeof key !== 'string') return undefined;
                const raw = get(key);
                return raw === undefined ? undefined : parse(raw);
            },
            set: (_target, key, value) => {
                if (typeof key !== 'string') return false;
                set(key, JSON.stringify(value === undefined ? null : value));
                return true;
            },
            has: (_target, key) => typeof key === 'string' && keys().includes(key),
            deleteProperty: (_target, key) => typeof key === 'string' && del(key),
            ownKeys: () => keys(),
            getOwnPropertyDescriptor: (_target, key) => {
                if (typeof key !== 'string' || !keys().includes(key)) return undefined;
                return { enumerable: true, configurable: true };
            },
        })",
    )?;
    let global: Value = global_wrapper.call((
        global_get_raw,
        global_set_raw,
        global_delete_raw,
        global_keys_raw,
        parse,
    ))?;
    rev.set("global", global)?;

    let click_controls = controls.clone();
    let click_session = session.clone();
    rev.set(
        "click",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, x: f64, y: f64, button: Opt<Value>| {
                reject_if_stopped(&ctx, &click_session)?;
                let x = validate_coordinate(x)?;
                let y = validate_coordinate(y)?;
                let button = parse_button(button)?;
                click_with_controls(&ctx, &click_controls, x, y, button)
            },
        )?,
    )?;

    let clickn_controls = controls.clone();
    let clickn_session = session.clone();
    rev.set(
        "clickn",
        Function::new(
            ctx.clone(),
            Async(move |ctx: Ctx<'js>, x: f64, y: f64, n: f64, button: Opt<Value>| {
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
                let session = clickn_session.clone();
                async move {
                    reject_if_stopped(&ctx, &session)?;
                    let (x, y, count, button) = arguments?;
                    for index in 0..count {
                        if index > 0 {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            reject_if_stopped(&ctx, &session)?;
                        }
                        click_with_controls(&ctx, &controls, x, y, button)?;
                        reject_if_stopped(&ctx, &session)?;
                    }

                    Ok::<(), Error>(())
                }
            }),
        )?,
    )?;

    let scroll_mouse = controls.mouse.clone();
    let scroll_session = session.clone();
    rev.set(
        "scroll",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, x: f64, y: f64, length: f64, axis: Opt<Value>| {
                reject_if_stopped(&ctx, &scroll_session)?;
                let x = validate_coordinate(x)?;
                let y = validate_coordinate(y)?;
                let length = validate_scroll_length(length)?;
                let axis = parse_scroll_axis(axis)?;
                scroll_mouse.borrow_mut().scroll(x, y, length, axis).map_err(|error| mouse_error(&ctx, error))
            },
        )?,
    )?;

    let drag_mouse = controls.mouse.clone();
    let drag_session = session.clone();
    rev.set(
        "drag",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, x1: f64, y1: f64, x2: f64, y2: f64| {
                reject_if_stopped(&ctx, &drag_session)?;
                let x1 = validate_coordinate(x1)?;
                let y1 = validate_coordinate(y1)?;
                let x2 = validate_coordinate(x2)?;
                let y2 = validate_coordinate(y2)?;
                drag_mouse.borrow_mut().drag(x1, y1, x2, y2).map_err(|error| mouse_error(&ctx, error))
            },
        )?,
    )?;

    let press_mouse = controls.mouse.clone();
    let press_session = session.clone();
    rev.set(
        "press",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, key: String| {
            reject_if_stopped(&ctx, &press_session)?;
            let key = key.to_ascii_lowercase();
            if !(key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric()
                || matches!(
                    key.as_str(),
                    "left" | "right" | "up" | "down" | "enter" | "escape" | "space" | "tab"
                        | "backspace" | "f1" | "f2" | "f3" | "f4" | "f5" | "f6" | "f7" | "f8"
                        | "f9" | "f10" | "f11" | "f12"
                ))
            {
                return Err(Error::new_from_js_message(
                    "string",
                    "supported keyboard key",
                    format!("unsupported key: {key}"),
                ));
            }
            press_mouse.borrow_mut().press(key).map_err(|error| mouse_error(&ctx, error))
        })?,
    )?;

    let clipboard_window = controls.window.clone();
    let clipboard_session = session.clone();
    rev.set(
        "write_clipboard",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, text: String| {
            reject_if_stopped(&ctx, &clipboard_session)?;
            clipboard_window.write_clipboard(&text).map_err(host_error)
        })?,
    )?;

    let read_clipboard_window = controls.window.clone();
    let read_clipboard_session = session.clone();
    rev.set(
        "read_clipboard",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>| {
            reject_if_stopped(&ctx, &read_clipboard_session)?;
            read_clipboard_window.read_clipboard().map_err(host_error)
        })?,
    )?;

    let resize_window = controls.window.clone();
    let resize_session = session.clone();
    rev.set("resize", Function::new(ctx.clone(), move |ctx: Ctx<'js>, width: f64, height: f64| {
        reject_if_stopped(&ctx, &resize_session)?;
        if !width.is_finite() || width.fract() != 0.0 || width <= 0.0 || width > i32::MAX as f64
            || !height.is_finite() || height.fract() != 0.0 || height <= 0.0 || height > i32::MAX as f64 {
            return Err(Error::new_from_js_message("number", "positive finite 32-bit integer dimensions", "invalid window dimensions"));
        }
        resize_window.resize_client(width as i32, height as i32).map_err(host_error)
    })?)?;

    let sleep_session = session.clone();
    rev.set(
        "sleep",
        Function::new(
            ctx.clone(),
            Async(move |ctx: Ctx<'js>, milliseconds: f64| {
                let session = sleep_session.clone();
                let deadline = if !milliseconds.is_finite()
                    || milliseconds < 0.0
                    || milliseconds.fract() != 0.0
                    || milliseconds >= u64::MAX as f64
                {
                    Err(Error::new_from_js_message(
                        "number",
                        "non-negative integer milliseconds",
                        "invalid sleep duration",
                    ))
                } else {
                    tokio::time::Instant::now()
                        .checked_add(Duration::from_millis(milliseconds as u64))
                        .ok_or_else(|| Error::new_from_js_message(
                            "number",
                            "non-negative integer milliseconds",
                            "sleep duration is too large",
                        ))
                };
                async move {
                let deadline = deadline?;
                reject_if_stopped(&ctx, &session)?;
                let mut stopped = session.subscribe_stopped();
                tokio::select! {
                    _ = tokio::time::sleep_until(deadline) => Ok::<(), Error>(()),
                    changed = stopped.changed() => {
                        let _ = changed;
                        Err(stopped_error(&ctx))
                    }
                }
                }
            }),
        )?,
    )?;

    let _: Object = freeze.call((rev.clone(),))?;

    Ok(rev)
}
