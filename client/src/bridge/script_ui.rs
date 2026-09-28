use super::{
    connection::{PacketContext, WsError},
    ScriptUiElementState, ScriptUiEvent, ScriptUiSnapshot, WsConnection,
};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct ScriptUiPublisher {
    snapshots: watch::Sender<ScriptUiSnapshot>,
}

impl Default for ScriptUiPublisher {
    fn default() -> Self {
        let (snapshots, _) = watch::channel(ScriptUiSnapshot { session_id: None, revision: 0, elements: Vec::new() });
        Self { snapshots }
    }
}

impl ScriptUiPublisher {
    pub(crate) fn subscribe(&self) -> watch::Receiver<ScriptUiSnapshot> {
        self.snapshots.subscribe()
    }

    pub(crate) fn replace(&self, session_id: Uuid, mut elements: Vec<ScriptUiElementState>) {
        self.snapshots.send_modify(|snapshot| {
            snapshot.session_id = Some(session_id);
            snapshot.revision += 1;
            std::mem::swap(&mut snapshot.elements, &mut elements);
        });
    }

    pub(crate) fn clear(&self, session_id: Uuid) {
        let mut elements = Vec::new();
        self.snapshots.send_if_modified(|snapshot| {
            if snapshot.session_id != Some(session_id) {
                return false;
            }
            snapshot.session_id = None;
            snapshot.revision += 1;
            std::mem::swap(&mut snapshot.elements, &mut elements);
            true
        });
    }
}

pub(crate) struct QueuedScriptUiEvent {
    pub(crate) event: ScriptUiEvent,
    pub(crate) generation: u64,
    pub(crate) capture_epoch: u64,
}

pub(crate) fn register_script_ui_handlers(
    connection: &WsConnection,
    events: mpsc::Sender<QueuedScriptUiEvent>,
) -> Result<(), WsError> {
    connection.handler::<ScriptUiEvent, _, _>(move |context: PacketContext, event| {
        let events = events.clone();
        async move {
            if !context.is_cancelled()
                && let Some(capture_epoch) = crate::capture::CaptureState.inactive_epoch()
            {
                if let Err(error) = events.try_send(QueuedScriptUiEvent {
                    event, generation: context.generation(), capture_epoch,
                }) {
                    eprintln!("[Script UI] dropped event: {error}");
                }
            }
            Ok(())
        }
    })
}

pub(crate) async fn publish_script_ui(
    connection: WsConnection,
    mut snapshots: watch::Receiver<ScriptUiSnapshot>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut generations = connection.connection_generation();
    let mut sent = None;
    loop {
        if *shutdown.borrow() {
            return;
        }
        let generation = *generations.borrow_and_update();
        let snapshot = snapshots.borrow_and_update().clone();
        if generation != 0 && sent != Some((generation, snapshot.revision)) {
            let revision = snapshot.revision;
            tokio::select! {
                biased;
                _ = shutdown.changed() => return,
                changed = generations.changed() => {
                    if changed.is_err() { return; }
                }
                result = connection.send_for_generation(snapshot, generation, || true) => {
                    if result.is_ok() {
                        sent = Some((generation, revision));
                    }
                }
            }
            continue;
        }
        tokio::select! {
            _ = shutdown.changed() => return,
            changed = generations.changed() => {
                if changed.is_err() { return; }
            }
            changed = snapshots.changed() => {
                if changed.is_err() { return; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use std::time::Duration;
    use tokio_tungstenite::{WebSocketStream, tungstenite::protocol::Role};

    #[test]
    fn script_ui_unload_publishes_empty_snapshot() {
        let publisher = ScriptUiPublisher::default();
        let mut snapshots = publisher.subscribe();
        let old = Uuid::new_v4();
        publisher.replace(old, Vec::new());
        assert_eq!(snapshots.borrow_and_update().revision, 1);
        publisher.clear(old);
        assert_eq!(snapshots.borrow_and_update().session_id, None);
        assert_eq!(snapshots.borrow().revision, 2);
        let current = Uuid::new_v4();
        publisher.replace(current, Vec::new());
        snapshots.borrow_and_update();
        publisher.clear(old);
        assert!(!snapshots.has_changed().unwrap());
        assert_eq!(snapshots.borrow().session_id, Some(current));
    }

    #[tokio::test]
    async fn script_ui_snapshot_coalesces_and_resends_latest() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let connection = WsConnection::connect_for_test(listener.local_addr().unwrap(), Duration::from_millis(10), Duration::from_secs(1));
        let publisher = ScriptUiPublisher::default();
        let session = Uuid::new_v4();
        for _ in 0..20 { publisher.replace(session, Vec::new()); }
        let (shutdown, shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(publish_script_ui(connection.clone(), publisher.subscribe(), shutdown_rx));
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
        let packet = tokio::time::timeout(Duration::from_secs(2), socket.next()).await.unwrap().unwrap().unwrap();
        let packet: serde_json::Value = serde_json::from_str(packet.to_text().unwrap()).unwrap();
        assert_eq!(packet["type"], "ScriptUiSnapshot");
        assert_eq!(packet["payload"]["revision"], 20);
        assert!(tokio::time::timeout(Duration::from_millis(40), socket.next()).await.is_err());
        socket.close(None).await.unwrap();
        drop(socket);
        publisher.clear(session);
        let (stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept()).await.unwrap().unwrap();
        let mut socket = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
        let packet = tokio::time::timeout(Duration::from_secs(2), socket.next()).await.unwrap().unwrap().unwrap();
        let packet: serde_json::Value = serde_json::from_str(packet.to_text().unwrap()).unwrap();
        assert_eq!(packet["payload"]["revision"], 21);
        assert!(packet["payload"]["sessionId"].is_null());
        shutdown.send(true).unwrap();
        task.await.unwrap();
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn script_ui_events_are_bounded_independent_notifications() {
        let (address, server) = super::super::test_support::raw_server().await;
        let connection = WsConnection::connect_for_test(address, Duration::from_millis(10), Duration::from_secs(1));
        let (events, mut receiver) = mpsc::channel(1);
        register_script_ui_handlers(&connection, events).unwrap();
        let mut socket = server.await.unwrap();
        let event = ScriptUiEvent {
            session_id: Uuid::new_v4(), element_id: "button".into(), instance_id: Uuid::new_v4(),
            events_version: 1, event: super::super::ScriptUiEventKind::Click,
        };
        for _ in 0..3 {
            socket.send(tokio_tungstenite::tungstenite::Message::Text(serde_json::json!({
                "uuid": Uuid::new_v4(), "type": "ScriptUiEvent", "payload": event,
            }).to_string().into())).await.unwrap();
        }
        assert!(tokio::time::timeout(Duration::from_millis(40), socket.next()).await.is_err());
        let queued = receiver.try_recv().unwrap();
        assert_eq!(queued.event, event);
        assert_eq!(queued.generation, *connection.connection_generation().borrow());
        assert!(receiver.try_recv().is_err());
        connection.shutdown().await;
    }
}
