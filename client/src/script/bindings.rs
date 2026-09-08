use crate::{
    app::ActionGate,
    global_state::GlobalState,
    window::{post_click_to_game, Axis, WindowControl},
};
use rquickjs::{
    function::{Async, Opt, Rest},
    Ctx,
    Error,
    Function,
    Object,
    Value,
};
use std::{
    cell::{Cell, RefCell},
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

pub(super) struct BridgeMouseInput;

pub(super) trait MouseInput {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), String>;
}

pub(super) fn click_at_with<C>(
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
    // A paused action is silently skipped rather than rejected: rejecting
    // would surface as a thrown/rejected JS exception on the script's next
    // host call, but pausing (F8) is a routine, frequent user action, not a
    // script error worth an exception a script would need to catch.
    if controls.actions_paused.is_paused() {
        return Ok(());
    }
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


pub(super) fn create_rev<'js>(
    ctx: Ctx<'js>,
    client: reqwest::Client,
    controls: HostControls,
    parse: Function<'js>,
    freeze: Function<'js>,
    stop_request: Rc<Cell<bool>>,
) -> rquickjs::Result<Object<'js>> {
    let rev = Object::new(ctx.clone())?;
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
    rev.set(
        "write_file",
        Function::new(ctx.clone(), |path: String, content: String| {
            std::fs::write(path, content).map_err(|error| host_error(error.to_string()))
        })?,
    )?;
    let state_client = client.clone();
    let state_raw = Function::new(
        ctx.clone(),
        Async(move |keys: Rest<String>| {
            let client = state_client.clone();
            async move {
                crate::bridge::request_state(&client, &keys.0)
                    .await
                    .map_err(|error| Error::new_from_js_message("state", "HTTP response", error))
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
    let invoke_client = client.clone();
    let invoke_controls = controls.clone();
    rev.set(
        "invoke",
        Function::new(ctx.clone(), Async(move |path: String| {
            let client = invoke_client.clone();
            let controls = invoke_controls.clone();
            async move {
                if path.trim().is_empty() {
                    return Err(host_error("button path must not be empty".to_owned()));
                }
                if controls.actions_paused.is_paused() {
                    return Ok(());
                }
                crate::bridge::invoke(&client, path).await.map_err(host_error)
            }
        }))?,
    )?;
    let transfer_client = client.clone();
    let transfer_controls = controls.clone();
    rev.set(
        "transfer",
        Function::new(ctx.clone(), Async(move |source: String, destination: String| {
            let client = transfer_client.clone();
            let controls = transfer_controls.clone();
            async move {
                if source.trim().is_empty() || destination.trim().is_empty() {
                    return Err(host_error("slot paths must not be empty".to_owned()));
                }
                if controls.actions_paused.is_paused() {
                    return Ok(());
                }
                crate::bridge::transfer(&client, source, destination)
                    .await
                    .map_err(host_error)
            }
        }))?,
    )?;
    rev.set(
        "stop",
        Function::new(ctx.clone(), move || stop_request.set(true))?,
    )?;

    let global_get_raw = Function::new(ctx.clone(), |key: String| {
        GlobalState.get(&key).map(|value| value.to_string())
    })?;
    let global_set_raw = Function::new(ctx.clone(), |key: String, json: String| {
        let value: serde_json::Value = serde_json::from_str(&json).map_err(|error| {
            Error::new_from_js_message("string", "JSON value", error.to_string())
        })?;
        GlobalState.set(key, value);
        Ok::<(), Error>(())
    })?;
    let global_delete_raw = Function::new(ctx.clone(), |key: String| GlobalState.delete(&key))?;
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
                if scroll_paused.is_paused() {
                    return Ok(());
                }
                scroll_window.scroll(x, y, length, axis).map_err(host_error)
            },
        )?,
    )?;

    let drag_window = controls.window.clone();
    let drag_paused = controls.actions_paused.clone();
    rev.set(
        "drag",
        Function::new(
            ctx.clone(),
            move |x1: f64, y1: f64, x2: f64, y2: f64| {
                let x1 = validate_coordinate(x1)?;
                let y1 = validate_coordinate(y1)?;
                let x2 = validate_coordinate(x2)?;
                let y2 = validate_coordinate(y2)?;
                if drag_paused.is_paused() {
                    return Ok(());
                }
                drag_window.drag(x1, y1, x2, y2).map_err(host_error)
            },
        )?,
    )?;

    let clipboard_window = controls.window.clone();
    let clipboard_paused = controls.actions_paused.clone();
    rev.set(
        "write_clipboard",
        Function::new(ctx.clone(), move |text: String| {
            if clipboard_paused.is_paused() {
                return Ok(());
            }
            clipboard_window.write_clipboard(&text).map_err(host_error)
        })?,
    )?;

    let read_clipboard_window = controls.window.clone();
    let read_clipboard_paused = controls.actions_paused.clone();
    rev.set(
        "read_clipboard",
        Function::new(ctx.clone(), move || {
            if read_clipboard_paused.is_paused() {
                return Ok(std::string::String::new());
            }
            read_clipboard_window.read_clipboard().map_err(host_error)
        })?,
    )?;

    let resize_window = controls.window.clone();
    let resize_paused = controls.actions_paused.clone();
    rev.set("resize", Function::new(ctx.clone(), move |width: f64, height: f64| {
        if !width.is_finite() || width.fract() != 0.0 || width <= 0.0 || width > i32::MAX as f64
            || !height.is_finite() || height.fract() != 0.0 || height <= 0.0 || height > i32::MAX as f64 {
            return Err(Error::new_from_js_message("number", "positive finite 32-bit integer dimensions", "invalid window dimensions"));
        }
        if resize_paused.is_paused() {
            return Ok(());
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

    let _: Object = freeze.call((rev.clone(),))?;

    Ok(rev)
}
