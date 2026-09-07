use super::{
    bindings::HostControls,
    loader::{ScriptModuleLoader, ScriptModuleResolver},
};
use rquickjs::{
    convert::Coerced,
    function::Rest,
    promise::MaybePromise,
    AsyncContext,
    AsyncRuntime,
    CaughtError,
    Error,
    Function,
    FromJs,
    Module,
    Object,
    Persistent,
    Value,
};
use std::{
    cell::Cell,
    rc::Rc,
};
#[cfg(test)]
use std::time::Duration;

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
    script: Persistent<Function<'static>>,
    before_pause: Option<Persistent<Function<'static>>>,
    after_resume: Option<Persistent<Function<'static>>>,
    parse: Persistent<Function<'static>>,
    freeze: Persistent<Function<'static>>,
    context: AsyncContext,
    _runtime: AsyncRuntime,
    client: reqwest::Client,
}

impl ScriptSession {
    #[cfg(test)]
    pub(super) async fn new(source: &str) -> Result<Self, String> {
        Self::new_with_client(
            source,
            "test.js",
            reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .build()
                .map_err(|error| error.to_string())?,
        )
        .await
    }

    /// `name` is the module specifier the entry script is declared under; it
    /// also anchors relative `import`s from that script's own directory (see
    /// `ScriptModuleResolver`). Production callers pass the script's real
    /// path; tests pass a synthetic name since none of them import anything.
    pub(super) async fn new_with_client(source: &str, name: &str, client: reqwest::Client) -> Result<Self, String> {
        let runtime = AsyncRuntime::new().map_err(|error| error.to_string())?;
        runtime.set_loader(ScriptModuleResolver, ScriptModuleLoader).await;
        let context = AsyncContext::full(&runtime).await.map_err(|error| error.to_string())?;
        let source = source.to_owned();
        let name = name.to_owned();

        let (script, before_pause, after_resume, parse, freeze) = context
            .async_with(async move |ctx| {
                let result: rquickjs::Result<_> = async {
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

                    let parse: Function = ctx.eval("JSON.parse")?;
                    let freeze: Function = ctx.eval("Object.freeze")?;

                    let (module, promise) = Module::declare(ctx.clone(), name, source)?.eval()?;
                    promise.into_future::<()>().await?;
                    let namespace = module.namespace()?;
                    let script: Function = namespace.get("default").map_err(|_| {
                        Error::new_from_js_message(
                            "module",
                            "JavaScript",
                            "script must `export default` the function to run",
                        )
                    })?;
                    // Both hooks are optional: a script that doesn't export
                    // them just runs without any pause/resume handler.
                    let before_pause: Option<Function> = namespace.get("beforePause").ok();
                    let after_resume: Option<Function> = namespace.get("afterResume").ok();

                    Ok((
                        Persistent::save(&ctx, script),
                        before_pause.map(|hook| Persistent::save(&ctx, hook)),
                        after_resume.map(|hook| Persistent::save(&ctx, hook)),
                        Persistent::save(&ctx, parse),
                        Persistent::save(&ctx, freeze),
                    ))
                }
                .await;

                result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
            })
            .await?;

        Ok(Self {
            script,
            before_pause,
            after_resume,
            parse,
            freeze,
            context,
            _runtime: runtime,
            client,
        })
    }

    /// Calls an optional lifecycle hook (`beforePause`/`afterResume`) if the
    /// script exported one, awaiting it if it returns a promise. A no-op
    /// (`Ok(())`) when the script didn't export that hook.
    async fn run_hook(&self, hook: &Option<Persistent<Function<'static>>>) -> Result<(), String> {
        let Some(hook) = hook else { return Ok(()) };
        let hook = hook.clone();

        self.context
            .async_with(async move |ctx| {
                let result: rquickjs::Result<()> = async {
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

    pub(super) async fn run_before_pause(&self) -> Result<(), String> {
        self.run_hook(&self.before_pause).await
    }

    pub(super) async fn run_after_resume(&self) -> Result<(), String> {
        self.run_hook(&self.after_resume).await
    }

    pub(super) async fn invoke<S, C: Into<HostControls>>(
        &self,
        _state: S,
        controls: C,
    ) -> Result<bool, String> {
        let controls = controls.into();
        let script = self.script.clone();
        let parse = self.parse.clone();
        let freeze = self.freeze.clone();
        let stop_requested = Rc::new(Cell::new(false));
        let stop_request = stop_requested.clone();

        let result = self.context
            .async_with(async move |ctx| {
                let result: rquickjs::Result<()> = async {
                    let script: Function = script.restore(&ctx)?;
                    let parse: Function = parse.restore(&ctx)?;
                    let freeze: Function = freeze.restore(&ctx)?;

                    let rev = super::bindings::create_rev(
                        ctx.clone(),
                        self.client.clone(),
                        controls.clone(),
                        parse.clone(),
                        freeze.clone(),
                        stop_request,
                    )?;
                    ctx.globals().set("rev", rev)?;
                    let result: MaybePromise = script.call(())?;
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
