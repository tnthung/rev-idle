use super::{connection::WsConnection, StateReq, StateRes};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{
        connection::WsConnection,
        test_support::raw_server,
    };
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use std::time::Duration;
    use tokio_tungstenite::tungstenite::Message;

    #[tokio::test(flavor = "current_thread")]
    async fn request_state_uses_state_packet_and_preserves_mixed_json() {
        let (address, peer_rx) = raw_server().await;
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let keys = ["score".to_owned(), "eternity.dtpSpent".to_owned()];
        let request = tokio::spawn({
            let connection = connection.clone();
            let keys = keys.to_vec();
            async move { request_state(&connection, &keys).await }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(envelope.get("type"), Some(&json!("StateReq")));
        assert_eq!(
            envelope.get("payload"),
            Some(&json!({ "keys": ["score", "eternity.dtpSpent"] })),
        );
        peer.send(Message::Text(
            json!({
                "uuid": envelope["uuid"],
                "type": "StateRes",
                "payload": {
                    "value": {
                        "score": "1e3",
                        "enabled": true,
                        "nested": { "value": null },
                        "items": [1, "two", false],
                    },
                },
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        assert_eq!(
            serde_json::from_str::<Value>(&request.await.unwrap().unwrap()).unwrap(),
            json!({
                "score": "1e3",
                "enabled": true,
                "nested": { "value": null },
                "items": [1, "two", false],
            }),
        );
        connection.shutdown().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_state_rejects_malformed_response_value() {
        let (address, peer_rx) = raw_server().await;
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let request = tokio::spawn({
            let connection = connection.clone();
            async move { request_state(&connection, &[]).await }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        peer.send(Message::Text(
            json!({
                "uuid": envelope["uuid"],
                "type": "StateRes",
                "payload": { "unexpected": true },
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        assert!(request.await.unwrap().is_err());
        connection.shutdown().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_state_rejects_non_object_response_value() {
        let (address, peer_rx) = raw_server().await;
        let connection = WsConnection::connect_for_test(
            address,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
            .await
            .unwrap()
            .unwrap();
        let request = tokio::spawn({
            let connection = connection.clone();
            async move { request_state(&connection, &[]).await }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        peer.send(Message::Text(
            json!({
                "uuid": envelope["uuid"],
                "type": "StateRes",
                "payload": { "value": [] },
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        assert_eq!(
            request.await.unwrap().unwrap_err(),
            "state response must be a JSON object",
        );
        connection.shutdown().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn disconnected_state_request_fails_immediately() {
        let connection = WsConnection::disconnected_for_test();

        assert_eq!(
            request_state(&connection, &[]).await.unwrap_err(),
            "NotConnected",
        );
    }
}

pub(crate) async fn request_state(
    connection: &WsConnection,
    keys: &[String],
) -> Result<String, String> {
    let StateRes { value } = connection
        .request(StateReq { keys: keys.to_vec() })
        .await
        .map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err("state response must be a JSON object".to_owned());
    }
    serde_json::to_string(&value).map_err(|error| error.to_string())
}
