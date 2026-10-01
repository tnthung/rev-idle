use super::{
    background::{BackgroundRegistry, ScriptBackground},
    bindings::HostControls,
    control::SessionControl,
    loader::ScriptModules,
    transfer::FunctionTransfer,
    ui::{ScriptUiBindings, ScriptUiState},
};
use crate::bridge::{ScriptUiPublisher, WsConnection};
use rquickjs::{
    convert::Coerced,
    function::Rest,
    loader::Loader,
    promise::MaybePromise,
    AsyncContext,
    AsyncRuntime,
    CaughtError,
    Class,
    Error,
    Exception,
    Function,
    FromJs,
    Object,
    Persistent,
    Symbol,
    Value,
};
use std::{
    io::{self, IsTerminal, Write},
    rc::Rc,
};

pub(super) fn format_console_message(args: Rest<Value>) -> rquickjs::Result<String> {
    args.0
        .into_iter()
        .map(|value| {
            if value.is_string() {
                value
                    .as_string()
                    .expect("string value must have a string representation")
                    .to_string()
            } else if value.is_error() {
                // `JSON.stringify` on an Error is always "{}" (message/stack
                // aren't enumerable own properties), so format it like the
                // console normally would instead of falling through below.
                // Unlike V8, QuickJS's `error.stack` holds only the trace
                // (no leading "Name: message" line), so build that header
                // from `name`/`message` ourselves and append the trace.
                match value.as_exception() {
                    Some(exception) => {
                        let name = exception
                            .as_object()
                            .get::<_, Coerced<std::string::String>>("name")
                            .map(|coerced| coerced.0)
                            .unwrap_or_else(|_| "Error".to_string());
                        let header = match exception.message() {
                            Some(message) if !message.is_empty() => format!("{name}: {message}"),
                            _ => name,
                        };
                        Ok(match exception.stack() {
                            Some(stack) if !stack.is_empty() => format!("{header}\n{stack}"),
                            _ => header,
                        })
                    }
                    None => {
                        let ctx = value.ctx().clone();
                        Ok(Coerced::<std::string::String>::from_js(&ctx, value)?.0)
                    }
                }
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

pub(super) struct ScriptSession {
    // Rust drops fields in declaration order. Persistent roots must be gone
    // before their context and runtime.
    background: Option<ScriptBackground>,
    script: Persistent<Function<'static>>,
    after_load: Option<Persistent<Function<'static>>>,
    on_connect: Option<Persistent<Function<'static>>>,
    on_disconnect: Option<Persistent<Function<'static>>>,
    before_pause: Option<Persistent<Function<'static>>>,
    after_resume: Option<Persistent<Function<'static>>>,
    parse: Persistent<Function<'static>>,
    freeze: Persistent<Function<'static>>,
    transfer: Option<Rc<FunctionTransfer>>,
    ui_bindings: Option<Rc<ScriptUiBindings>>,
    ui: Option<Rc<ScriptUiState>>,
    context: AsyncContext,
    runtime: AsyncRuntime,
    background_registry: Rc<BackgroundRegistry>,
    connection: WsConnection,
    session_control: SessionControl,
    pub(super) screen_ownership: Rc<super::ownership::ScreenOwnershipState>,
    pub(super) mutex: Rc<super::mutex::MutexRegistry>,
}

impl Drop for ScriptSession {
    fn drop(&mut self) {
        self.session_control.stop();
        self.background.take();
        self.ui_bindings.take();
        self.transfer.take();
        self.ui.take();
        self.screen_ownership.close();
        self.mutex.close();
    }
}

impl ScriptSession {
    #[cfg(test)]
    pub(super) async fn new(source: &str) -> Result<Self, String> {
        Self::new_with_connection(source, "test.js", WsConnection::disconnected_for_test()).await
    }

    /// `name` is the module specifier the entry script is declared under; it
    /// also anchors relative `import`s from that script's own directory (see
    /// `ScriptModules`). Production callers pass the script's real
    /// path; tests pass a synthetic name since none of them import anything.
    #[cfg(test)]
    pub(super) async fn new_with_connection(source: &str, name: &str, connection: WsConnection) -> Result<Self, String> {
        Self::new_with_connection_and_control(
            source,
            name,
            connection,
            SessionControl::standalone(),
            ScriptUiPublisher::default(),
        )
        .await
    }

    pub(super) async fn new_with_connection_and_control(
        source: &str,
        name: &str,
        connection: WsConnection,
        session_control: SessionControl,
        ui_publisher: ScriptUiPublisher,
    ) -> Result<Self, String> {
        let modules = ScriptModules::default();
        modules.insert(name, source.to_owned())?;
        let ui = Rc::new(ScriptUiState::new(ui_publisher, session_control.clone()));
        let background_registry = Rc::new(BackgroundRegistry::new(session_control.clone()));
        let screen_ownership = Rc::new(super::ownership::ScreenOwnershipState::default());
        let mutex = Rc::new(super::mutex::MutexRegistry::default());
        let runtime = AsyncRuntime::new().map_err(|error| error.to_string())?;
        runtime.set_loader(modules.clone(), modules.clone()).await;
        let interrupt_control = session_control.clone();
        runtime
            .set_interrupt_handler(Some(Box::new(move || interrupt_control.is_stopped())))
            .await;
        let context = AsyncContext::full(&runtime).await.map_err(|error| error.to_string())?;
        let name = name.to_owned();

        let context_session_control = session_control.clone();
        let context_connection = connection.clone();
        let context_ui = ui.clone();
        let context_background = background_registry.clone();
        let context_modules = modules.clone();
        let (script, after_load, on_connect, on_disconnect, before_pause, after_resume, parse, freeze, transfer, ui_bindings) = context
            .async_with(async move |ctx| {
                let result: rquickjs::Result<_> = async {
                    context_modules.install_stack_trace(&ctx)?;
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
                    console.set(
                        "clear",
                        Function::new(ctx.clone(), || {
                            print!("\x1B[2J\x1B[H{}", if io::stdout().is_terminal() { "" } else { "\n" });
                            io::stdout().flush().map_err(|error| {
                                Error::new_from_js_message("console", "JavaScript", error.to_string())
                            })
                        })?,
                    )?;
                    ctx.globals().set("console", console)?;

                    let parse: Function = ctx.eval("JSON.parse")?;
                    let freeze: Function = ctx.eval("Object.freeze")?;
                    let ownership_prototype = Class::<super::ownership::ScreenOwnership>::prototype(&ctx)?.unwrap();
                    ownership_prototype.set(
                        ctx.eval::<Symbol, _>("Symbol.dispose")?,
                        ownership_prototype.get::<_, Function>("release")?,
                    )?;
                    let mutex_prototype = Class::<super::mutex::MutexGuard>::prototype(&ctx)?.unwrap();
                    mutex_prototype.set(
                        ctx.eval::<Symbol, _>("Symbol.dispose")?,
                        mutex_prototype.get::<_, Function>("release")?,
                    )?;

                    let transfer = Rc::new(FunctionTransfer::new(&ctx)?);
                    let ui_bindings = Rc::new(ScriptUiBindings::new(
                        &ctx,
                        context_ui,
                        context_connection,
                        context_session_control.clone(),
                        transfer.clone(),
                        context_background,
                    )?);

                    if context_session_control.is_stopped() {
                        return Err(Exception::throw_message(
                            &ctx,
                            SessionControl::error_message(),
                        ));
                    }

                    let mut modules = context_modules.clone();
                    let (module, promise) = modules.load(&ctx, &name, None)?.eval()?;
                    promise.into_future::<()>().await?;
                    let namespace = module.namespace()?;
                    let script: Function = namespace.get("default").map_err(|_| {
                        Error::new_from_js_message(
                            "module",
                            "JavaScript",
                            "script must `export default` the function to run",
                        )
                    })?;
                    // Hooks are optional: a script that doesn't export one
                    // just runs without that lifecycle handler.
                    let after_load: Option<Function> = namespace.get("afterLoad").ok();
                    let on_connect: Option<Function> = namespace.get("onConnect").ok();
                    let on_disconnect: Option<Function> = namespace.get("onDisconnect").ok();
                    let before_pause: Option<Function> = namespace.get("beforePause").ok();
                    let after_resume: Option<Function> = namespace.get("afterResume").ok();

                    Ok((
                        Persistent::save(&ctx, script),
                        after_load.map(|hook| Persistent::save(&ctx, hook)),
                        on_connect.map(|hook| Persistent::save(&ctx, hook)),
                        on_disconnect.map(|hook| Persistent::save(&ctx, hook)),
                        before_pause.map(|hook| Persistent::save(&ctx, hook)),
                        after_resume.map(|hook| Persistent::save(&ctx, hook)),
                        Persistent::save(&ctx, parse),
                        Persistent::save(&ctx, freeze),
                        transfer,
                        ui_bindings,
                    ))
                }
                .await;

                result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
            })
            .await?;

        let background = ScriptBackground::new(
            modules,
            background_registry.clone(),
            ui.clone(),
            connection.clone(),
            session_control.clone(),
            screen_ownership.clone(),
            mutex.clone(),
        ).await?;

        Ok(Self {
            background: Some(background),
            script,
            after_load,
            on_connect,
            on_disconnect,
            before_pause,
            after_resume,
            parse,
            freeze,
            transfer: Some(transfer),
            ui_bindings: Some(ui_bindings),
            context,
            runtime,
            background_registry,
            connection,
            session_control,
            ui: Some(ui),
            screen_ownership,
            mutex,
        })
    }

    /// Calls an optional lifecycle hook if the script exported one, awaiting
    /// it if it returns a promise. A no-op when the hook wasn't exported.
    async fn run_hook(
        &self,
        hook: &Option<Persistent<Function<'static>>>,
        controls: HostControls,
    ) -> Result<(), String> {
        let Some(hook) = hook else { return Ok(()) };
        let hook = hook.clone();
        let connection = self.connection.clone();
        let parse = self.parse.clone();
        let freeze = self.freeze.clone();
        let session_control = self.session_control.clone();
        let ui = self.ui_bindings.as_ref().expect("script UI bindings must exist").clone();
        let transfer = self.transfer.as_ref().expect("script function transfer must exist").clone();
        let background = self.background_registry.clone();
        let screen_ownership = self.screen_ownership.clone();
        let mutex = self.mutex.clone();

        self.context
            .async_with(async move |ctx| {
                let result: rquickjs::Result<()> = async {
                    let parse: Function = parse.restore(&ctx)?;
                    let freeze: Function = freeze.restore(&ctx)?;
                    let rev = super::bindings::create_rev(
                        ctx.clone(),
                        connection,
                        controls,
                        parse,
                        freeze,
                        session_control,
                        screen_ownership,
                        mutex,
                        ui,
                        transfer,
                        background,
                    )?;
                    ctx.globals().set("rev", rev)?;
                    let hook: Function = hook.restore(&ctx)?;
                    let result: MaybePromise = hook.call(())?;
                    let _: Value = result.into_future().await?;
                    Ok(())
                }
                .await;

                result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
            })
            .await
    }

    pub(super) async fn run_after_load(&self, controls: HostControls) -> Result<(), String> {
        self.run_hook(&self.after_load, controls).await
    }

    pub(super) async fn run_on_connect(&self, controls: HostControls) -> Result<(), String> {
        self.run_hook(&self.on_connect, controls).await
    }

    pub(super) async fn run_on_disconnect(&self, controls: HostControls) -> Result<(), String> {
        self.run_hook(&self.on_disconnect, controls).await
    }

    pub(super) fn terminate(&self) {
        self.session_control.stop();
        self.screen_ownership.close();
        self.mutex.close();
    }

    #[cfg(test)]
    pub(super) async fn dispatch_ui_event(
        &self,
        event: crate::bridge::ScriptUiEvent,
        controls: HostControls,
    ) -> Result<(), String> {
        self.dispatch_ui_event_if_current(event, controls, || true).await
    }

    pub(super) async fn dispatch_ui_event_if_current(
        &self,
        event: crate::bridge::ScriptUiEvent,
        _controls: HostControls,
        current: impl Fn() -> bool + 'static,
    ) -> Result<(), String> {
        self.background_registry.dispatch_ui(event, current).await
    }

    pub(super) async fn run_before_pause(&self, controls: HostControls) -> Result<(), String> {
        self.run_hook(&self.before_pause, controls).await
    }

    pub(super) async fn run_after_resume(&self, controls: HostControls) -> Result<(), String> {
        self.run_hook(&self.after_resume, controls).await
    }

    pub(super) fn has_connection_hooks(&self) -> bool {
        self.on_connect.is_some() || self.on_disconnect.is_some()
    }

    pub(super) fn is_stopped(&self) -> bool {
        self.session_control.is_stopped()
    }

    pub(super) fn is_paused(&self) -> bool {
        self.session_control.is_paused()
    }

    pub(super) fn acknowledge_pause(&self, paused: bool) {
        self.session_control.acknowledge_pause(paused);
    }

    #[cfg(test)]
    pub(super) async fn invoke<S, C: Into<HostControls>>(
        &self,
        state: S,
        controls: C,
    ) -> Result<bool, String> {
        self.invoke_classified(state, controls)
            .await
            .map_err(|error| error.message)
    }

    pub(super) async fn invoke_classified<S, C: Into<HostControls>>(
        &self,
        _state: S,
        controls: C,
    ) -> Result<bool, ScriptInvocationError> {
        let controls = controls.into();
        let script = self.script.clone();
        let parse = self.parse.clone();
        let freeze = self.freeze.clone();
        let session_control = self.session_control.clone();
        let ui = self.ui_bindings.as_ref().expect("script UI bindings must exist").clone();
        let transfer = self.transfer.as_ref().expect("script function transfer must exist").clone();
        let background = self.background_registry.clone();
        let screen_ownership = self.screen_ownership.clone();
        let mutex = self.mutex.clone();
        let connection = self.connection.clone();

        let result = self.context
            .async_with(async move |ctx| {
                let result: rquickjs::Result<()> = async {
                    let script: Function = script.restore(&ctx)?;
                    let parse: Function = parse.restore(&ctx)?;
                    let freeze: Function = freeze.restore(&ctx)?;

                    let rev = super::bindings::create_rev(
                        ctx.clone(),
                        connection,
                        controls.clone(),
                        parse.clone(),
                        freeze.clone(),
                        session_control,
                        screen_ownership,
                        mutex,
                        ui,
                        transfer,
                        background,
                    )?;
                    ctx.globals().set("rev", rev)?;
                    let result: MaybePromise = script.call(())?;
                    let _: Value = result.into_future().await?;
                    Ok(())
                }
                .await;

                result.map_err(|error| {
                    let error = CaughtError::from_error(&ctx, error);
                    let disconnected = match &error {
                        CaughtError::Exception(exception) => exception
                            .as_object()
                            .get::<_, bool>("__revIdleDisconnectedTransport")
                            .unwrap_or(false),
                        _ => false,
                    };
                    ScriptInvocationError {
                        message: error.to_string(),
                        disconnected,
                    }
                })
            })
            .await;

        result.map(|()| false)
    }

    pub(super) fn drive(&self) -> impl std::future::Future<Output = ()> + use<> {
        self.runtime.drive()
    }

    pub(super) fn drive_background(&self, controls: HostControls) -> impl std::future::Future<Output = ()> + '_ {
        self.background.as_ref().expect("script background must exist").run(controls)
    }
}

pub(super) struct ScriptInvocationError {
    pub(super) message: String,
    pub(super) disconnected: bool,
}

impl std::fmt::Display for ScriptInvocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
