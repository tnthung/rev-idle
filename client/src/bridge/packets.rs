use super::connection::{Packet, Requestable};
use crate::app::{ScriptPhase, StateUpdate};
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
pub(crate) struct UiPathReq {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct UiPathRes {
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

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ClickCommand {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ScrollCommand {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) length: i32,
    pub(crate) axis: u32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DragCommand {
    #[serde(rename = "startX")]
    pub(crate) start_x: i32,
    #[serde(rename = "startY")]
    pub(crate) start_y: i32,
    #[serde(rename = "endX")]
    pub(crate) end_x: i32,
    #[serde(rename = "endY")]
    pub(crate) end_y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct PressCommand {
    pub(crate) key: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ReloadScript {}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ReloadLockedScript {}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct StopScript {}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct PauseScript {}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ResumeScript {}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ResumeLockedScript {}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct StartCapture {}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct StopCapture {}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct LockScript {}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct LoadScript {
    pub(crate) path: String,
    pub(crate) locked: bool,
}
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct RemoveScriptHistory {
    pub(crate) path: String,
}

impl Packet for StateReq {
    const TYPE: &'static str = "StateReq";
}

impl Packet for StateRes {
    const TYPE: &'static str = "StateRes";
}

impl Packet for UiPathReq {
    const TYPE: &'static str = "UiPathReq";
}

impl Packet for UiPathRes {
    const TYPE: &'static str = "UiPathRes";
}

impl Packet for ReloadScript { const TYPE: &'static str = "ReloadScript"; }
impl Packet for ReloadLockedScript { const TYPE: &'static str = "ReloadLockedScript"; }
impl Packet for StopScript { const TYPE: &'static str = "StopScript"; }
impl Packet for PauseScript { const TYPE: &'static str = "PauseScript"; }
impl Packet for ResumeScript { const TYPE: &'static str = "ResumeScript"; }
impl Packet for ResumeLockedScript { const TYPE: &'static str = "ResumeLockedScript"; }
impl Packet for StartCapture { const TYPE: &'static str = "StartCapture"; }
impl Packet for StopCapture { const TYPE: &'static str = "StopCapture"; }
impl Packet for LockScript { const TYPE: &'static str = "LockScript"; }
impl Packet for LoadScript { const TYPE: &'static str = "LoadScript"; }
impl Packet for RemoveScriptHistory { const TYPE: &'static str = "RemoveScriptHistory"; }
impl Packet for StateUpdate { const TYPE: &'static str = "StateUpdate"; }

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

impl Packet for ClickCommand {
    const TYPE: &'static str = "ClickCommand";
}

impl Packet for ScrollCommand {
    const TYPE: &'static str = "ScrollCommand";
}

impl Packet for DragCommand {
    const TYPE: &'static str = "DragCommand";
}

impl Packet for PressCommand {
    const TYPE: &'static str = "PressCommand";
}

impl Requestable for StateReq {
    type Response = StateRes;
}

impl Requestable for UiPathReq {
    type Response = UiPathRes;
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
                UiPathReq::TYPE,
                serde_json::to_value(UiPathReq { x: 123, y: -45, width: 1920, height: 1080 }).unwrap(),
            ),
            (
                UiPathRes::TYPE,
                serde_json::to_value(UiPathRes {
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
            (
                ClickCommand::TYPE,
                serde_json::to_value(ClickCommand { x: 1200, y: 80, width: 1920, height: 1080 }).unwrap(),
            ),
            (
                ScrollCommand::TYPE,
                serde_json::to_value(ScrollCommand {
                    x: 600,
                    y: 400,
                    length: -1,
                    axis: 1,
                    width: 1920,
                    height: 1080,
                })
                .unwrap(),
            ),
            (
                DragCommand::TYPE,
                serde_json::to_value(DragCommand {
                    start_x: 1200,
                    start_y: 80,
                    end_x: 600,
                    end_y: 400,
                    width: 1920,
                    height: 1080,
                })
                .unwrap(),
            ),
            (
                PressCommand::TYPE,
                serde_json::to_value(PressCommand { key: "enter".to_owned() }).unwrap(),
            ),
            (ReloadScript::TYPE, serde_json::to_value(ReloadScript {}).unwrap()),
            (ReloadLockedScript::TYPE, serde_json::to_value(ReloadLockedScript {}).unwrap()),
            (StopScript::TYPE, serde_json::to_value(StopScript {}).unwrap()),
            (PauseScript::TYPE, serde_json::to_value(PauseScript {}).unwrap()),
            (ResumeScript::TYPE, serde_json::to_value(ResumeScript {}).unwrap()),
            (ResumeLockedScript::TYPE, serde_json::to_value(ResumeLockedScript {}).unwrap()),
            (StartCapture::TYPE, serde_json::to_value(StartCapture {}).unwrap()),
            (StopCapture::TYPE, serde_json::to_value(StopCapture {}).unwrap()),
            (LockScript::TYPE, serde_json::to_value(LockScript {}).unwrap()),
            (LoadScript::TYPE, serde_json::to_value(LoadScript { path: r"C:\scripts\unity_loop2.js".to_owned(), locked: true }).unwrap()),
            (RemoveScriptHistory::TYPE, serde_json::to_value(RemoveScriptHistory { path: r"C:\scripts\test.js".to_owned() }).unwrap()),
            (StateUpdate::TYPE, serde_json::to_value(StateUpdate {
                phase: ScriptPhase::Paused,
                capture: true,
                locked: false,
                scripts: vec![r"C:\scripts\test.js".to_owned(), r"C:\scripts\unity_loop2.js".to_owned()],
            }).unwrap()),
        ];

        for (packet_type, packet) in packets {
            assert_eq!(packet, fixture[packet_type]);
        }
    }

    #[test]
    fn capture_response_serializes_nullable_fields() {
        assert_eq!(
            serde_json::to_value(UiPathRes {
                target_type: None,
                path: None,
            })
            .unwrap(),
            serde_json::json!({ "type": null, "path": null }),
        );
    }
}
