use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use futures_util::{FutureExt, SinkExt, StreamExt};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{
        atomic::{AtomicU8, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, watch},
};
use tokio_tungstenite::{
    tungstenite::{protocol::Role, Message},
    WebSocketStream,
};
use uuid::Uuid;

const OUTBOUND_CAPACITY: usize = 256;
const TIMED_OUT_CAPACITY: usize = 1_024;
const RESPONSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
const RECONNECT_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

pub(crate) trait Packet: Serialize + DeserializeOwned + Send + 'static {
    const TYPE: &'static str;
}

pub(crate) trait Requestable: Packet {
    type Response: Packet;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WsError {
    NotConnected,
    Closed,
    Timeout,
    Remote(String),
    UnexpectedResponse { expected: &'static str, actual: String },
    Protocol(String),
    OutboundFull,
    AlreadyResponded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConnectionEvent {
    Connected,
    Disconnected,
}

impl fmt::Display for WsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WsError {}

#[derive(Clone, Deserialize, Serialize)]
struct Envelope {
    uuid: Uuid,
    #[serde(rename = "type")]
    packet_type: String,
    payload: Value,
}

impl Envelope {
    fn new<P: Serialize>(uuid: Uuid, packet_type: &str, payload: P) -> Result<Self, serde_json::Error> {
        Ok(Self {
            uuid,
            packet_type: packet_type.to_owned(),
            payload: serde_json::to_value(payload)?,
        })
    }
}

#[derive(Clone)]
struct Active {
    generation: u64,
    outbound: mpsc::Sender<Outbound>,
}

struct Outbound {
    envelope: Envelope,
    written: oneshot::Sender<Result<(), WsError>>,
}

struct Pending {
    generation: u64,
    expected: &'static str,
    response: oneshot::Sender<Result<Value, WsError>>,
}

struct PendingGuard {
    inner: Arc<Inner>,
    generation: u64,
    uuid: Uuid,
    armed: bool,
}

impl PendingGuard {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        if self.armed && self.inner.remove_pending_and_tombstone(self.uuid, self.generation) {
            send_best_effort(
                &self.inner,
                self.generation,
                Envelope::new(self.uuid, "Cancel", Value::Null).unwrap(),
            );
        }
    }
}

type HandlerFuture = Pin<Box<dyn Future<Output = Result<(), WsError>> + Send>>;
type Handler = Arc<dyn Fn(PacketContext, Value) -> HandlerFuture + Send + Sync>;
type HandlerTask = Pin<Box<dyn Future<Output = ()> + Send>>;

const INBOUND_CANCELLED: u8 = 1;
const INBOUND_STARTED: u8 = 2;
const INBOUND_RESPONDED: u8 = 4;

struct Inbound {
    cancellation: watch::Sender<bool>,
    state: AtomicU8,
}

impl Inbound {
    fn cancel(&self) {
        loop {
            let state = self.state.load(Ordering::Acquire);
            if state & INBOUND_CANCELLED != 0 {
                return;
            }
            if self
                .state
                .compare_exchange(
                    state,
                    state | INBOUND_CANCELLED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                self.cancellation.send(true).ok();
                return;
            }
        }
    }

    fn try_start(&self) -> bool {
        loop {
            let state = self.state.load(Ordering::Acquire);
            if state & INBOUND_CANCELLED != 0 || state & INBOUND_STARTED != 0 {
                return false;
            }
            if self
                .state
                .compare_exchange(
                    state,
                    state | INBOUND_STARTED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return true;
            }
        }
    }

    fn reserve_response(&self) -> Result<(), WsError> {
        loop {
            let state = self.state.load(Ordering::Acquire);
            if state & INBOUND_CANCELLED != 0 {
                return Err(WsError::Closed);
            }
            if state & INBOUND_RESPONDED != 0 {
                return Err(WsError::AlreadyResponded);
            }
            if self
                .state
                .compare_exchange(
                    state,
                    state | INBOUND_RESPONDED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::Acquire) & INBOUND_CANCELLED != 0
    }

    fn has_responded(&self) -> bool {
        self.state.load(Ordering::Acquire) & INBOUND_RESPONDED != 0
    }
}

struct InboundGuard {
    inner: Arc<Inner>,
    generation: u64,
    uuid: Uuid,
    inbound: Arc<Inbound>,
}

impl Drop for InboundGuard {
    fn drop(&mut self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state
            .inbound
            .get(&(self.generation, self.uuid))
            .is_some_and(|current| Arc::ptr_eq(current, &self.inbound))
        {
            state.inbound.remove(&(self.generation, self.uuid));
        }
    }
}

struct State {
    active: Option<Active>,
    connection_events: Option<mpsc::UnboundedSender<ConnectionEvent>>,
    next_generation: u64,
    pending: HashMap<Uuid, Pending>,
    timed_out: VecDeque<Uuid>,
    timed_out_set: HashSet<Uuid>,
    handlers: HashMap<&'static str, Handler>,
    inbound: HashMap<(u64, Uuid), Arc<Inbound>>,
    shutting_down: bool,
}

struct Inner {
    state: Mutex<State>,
    generation: watch::Sender<u64>,
    _generation_receiver: watch::Receiver<u64>,
    reported: AtomicUsize,
}

impl Inner {
    fn new() -> Self {
        let (generation, generation_receiver) = watch::channel(0);
        Self {
            state: Mutex::new(State {
                active: None,
                connection_events: None,
                next_generation: 0,
                pending: HashMap::new(),
                timed_out: VecDeque::new(),
                timed_out_set: HashSet::new(),
                handlers: HashMap::new(),
                inbound: HashMap::new(),
                shutting_down: false,
            }),
            generation,
            _generation_receiver: generation_receiver,
            reported: AtomicUsize::new(0),
        }
    }

    fn active(&self) -> Result<Active, WsError> {
        let state = self.state.lock().unwrap();
        if state.shutting_down {
            return Err(WsError::NotConnected);
        }
        state.active.clone().ok_or(WsError::NotConnected)
    }

    fn activate(&self, outbound: mpsc::Sender<Outbound>) -> u64 {
        let mut state = self.state.lock().unwrap();
        state.next_generation += 1;
        let generation = state.next_generation;
        state.active = Some(Active { generation, outbound });
        self.generation.send(generation).ok();
        if let Some(events) = state.connection_events.as_ref() {
            events.send(ConnectionEvent::Connected).ok();
        }
        generation
    }

    fn is_active(&self, generation: u64) -> bool {
        self.state
            .lock()
            .unwrap()
            .active
            .as_ref()
            .is_some_and(|active| active.generation == generation)
    }

    fn remove_pending(&self, uuid: Uuid, generation: u64) -> bool {
        let mut state = self.state.lock().unwrap();
        if state
            .pending
            .get(&uuid)
            .is_some_and(|pending| pending.generation == generation)
        {
            state.pending.remove(&uuid);
            true
        } else {
            false
        }
    }

    fn remove_pending_and_tombstone(&self, uuid: Uuid, generation: u64) -> bool {
        let mut state = self.state.lock().unwrap();
        if state
            .pending
            .get(&uuid)
            .is_some_and(|pending| pending.generation == generation)
        {
            state.pending.remove(&uuid);
            state.timed_out.push_back(uuid);
            state.timed_out_set.insert(uuid);
            while state.timed_out.len() > TIMED_OUT_CAPACITY {
                if let Some(oldest) = state.timed_out.pop_front() {
                    state.timed_out_set.remove(&oldest);
                }
            }
            true
        } else {
            false
        }
    }

    fn report(&self, error: WsError) {
        self.reported.fetch_add(1, Ordering::Relaxed);
        eprintln!("[WebSocket] {error}");
    }

    #[cfg(test)]
    fn report_count(&self) -> usize {
        self.reported.load(Ordering::Relaxed)
    }

    fn close_generation(&self, generation: u64) {
        let (pending, inbound) = {
            let mut state = self.state.lock().unwrap();
            if state
                .active
                .as_ref()
                .is_some_and(|active| active.generation == generation)
            {
                state.active = None;
                self.generation.send(0).ok();
                if let Some(events) = state.connection_events.as_ref() {
                    events.send(ConnectionEvent::Disconnected).ok();
                }
                state.timed_out.clear();
                state.timed_out_set.clear();
            }
            let pending = state
                .pending
                .keys()
                .filter(|uuid| {
                    state
                        .pending
                        .get(uuid)
                        .is_some_and(|pending| pending.generation == generation)
                })
                .copied()
                .collect::<Vec<_>>();
            let pending = pending
                .into_iter()
                .filter_map(|uuid| state.pending.remove(&uuid))
                .collect::<Vec<_>>();
            let inbound = state
                .inbound
                .keys()
                .filter(|(inbound_generation, _)| *inbound_generation == generation)
                .copied()
                .collect::<Vec<_>>();
            let inbound = inbound
                .into_iter()
                .filter_map(|key| state.inbound.remove(&key))
                .collect::<Vec<_>>();
            (pending, inbound)
        };
        for pending in pending {
            pending.response.send(Err(WsError::Closed)).ok();
        }
        for cancellation in inbound {
            cancellation.cancel();
        }
    }

    fn close_all(&self) {
        let (pending, inbound) = {
            let mut state = self.state.lock().unwrap();
            state.shutting_down = true;
            if state.active.take().is_some() {
                self.generation.send(0).ok();
                if let Some(events) = state.connection_events.as_ref() {
                    events.send(ConnectionEvent::Disconnected).ok();
                }
            }
            state.connection_events = None;
            state.timed_out.clear();
            state.timed_out_set.clear();
            let pending = state.pending.drain().map(|(_, pending)| pending).collect::<Vec<_>>();
            let inbound = state.inbound.drain().map(|(_, cancellation)| cancellation).collect::<Vec<_>>();
            (pending, inbound)
        };
        for pending in pending {
            pending.response.send(Err(WsError::Closed)).ok();
        }
        for cancellation in inbound {
            cancellation.cancel();
        }
    }

    async fn enqueue(&self, active: Active, envelope: Envelope) -> Result<(), WsError> {
        if !self.is_active(active.generation) {
            return Err(WsError::Closed);
        }
        let (written, result) = oneshot::channel();
        active
            .outbound
            .send(Outbound { envelope, written })
            .await
            .map_err(|_| WsError::Closed)?;
        result.await.unwrap_or(Err(WsError::Closed))
    }
}

#[derive(Clone)]
pub(crate) struct PacketContext {
    inner: Arc<Inner>,
    generation: u64,
    uuid: Uuid,
    inbound: Arc<Inbound>,
}

impl PacketContext {
    pub(crate) async fn send<P: Packet>(&self, packet: P) -> Result<(), WsError> {
        let active = self.inner.active().map_err(|_| WsError::Closed)?;
        if active.generation != self.generation {
            return Err(WsError::Closed);
        }
        let envelope = Envelope::new(self.uuid, P::TYPE, packet)
            .map_err(|error| WsError::Protocol(error.to_string()))?;
        self.inbound.reserve_response()?;
        self.inner.enqueue(active, envelope).await
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.inbound.is_cancelled()
    }

    pub(crate) async fn cancelled(&self) {
        let mut cancelled = self.inbound.cancellation.subscribe();
        if self.inbound.is_cancelled() {
            return;
        }
        cancelled.changed().await.ok();
    }
}

#[derive(Clone)]
pub(crate) struct WsConnection {
    inner: Arc<Inner>,
    shutdown: watch::Sender<bool>,
    supervisor: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    response_timeout: Duration,
}

impl WsConnection {
    #[cfg(test)]
    pub(crate) fn disconnected_for_test() -> Self {
        let (shutdown, _) = watch::channel(false);
        Self {
            inner: Arc::new(Inner::new()),
            shutdown,
            supervisor: Arc::new(Mutex::new(None)),
            response_timeout: RESPONSE_TIMEOUT,
        }
    }

    pub(crate) fn connect(address: SocketAddr) -> Self {
        Self::connect_with_durations(address, RECONNECT_DELAY, RESPONSE_TIMEOUT)
    }

    #[cfg(test)]
    pub(crate) fn connect_for_test(
        address: SocketAddr,
        reconnect_delay: Duration,
        response_timeout: Duration,
    ) -> Self {
        Self::connect_with_durations(address, reconnect_delay, response_timeout)
    }

    fn connect_with_durations(
        address: SocketAddr,
        reconnect_delay: Duration,
        response_timeout: Duration,
    ) -> Self {
        let inner = Arc::new(Inner::new());
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let supervisor = tokio::spawn(supervise(
            inner.clone(),
            address,
            reconnect_delay,
            shutdown_receiver,
        ));
        Self {
            inner,
            shutdown,
            supervisor: Arc::new(Mutex::new(Some(supervisor))),
            response_timeout,
        }
    }

    pub(crate) async fn shutdown(&self) {
        self.shutdown.send(true).ok();
        let active_generation = self
            .inner
            .state
            .lock()
            .unwrap()
            .active
            .as_ref()
            .map(|active| active.generation);
        if let Some(active_generation) = active_generation {
            self.inner.close_generation(active_generation);
        }
        self.inner.close_all();
        let supervisor = self.supervisor.lock().unwrap().take();
        if let Some(supervisor) = supervisor {
            supervisor.await.ok();
        }
    }

    pub(crate) fn connection_generation(&self) -> watch::Receiver<u64> {
        self.inner.generation.subscribe()
    }

    pub(crate) fn connection_events(&self) -> mpsc::UnboundedReceiver<ConnectionEvent> {
        let (events, receiver) = mpsc::unbounded_channel();
        self.inner.state.lock().unwrap().connection_events = Some(events);
        receiver
    }

    #[cfg(test)]
    fn report_count_for_test(&self) -> usize {
        self.inner.report_count()
    }

    pub(crate) async fn send<P: Packet>(&self, packet: P) -> Result<(), WsError> {
        let active = self.inner.active()?;
        let envelope = Envelope::new(Uuid::new_v4(), P::TYPE, packet)
            .map_err(|error| WsError::Protocol(error.to_string()))?;
        self.inner.enqueue(active, envelope).await
    }

    pub(crate) fn try_send<P: Packet>(&self, packet: P) -> Result<(), WsError> {
        let active = self.inner.active()?;
        let envelope = Envelope::new(Uuid::new_v4(), P::TYPE, packet)
            .map_err(|error| WsError::Protocol(error.to_string()))?;
        if !self.inner.is_active(active.generation) {
            return Err(WsError::Closed);
        }
        let (written, _) = oneshot::channel();
        active
            .outbound
            .try_send(Outbound { envelope, written })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => WsError::OutboundFull,
                mpsc::error::TrySendError::Closed(_) => WsError::Closed,
            })
    }

    pub(crate) fn handler<P, F, Fut>(&self, handler: F) -> Result<(), WsError>
    where
        P: Packet,
        F: Fn(PacketContext, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), WsError>> + Send + 'static,
    {
        let handler = Arc::new(handler);
        let erased: Handler = Arc::new(move |context, payload| {
            let handler = handler.clone();
            Box::pin(async move {
                let packet = serde_json::from_value(payload)
                    .map_err(|error| WsError::Protocol(error.to_string()))?;
                handler(context, packet).await
            })
        });
        let mut state = self.inner.state.lock().unwrap();
        if state.handlers.contains_key(P::TYPE) {
            return Err(WsError::Protocol(format!(
                "handler already registered for {}",
                P::TYPE
            )));
        }
        state.handlers.insert(P::TYPE, erased);
        Ok(())
    }

    pub(crate) async fn request<P: Requestable>(&self, packet: P) -> Result<P::Response, WsError> {
        let active = self.inner.active()?;
        let uuid = Uuid::new_v4();
        let envelope = Envelope::new(uuid, P::TYPE, packet)
            .map_err(|error| WsError::Protocol(error.to_string()))?;
        let (response, mut response_receiver) = oneshot::channel();
        {
            let mut state = self.inner.state.lock().unwrap();
            if state
                .active
                .as_ref()
                .is_none_or(|current| current.generation != active.generation)
            {
                return Err(WsError::Closed);
            }
            state.pending.insert(
                uuid,
                Pending {
                    generation: active.generation,
                    expected: P::Response::TYPE,
                    response,
                },
            );
        }
        let mut pending_guard = PendingGuard {
            inner: self.inner.clone(),
            generation: active.generation,
            uuid,
            armed: true,
        };
        if let Err(error) = self.inner.enqueue(active.clone(), envelope).await {
            self.inner.remove_pending(uuid, active.generation);
            pending_guard.disarm();
            return Err(error);
        }
        match tokio::time::timeout(self.response_timeout, &mut response_receiver).await {
            Ok(Ok(Ok(payload))) => {
                pending_guard.disarm();
                serde_json::from_value(payload).map_err(|error| WsError::Protocol(error.to_string()))
            }
            Ok(Ok(Err(error))) => {
                pending_guard.disarm();
                Err(error)
            }
            Ok(Err(_)) => {
                pending_guard.disarm();
                Err(WsError::Closed)
            }
            Err(_) => {
                let timed_out = self
                    .inner
                    .remove_pending_and_tombstone(uuid, active.generation);
                pending_guard.disarm();
                if timed_out {
                    send_best_effort(
                        &self.inner,
                        active.generation,
                        Envelope::new(uuid, "Cancel", Value::Null).unwrap(),
                    );
                    Err(WsError::Timeout)
                } else {
                    match response_receiver.await {
                        Ok(Ok(payload)) => serde_json::from_value(payload)
                            .map_err(|error| WsError::Protocol(error.to_string())),
                        Ok(Err(error)) => Err(error),
                        Err(_) => Err(WsError::Closed),
                    }
                }
            }
        }
    }
}

async fn supervise(
    inner: Arc<Inner>,
    address: SocketAddr,
    reconnect_delay: Duration,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            break;
        }
        let attempt_started = tokio::time::Instant::now();
        let stream = tokio::select! {
            stream = tokio::time::timeout(reconnect_delay, TcpStream::connect(address)) => {
                stream.ok().and_then(Result::ok)
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
        };
        if *shutdown.borrow() {
            break;
        }
        let connected = stream.is_some();
        if let Some(stream) = stream {
            run_session(inner.clone(), stream, shutdown.clone()).await;
        }
        if *shutdown.borrow() {
            break;
        }
        let wait = if connected {
            reconnect_delay
        } else {
            reconnect_delay.saturating_sub(attempt_started.elapsed())
        };
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }
    inner.close_all();
}

async fn run_session(
    inner: Arc<Inner>,
    stream: TcpStream,
    mut shutdown: watch::Receiver<bool>,
) {
    let socket = WebSocketStream::from_raw_socket(stream, Role::Client, None).await;
    let (sink, stream) = socket.split();
    let (outbound, outbound_receiver) = mpsc::channel(OUTBOUND_CAPACITY);
    let generation = inner.activate(outbound);
    let (session_close, session_close_receiver) = watch::channel(false);
    let (handler_sender, mut handler_receiver) = mpsc::unbounded_channel();
    let mut writer = tokio::spawn(run_writer(
        sink,
        outbound_receiver,
        session_close_receiver.clone(),
        shutdown.clone(),
    ));
    let mut reader = tokio::spawn(run_reader(
        inner.clone(),
        generation,
        stream,
        session_close_receiver,
        shutdown.clone(),
        handler_sender,
    ));
    let mut writer_finished = false;
    let mut reader_finished = false;
    let mut handler_tasks = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            writer_result = &mut writer => {
                writer_finished = true;
                if let Err(error) = writer_result {
                    inner.report(WsError::Protocol(format!("writer task failed: {error}")));
                }
                break;
            }
            reader_result = &mut reader => {
                reader_finished = true;
                if let Err(error) = reader_result {
                    inner.report(WsError::Protocol(format!("reader task failed: {error}")));
                }
                break;
            }
            task = handler_receiver.recv() => {
                if let Some(task) = task {
                    handler_tasks.spawn(task);
                } else {
                    break;
                }
            }
            result = handler_tasks.join_next(), if !handler_tasks.is_empty() => {
                if let Some(Err(error)) = result {
                    inner.report(WsError::Protocol(format!("handler task failed: {error}")));
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    session_close.send(true).ok();
                    break;
                }
            }
        }
    }
    session_close.send(true).ok();
    inner.close_generation(generation);
    handler_tasks.abort_all();
    if !writer_finished {
        if let Err(error) = writer.await {
            inner.report(WsError::Protocol(format!("writer task failed: {error}")));
        }
    }
    if !reader_finished {
        if let Err(error) = reader.await {
            inner.report(WsError::Protocol(format!("reader task failed: {error}")));
        }
    }
    inner.close_generation(generation);
    while handler_receiver.try_recv().is_ok() {}
    while let Some(result) = handler_tasks.try_join_next() {
        if let Err(error) = result {
            inner.report(WsError::Protocol(format!("handler task failed: {error}")));
        }
    }
}

async fn run_writer(
    mut sink: futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
    mut outbound: mpsc::Receiver<Outbound>,
    mut session_close: watch::Receiver<bool>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        let outbound = tokio::select! {
            outbound = outbound.recv() => outbound,
            changed = session_close.changed() => {
                if changed.is_err() || *session_close.borrow() {
                    break;
                }
                continue;
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
        };
        let Some(outbound) = outbound else {
            break;
        };
        if *session_close.borrow() || *shutdown.borrow() {
            outbound.written.send(Err(WsError::Closed)).ok();
            break;
        }
        let json = match serde_json::to_string(&outbound.envelope) {
            Ok(json) => json,
            Err(error) => {
                outbound
                    .written
                    .send(Err(WsError::Protocol(error.to_string())))
                    .ok();
                continue;
            }
        };
        let result = tokio::select! {
            result = sink.send(Message::Text(json.into())) => result,
            changed = session_close.changed() => {
                if changed.is_err() || *session_close.borrow() {
                    outbound.written.send(Err(WsError::Closed)).ok();
                    break;
                }
                continue;
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    outbound.written.send(Err(WsError::Closed)).ok();
                    break;
                }
                continue;
            }
        };
        match result {
            Ok(()) => outbound.written.send(Ok(())).ok(),
            Err(_) => {
                outbound.written.send(Err(WsError::Closed)).ok();
                break;
            }
        };
    }
    while let Ok(outbound) = outbound.try_recv() {
        outbound.written.send(Err(WsError::Closed)).ok();
    }
}

async fn run_reader(
    inner: Arc<Inner>,
    generation: u64,
    mut stream: futures_util::stream::SplitStream<WebSocketStream<TcpStream>>,
    mut session_close: watch::Receiver<bool>,
    mut shutdown: watch::Receiver<bool>,
    handler_sender: mpsc::UnboundedSender<HandlerTask>,
) {
    loop {
        tokio::select! {
            message = stream.next() => {
                match message {
                    Some(Ok(Message::Text(message))) => {
                        match serde_json::from_str::<Envelope>(message.as_ref()) {
                            Ok(envelope) => {
                                if let Some(task) = handle_envelope(inner.clone(), generation, envelope).await {
                                    if handler_sender.send(task).is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(error) => inner.report(WsError::Protocol(format!("malformed packet: {error}"))),
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
            changed = session_close.changed() => {
                if changed.is_err() || *session_close.borrow() {
                    break;
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }
}

async fn handle_envelope(
    inner: Arc<Inner>,
    generation: u64,
    envelope: Envelope,
) -> Option<HandlerTask> {
    if !inner
        .state
        .lock()
        .unwrap()
        .active
        .as_ref()
        .is_some_and(|active| active.generation == generation)
    {
        return None;
    }
    if envelope.packet_type == "Cancel" {
        let cancellation = inner
            .state
            .lock()
            .unwrap()
            .inbound
            .get(&(generation, envelope.uuid))
            .cloned();
        if let Some(cancellation) = cancellation {
            cancellation.cancel();
        }
        return None;
    }

    let pending = {
        let mut state = inner.state.lock().unwrap();
        state
            .pending
            .get(&envelope.uuid)
            .is_some_and(|pending| pending.generation == generation)
            .then(|| state.pending.remove(&envelope.uuid).unwrap())
    };
    if let Some(pending) = pending {
        let result = if envelope.packet_type == "RemoteError" {
            serde_json::from_value::<RemoteError>(envelope.payload)
                .map(|error| Err(WsError::Remote(error.message)))
                .unwrap_or_else(|error| Err(WsError::Protocol(error.to_string())))
        } else if envelope.packet_type != pending.expected {
            Err(WsError::UnexpectedResponse {
                expected: pending.expected,
                actual: envelope.packet_type,
            })
        } else {
            Ok(envelope.payload)
        };
        pending.response.send(result).ok();
        return None;
    }

    if envelope.packet_type == "RemoteError" {
        match serde_json::from_value::<RemoteError>(envelope.payload) {
            Ok(error) => inner.report(WsError::Remote(error.message)),
            Err(error) => inner.report(WsError::Protocol(error.to_string())),
        }
        return None;
    }

    {
        let state = inner.state.lock().unwrap();
        if state.timed_out_set.contains(&envelope.uuid) {
            return None;
        }
    }

    let handler = inner
        .state
        .lock()
        .unwrap()
        .handlers
        .get(envelope.packet_type.as_str())
        .cloned();
    let Some(handler) = handler else {
        let task = Box::pin(async move {
            if let Err(error) = send_required(
                &inner,
                generation,
                Envelope::new(
                    envelope.uuid,
                    "RemoteError",
                    RemoteError {
                        message: format!("unknown packet type: {}", envelope.packet_type),
                    },
                )
                .unwrap(),
            )
            .await
            {
                inner.report(error);
            }
        });
        return Some(task);
    };

    let (cancellation, _) = watch::channel(false);
    let inbound = Arc::new(Inbound {
        cancellation,
        state: AtomicU8::new(0),
    });
    let inserted = {
        let mut state = inner.state.lock().unwrap();
        if !state
            .active
            .as_ref()
            .is_some_and(|active| active.generation == generation)
            || state.inbound.contains_key(&(generation, envelope.uuid))
        {
            false
        } else {
            state
                .inbound
                .insert((generation, envelope.uuid), inbound.clone());
            true
        }
    };
    if !inserted {
        return None;
    }
    let context = PacketContext {
        inner: inner.clone(),
        generation,
        uuid: envelope.uuid,
        inbound: inbound.clone(),
    };
    let task = Box::pin(async move {
        let _inbound_guard = InboundGuard {
            inner: inner.clone(),
            generation,
            uuid: envelope.uuid,
            inbound: inbound.clone(),
        };
        tokio::task::yield_now().await;
        if !inbound.try_start() {
            return;
        }
        let result = std::panic::AssertUnwindSafe(async {
            handler(context.clone(), envelope.payload).await
        })
        .catch_unwind()
        .await;
        let error = match result {
            Ok(result) => result.err(),
            Err(panic) => {
                let message = panic
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_owned())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_owned());
                Some(WsError::Protocol(format!("handler panicked: {message}")))
            }
        };
        if let Some(error) = error {
            if !inbound.has_responded() {
                if let Err(send_error) = send_required(
                    &inner,
                    generation,
                    Envelope::new(
                        envelope.uuid,
                        "RemoteError",
                        RemoteError {
                            message: error.to_string(),
                        },
                    )
                    .unwrap(),
                )
                .await
                {
                    inner.report(send_error);
                }
            } else {
                inner.report(error);
            }
        }
    });
    Some(task)
}

#[derive(Deserialize, Serialize)]
struct RemoteError {
    message: String,
}

fn send_best_effort(inner: &Arc<Inner>, generation: u64, envelope: Envelope) {
    let active = inner.state.lock().unwrap().active.clone();
    if let Some(active) = active.filter(|active| active.generation == generation) {
        let (written, _) = oneshot::channel();
        active
            .outbound
            .try_send(Outbound { envelope, written })
            .ok();
    }
}

async fn send_required(inner: &Arc<Inner>, generation: u64, envelope: Envelope) -> Result<(), WsError> {
    let active = inner.active().map_err(|_| WsError::Closed)?;
    if active.generation != generation {
        return Err(WsError::Closed);
    }
    inner.enqueue(active, envelope).await
}

#[cfg(test)]
mod tests {
    use super::Packet;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::Value;
    use std::{
        collections::HashSet,
        net::SocketAddr,
        sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc},
        time::Duration,
    };
    use tokio::sync::oneshot;
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};

    #[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
    struct TestReq {
        value: String,
    }

    #[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
    struct TestRes {
        value: String,
    }

    #[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
    struct OtherRes {
        value: String,
    }

    #[derive(Debug, serde::Deserialize)]
    struct FailingRes;

    impl serde::Serialize for FailingRes {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(<S::Error as serde::ser::Error>::custom(
                "intentional serialization failure",
            ))
        }
    }

    #[derive(Debug, serde::Deserialize, serde::Serialize)]
    struct DelayReq {
        gate: u8,
    }

    impl super::Packet for TestReq {
        const TYPE: &'static str = "TestReq";
    }

    impl super::Packet for TestRes {
        const TYPE: &'static str = "TestRes";
    }

    impl super::Packet for OtherRes {
        const TYPE: &'static str = "OtherRes";
    }

    impl super::Packet for FailingRes {
        const TYPE: &'static str = "FailingRes";
    }

    impl super::Packet for DelayReq {
        const TYPE: &'static str = "DelayReq";
    }

    impl super::Requestable for TestReq {
        type Response = TestRes;
    }

    #[test]
    fn envelope_matches_shared_fixture() {
        let envelope = super::Envelope::new(
            uuid::Uuid::parse_str("7747b71a-66bc-4fc6-bf85-e9828277addf").unwrap(),
            TestReq::TYPE,
            TestReq { value: "hello".to_owned() },
        ).unwrap();

        assert_eq!(
            serde_json::to_string(&envelope).unwrap(),
            include_str!("../../../protocol/fixtures/test-request.json").trim_end(),
        );
    }

    #[tokio::test]
    async fn disconnected_calls_fail_immediately() {
        let connection = super::WsConnection::disconnected_for_test();

        assert_eq!(
            connection.request(TestReq { value: "request".to_owned() }).await.unwrap_err(),
            super::WsError::NotConnected,
        );
        assert_eq!(
            connection.send(TestReq { value: "send".to_owned() }).await.unwrap_err(),
            super::WsError::NotConnected,
        );
    }

    #[tokio::test]
    async fn connection_generation_tracks_reconnects() {
        let (address, first_peer_rx, second_peer_rx) = raw_server_with_reconnect().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let mut generation = connection.connection_generation();
        assert_eq!(*generation.borrow(), 0);
        let mut first_peer = tokio::time::timeout(Duration::from_secs(1), first_peer_rx)
            .await
            .unwrap()
            .unwrap();
        generation.changed().await.unwrap();
        assert_eq!(*generation.borrow(), 1);
        first_peer.close(None).await.unwrap();
        generation.changed().await.unwrap();
        assert_eq!(*generation.borrow(), 0);
        let _second_peer = tokio::time::timeout(Duration::from_secs(1), second_peer_rx)
            .await
            .unwrap()
            .unwrap();
        generation.changed().await.unwrap();
        assert_eq!(*generation.borrow(), 2);
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn connection_events_preserve_every_rapid_transition() {
        let connection = super::WsConnection::disconnected_for_test();
        let mut events = connection.connection_events();
        let (first_outbound, _) = tokio::sync::mpsc::channel(1);
        let first = connection.inner.activate(first_outbound);
        connection.inner.close_generation(first);
        let (second_outbound, _) = tokio::sync::mpsc::channel(1);
        let second = connection.inner.activate(second_outbound);
        connection.inner.close_generation(second);
        let (third_outbound, _) = tokio::sync::mpsc::channel(1);
        connection.inner.activate(third_outbound);

        for expected in [
            super::ConnectionEvent::Connected,
            super::ConnectionEvent::Disconnected,
            super::ConnectionEvent::Connected,
            super::ConnectionEvent::Disconnected,
            super::ConnectionEvent::Connected,
        ] {
            assert_eq!(events.recv().await.unwrap(), expected);
        }
    }

    #[tokio::test]
    async fn connection_generation_late_subscription_observes_active_generation() {
        let connection = super::WsConnection::disconnected_for_test();
        let (outbound, _) = tokio::sync::mpsc::channel(1);
        assert_eq!(connection.inner.activate(outbound), 1);
        let generation = connection.connection_generation();
        assert_eq!(*generation.borrow(), 1);
        connection.inner.close_generation(1);
    }

    #[tokio::test]
    async fn connection_generation_shutdown_publishes_disconnected() {
        let connection = super::WsConnection::disconnected_for_test();
        let (outbound, _) = tokio::sync::mpsc::channel(1);
        connection.inner.activate(outbound);
        let mut generation = connection.connection_generation();
        assert_eq!(*generation.borrow(), 1);
        connection.shutdown().await;
        tokio::time::timeout(Duration::from_secs(1), generation.changed())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*generation.borrow(), 0);
    }

    async fn raw_server() -> (
        SocketAddr,
        oneshot::Receiver<WebSocketStream<tokio::net::TcpStream>>,
    ) {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            tx.send(
                WebSocketStream::from_raw_socket(stream, Role::Server, None).await,
            )
            .ok();
        });
        (address, rx)
    }

    async fn raw_server_with_reconnect() -> (
        SocketAddr,
        oneshot::Receiver<WebSocketStream<tokio::net::TcpStream>>,
        oneshot::Receiver<WebSocketStream<tokio::net::TcpStream>>,
    ) {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let (first_tx, first_rx) = oneshot::channel();
        let (second_tx, second_rx) = oneshot::channel();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            first_tx
                .send(WebSocketStream::from_raw_socket(stream, Role::Server, None).await)
                .ok();
            let (stream, _) = listener.accept().await.unwrap();
            second_tx
                .send(WebSocketStream::from_raw_socket(stream, Role::Server, None).await)
                .ok();
        });
        (address, first_rx, second_rx)
    }

    #[tokio::test]
    async fn concurrent_sends_share_one_raw_connection() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(20),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();

        let results = futures_util::future::join_all((0..32).map(|value| {
            let connection = connection.clone();
            async move { connection.send(TestReq { value: value.to_string() }).await }
        }))
        .await;

        let mut uuids = HashSet::new();
        let mut values = HashSet::new();
        for result in results {
            result.unwrap();
            let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let envelope: serde_json::Value = serde_json::from_str(
                message.into_text().unwrap().as_ref(),
            )
            .unwrap();
            uuids.insert(envelope["uuid"].as_str().unwrap().to_owned());
            values.insert(envelope["payload"]["value"].as_str().unwrap().to_owned());
        }

        assert_eq!(uuids.len(), 32);
        assert_eq!(values.len(), 32);
        assert_eq!(values, (0..32).map(|value| value.to_string()).collect());

        connection.shutdown().await;
    }

    #[tokio::test]
    async fn request_resolves_only_matching_uuid_and_type() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let request = tokio::spawn({
            let connection = connection.clone();
            async move {
                connection
                    .request(TestReq { value: "request".to_owned() })
                    .await
            }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: super::Envelope = serde_json::from_str(message.into_text().unwrap().as_ref())
            .unwrap();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    envelope.uuid,
                    TestRes::TYPE,
                    TestRes { value: "response".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();

        assert_eq!(request.await.unwrap().unwrap().value, "response");
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn response_type_mismatch_fails_the_waiter() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let request = tokio::spawn({
            let connection = connection.clone();
            async move {
                connection
                    .request(TestReq { value: "request".to_owned() })
                    .await
            }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: super::Envelope = serde_json::from_str(message.into_text().unwrap().as_ref())
            .unwrap();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    envelope.uuid,
                    OtherRes::TYPE,
                    OtherRes { value: "wrong".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();

        assert_eq!(
            request.await.unwrap().unwrap_err(),
            super::WsError::UnexpectedResponse {
                expected: TestRes::TYPE,
                actual: OtherRes::TYPE.to_owned(),
            },
        );
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn handlers_run_concurrently_and_reply_out_of_order() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let release = Arc::new(tokio::sync::Notify::new());
        let handler_release = release.clone();
        connection
            .handler::<DelayReq, _, _>(move |context, packet| {
                let release = handler_release.clone();
                async move {
                    if packet.gate == 0 {
                        release.notified().await;
                    }
                    context
                        .send(TestRes { value: packet.gate.to_string() })
                        .await
                }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let first_uuid = uuid::Uuid::new_v4();
        let second_uuid = uuid::Uuid::new_v4();
        for (uuid, gate) in [(first_uuid, 0), (second_uuid, 1)] {
            peer.send(super::Message::Text(
                serde_json::to_string(
                    &super::Envelope::new(uuid, DelayReq::TYPE, DelayReq { gate }).unwrap(),
                )
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();
        }

        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: super::Envelope = serde_json::from_str(message.into_text().unwrap().as_ref())
            .unwrap();
        assert_eq!(envelope.uuid, second_uuid);
        release.notify_one();
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: super::Envelope = serde_json::from_str(message.into_text().unwrap().as_ref())
            .unwrap();
        assert_eq!(envelope.uuid, first_uuid);
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn disconnect_fails_pending_and_queued_work_without_replay() {
        let (address, first_peer_rx, second_peer_rx) = raw_server_with_reconnect().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let mut first_peer = tokio::time::timeout(Duration::from_secs(1), first_peer_rx)
            .await
            .unwrap()
            .unwrap();
        let request = tokio::spawn({
            let connection = connection.clone();
            async move {
                connection
                    .request(TestReq { value: "pending".to_owned() })
                    .await
            }
        });
        let _ = tokio::time::timeout(Duration::from_secs(1), first_peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let sends = (0..512)
            .map(|value| {
                let connection = connection.clone();
                tokio::spawn(async move {
                    connection.send(TestReq { value: value.to_string() }).await
                })
            })
            .collect::<Vec<_>>();
        first_peer.close(None).await.unwrap();

        assert_eq!(request.await.unwrap().unwrap_err(), super::WsError::Closed);
        let mut closed = 0;
        for send in sends {
            let result = tokio::time::timeout(Duration::from_secs(1), send)
                .await
                .unwrap()
                .unwrap();
            if let Err(error) = result {
                assert_eq!(error, super::WsError::Closed);
                closed += 1;
            }
        }
        assert!(closed > 0);

        let mut second_peer = tokio::time::timeout(Duration::from_secs(1), second_peer_rx)
            .await
            .unwrap()
            .unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(100), second_peer.next()).await.is_err());
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn disconnected_queued_handler_does_not_start() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let disconnected = Arc::new(AtomicBool::new(false));
        let starts_after_disconnect = Arc::new(AtomicUsize::new(0));
        let handler_disconnected = disconnected.clone();
        let handler_starts = starts_after_disconnect.clone();
        connection
            .handler::<DelayReq, _, _>(move |_context, _packet| {
                if handler_disconnected.load(Ordering::Acquire) {
                    handler_starts.fetch_add(1, Ordering::SeqCst);
                }
                async {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    Ok(())
                }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        for gate in 0..1 {
            peer.send(super::Message::Text(
                serde_json::to_string(
                    &super::Envelope::new(
                        uuid::Uuid::new_v4(),
                        DelayReq::TYPE,
                        DelayReq { gate },
                    )
                    .unwrap(),
                )
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();
        }
        peer.close(None).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if connection.inner.state.lock().unwrap().active.is_none() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        disconnected.store(true, Ordering::Release);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(starts_after_disconnect.load(Ordering::SeqCst), 0);
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn buffered_frame_after_session_close_does_not_dispatch_old_generation() {
        let connection = super::WsConnection::disconnected_for_test();
        let started = Arc::new(AtomicUsize::new(0));
        let started_clone = started.clone();
        connection
            .handler::<DelayReq, _, _>(move |_context, _packet| {
                started_clone.fetch_add(1, Ordering::SeqCst);
                async { Ok(()) }
            })
            .unwrap();
        let (old_outbound, _) = tokio::sync::mpsc::channel(1);
        let old_generation = connection.inner.activate(old_outbound);
        let uuid = uuid::Uuid::new_v4();
        let frame = super::Envelope::new(
            uuid,
            DelayReq::TYPE,
            DelayReq { gate: 7 },
        )
        .unwrap();
        connection.inner.close_generation(old_generation);
        let (new_outbound, _) = tokio::sync::mpsc::channel(1);
        let new_generation = connection.inner.activate(new_outbound);

        assert!(
            super::handle_envelope(connection.inner.clone(), old_generation, frame.clone())
                .await
                .is_none()
        );
        assert!(!connection
            .inner
            .state
            .lock()
            .unwrap()
            .inbound
            .contains_key(&(old_generation, uuid)));

        let task = super::handle_envelope(connection.inner.clone(), new_generation, frame)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), tokio::spawn(task))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(started.load(Ordering::SeqCst), 1);
        connection.inner.close_generation(new_generation);
    }

    #[tokio::test]
    async fn supervisor_retries_until_server_appears() {
        let reserved = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(20),
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
        let listener = tokio::net::TcpListener::bind(address).await.unwrap();
        tokio::time::timeout(Duration::from_millis(25), listener.accept())
            .await
            .unwrap()
            .unwrap();
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn shutdown_interrupts_supervisor_retry_wait() {
        let reserved = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_secs(1),
            Duration::from_millis(20),
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
        tokio::time::timeout(Duration::from_millis(100), connection.shutdown())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn timeout_sends_cancel_and_late_response_is_discarded() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(20),
        );
        let handled = Arc::new(AtomicUsize::new(0));
        let handled_clone = handled.clone();
        connection
            .handler::<TestRes, _, _>(move |_context, _packet| {
                handled_clone.fetch_add(1, Ordering::SeqCst);
                async { Ok(()) }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let request = tokio::spawn({
            let connection = connection.clone();
            async move {
                connection
                    .request(TestReq { value: "timeout".to_owned() })
                    .await
            }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let request_envelope: super::Envelope =
            serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(request.await.unwrap().unwrap_err(), super::WsError::Timeout);

        let cancel = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let cancel_envelope: super::Envelope =
            serde_json::from_str(cancel.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(cancel_envelope.uuid, request_envelope.uuid);
        assert_eq!(cancel_envelope.packet_type, "Cancel");

        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    request_envelope.uuid,
                    TestRes::TYPE,
                    TestRes { value: "late".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(handled.load(Ordering::SeqCst), 0);
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn timeout_waits_for_response_after_reader_removes_pending() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(20),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let request = tokio::spawn({
            let connection = connection.clone();
            async move { connection.request(TestReq { value: "race".to_owned() }).await }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: super::Envelope =
            serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        let pending = connection
            .inner
            .state
            .lock()
            .unwrap()
            .pending
            .remove(&envelope.uuid)
            .unwrap();
        tokio::time::sleep(Duration::from_millis(60)).await;
        pending
            .response
            .send(Ok(serde_json::to_value(TestRes { value: "race-ok".to_owned() }).unwrap()))
            .ok();
        assert_eq!(
            request.await.unwrap(),
            Ok(TestRes { value: "race-ok".to_owned() }),
        );
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn remote_error_fails_only_its_matching_waiter() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let first = tokio::spawn({
            let connection = connection.clone();
            async move {
                connection
                    .request(TestReq { value: "first".to_owned() })
                    .await
            }
        });
        let second = tokio::spawn({
            let connection = connection.clone();
            async move {
                connection
                    .request(TestReq { value: "second".to_owned() })
                    .await
            }
        });
        let mut requests = HashSet::new();
        let mut envelopes = Vec::new();
        for _ in 0..2 {
            let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let envelope: super::Envelope =
                serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
            requests.insert(envelope.payload["value"].as_str().unwrap().to_owned());
            envelopes.push(envelope);
        }
        let first_envelope = envelopes
            .iter()
            .find(|envelope| envelope.payload["value"] == "first")
            .unwrap();
        let second_envelope = envelopes
            .iter()
            .find(|envelope| envelope.payload["value"] == "second")
            .unwrap();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    first_envelope.uuid,
                    "RemoteError",
                    super::RemoteError { message: "failed".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    second_envelope.uuid,
                    TestRes::TYPE,
                    TestRes { value: "second response".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();

        assert_eq!(
            first.await.unwrap().unwrap_err(),
            super::WsError::Remote("failed".to_owned())
        );
        assert_eq!(second.await.unwrap().unwrap().value, "second response");
        assert_eq!(requests.len(), 2);
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn malformed_and_unknown_packets_do_not_stop_the_reader() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let handled = Arc::new(AtomicUsize::new(0));
        let handled_clone = handled.clone();
        connection
            .handler::<TestReq, _, _>(move |context, packet| {
                handled_clone.fetch_add(1, Ordering::SeqCst);
                async move { context.send(TestRes { value: packet.value }).await }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        peer.send(super::Message::Text("not-json".to_owned().into()))
            .await
            .unwrap();
        let unknown_uuid = uuid::Uuid::new_v4();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(unknown_uuid, "Mystery", Value::Null).unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        let valid_uuid = uuid::Uuid::new_v4();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    valid_uuid,
                    TestReq::TYPE,
                    TestReq { value: "valid".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();

        let mut responses = Vec::new();
        for _ in 0..2 {
            let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            responses.push(
                serde_json::from_str::<super::Envelope>(message.into_text().unwrap().as_ref())
                    .unwrap(),
            );
        }
        assert!(responses.iter().any(|response| {
            response.uuid == unknown_uuid && response.packet_type == "RemoteError"
        }));
        assert!(responses
            .iter()
            .any(|response| response.uuid == valid_uuid && response.packet_type == TestRes::TYPE));
        assert_eq!(handled.load(Ordering::SeqCst), 1);
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn cancel_prevents_queued_handler_and_response_after_cancel() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let started = Arc::new(AtomicUsize::new(0));
        let started_clone = started.clone();
        connection
            .handler::<DelayReq, _, _>(move |context, _packet| {
                started_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    context.cancelled().await;
                    assert!(context.is_cancelled());
                    assert_eq!(
                        context.send(TestRes { value: "late".to_owned() }).await,
                        Err(super::WsError::Closed),
                    );
                    Ok(())
                }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let uuid = uuid::Uuid::new_v4();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(uuid, DelayReq::TYPE, DelayReq { gate: 0 }).unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        peer.send(super::Message::Text(
            serde_json::to_string(&super::Envelope::new(uuid, "Cancel", Value::Null).unwrap())
                .unwrap()
                .into(),
        ))
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(started.load(Ordering::SeqCst), 0);
        let second = tokio::time::timeout(Duration::from_millis(100), peer.next()).await;
        if let Ok(Some(Ok(message))) = second {
            panic!("unexpected duplicate response: {message:?}");
        }
        assert!(second.is_err());
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn duplicate_inbound_uuid_keeps_one_handler_and_reply() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let started = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(tokio::sync::Notify::new());
        let handler_release = release.clone();
        let handler_started = started.clone();
        connection
            .handler::<DelayReq, _, _>(move |context, packet| {
                handler_started.fetch_add(1, Ordering::SeqCst);
                let release = handler_release.clone();
                async move {
                    if packet.gate == 0 {
                        release.notified().await;
                    }
                    context.send(TestRes { value: packet.gate.to_string() }).await
                }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let uuid = uuid::Uuid::new_v4();
        for gate in [0, 1] {
            peer.send(super::Message::Text(
                serde_json::to_string(
                    &super::Envelope::new(uuid, DelayReq::TYPE, DelayReq { gate }).unwrap(),
                )
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();
            if gate == 0 {
                tokio::time::timeout(Duration::from_secs(1), async {
                    while started.load(Ordering::SeqCst) == 0 {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
        release.notify_one();
        let response = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let response: super::Envelope =
            serde_json::from_str(response.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(response.uuid, uuid);
        assert_eq!(started.load(Ordering::SeqCst), 1);
        let second = tokio::time::timeout(Duration::from_millis(100), peer.next()).await;
        if let Ok(Some(Ok(message))) = second {
            panic!("unexpected duplicate response: {message:?}");
        }
        assert!(second.is_err());
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn aborted_request_removes_pending_and_sends_cancel() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let handled = Arc::new(AtomicUsize::new(0));
        let handled_clone = handled.clone();
        connection
            .handler::<TestRes, _, _>(move |_context, _packet| {
                handled_clone.fetch_add(1, Ordering::SeqCst);
                async { Ok(()) }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let request = tokio::spawn({
            let connection = connection.clone();
            async move {
                connection
                    .request(TestReq { value: "aborted".to_owned() })
                    .await
            }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let request_envelope: super::Envelope =
            serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());

        let cancel = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let cancel_envelope: super::Envelope =
            serde_json::from_str(cancel.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(cancel_envelope.uuid, request_envelope.uuid);
        assert_eq!(cancel_envelope.packet_type, "Cancel");
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    request_envelope.uuid,
                    TestRes::TYPE,
                    TestRes { value: "late".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(handled.load(Ordering::SeqCst), 0);
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn handler_failure_sends_required_remote_error() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        connection
            .handler::<DelayReq, _, _>(move |_context, _packet| async {
                Err(super::WsError::Remote("handler failed".to_owned()))
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let uuid = uuid::Uuid::new_v4();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(uuid, DelayReq::TYPE, DelayReq { gate: 0 }).unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: super::Envelope =
            serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(envelope.uuid, uuid);
        assert_eq!(envelope.packet_type, "RemoteError");
        assert_eq!(envelope.payload["message"], "Remote(\"handler failed\")");
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn response_serialization_failure_sends_required_remote_error() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        connection
            .handler::<DelayReq, _, _>(move |context, _packet| async move {
                context.send(FailingRes).await
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let uuid = uuid::Uuid::new_v4();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(uuid, DelayReq::TYPE, DelayReq { gate: 0 }).unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let response: super::Envelope =
            serde_json::from_str(response.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(response.uuid, uuid);
        assert_eq!(response.packet_type, "RemoteError");
        assert_eq!(
            response.payload["message"],
            "Protocol(\"intentional serialization failure\")",
        );
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn handler_panic_before_response_sends_remote_error() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = calls.clone();
        connection
            .handler::<DelayReq, _, _>(move |context, packet| {
                let call = handler_calls.fetch_add(1, Ordering::SeqCst);
                async move {
                    if call == 0 {
                        panic!("pre-response panic");
                    }
                    context.send(TestRes { value: packet.gate.to_string() }).await
                }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let uuid = uuid::Uuid::new_v4();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(uuid, DelayReq::TYPE, DelayReq { gate: 0 }).unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        let remote_error = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let remote_error: super::Envelope =
            serde_json::from_str(remote_error.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(remote_error.uuid, uuid);
        assert_eq!(remote_error.packet_type, "RemoteError");
        assert_eq!(
            remote_error.payload["message"],
            "Protocol(\"handler panicked: pre-response panic\")",
        );
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(uuid, DelayReq::TYPE, DelayReq { gate: 1 }).unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let response: super::Envelope =
            serde_json::from_str(response.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(response.uuid, uuid);
        assert_eq!(response.packet_type, TestRes::TYPE);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn malformed_and_unmatched_errors_are_reported_locally() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        peer.send(super::Message::Text("not-json".to_owned().into()))
            .await
            .unwrap();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    uuid::Uuid::new_v4(),
                    "RemoteError",
                    super::RemoteError { message: "unmatched".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while connection.report_count_for_test() < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn post_response_errors_and_handler_panics_are_observed() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        connection
            .handler::<TestReq, _, _>(move |context, packet| async move {
                context.send(TestRes { value: packet.value }).await?;
                Err(super::WsError::Remote("after response".to_owned()))
            })
            .unwrap();
        connection
            .handler::<OtherRes, _, _>(move |_context, _packet| async move {
                panic!("handler panic");
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let response_uuid = uuid::Uuid::new_v4();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    response_uuid,
                    TestReq::TYPE,
                    TestReq { value: "response".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let response: super::Envelope =
            serde_json::from_str(response.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(response.uuid, response_uuid);
        tokio::time::timeout(Duration::from_secs(1), async {
            while connection.report_count_for_test() < 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let panic_uuid = uuid::Uuid::new_v4();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    panic_uuid,
                    OtherRes::TYPE,
                    OtherRes { value: "panic".to_owned() },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        let remote_error = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let remote_error: super::Envelope =
            serde_json::from_str(remote_error.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(remote_error.uuid, panic_uuid);
        assert_eq!(remote_error.packet_type, "RemoteError");
        connection.shutdown().await;
        assert!(connection.report_count_for_test() >= 1);
    }

    #[tokio::test]
    async fn panicked_handler_reports_and_releases_uuid_before_shutdown() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = calls.clone();
        connection
            .handler::<DelayReq, _, _>(move |context, packet| {
                let call = handler_calls.fetch_add(1, Ordering::SeqCst);
                async move {
                    if call == 0 {
                        panic!("first handler panic");
                    }
                    context.send(TestRes { value: packet.gate.to_string() }).await
                }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let uuid = uuid::Uuid::new_v4();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(uuid, DelayReq::TYPE, DelayReq { gate: 0 }).unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        let remote_error = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let remote_error: super::Envelope =
            serde_json::from_str(remote_error.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(remote_error.uuid, uuid);
        assert_eq!(remote_error.packet_type, "RemoteError");
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(uuid, DelayReq::TYPE, DelayReq { gate: 1 }).unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let response: super::Envelope =
            serde_json::from_str(response.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(response.uuid, uuid);
        assert_eq!(response.packet_type, TestRes::TYPE);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        connection.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_does_not_wait_for_non_yielding_handler() {
        let (address, peer_rx) = raw_server().await;
        let connection = super::WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_millis(100),
        );
        let started = Arc::new(AtomicBool::new(false));
        let handler_started = started.clone();
        let running = Arc::new(AtomicBool::new(true));
        let handler_running = running.clone();
        connection
            .handler::<DelayReq, _, _>(move |_context, _packet| {
                handler_started.store(true, Ordering::Release);
                let running = handler_running.clone();
                async move {
                    while running.load(Ordering::Acquire) {
                        std::hint::spin_loop();
                    }
                    Ok(())
                }
            })
            .unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        peer.send(super::Message::Text(
            serde_json::to_string(
                &super::Envelope::new(
                    uuid::Uuid::new_v4(),
                    DelayReq::TYPE,
                    DelayReq { gate: 0 },
                )
                .unwrap(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !started.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut shutdown = tokio::spawn({
            let connection = connection.clone();
            async move { connection.shutdown().await }
        });
        let bounded = tokio::time::timeout(Duration::from_millis(100), &mut shutdown)
            .await
            .is_ok();
        running.store(false, Ordering::Release);
        if !bounded {
            tokio::time::timeout(Duration::from_secs(1), shutdown)
                .await
                .unwrap()
                .unwrap();
        }
        assert!(bounded);
    }
}
