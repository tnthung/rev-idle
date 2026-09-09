use super::connection::{Packet, Requestable};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct StateReq {
    pub(crate) keys: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct StateRes {
    pub(crate) value: Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct CaptureReq {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct CaptureRes {
    #[serde(rename = "type")]
    pub(crate) target_type: Option<String>,
    pub(crate) path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct InvokeReq {
    pub(crate) path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct InvokeRes {}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TransferReq {
    pub(crate) source: String,
    pub(crate) destination: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TransferRes {}

impl Packet for StateReq {
    const TYPE: &'static str = "StateReq";
}

impl Packet for StateRes {
    const TYPE: &'static str = "StateRes";
}

impl Packet for CaptureReq {
    const TYPE: &'static str = "CaptureReq";
}

impl Packet for CaptureRes {
    const TYPE: &'static str = "CaptureRes";
}

impl Packet for InvokeReq {
    const TYPE: &'static str = "InvokeReq";
}

impl Packet for InvokeRes {
    const TYPE: &'static str = "InvokeRes";
}

impl Packet for TransferReq {
    const TYPE: &'static str = "TransferReq";
}

impl Packet for TransferRes {
    const TYPE: &'static str = "TransferRes";
}

impl Requestable for StateReq {
    type Response = StateRes;
}

impl Requestable for CaptureReq {
    type Response = CaptureRes;
}

impl Requestable for InvokeReq {
    type Response = InvokeRes;
}

impl Requestable for TransferReq {
    type Response = TransferRes;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_payloads_match_shared_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(
            include_str!("../../../protocol/fixtures/bridge-packets.json"),
        )
        .unwrap();
        let packets = [
            (
                StateReq::TYPE,
                serde_json::to_value(StateReq {
                    keys: vec!["score".to_owned(), "eternity.dtpSpent".to_owned()],
                })
                .unwrap(),
            ),
            (
                StateRes::TYPE,
                serde_json::to_value(StateRes {
                    value: serde_json::json!({
                        "score": "1e3",
                        "enabled": true,
                        "nested": { "value": null },
                        "items": [1, "two", false],
                    }),
                })
                .unwrap(),
            ),
            (
                CaptureReq::TYPE,
                serde_json::to_value(CaptureReq { x: 123, y: -45 }).unwrap(),
            ),
            (
                CaptureRes::TYPE,
                serde_json::to_value(CaptureRes {
                    target_type: Some("slot".to_owned()),
                    path: Some("scene:1/Canvas[0]/Inventory/3".to_owned()),
                })
                .unwrap(),
            ),
            (
                InvokeReq::TYPE,
                serde_json::to_value(InvokeReq {
                    path: "scene:1/Canvas[0]/Buy DTP & More[0]".to_owned(),
                })
                .unwrap(),
            ),
            (InvokeRes::TYPE, serde_json::to_value(InvokeRes {}).unwrap()),
            (
                TransferReq::TYPE,
                serde_json::to_value(TransferReq {
                    source: "scene:1/Canvas[0]/Inventory/3".to_owned(),
                    destination: "scene:1/Canvas[0]/Combine/0".to_owned(),
                })
                .unwrap(),
            ),
            (TransferRes::TYPE, serde_json::to_value(TransferRes {}).unwrap()),
        ];

        for (packet_type, packet) in packets {
            assert_eq!(packet, fixture[packet_type]);
        }
    }

    #[test]
    fn capture_response_serializes_nullable_fields() {
        assert_eq!(
            serde_json::to_value(CaptureRes {
                target_type: None,
                path: None,
            })
            .unwrap(),
            serde_json::json!({ "type": null, "path": null }),
        );
    }
}
