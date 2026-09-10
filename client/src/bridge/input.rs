use super::{ClickCommand, DragCommand, ScrollCommand, WsConnection};
use crate::window::Axis;

pub(crate) fn click(connection: &WsConnection, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
    connection
        .try_send(ClickCommand { x, y, width, height })
        .map_err(|error| error.to_string())
}

pub(crate) fn scroll(
    connection: &WsConnection,
    x: i32,
    y: i32,
    length: i32,
    axis: Axis,
    width: i32,
    height: i32,
) -> Result<(), String> {
    connection
        .try_send(ScrollCommand {
            x,
            y,
            length,
            axis: match axis {
                Axis::Vertical => 0,
                Axis::Horizontal => 1,
            },
            width,
            height,
        })
        .map_err(|error| error.to_string())
}

pub(crate) fn drag(
    connection: &WsConnection,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    connection
        .try_send(DragCommand {
            start_x,
            start_y,
            end_x,
            end_y,
            width,
            height,
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bridge::{test_support::raw_server, WsConnection}, window::Axis};
    use futures_util::StreamExt;
    use serde_json::{json, Value};
    use std::time::Duration;

    #[tokio::test(flavor = "current_thread")]
    async fn input_commands_use_the_packet_connection() {
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

        click(&connection, 1200, 80, 1920, 1080).unwrap();
        scroll(&connection, 600, 400, -1, Axis::Horizontal, 1920, 1080).unwrap();
        drag(&connection, 1200, 80, 600, 400, 1920, 1080).unwrap();

        for (packet_type, payload) in [
            ("ClickCommand", json!({ "x": 1200, "y": 80, "width": 1920, "height": 1080 })),
            ("ScrollCommand", json!({ "x": 600, "y": 400, "length": -1, "axis": 1, "width": 1920, "height": 1080 })),
            ("DragCommand", json!({ "startX": 1200, "startY": 80, "endX": 600, "endY": 400, "width": 1920, "height": 1080 })),
        ] {
            let message = tokio::time::timeout(Duration::from_secs(1), peer.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let envelope: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
            assert_eq!(envelope.get("type"), Some(&json!(packet_type)));
            assert_eq!(envelope.get("payload"), Some(&payload));
        }

        connection.shutdown().await;
    }

    #[test]
    fn disconnected_input_command_fails_immediately() {
        assert_eq!(
            click(&WsConnection::disconnected_for_test(), 1, 2, 1280, 720).unwrap_err(),
            "NotConnected",
        );
    }
}
