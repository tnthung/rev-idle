use super::{
    connection::{PacketContext, WsError},
    packets::{PauseScript, ReloadScript, ResumeScript, StartCapture, StopCapture, StopScript},
    WsConnection,
};
use crate::app::{ScriptCommand, StateUpdate};
use tokio::sync::{mpsc, watch};

pub(crate) fn register_control_handlers(
    connection: &WsConnection,
    command_tx: mpsc::Sender<ScriptCommand>,
) -> Result<(), WsError> {
    let command_sender = command_tx.clone();
    connection.handler::<ReloadScript, _, _>(move |_context: PacketContext, _packet| {
        let command_tx = command_sender.clone();
        async move {
            command_tx.try_send(ScriptCommand::Reload).ok();
            Ok(())
        }
    })?;
    let command_sender = command_tx.clone();
    connection.handler::<StopScript, _, _>(move |_context: PacketContext, _packet| {
        let command_tx = command_sender.clone();
        async move {
            command_tx.try_send(ScriptCommand::Stop).ok();
            Ok(())
        }
    })?;
    let command_sender = command_tx.clone();
    connection.handler::<PauseScript, _, _>(move |_context: PacketContext, _packet| {
        let command_tx = command_sender.clone();
        async move {
            command_tx.try_send(ScriptCommand::Pause).ok();
            Ok(())
        }
    })?;
    let command_sender = command_tx.clone();
    connection.handler::<ResumeScript, _, _>(move |_context: PacketContext, _packet| {
        let command_tx = command_sender.clone();
        async move {
            command_tx.try_send(ScriptCommand::Resume).ok();
            Ok(())
        }
    })?;
    let command_sender = command_tx.clone();
    connection.handler::<StartCapture, _, _>(move |_context: PacketContext, _packet| {
        let command_tx = command_sender.clone();
        async move {
            command_tx.try_send(ScriptCommand::StartCapture).ok();
            Ok(())
        }
    })?;
    let command_sender = command_tx;
    connection.handler::<StopCapture, _, _>(move |_context: PacketContext, _packet| {
        let command_tx = command_sender.clone();
        async move {
            command_tx.try_send(ScriptCommand::StopCapture).ok();
            Ok(())
        }
    })?;
    Ok(())
}

pub(crate) async fn publish_state(
    connection: WsConnection,
    mut state_updates: watch::Receiver<StateUpdate>,
    mut generations: watch::Receiver<u64>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut state_channel_open = true;
    let mut generation_channel_open = true;
    let generation = *generations.borrow_and_update();
    if generation != 0 {
        let state = *state_updates.borrow_and_update();
        if *shutdown.borrow() {
            return;
        }
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
            result = connection.send(state) => {
                let _ = result;
            }
        }
    }

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
            changed = generations.changed(), if generation_channel_open => {
                if changed.is_err() {
                    generation_channel_open = false;
                    continue;
                }
                let generation = *generations.borrow_and_update();
                if generation != 0 {
                    let state = *state_updates.borrow_and_update();
                    if *shutdown.borrow() {
                        return;
                    }
                    tokio::select! {
                        biased;
                        changed = shutdown.changed() => {
                            if changed.is_err() || *shutdown.borrow() {
                                return;
                            }
                        }
                        result = connection.send(state) => {
                            let _ = result;
                        }
                    }
                }
            }
            changed = state_updates.changed(), if state_channel_open => {
                if changed.is_err() {
                    state_channel_open = false;
                    continue;
                }
                let state = *state_updates.borrow_and_update();
                if *generations.borrow() != 0 {
                    if *shutdown.borrow() {
                        return;
                    }
                    tokio::select! {
                        biased;
                        changed = shutdown.changed() => {
                            if changed.is_err() || *shutdown.borrow() {
                                return;
                            }
                        }
                        result = connection.send(state) => {
                            let _ = result;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        test_support::raw_server, ReloadScript, ResumeScript, StartCapture, StopCapture,
        StopScript, PauseScript, WsConnection,
    };
    use crate::app::{ScriptCommand, ScriptPhase, StateUpdate};
    use crate::bridge::connection::Packet;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use std::time::Duration;
    use tokio::sync::{mpsc, oneshot, watch};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;
    use super::{publish_state, register_control_handlers};

    #[tokio::test]
    async fn notification_handlers_enqueue_commands_without_responses() {
        let (address, peer_rx) = raw_server().await;
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let (command_tx, mut command_rx) = mpsc::channel(8);
        register_control_handlers(&connection, command_tx).unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let packets = [
            (ReloadScript::TYPE, ScriptCommand::Reload),
            (StopScript::TYPE, ScriptCommand::Stop),
            (PauseScript::TYPE, ScriptCommand::Pause),
            (ResumeScript::TYPE, ScriptCommand::Resume),
            (StartCapture::TYPE, ScriptCommand::StartCapture),
            (StopCapture::TYPE, ScriptCommand::StopCapture),
        ];
        for (packet_type, expected) in packets {
            peer.send(Message::Text(
                json!({
                    "uuid": uuid::Uuid::new_v4(),
                    "type": packet_type,
                    "payload": {},
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), command_rx.recv())
                    .await
                    .unwrap()
                    .unwrap(),
                expected,
            );
        }
        assert!(tokio::time::timeout(Duration::from_millis(30), peer.next())
            .await
            .is_err());
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn closed_command_receiver_does_not_reply_or_close_connection() {
        let (address, peer_rx) = raw_server().await;
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let (command_tx, command_rx) = mpsc::channel(1);
        register_control_handlers(&connection, command_tx).unwrap();
        drop(command_rx);
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        peer.send(Message::Text(
            json!({
                "uuid": uuid::Uuid::new_v4(),
                "type": ReloadScript::TYPE,
                "payload": {},
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(50), peer.next())
            .await
            .is_err());
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn state_publisher_sends_latest_state_on_connection_without_state_request() {
        let (address, peer_rx) = raw_server().await;
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let (state_tx, state_rx) = watch::channel(StateUpdate {
            phase: ScriptPhase::Paused,
            capture: true,
        });
        let generation = connection.connection_generation();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let publisher = tokio::spawn(publish_state(
            connection.clone(),
            state_rx,
            generation,
            shutdown_rx,
        ));
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: serde_json::Value = serde_json::from_str(message.into_text().unwrap().as_ref())
            .unwrap();
        assert_eq!(envelope["type"], StateUpdate::TYPE);
        assert_eq!(
            envelope["payload"],
            json!({ "phase": "paused", "capture": true }),
        );
        assert!(tokio::time::timeout(Duration::from_millis(30), peer.next())
            .await
            .is_err());
        state_tx.send_replace(StateUpdate {
            phase: ScriptPhase::Running,
            capture: false,
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: serde_json::Value = serde_json::from_str(message.into_text().unwrap().as_ref())
            .unwrap();
        assert_eq!(envelope["type"], StateUpdate::TYPE);
        assert_eq!(
            envelope["payload"],
            json!({ "phase": "running", "capture": false }),
        );
        shutdown_tx.send_replace(true);
        tokio::time::timeout(Duration::from_secs(1), publisher)
            .await
            .unwrap()
            .unwrap();
        connection.shutdown().await;
    }

    #[tokio::test]
    async fn state_publisher_stays_quiet_while_disconnected_and_stops_on_shutdown() {
        let connection = WsConnection::disconnected_for_test();
        let (state_tx, state_rx) = watch::channel(StateUpdate {
            phase: ScriptPhase::Stopped,
            capture: false,
        });
        let generation = connection.connection_generation();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let publisher = tokio::spawn(publish_state(
            connection,
            state_rx,
            generation,
            shutdown_rx,
        ));
        state_tx.send_replace(StateUpdate {
            phase: ScriptPhase::Running,
            capture: false,
        });
        tokio::task::yield_now().await;
        assert!(!publisher.is_finished());
        shutdown_tx.send_replace(true);
        tokio::time::timeout(Duration::from_secs(1), publisher)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn reconnect_publisher_sends_only_latest_state_for_new_generation() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let (first_tx, first_rx) = oneshot::channel();
        let (second_tx, second_rx) = oneshot::channel();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            first_tx
                .send(tokio_tungstenite::WebSocketStream::from_raw_socket(
                    stream,
                    tokio_tungstenite::tungstenite::protocol::Role::Server,
                    None,
                ).await)
                .ok();
            let (stream, _) = listener.accept().await.unwrap();
            second_tx
                .send(tokio_tungstenite::WebSocketStream::from_raw_socket(
                    stream,
                    tokio_tungstenite::tungstenite::protocol::Role::Server,
                    None,
                ).await)
                .ok();
        });
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let (state_tx, state_rx) = watch::channel(StateUpdate {
            phase: ScriptPhase::Running,
            capture: false,
        });
        let mut generation = connection.connection_generation();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let publisher = tokio::spawn(publish_state(
            connection.clone(),
            state_rx,
            generation.clone(),
            shutdown_rx,
        ));
        let mut first_peer = tokio::time::timeout(Duration::from_secs(1), first_rx)
            .await
            .unwrap()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), first_peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        first_peer.send(Message::Close(None)).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                generation.changed().await.unwrap();
                if *generation.borrow() == 0 {
                    break;
                }
            }
        })
        .await
        .unwrap();
        state_tx.send_replace(StateUpdate {
            phase: ScriptPhase::Stopped,
            capture: false,
        });
        state_tx.send_replace(StateUpdate {
            phase: ScriptPhase::Paused,
            capture: true,
        });
        let mut second_peer = tokio::time::timeout(Duration::from_secs(1), second_rx)
            .await
            .unwrap()
            .unwrap();
        let message = tokio::time::timeout(Duration::from_secs(1), second_peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: serde_json::Value = serde_json::from_str(message.into_text().unwrap().as_ref())
            .unwrap();
        assert_eq!(envelope["payload"], json!({ "phase": "paused", "capture": true }));
        assert!(tokio::time::timeout(Duration::from_millis(30), second_peer.next())
            .await
            .is_err());
        shutdown_tx.send_replace(true);
        tokio::time::timeout(Duration::from_secs(1), publisher)
            .await
            .unwrap()
            .unwrap();
        connection.shutdown().await;
    }
}
