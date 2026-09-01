use crate::{console::ScriptCommand, udp::State, window::{Win32WindowControl, WindowControl}};
use enigo::{Button, Coordinate, Direction, Enigo, Mouse};
use rquickjs::{
    function::Async,
    function::Opt,
    promise::MaybePromise,
    AsyncContext,
    AsyncRuntime,
    Error,
    Function,
    Object,
    Persistent,
    Value,
};
use std::{cell::RefCell, path::Path, rc::Rc, sync::{atomic::{AtomicBool, Ordering}, Arc}, time::Duration};
use tokio::sync::{mpsc, watch};

trait MouseInput {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), String>;
}

impl MouseInput for Enigo {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), String> {
        self.move_mouse(x, y, Coordinate::Abs)
            .map_err(|error| error.to_string())?;
        self.button(button, Direction::Click)
            .map_err(|error| error.to_string())
    }
}

type SharedMouse = Rc<RefCell<dyn MouseInput>>;
type SharedWindow = Rc<dyn WindowControl>;

#[derive(Clone)]
struct HostControls {
    mouse: SharedMouse,
    window: SharedWindow,
    actions_paused: Arc<AtomicBool>,
}

fn host_error(message: String) -> Error {
    Error::new_from_js_message("host control", "JavaScript", message)
}

fn ensure_actions_running(paused: &AtomicBool) -> Result<(), Error> {
    if paused.load(Ordering::Acquire) {
        Err(Error::new_from_js_message("actions", "JavaScript", "actions are paused"))
    } else { Ok(()) }
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
    ) -> rquickjs::Result<()> {
        let controls = controls.into();
        let script = self.script.clone();
        let memory = self.memory.clone();
        let freeze = self.freeze.clone();

        self.context
            .async_with(async move |ctx| {
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

                let click_controls = controls.clone();
                rev.set(
                    "click",
                    Function::new(
                        ctx.clone(),
                        move |x: f64, y: f64, button: Opt<Value>| {
                            if !x.is_finite()
                                || x.fract() != 0.0
                                || x < i32::MIN as f64
                                || x > i32::MAX as f64
                                || !y.is_finite()
                                || y.fract() != 0.0
                                || y < i32::MIN as f64
                                || y > i32::MAX as f64
                            {
                                return Err(Error::new_from_js_message(
                                    "number",
                                    "finite 32-bit integer coordinates",
                                    "invalid mouse coordinates",
                                ));
                            }

                            let button = match button.0 {
                                None => Button::Left,
                                Some(value) if value.is_undefined() => Button::Left,
                                Some(value) => {
                                    let type_name = value.type_name();
                                    let Some(value) = value.as_string() else {
                                        return Err(Error::new_from_js(type_name, "mouse button"));
                                    };
                                    let value = value.to_string()?;
                                    match value.as_str() {
                                        "left" => Button::Left,
                                        "right" => Button::Right,
                                        "middle" => Button::Middle,
                                        value => {
                                            return Err(Error::new_from_js_message(
                                                "string",
                                                "mouse button",
                                                format!("unsupported button: {value}"),
                                            ));
                                        }
                                    }
                                }
                            };

                            ensure_actions_running(&click_controls.actions_paused)?;
                            let (screen_x, screen_y) = click_controls.window
                                .focus_and_translate(x as i32, y as i32)
                                .map_err(host_error)?;
                            ensure_actions_running(&click_controls.actions_paused)?;
                            click_controls.mouse
                                .borrow_mut()
                                .click_at(screen_x, screen_y, button)
                                .map_err(|message| {
                                    Error::new_from_js_message(
                                        "mouse input",
                                        "JavaScript",
                                        message,
                                    )
                                })
                        },
                    )?,
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
            })
            .await
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
    initial_path: std::path::PathBuf,
) -> Result<(), String> {
    let mouse: SharedMouse = Rc::new(RefCell::new(
        Enigo::new(&enigo::Settings::default())
            .map_err(|error| format!("failed to initialize mouse input: {error}"))?,
    ));

    run_with_controls(
        commands,
        states,
        initial_path,
        HostControls { mouse, window: Rc::new(Win32WindowControl), actions_paused: Arc::new(AtomicBool::new(false)) },
        Duration::from_millis(50),
    )
    .await
}

async fn run_with_controls(
    mut commands: mpsc::Receiver<ScriptCommand>,
    mut states: watch::Receiver<State>,
    initial_path: std::path::PathBuf,
    controls: HostControls,
    loop_delay: Duration,
) -> Result<(), String> {
    let mut current_path = initial_path;
    let mut session = match load_path(&current_path).await {
        Ok(session) => {
            println!("running {}", current_path.display());
            Some(session)
        }
        Err(error) => {
            eprintln!("{error}");
            None
        }
    };
    controls.actions_paused.store(false, Ordering::Release);
    let mut paused = false;

    loop {
        let command = if session.is_some() && !paused {
            match commands.try_recv() {
                Ok(command) => Some(command),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    return Ok(());
                }
            }
        } else {
            match commands.recv().await {
                Some(command) => Some(command),
                None => return Ok(()),
            }
        };

        if let Some(command) = command {
            match command {
                ScriptCommand::Load(path) => {
                    controls.actions_paused.store(false, Ordering::Release);
                    session = None;
                    paused = false;
                    current_path = path;
                    match load_path(&current_path).await {
                        Ok(loaded) => {
                            session = Some(loaded);
                            println!("running {}", current_path.display());
                        }
                        Err(error) => eprintln!("{error}"),
                    }
                }
                ScriptCommand::Reload => {
                    controls.actions_paused.store(false, Ordering::Release);
                    session = None;
                    paused = false;
                    match load_path(&current_path).await {
                        Ok(loaded) => {
                            session = Some(loaded);
                            println!("reloaded {}", current_path.display());
                        }
                        Err(error) => eprintln!("{error}"),
                    }
                }
                ScriptCommand::Pause => {
                    if session.is_none() {
                        controls.actions_paused.store(false, Ordering::Release);
                        println!("no script is running");
                    } else if paused {
                        controls.actions_paused.store(true, Ordering::Release);
                        println!("script is already paused");
                    } else {
                        controls.actions_paused.store(true, Ordering::Release);
                        paused = true;
                        println!("script paused");
                    }
                }
                ScriptCommand::Resume => {
                    if session.is_none() {
                        controls.actions_paused.store(false, Ordering::Release);
                        println!("script is stopped; use reload or load");
                    } else if paused {
                        controls.actions_paused.store(false, Ordering::Release);
                        paused = false;
                        println!("script resumed");
                    } else {
                        controls.actions_paused.store(false, Ordering::Release);
                        println!("script is already running");
                    }
                }
                ScriptCommand::Stop => {
                    controls.actions_paused.store(false, Ordering::Release);
                    if session.take().is_some() {
                        paused = false;
                        println!("script stopped");
                    } else {
                        println!("script is already stopped");
                    }
                }
                ScriptCommand::SetPaused(requested_paused) => {
                    if session.is_none() {
                        controls.actions_paused.store(false, Ordering::Release);
                        println!("no script is running");
                    } else if requested_paused {
                        controls.actions_paused.store(true, Ordering::Release);
                        if paused {
                            println!("script is already paused");
                        } else {
                            paused = true;
                            println!("script paused");
                        }
                    } else {
                        controls.actions_paused.store(false, Ordering::Release);
                        if paused {
                            paused = false;
                            println!("script resumed");
                        } else {
                            println!("script is already running");
                        }
                    }
                }
            }

            continue;
        }

        let Some(active) = session.as_ref() else {
            continue;
        };
        let snapshot = states.borrow_and_update().clone();

        if let Err(error) = active.invoke(snapshot, controls.clone()).await {
            eprintln!("script invocation failed: {error}");
        }

        tokio::time::sleep(loop_delay).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::udp::State;
    use enigo::Button;
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

    impl WindowControl for FakeWindow {
        fn resize_client(&self, _width: i32, _height: i32) -> Result<(), String> { Ok(()) }
        fn focus_and_translate(&self, x: i32, y: i32) -> Result<(i32, i32), String> { Ok((x, y)) }
    }

    impl From<SharedMouse> for HostControls {
        fn from(mouse: SharedMouse) -> Self {
            Self { mouse, window: Rc::new(FakeWindow), actions_paused: Arc::new(AtomicBool::new(false)) }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum HostEvent { Resize(i32, i32), FocusAndTranslate(i32, i32), Click(i32, i32, Button) }

    struct RecordingWindow { events: Rc<RefCell<Vec<HostEvent>>>, translated: (i32, i32), error: bool }
    impl WindowControl for RecordingWindow {
        fn resize_client(&self, width: i32, height: i32) -> Result<(), String> {
            self.events.borrow_mut().push(HostEvent::Resize(width, height)); Ok(())
        }
        fn focus_and_translate(&self, x: i32, y: i32) -> Result<(i32, i32), String> {
            self.events.borrow_mut().push(HostEvent::FocusAndTranslate(x, y));
            if self.error { Err("coordinates outside client area".into()) } else { Ok(self.translated) }
        }
    }
    struct RecordingMouse { events: Rc<RefCell<Vec<HostEvent>>> }
    impl MouseInput for RecordingMouse {
        fn click_at(&mut self, x: i32, y: i32, button: Button) -> Result<(), String> {
            self.events.borrow_mut().push(HostEvent::Click(x, y, button)); Ok(())
        }
    }
    fn recording_controls(translated: (i32, i32)) -> (HostControls, Rc<RefCell<Vec<HostEvent>>>) {
        let events = Rc::new(RefCell::new(Vec::new()));
        (HostControls { mouse: Rc::new(RefCell::new(RecordingMouse { events: events.clone() })), window: Rc::new(RecordingWindow { events: events.clone(), translated, error: false }), actions_paused: Arc::new(AtomicBool::new(false)) }, events)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resize_and_relative_click_use_the_window_controller_in_order() {
        let session = ScriptSession::new(r#"((rev) => { rev.resize(1280, 720); rev.click(10, 20, "right"); })"#).await.unwrap();
        let (controls, events) = recording_controls((1010, 2020));
        session.invoke(State::default(), controls).await.unwrap();
        assert_eq!(*events.borrow(), vec![HostEvent::Resize(1280, 720), HostEvent::FocusAndTranslate(10, 20), HostEvent::Click(1010, 2020, Button::Right)]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resize_rejects_invalid_dimensions_without_window_work() {
        for source in [r#"((rev) => rev.resize(0, 720))"#, r#"((rev) => rev.resize(-1, 720))"#, r#"((rev) => rev.resize(1.5, 720))"#, r#"((rev) => rev.resize(Infinity, 720))"#] {
            let (controls, events) = recording_controls((0, 0));
            let session = ScriptSession::new(source).await.unwrap();
            assert!(session.invoke(State::default(), controls).await.is_err());
            assert!(events.borrow().is_empty());
        }
        let (controls, events) = recording_controls((0, 0));
        let session = ScriptSession::new(r#"((rev) => rev.resize(1280, 720))"#).await.unwrap();
        session.invoke(State::default(), controls).await.unwrap();
        assert_eq!(*events.borrow(), vec![HostEvent::Resize(1280, 720)]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn translation_error_prevents_click() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let controls = HostControls {
            mouse: Rc::new(RefCell::new(RecordingMouse { events: events.clone() })),
            window: Rc::new(RecordingWindow { events: events.clone(), translated: (0, 0), error: true }),
            actions_paused: Arc::new(AtomicBool::new(false)),
        };
        let session = ScriptSession::new(r#"((rev) => rev.click(10, 20))"#).await.unwrap();
        assert!(session.invoke(State::default(), controls).await.is_err());
        assert_eq!(*events.borrow(), vec![HostEvent::FocusAndTranslate(10, 20)]);
    }

    async fn run_with_mouse(
        commands: mpsc::Receiver<ScriptCommand>, states: watch::Receiver<State>,
        initial_path: std::path::PathBuf, mouse: SharedMouse, loop_delay: Duration,
    ) -> Result<(), String> {
        run_with_controls(commands, states, initial_path, mouse.into(), loop_delay).await
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
        use std::sync::atomic::Ordering;
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
            actions_paused: Arc::new(AtomicBool::new(false)),
        };
        let gate = controls.actions_paused.clone();
        let (command_tx, command_rx) = mpsc::channel(8);
        let (_state_tx, state_rx) = watch::channel(State::default());
        let runner_path = path.clone();
        let local = tokio::task::LocalSet::new();

        local
            .run_until(async move {
                let runner = tokio::task::spawn_local(run_with_controls(
                    command_rx,
                    state_rx,
                    runner_path,
                    controls,
                    Duration::from_millis(1),
                ));

                tokio::time::sleep(Duration::from_millis(5)).await;
                gate.store(true, Ordering::Release);
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

                gate.store(false, Ordering::Release);
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
        use std::sync::atomic::Ordering;
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
            actions_paused: Arc::new(AtomicBool::new(false)),
        };
        let gate = controls.actions_paused.clone();
        let (command_tx, command_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(State::default());
        let runner_path = first.clone();
        let local = tokio::task::LocalSet::new();

        local
            .run_until(async move {
                let runner = tokio::task::spawn_local(run_with_controls(
                    command_rx,
                    state_rx,
                    runner_path,
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
                        if gate.load(Ordering::Acquire) {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(gate.load(Ordering::Acquire));

                command_tx
                    .send(ScriptCommand::SetPaused(false))
                    .await
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if !gate.load(Ordering::Acquire) {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(!gate.load(Ordering::Acquire));

                command_tx.send(ScriptCommand::Resume).await.unwrap();
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap();
                assert!(!gate.load(Ordering::Acquire));

                command_tx.send(ScriptCommand::Pause).await.unwrap();
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if gate.load(Ordering::Acquire) {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(gate.load(Ordering::Acquire));

                command_tx
                    .send(ScriptCommand::Load(second.clone()))
                    .await
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap();
                assert!(!gate.load(Ordering::Acquire));

                command_tx.send(ScriptCommand::Pause).await.unwrap();
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if gate.load(Ordering::Acquire) {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(gate.load(Ordering::Acquire));

                command_tx.send(ScriptCommand::Reload).await.unwrap();
                tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap();
                assert!(!gate.load(Ordering::Acquire));

                command_tx.send(ScriptCommand::Pause).await.unwrap();
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if gate.load(Ordering::Acquire) {
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
                        if !gate.load(Ordering::Acquire) {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(!gate.load(Ordering::Acquire));
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
                assert!(!gate.load(Ordering::Acquire));

                command_tx.send(ScriptCommand::Pause).await.unwrap();
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if gate.load(Ordering::Acquire) {
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
                        if !gate.load(Ordering::Acquire) {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(!gate.load(Ordering::Acquire));
                assert!(tokio::time::timeout(
                    Duration::from_millis(15),
                    event_rx.recv(),
                )
                .await
                .is_err());

                gate.store(true, Ordering::Release);
                command_tx
                    .send(ScriptCommand::SetPaused(true))
                    .await
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if !gate.load(Ordering::Acquire) {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(!gate.load(Ordering::Acquire));

                gate.store(true, Ordering::Release);
                command_tx
                    .send(ScriptCommand::SetPaused(false))
                    .await
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if !gate.load(Ordering::Acquire) {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(!gate.load(Ordering::Acquire));

                runner.abort();
                let _ = runner.await;
            })
            .await;

        fs::remove_dir_all(root).unwrap();
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
}
