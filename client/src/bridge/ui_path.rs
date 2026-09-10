use super::{connection::WsConnection, UiPathReq, UiPathRes};

#[derive(serde::Deserialize)]
pub(crate) struct UiPathTarget {
    #[serde(rename = "type")]
    pub(crate) target_type: Option<String>,
    pub(crate) path: Option<String>,
}

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
    async fn request_ui_path_routes_coordinates_and_maps_target_fields() {
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
            async move { request_ui_path(&connection, 123, -45, 1920, 1080).await }
        });
        let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
        assert_eq!(envelope.get("type"), Some(&json!("UiPathReq")));
        assert_eq!(
            envelope.get("payload"),
            Some(&json!({ "x": 123, "y": -45, "width": 1920, "height": 1080 })),
        );
        peer.send(Message::Text(
            json!({
                "uuid": envelope["uuid"],
                "type": "UiPathRes",
                "payload": {
                    "type": "slot",
                    "path": "scene:1/Canvas[0]/Inventory/3",
                },
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        let target = request.await.unwrap().unwrap();
        assert_eq!(target.target_type.as_deref(), Some("slot"));
        assert_eq!(target.path.as_deref(), Some("scene:1/Canvas[0]/Inventory/3"));
        connection.shutdown().await;
    }
}

pub(crate) async fn request_ui_path(
    connection: &WsConnection,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<UiPathTarget, String> {
    let UiPathRes { target_type, path } = connection
        .request(UiPathReq { x, y, width, height })
        .await
        .map_err(|error| error.to_string())?;
    Ok(UiPathTarget {
        target_type,
        path,
    })
}
