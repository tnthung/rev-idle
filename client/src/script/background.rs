use super::{
    bindings::{HostControls, create_rev},
    control::SessionControl,
    loader::ScriptModules,
    ownership::ScreenOwnershipState,
    mutex::MutexRegistry,
    transfer::FunctionDescriptor,
    ui::{ScriptUiBindings, ScriptUiState, UiEventKind},
};
use crate::bridge::{ScriptUiEvent, WsConnection};
use rquickjs::{
    function::{Rest, This},
    loader::Loader,
    promise::{Promise, PromiseState},
    AsyncContext,
    AsyncRuntime,
    CaughtError,
    Class,
    Error,
    Function,
    Object,
    Persistent,
    Symbol,
    Value,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    io::{self, IsTerminal, Write},
    rc::Rc,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BackgroundTarget {
    Daemon(String),
    Ui { name: String, instance: Uuid, event: UiEventKind },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BackgroundRegistrationStatus {
    Pending,
    Active,
    Failed,
}

#[derive(Clone)]
struct BackgroundRegistration {
    target: BackgroundTarget,
    function: FunctionDescriptor,
    status: BackgroundRegistrationStatus,
}

enum BackgroundRequest {
    Install(Uuid),
    DispatchUi {
        event: ScriptUiEvent,
        current: Box<dyn Fn() -> bool>,
        response: oneshot::Sender<Result<(), String>>,
    },
}

struct BackgroundRegistryState {
    current: Vec<(BackgroundTarget, Uuid)>,
    registrations: HashMap<Uuid, BackgroundRegistration>,
}

struct BackgroundInstallation {
    target: BackgroundTarget,
    promise: Persistent<Promise<'static>>,
    namespace: Persistent<Object<'static>>,
}

enum BackgroundInvocationTarget {
    Daemon(String),
    UiStateUpdate(String),
    Ui {
        event: ScriptUiEvent,
        response: oneshot::Sender<Result<(), String>>,
    },
}

struct BackgroundInvocation {
    promise: Persistent<Promise<'static>>,
    target: BackgroundInvocationTarget,
}

pub(super) struct ScriptBackground {
    // Rust drops fields in declaration order. Every persistent root and every
    // adapter that can own roots must be gone before the context and runtime.
    rev: RefCell<Option<Persistent<Object<'static>>>>,
    installations: RefCell<HashMap<Uuid, BackgroundInstallation>>,
    installed: RefCell<HashMap<Uuid, Persistent<Function<'static>>>>,
    invocations: RefCell<HashMap<Uuid, BackgroundInvocation>>,
    active_daemons: RefCell<HashMap<String, Uuid>>,
    codec: Persistent<Object<'static>>,
    freeze: Persistent<Function<'static>>,
    transfer: Rc<super::transfer::FunctionTransfer>,
    bindings: Rc<ScriptUiBindings>,
    modules: ScriptModules,
    registry: Rc<BackgroundRegistry>,
    ui: Rc<ScriptUiState>,
    connection: WsConnection,
    session: SessionControl,
    ownership: Rc<ScreenOwnershipState>,
    mutex: Rc<MutexRegistry>,
    context: AsyncContext,
    runtime: AsyncRuntime,
}

pub(super) struct BackgroundRegistry {
    session: SessionControl,
    state: RefCell<BackgroundRegistryState>,
    requests: mpsc::UnboundedSender<BackgroundRequest>,
    receiver: RefCell<Option<mpsc::UnboundedReceiver<BackgroundRequest>>>,
}

impl BackgroundRegistry {
    pub(super) fn new(session: SessionControl) -> Self {
        let (requests, receiver) = mpsc::unbounded_channel();
        Self {
            session,
            state: RefCell::new(BackgroundRegistryState {
                current: Vec::new(),
                registrations: HashMap::new(),
            }),
            requests,
            receiver: RefCell::new(Some(receiver)),
        }
    }

    pub(super) fn register(&self, target: BackgroundTarget, function: FunctionDescriptor) -> Uuid {
        let registration = Uuid::new_v4();
        let mut state = self.state.borrow_mut();
        if let Some(index) = state.current.iter().position(|(candidate, _)| candidate == &target) {
            let current = state.current[index].1;
            state.registrations.remove(&current);
            state.current[index].1 = registration;
        } else {
            state.current.push((target.clone(), registration));
        }
        state.registrations.insert(registration, BackgroundRegistration {
            target,
            function,
            status: BackgroundRegistrationStatus::Pending,
        });
        drop(state);
        let _ = self.requests.send(BackgroundRequest::Install(registration));
        registration
    }

    pub(super) fn unregister(&self, target: &BackgroundTarget) {
        let mut state = self.state.borrow_mut();
        if let Some(index) = state.current.iter().position(|(candidate, _)| candidate == target) {
            let (_, registration) = state.current.remove(index);
            state.registrations.remove(&registration);
        }
    }

    pub(super) fn is_current(&self, registration: Uuid) -> bool {
        self.state.borrow().registrations.contains_key(&registration)
    }

    pub(super) fn status(&self, registration: Uuid) -> Option<BackgroundRegistrationStatus> {
        self.state.borrow().registrations.get(&registration).map(|record| record.status)
    }

    pub(super) fn contains_daemon(&self, name: &str) -> bool {
        self.state.borrow().current.iter().any(|(target, _)| {
            matches!(target, BackgroundTarget::Daemon(candidate) if candidate == name)
        })
    }

    pub(super) fn daemon_names(&self) -> Vec<String> {
        self.state.borrow().current.iter().filter_map(|(target, _)| match target {
            BackgroundTarget::Daemon(name) => Some(name.clone()),
            BackgroundTarget::Ui { .. } => None,
        }).collect()
    }

    pub(super) async fn dispatch_ui(
        &self,
        event: ScriptUiEvent,
        current: impl Fn() -> bool + 'static,
    ) -> Result<(), String> {
        if self.session.is_stopped() {
            return Err(SessionControl::error_message().to_owned());
        }
        let (response, result) = oneshot::channel();
        self.requests.send(BackgroundRequest::DispatchUi {
            event,
            current: Box::new(current),
            response,
        }).map_err(|_| SessionControl::error_message().to_owned())?;
        result.await.unwrap_or_else(|_| Err(SessionControl::error_message().to_owned()))
    }

    fn registration(&self, registration: Uuid) -> Option<BackgroundRegistration> {
        self.state.borrow().registrations.get(&registration).cloned()
    }

    fn set_status(&self, registration: Uuid, status: BackgroundRegistrationStatus) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(record) = state.registrations.get_mut(&registration) else { return false };
        record.status = status;
        true
    }

    fn take_receiver(&self) -> Option<mpsc::UnboundedReceiver<BackgroundRequest>> {
        self.receiver.borrow_mut().take()
    }
}

impl ScriptBackground {
    pub(super) async fn new(
        modules: ScriptModules,
        registry: Rc<BackgroundRegistry>,
        ui: Rc<ScriptUiState>,
        connection: WsConnection,
        session: SessionControl,
        ownership: Rc<ScreenOwnershipState>,
        mutex: Rc<MutexRegistry>,
    ) -> Result<Self, String> {
        let runtime = AsyncRuntime::new().map_err(|error| error.to_string())?;
        runtime.set_loader(modules.clone(), modules.clone()).await;
        let interrupt_session = session.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || interrupt_session.is_stopped()))).await;
        let context = AsyncContext::full(&runtime).await.map_err(|error| error.to_string())?;
        let bindings_connection = connection.clone();
        let bindings_session = session.clone();
        let bindings_registry = registry.clone();
        let bindings_ui = ui.clone();
        let context_modules = modules.clone();
        let (codec, freeze, transfer, bindings) = context.async_with(async move |ctx| {
            let result: rquickjs::Result<_> = (|| {
                context_modules.install_stack_trace(&ctx)?;
                let console = Object::new(ctx.clone())?;
                console.set(
                    "log",
                    Function::new(ctx.clone(), move |args: Rest<Value>| {
                        println!("{}", super::session::format_console_message(args)?);
                        Ok::<(), Error>(())
                    })?,
                )?;
                console.set(
                    "error",
                    Function::new(ctx.clone(), move |args: Rest<Value>| {
                        eprintln!("{}", super::session::format_console_message(args)?);
                        Ok::<(), Error>(())
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
                super::bignum::install(&ctx)?;
                super::color::install(&ctx)?;
                let codec = super::value::codec(&ctx)?;
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
                let transfer = Rc::new(super::transfer::FunctionTransfer::new(&ctx)?);
                let bindings = Rc::new(ScriptUiBindings::new(
                    &ctx,
                    codec.clone(),
                    bindings_ui,
                    bindings_connection,
                    bindings_session,
                    transfer.clone(),
                    bindings_registry,
                )?);
                Ok((
                    Persistent::save(&ctx, codec),
                    Persistent::save(&ctx, freeze),
                    transfer,
                    bindings,
                ))
            })();
            result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
        }).await?;

        Ok(Self {
            rev: RefCell::new(None),
            installations: RefCell::new(HashMap::new()),
            installed: RefCell::new(HashMap::new()),
            invocations: RefCell::new(HashMap::new()),
            active_daemons: RefCell::new(HashMap::new()),
            codec,
            freeze,
            transfer,
            bindings,
            modules,
            registry,
            ui,
            connection,
            session,
            ownership,
            mutex,
            context,
            runtime,
        })
    }

    pub(super) async fn run(&self, controls: HostControls) {
        let Some(mut requests) = self.registry.take_receiver() else { return };
        let codec = self.codec.clone();
        let freeze = self.freeze.clone();
        let connection = self.connection.clone();
        let session = self.session.clone();
        let keepalive_session = session.clone();
        let ownership = self.ownership.clone();
        let mutex = self.mutex.clone();
        let bindings = self.bindings.clone();
        let transfer = self.transfer.clone();
        let registry = self.registry.clone();
        let initialized = self.context.async_with(async move |ctx| {
            let result: rquickjs::Result<Persistent<Object<'static>>> = (|| {
                let codec: Object = codec.restore(&ctx)?;
                let freeze: Function = freeze.restore(&ctx)?;
                let rev = create_rev(
                    ctx.clone(),
                    connection,
                    controls,
                    codec,
                    freeze,
                    session,
                    ownership,
                    mutex,
                    bindings,
                    transfer,
                    registry,
                )?;
                ctx.globals().set("rev", rev.clone())?;
                let mut stopped = keepalive_session.subscribe_stopped();
                ctx.spawn(async move {
                    while !*stopped.borrow_and_update() && stopped.changed().await.is_ok() {}
                });
                Ok(Persistent::save(&ctx, rev))
            })();
            result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
        }).await;
        match initialized {
            Ok(rev) => *self.rev.borrow_mut() = Some(rev),
            Err(error) => {
                eprintln!("[Script background] initialization failed: {error}");
                self.session.stop();
                return;
            }
        }

        let mut stopped = self.session.subscribe_stopped();
        let mut cadence = tokio::time::interval(Duration::from_millis(5));
        cadence.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                changed = stopped.changed() => {
                    if changed.is_err() || *stopped.borrow() { break; }
                }
                request = requests.recv() => match request {
                    Some(request) => self.handle_request(request).await,
                    None => break,
                },
                _ = cadence.tick() => self.poll_work().await,
                _ = self.runtime.drive() => {
                    if !self.session.is_stopped() {
                        tokio::task::yield_now().await;
                    }
                }
            }
        }
        self.rev.borrow_mut().take();
        self.installations.borrow_mut().clear();
        self.installed.borrow_mut().clear();
        self.invocations.borrow_mut().clear();
        self.active_daemons.borrow_mut().clear();
    }

    async fn handle_request(&self, request: BackgroundRequest) {
        match request {
            BackgroundRequest::Install(registration) => self.start_install(registration).await,
            BackgroundRequest::DispatchUi { event, current, response } => {
                self.start_ui(event, current, response).await;
            }
        }
    }

    async fn start_install(&self, registration: Uuid) {
        let Some(record) = self.registry.registration(registration) else { return };
        let module = match self.modules.transfer_module(registration, &record.function) {
            Ok(module) => module,
            Err(error) => {
                self.fail_install(registration, record.target, error);
                return;
            }
        };
        let mut modules = self.modules.clone();
        let result = self.context.async_with(async move |ctx| {
            let result: rquickjs::Result<_> = (|| {
                let (module, promise) = modules.load(&ctx, &module, None)?.eval()?;
                Ok((
                    Persistent::save(&ctx, promise),
                    Persistent::save(&ctx, module.namespace()?),
                ))
            })();
            result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
        }).await;
        if !self.registry.is_current(registration) || self.session.is_stopped() {
            return;
        }
        match result {
            Ok((promise, namespace)) => {
                self.installations.borrow_mut().insert(registration, BackgroundInstallation {
                    target: record.target,
                    promise,
                    namespace,
                });
            }
            Err(error) => self.fail_install(registration, record.target, error),
        }
    }

    fn fail_install(&self, registration: Uuid, target: BackgroundTarget, error: String) {
        if !self.registry.set_status(registration, BackgroundRegistrationStatus::Failed) {
            return;
        }
        match &target {
            BackgroundTarget::Daemon(name) => eprintln!("[Script background] daemon {name}: {error}"),
            BackgroundTarget::Ui { name, instance, event } => {
                self.ui.complete_handler_install(name, *instance, *event, registration, false);
                eprintln!("[Script background] {event:?} handler for UI element {name:?}: {error}");
            }
        }
    }

    async fn poll_work(&self) {
        let installations = self.installations.borrow().iter().map(|(registration, installation)| {
            (*registration, installation.promise.clone(), installation.namespace.clone())
        }).collect::<Vec<_>>();
        let invocations = self.invocations.borrow().iter().map(|(invocation, record)| {
            (*invocation, record.promise.clone())
        }).collect::<Vec<_>>();
        let (installation_results, invocation_results) = self.context.async_with(async move |ctx| {
            let installation_results = installations.into_iter().map(|(registration, promise, namespace)| {
                let promise: Promise = promise.restore(&ctx)?;
                let result = match promise.state() {
                    PromiseState::Pending => None,
                    PromiseState::Resolved => {
                        let namespace: Object = namespace.restore(&ctx)?;
                        Some(namespace.get::<_, Function>("default")
                            .map(|function| Persistent::save(&ctx, function))
                            .map_err(|error| CaughtError::from_error(&ctx, error).to_string()))
                    }
                    PromiseState::Rejected => Some(promise.result::<Value>().unwrap()
                        .map(|_| unreachable!())
                        .map_err(|error| CaughtError::from_error(&ctx, error).to_string())),
                };
                Ok::<_, Error>((registration, result))
            }).collect::<rquickjs::Result<Vec<_>>>()?;
            let invocation_results = invocations.into_iter().map(|(invocation, promise)| {
                let promise: Promise = promise.restore(&ctx)?;
                let result = promise.result::<Value>().map(|result| result
                    .map(|_| ())
                    .map_err(|error| CaughtError::from_error(&ctx, error).to_string()));
                Ok::<_, Error>((invocation, result))
            }).collect::<rquickjs::Result<Vec<_>>>()?;
            Ok::<_, Error>((installation_results, invocation_results))
        }).await.unwrap_or_else(|error| {
            eprintln!("[Script background] runtime polling failed: {error}");
            self.session.stop();
            (Vec::new(), Vec::new())
        });

        for (registration, result) in installation_results {
            let Some(result) = result else { continue };
            let Some(installation) = self.installations.borrow_mut().remove(&registration) else { continue };
            if !self.registry.is_current(registration) || self.session.is_stopped() {
                continue;
            }
            match result {
                Ok(function) => {
                    self.installed.borrow_mut().insert(registration, function);
                    if !self.registry.set_status(registration, BackgroundRegistrationStatus::Active) {
                        self.installed.borrow_mut().remove(&registration);
                        continue;
                    }
                    match installation.target {
                        BackgroundTarget::Daemon(name) => self.start_daemon(&name).await,
                        BackgroundTarget::Ui { name, instance, event } => {
                            if !self.ui.complete_handler_install(&name, instance, event, registration, true) {
                                self.registry.unregister(&BackgroundTarget::Ui { name, instance, event });
                                self.installed.borrow_mut().remove(&registration);
                            }
                        }
                    }
                }
                Err(error) => self.fail_install(registration, installation.target, error),
            }
        }

        for (invocation, result) in invocation_results {
            let Some(result) = result else { continue };
            let Some(record) = self.invocations.borrow_mut().remove(&invocation) else { continue };
            match record.target {
                BackgroundInvocationTarget::Ui { event, response } => {
                    let _ = response.send(result.map_err(|error| format!(
                        "{:?} handler for UI element {:?}: {error}", event.event, event.element_id,
                    )));
                }
                BackgroundInvocationTarget::Daemon(name) => {
                    self.active_daemons.borrow_mut().remove(&name);
                    if let Err(error) = result {
                        eprintln!("[Script background] daemon {name}: {error}");
                    }
                    self.start_daemon(&name).await;
                }
                BackgroundInvocationTarget::UiStateUpdate(name) => {
                    if let Err(error) = result {
                        eprintln!("[Script background] state update handler for UI element {name:?}: {error}");
                    }
                }
            }
        }

        let state_updates = std::mem::take(&mut *self.ui.state_updates.borrow_mut());
        for registration in state_updates {
            let Some(record) = self.registry.registration(registration) else { continue };
            match record.status {
                BackgroundRegistrationStatus::Pending => self.ui.state_updates.borrow_mut().push(registration),
                BackgroundRegistrationStatus::Active => {
                    if let BackgroundTarget::Ui { name, instance, event: UiEventKind::StateUpdate } = record.target {
                        self.start_state_update(registration, name, instance).await;
                    }
                }
                BackgroundRegistrationStatus::Failed => {}
            }
        }

        self.installations.borrow_mut().retain(|registration, _| self.registry.is_current(*registration));
        self.installed.borrow_mut().retain(|registration, _| self.registry.is_current(*registration));
    }

    async fn start_daemon(&self, name: &str) {
        if self.active_daemons.borrow().contains_key(name) {
            return;
        }
        let Some(registration) = self.registry.state.borrow().current.iter().find_map(|(target, registration)| {
            matches!(target, BackgroundTarget::Daemon(candidate) if candidate == name).then_some(*registration)
        }) else { return };
        if self.registry.status(registration) != Some(BackgroundRegistrationStatus::Active) {
            return;
        }
        let Some(function) = self.installed.borrow_mut().remove(&registration) else { return };
        let result = self.context.async_with(async move |ctx| {
            let result: rquickjs::Result<Persistent<Promise<'static>>> = (|| {
                let function: Function = function.restore(&ctx)?;
                let value: Value = function.call(())?;
                let promise: Object = ctx.globals().get("Promise")?;
                let resolve: Function = promise.get("resolve")?;
                let promise: Promise = resolve.call((This(promise), value))?;
                Ok(Persistent::save(&ctx, promise))
            })();
            result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
        }).await;
        match result {
            Ok(promise) => {
                let invocation = Uuid::new_v4();
                self.active_daemons.borrow_mut().insert(name.to_owned(), registration);
                self.invocations.borrow_mut().insert(invocation, BackgroundInvocation {
                    promise,
                    target: BackgroundInvocationTarget::Daemon(name.to_owned()),
                });
            }
            Err(error) => eprintln!("[Script background] daemon {name}: {error}"),
        }
    }

    async fn start_state_update(&self, registration: Uuid, name: String, instance: Uuid) {
        let Some(function) = self.installed.borrow().get(&registration).cloned() else { return };
        let bindings = self.bindings.clone();
        let registry = self.registry.clone();
        let session = self.session.clone();
        let element_name = name.clone();
        let result = self.context.async_with(async move |ctx| {
            let result: rquickjs::Result<Option<Persistent<Promise<'static>>>> = (|| {
                if session.is_stopped() || !registry.is_current(registration) { return Ok(None); }
                let function: Function = function.restore(&ctx)?;
                let receiver = bindings.element(&ctx, &element_name, instance)?;
                let value: Value = function.call((This(receiver),))?;
                let promise: Object = ctx.globals().get("Promise")?;
                let resolve: Function = promise.get("resolve")?;
                let promise: Promise = resolve.call((This(promise), value))?;
                Ok(Some(Persistent::save(&ctx, promise)))
            })();
            result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
        }).await;
        match result {
            Ok(Some(promise)) => {
                self.invocations.borrow_mut().insert(Uuid::new_v4(), BackgroundInvocation {
                    promise,
                    target: BackgroundInvocationTarget::UiStateUpdate(name),
                });
            }
            Ok(None) => {}
            Err(error) => eprintln!("[Script background] state update handler for UI element {name:?}: {error}"),
        }
    }

    async fn start_ui(
        &self,
        event: ScriptUiEvent,
        current: Box<dyn Fn() -> bool>,
        response: oneshot::Sender<Result<(), String>>,
    ) {
        let Some(registration) = self.ui.installed_handler_registration(&event) else {
            let _ = response.send(Ok(()));
            return;
        };
        let Some(function) = self.installed.borrow().get(&registration).cloned() else {
            let _ = response.send(Ok(()));
            return;
        };
        let ui = self.ui.clone();
        let bindings = self.bindings.clone();
        let event_for_call = event.clone();
        let result = self.context.async_with(async move |ctx| {
            let result: rquickjs::Result<Option<Persistent<Promise<'static>>>> = (|| {
                if !current() || ui.installed_handler_registration(&event_for_call) != Some(registration) {
                    return Ok(None);
                }
                let function: Function = function.restore(&ctx)?;
                let receiver = bindings.element(&ctx, &event_for_call.element_id, event_for_call.instance_id)?;
                let value: Value = function.call((This(receiver),))?;
                let promise: Object = ctx.globals().get("Promise")?;
                let resolve: Function = promise.get("resolve")?;
                let promise: Promise = resolve.call((This(promise), value))?;
                Ok(Some(Persistent::save(&ctx, promise)))
            })();
            result.map_err(|error| CaughtError::from_error(&ctx, error).to_string())
        }).await;
        match result {
            Ok(Some(promise)) => {
                self.invocations.borrow_mut().insert(Uuid::new_v4(), BackgroundInvocation {
                    promise,
                    target: BackgroundInvocationTarget::Ui { event, response },
                });
            }
            Ok(None) => { let _ = response.send(Ok(())); }
            Err(error) => { let _ = response.send(Err(format!(
                "{:?} handler for UI element {:?}: {error}", event.event, event.element_id,
            ))); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        global_state::GlobalState,
        script::{
            bindings::{Button, HostControls, MouseInput, MouseInputError},
            control::SessionControl,
            session::ScriptSession,
            transfer::FunctionDescriptor,
        },
        window::WindowControl,
    };
    use std::{cell::RefCell, rc::Rc, time::Duration};

    struct NoopMouse;

    impl MouseInput for NoopMouse {
        fn click_at(&mut self, _x: i32, _y: i32, _button: Button) -> Result<(), MouseInputError> { Ok(()) }
    }

    struct NoopWindow;

    impl WindowControl for NoopWindow {
        fn resize_client(&self, _width: i32, _height: i32) -> Result<(), String> { Ok(()) }
    }

    #[test]
    fn background_registry_invalidates_superseded_and_deleted_daemons() {
        let registry = BackgroundRegistry::new(SessionControl::standalone());
        let function = FunctionDescriptor {
            source: "function () {}".to_owned(),
            module_id: Some("test.js".to_owned()),
            line: Some(1),
            column: Some(1),
        };
        let first = registry.register(BackgroundTarget::Daemon("monitor".to_owned()), function.clone());
        assert!(registry.is_current(first));
        assert_eq!(registry.status(first), Some(BackgroundRegistrationStatus::Pending));
        assert!(registry.contains_daemon("monitor"));
        assert_eq!(registry.daemon_names(), vec!["monitor".to_owned()]);

        let second = registry.register(BackgroundTarget::Daemon("monitor".to_owned()), function);
        assert!(!registry.is_current(first));
        assert_eq!(registry.status(first), None);
        assert!(registry.is_current(second));

        registry.unregister(&BackgroundTarget::Daemon("monitor".to_owned()));
        assert!(!registry.is_current(second));
        assert_eq!(registry.status(second), None);
        assert!(!registry.contains_daemon("monitor"));
        assert!(registry.daemon_names().is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sleep_deadline_is_captured_before_the_host_future_is_polled() {
        GlobalState.delete("__background_deadline_created");
        GlobalState.delete("__background_deadline_done");
        let session = ScriptSession::new(r#"
            export default function() {
                rev.sleep(100).then(() => rev.global.__background_deadline_done = true);
                rev.global.__background_deadline_created = true;
            }
        "#).await.unwrap();
        let controls = HostControls {
            mouse: Rc::new(RefCell::new(NoopMouse)),
            window: Rc::new(NoopWindow),
            actions_paused: crate::app::ActionGate::default(),
        };
        session.invoke((), controls).await.unwrap();
        assert_eq!(GlobalState.get("__background_deadline_created").map(|value| value["value"].clone()), Some(serde_json::json!(true)));

        tokio::time::sleep(Duration::from_millis(150)).await;
        let drive = session.drive();
        tokio::pin!(drive);
        tokio::time::timeout(Duration::from_millis(50), async {
            loop {
                if GlobalState.get("__background_deadline_done").map(|value| value["value"].clone()) == Some(serde_json::json!(true)) {
                    break;
                }
                tokio::select! {
                    _ = &mut drive => panic!("main runtime driver stopped unexpectedly"),
                    _ = tokio::time::sleep(Duration::from_millis(1)) => {}
                }
            }
        }).await.unwrap();
        assert_eq!(GlobalState.get("__background_deadline_done").map(|value| value["value"].clone()), Some(serde_json::json!(true)));
        GlobalState.delete("__background_deadline_created");
        GlobalState.delete("__background_deadline_done");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn daemon_registry_requires_calls_for_registration_and_removal() {
        let session = ScriptSession::new(r#"
            export default function() {
                function assert(condition) { if (!condition) throw new Error('assertion failed'); }
                function throws(action) { let thrown = false; try { action(); } catch (_) { thrown = true; } assert(thrown); }
                assert(typeof rev.daemon === 'function');
                assert(rev.daemon('monitor', function() {}) === undefined);
                assert('monitor' in rev.daemon && Object.keys(rev.daemon).join() === 'monitor');
                throws(() => rev.daemon.monitor = function() {});
                throws(() => delete rev.daemon.monitor);
                throws(() => delete rev.daemon.absent);
                throws(() => Object.defineProperty(rev.daemon, 'monitor', { value: function() {} }));
                assert('monitor' in rev.daemon);
                for (const value of [undefined, false, 1, '', {}, []]) throws(() => rev.daemon('monitor', value));
                for (const name of ['', 1, null, Symbol()]) {
                    throws(() => rev.daemon(name, function() {}));
                    throws(() => rev.daemon(name, null));
                }
                assert('monitor' in rev.daemon && Object.keys(rev.daemon).join() === 'monitor');
                assert(rev.daemon('monitor', null) === undefined);
                assert(!('monitor' in rev.daemon) && Object.keys(rev.daemon).length === 0);
                assert(rev.daemon('monitor', null) === undefined);
                for (const name of ['name', 'length', 'prototype', '__proto__', 'constructor']) {
                    rev.daemon(name, function() {});
                    assert(name in rev.daemon && Object.keys(rev.daemon).join() === name);
                    rev.daemon(name, null);
                }
            }
        "#).await.unwrap();
        session.invoke((), HostControls {
            mouse: Rc::new(RefCell::new(NoopMouse)),
            window: Rc::new(NoopWindow),
            actions_paused: crate::app::ActionGate::default(),
        }).await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn two_runtime_daemon_runs_once_with_undefined_receiver() {
        GlobalState.delete("__background_daemon_runs");
        GlobalState.delete("__background_daemon_receiver");
        let session = ScriptSession::new(r#"
            export function afterLoad() {
                rev.daemon('monitor', function() {
                    "use strict";
                    rev.global.__background_daemon_runs = (rev.global.__background_daemon_runs ?? 0) + 1;
                    rev.global.__background_daemon_receiver = this === undefined;
                });
            }
            export default function() {}
        "#).await.unwrap();
        let controls = HostControls {
            mouse: Rc::new(RefCell::new(NoopMouse)),
            window: Rc::new(NoopWindow),
            actions_paused: crate::app::ActionGate::default(),
        };
        let mut background = Box::pin(session.drive_background(controls.clone()));
        session.run_after_load(controls).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if GlobalState.get("__background_daemon_runs").map(|value| value["value"].clone()) == Some(serde_json::json!(1)) {
                    break;
                }
                tokio::select! {
                    _ = &mut background => panic!("background service stopped unexpectedly"),
                    _ = tokio::time::sleep(Duration::from_millis(1)) => {}
                }
            }
        }).await.unwrap();
        assert_eq!(GlobalState.get("__background_daemon_receiver").map(|value| value["value"].clone()), Some(serde_json::json!(true)));
        tokio::select! {
            _ = &mut background => panic!("background service stopped unexpectedly"),
            _ = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
        assert_eq!(GlobalState.get("__background_daemon_runs").map(|value| value["value"].clone()), Some(serde_json::json!(1)));
        session.terminate();
        tokio::time::timeout(Duration::from_secs(1), background).await.unwrap();
        GlobalState.delete("__background_daemon_runs");
        GlobalState.delete("__background_daemon_receiver");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn native_values_survive_main_background_global_exchange() {
        let session = ScriptSession::new(r#"
            const initial = new BigNum('1e300');
            const color = Color.fromRgb(100.5, 40.25, 20.75, 127.5);
            export function afterLoad() {
                rev.global.__native_bignum_exchange = { amount: initial, text: '1e300', color };
                rev.daemon('nativeBigNum', function() {
                    const value = rev.global.__native_bignum_exchange;
                    if (!(value.amount instanceof BigNum) || value.text !== '1e300') throw Error('background value');
                    if (!(value.color instanceof Color) || !value.color.equals(Color.fromRgb(100.5, 40.25, 20.75, 127.5))) throw Error('background color');
                    rev.global.__native_bignum_exchange = { amount: value.amount.mul(2), text: value.text, color: value.color.brightness(2) };
                    rev.global.__native_bignum_done = true;
                });
            }
            export default function() {
                const value = rev.global.__native_bignum_exchange;
                if (!(value.amount instanceof BigNum) || !value.amount.eq(new BigNum('2e300'))) throw Error('main value');
                if (value.text !== '1e300') throw Error('string changed');
                if (!(value.color instanceof Color) || !value.color.equals(Color.fromRgb(201, 80.5, 41.5, 127.5))) throw Error('main color');
                delete rev.global.__native_bignum_exchange;
                delete rev.global.__native_bignum_done;
            }
        "#).await.unwrap();
        let controls = HostControls {
            mouse: Rc::new(RefCell::new(NoopMouse)),
            window: Rc::new(NoopWindow),
            actions_paused: crate::app::ActionGate::default(),
        };
        let mut background = Box::pin(session.drive_background(controls.clone()));
        session.run_after_load(controls.clone()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if GlobalState.get("__native_bignum_done").is_some() { break; }
                tokio::select! {
                    _ = &mut background => panic!("background service stopped unexpectedly"),
                    _ = tokio::time::sleep(Duration::from_millis(1)) => {}
                }
            }
        }).await.unwrap();
        session.invoke((), controls).await.unwrap();
        session.terminate();
        tokio::time::timeout(Duration::from_secs(1), background).await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn transferred_callbacks_from_imported_module_run_in_background() {
        use crate::bridge::{ScriptUiEvent, ScriptUiEventKind, ScriptUiPublisher};
        let root = std::env::temp_dir().join(format!("rev-idle-background-transfer-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("dependency.js"), "export const value = 42; export function monitor() { rev.global.__background_imported_value = value; } export function clicked() { this.text = String(value); }").unwrap();
        GlobalState.delete("__background_imported_value");
        let publisher = ScriptUiPublisher::default();
        let snapshots = publisher.subscribe();
        let session = ScriptSession::new_with_connection_and_control(
            r#"import { monitor, clicked } from './dependency.js';
                export function afterLoad() {
                    rev.daemon('monitor', monitor);
                    rev.ui('button', { text: 'ready' });
                    rev.ui.button.setOnClick(clicked);
                }
                export default function() {}"#,
            &root.join("entry.js").to_string_lossy(),
            WsConnection::disconnected_for_test(),
            SessionControl::standalone(),
            publisher,
        )
        .await
        .unwrap();
        let controls = HostControls {
            mouse: Rc::new(RefCell::new(NoopMouse)),
            window: Rc::new(NoopWindow),
            actions_paused: crate::app::ActionGate::default(),
        };
        let mut background = Box::pin(session.drive_background(controls.clone()));
        session.run_after_load(controls.clone()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if GlobalState.get("__background_imported_value").map(|value| value["value"].clone()) == Some(serde_json::json!(42)) {
                    break;
                }
                tokio::select! {
                    _ = &mut background => panic!("background service stopped unexpectedly"),
                    _ = tokio::time::sleep(Duration::from_millis(1)) => {}
                }
            }
        }).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if snapshots.borrow().elements[0].events.iter().any(|event| matches!(event, ScriptUiEventKind::Click)) {
                    break;
                }
                tokio::select! {
                    _ = &mut background => panic!("background service stopped unexpectedly"),
                    _ = tokio::time::sleep(Duration::from_millis(1)) => {}
                }
            }
        }).await.unwrap();
        let snapshot = snapshots.borrow().clone();
        tokio::select! {
            _ = &mut background => panic!("background service stopped unexpectedly"),
            result = session.dispatch_ui_event(ScriptUiEvent {
                session_id: snapshot.session_id.unwrap(),
                element_id: "button".to_owned(),
                instance_id: snapshot.elements[0].instance_id,
                events_version: snapshot.elements[0].events_version,
                event: ScriptUiEventKind::Click,
            }, controls.clone()) => result.unwrap(),
        }
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if snapshots.borrow().elements[0].text == "42" {
                    break;
                }
                tokio::select! {
                    _ = &mut background => panic!("background service stopped unexpectedly"),
                    _ = tokio::time::sleep(Duration::from_millis(1)) => {}
                }
            }
        }).await.unwrap();
        session.terminate();
        tokio::time::timeout(Duration::from_secs(1), background).await.unwrap();
        GlobalState.delete("__background_imported_value");
        std::fs::remove_file(root.join("dependency.js")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn two_runtime_daemon_retirement_keeps_latest_replacement_and_releases_after_rejection() {
        let session = ScriptSession::new(r#"
            export function afterLoad() {
                rev.global.__daemon_retirement_order = [];
                rev.global.__daemon_retirement_old_release = false;
                rev.global.__daemon_retirement_child_release = false;
                rev.global.__daemon_retirement_probe = 0;
                rev.daemon('worker', async function() {
                    rev.global.__daemon_retirement_order = [...rev.global.__daemon_retirement_order, 'old-start'];
                    while (!rev.global.__daemon_retirement_old_release) await rev.sleep(1);
                    rev.global.__daemon_retirement_order = [...rev.global.__daemon_retirement_order, 'old-end'];
                    throw new Error('expected retiring daemon rejection');
                });
            }
            export default function() {
                const phase = rev.global.__daemon_retirement_phase;
                if (phase === 1) {
                    rev.daemon('worker', null);
                    if ('worker' in rev.daemon) throw new Error('deleted daemon still registered');
                    rev.daemon('worker', function() {
                        rev.global.__daemon_retirement_order = [...rev.global.__daemon_retirement_order, 'superseded'];
                    });
                    rev.daemon('worker', function() {
                        rev.global.__daemon_retirement_order = [...rev.global.__daemon_retirement_order, 'latest'];
                        (async function() {
                            while (!rev.global.__daemon_retirement_child_release) await rev.sleep(1);
                            rev.global.__daemon_retirement_order = [...rev.global.__daemon_retirement_order, 'child-end'];
                        })();
                    });
                    rev.daemon('probe', async function() {
                        for (let tick = 0; tick < 5; tick++) {
                            await rev.sleep(1);
                            rev.global.__daemon_retirement_probe = tick + 1;
                        }
                    });
                } else if (phase === 2) {
                    rev.global.__daemon_retirement_old_release = true;
                } else if (phase === 3) {
                    rev.daemon('worker', function() {
                        rev.global.__daemon_retirement_order = [...rev.global.__daemon_retirement_order, 'after'];
                    });
                } else if (phase === 4) {
                    rev.global.__daemon_retirement_child_release = true;
                }
            }
        "#).await.unwrap();
        let controls = HostControls {
            mouse: Rc::new(RefCell::new(NoopMouse)),
            window: Rc::new(NoopWindow),
            actions_paused: crate::app::ActionGate::default(),
        };
        let mut background = Box::pin(session.drive_background(controls.clone()));
        session.run_after_load(controls.clone()).await.unwrap();
        for phase in 0..5 {
            if phase != 0 {
                GlobalState.set("__daemon_retirement_phase".to_owned(), serde_json::json!({ "value": phase }));
                session.invoke((), controls.clone()).await.unwrap();
            }
            let expected = match phase {
                0 | 1 => serde_json::json!(["old-start"]),
                2 => serde_json::json!(["old-start", "old-end", "latest"]),
                3 => serde_json::json!(["old-start", "old-end", "latest", "after"]),
                _ => serde_json::json!(["old-start", "old-end", "latest", "after", "child-end"]),
            };
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if if phase == 1 {
                        GlobalState.get("__daemon_retirement_probe").map(|value| value["value"].clone()) == Some(serde_json::json!(5))
                    } else {
                        GlobalState.get("__daemon_retirement_order").map(|value| value["value"].clone()).is_some_and(|order| {
                            order.as_array().unwrap().last() == expected.as_array().unwrap().last()
                        })
                    } { break; }
                    tokio::select! {
                        _ = &mut background => panic!("background service stopped unexpectedly"),
                        _ = tokio::time::sleep(Duration::from_millis(1)) => {}
                    }
                }
            }).await.unwrap();
            assert_eq!(GlobalState.get("__daemon_retirement_order").map(|value| value["value"].clone()), Some(expected));
        }
        session.terminate();
        tokio::time::timeout(Duration::from_secs(1), background).await.unwrap();
        for key in ["__daemon_retirement_order", "__daemon_retirement_old_release", "__daemon_retirement_child_release", "__daemon_retirement_probe", "__daemon_retirement_phase"] {
            GlobalState.delete(key);
        }
    }
}
