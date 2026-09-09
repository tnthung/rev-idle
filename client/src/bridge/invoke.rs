use super::{connection::WsConnection, InvokeReq};

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
    async fn invoke_routes_path_and_accepts_empty_success_response() {
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
            async move {
                invoke(
                    &connection,
                    "scene:1/Canvas[0]/Buy DTP & More[0]".to_owned(),
                )
                .await
            }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(envelope.get("type"), Some(&json!("InvokeReq")));
        assert_eq!(
            envelope.get("payload"),
            Some(&json!({ "path": "scene:1/Canvas[0]/Buy DTP & More[0]" })),
        );
        peer.send(Message::Text(
            json!({
                "uuid": envelope["uuid"],
                "type": "InvokeRes",
                "payload": {},
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        request.await.unwrap().unwrap();
        connection.shutdown().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invoke_exposes_correlated_remote_error() {
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
            async move { invoke(&connection, "button".to_owned()).await }
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
                "type": "RemoteError",
                "payload": { "message": "failed" },
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        assert_eq!(request.await.unwrap().unwrap_err(), "Remote(\"failed\")");
        connection.shutdown().await;
    }
}

pub(crate) async fn invoke(
    connection: &WsConnection,
    path: String,
) -> Result<(), String> {
    connection
        .request(InvokeReq { path })
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}
