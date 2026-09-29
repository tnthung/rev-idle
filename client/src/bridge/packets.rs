use super::connection::{Packet, Requestable};
use crate::app::{ScriptPhase, StateUpdate};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScriptUiSnapshot {
    pub(crate) session_id: Option<Uuid>,
    pub(crate) revision: u64,
    pub(crate) elements: Vec<ScriptUiElementState>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScriptUiElementState {
    pub(crate) id: String,
    pub(crate) instance_id: Uuid,
    pub(crate) events_version: u64,
    #[serde(default)]
    pub(crate) hidden: bool,
    pub(crate) text: String,
    pub(crate) font: String,
    pub(crate) align_x: String,
    pub(crate) align_y: String,
    pub(crate) pos_x: f64,
    pub(crate) pos_y: f64,
    pub(crate) len_x: ScriptUiLengthState,
    pub(crate) len_y: ScriptUiLengthState,
    pub(crate) color: [u8; 4],
    pub(crate) text_color: [u8; 4],
    pub(crate) border: ScriptUiBorderState,
    pub(crate) corner: ScriptUiCornerState,
    pub(crate) padding: ScriptUiPaddingState,
    pub(crate) events: Vec<ScriptUiEventKind>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScriptUiLengthState {
    pub(crate) fixed: Option<f64>,
    pub(crate) min: f64,
    pub(crate) max: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScriptUiBorderState {
    pub(crate) thickness: f64,
    pub(crate) color: [u8; 4],
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScriptUiCornerState {
    pub(crate) top_left: f64,
    pub(crate) top_right: f64,
    pub(crate) bottom_left: f64,
    pub(crate) bottom_right: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScriptUiPaddingState {
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
    pub(crate) left: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ScriptUiEventKind { Hover, Leave, Click }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScriptUiEvent {
    pub(crate) session_id: Uuid,
    pub(crate) element_id: String,
    pub(crate) instance_id: Uuid,
    pub(crate) events_version: u64,
    pub(crate) event: ScriptUiEventKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ScriptUiPointerPhase { Down, Up }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScriptUiPointer {
    pub(crate) session_id: Uuid,
    pub(crate) press_id: u64,
    pub(crate) phase: ScriptUiPointerPhase,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

impl Packet for ScriptUiSnapshot {
    const TYPE: &'static str = "ScriptUiSnapshot";
}

impl Packet for ScriptUiEvent {
    const TYPE: &'static str = "ScriptUiEvent";
}

impl Packet for ScriptUiPointer {
    const TYPE: &'static str = "ScriptUiPointer";
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScriptUiMeasureReq {
    pub(crate) session_id: Uuid,
    pub(crate) revision: u64,
    pub(crate) element_id: String,
    pub(crate) instance_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScriptUiMeasureRes {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl Packet for ScriptUiMeasureReq {
    const TYPE: &'static str = "ScriptUiMeasureReq";
}

impl Packet for ScriptUiMeasureRes {
    const TYPE: &'static str = "ScriptUiMeasureRes";
}

impl Requestable for ScriptUiMeasureReq {
    type Response = ScriptUiMeasureRes;
}

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
pub(crate) struct InputReq {
    pub(crate) path: String,
    pub(crate) text: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct InputRes {}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ScrollIntoViewReq {
    pub(crate) path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ScrollIntoViewRes {}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TransferReq {
    pub(crate) source: String,
    pub(crate) destination: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TransferRes {}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SlotReq {
    pub(crate) path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SlotRes {
    pub(crate) value: Value,
}

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

impl Packet for InputReq {
    const TYPE: &'static str = "InputReq";
}

impl Packet for InputRes {
    const TYPE: &'static str = "InputRes";
}

impl Packet for ScrollIntoViewReq {
    const TYPE: &'static str = "ScrollIntoViewReq";
}

impl Packet for ScrollIntoViewRes {
    const TYPE: &'static str = "ScrollIntoViewRes";
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

impl Requestable for InputReq {
    type Response = InputRes;
}

impl Requestable for ScrollIntoViewReq {
    type Response = ScrollIntoViewRes;
}

impl Requestable for TransferReq {
    type Response = TransferRes;
}

impl Packet for SlotReq {
    const TYPE: &'static str = "SlotReq";
}

impl Packet for SlotRes {
    const TYPE: &'static str = "SlotRes";
}

impl Requestable for SlotReq {
    type Response = SlotRes;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_ui_packet_payloads_match_shared_fixture() {
        let fixture: Value = serde_json::from_str(include_str!("../../../protocol/fixtures/bridge-packets.json")).unwrap();
        let snapshot: ScriptUiSnapshot = serde_json::from_value(fixture["ScriptUiSnapshot"].clone()).unwrap();
        assert_eq!(snapshot.elements[0].padding.right, 8.0);
        assert_eq!(snapshot.elements[0].corner.top_left, 0.0);
        assert_eq!(snapshot.elements[0].border.color, [0, 255, 0, 128]);
        assert_eq!(serde_json::to_value(snapshot).unwrap(), fixture["ScriptUiSnapshot"]);
        let clear: ScriptUiSnapshot = serde_json::from_value(fixture["ScriptUiClear"].clone()).unwrap();
        assert_eq!(clear.session_id, None);
        assert_eq!(serde_json::to_value(clear).unwrap(), fixture["ScriptUiClear"]);
        let event: ScriptUiEvent = serde_json::from_value(fixture["ScriptUiEvent"].clone()).unwrap();
        assert_eq!(serde_json::to_value(event).unwrap(), fixture["ScriptUiEvent"]);
        let measure: ScriptUiMeasureReq = serde_json::from_value(fixture["ScriptUiMeasureReq"].clone()).unwrap();
        assert_eq!(serde_json::to_value(measure).unwrap(), fixture["ScriptUiMeasureReq"]);
        let measured: ScriptUiMeasureRes = serde_json::from_value(fixture["ScriptUiMeasureRes"].clone()).unwrap();
        assert_eq!(serde_json::to_value(measured).unwrap(), fixture["ScriptUiMeasureRes"]);
        for key in ["ScriptUiPointer", "ScriptUiPointerUp"] {
            let pointer: ScriptUiPointer = serde_json::from_value(fixture[key].clone()).unwrap();
            assert_eq!(serde_json::to_value(pointer).unwrap(), fixture[key]);
        }
    }

    #[test]
    fn script_ui_hidden_defaults_false_when_omitted() {
        let fixture: Value = serde_json::from_str(include_str!("../../../protocol/fixtures/bridge-packets.json")).unwrap();
        let mut element = fixture["ScriptUiSnapshot"]["elements"][0].clone();
        element.as_object_mut().unwrap().remove("hidden");
        let element: ScriptUiElementState = serde_json::from_value(element).unwrap();
        assert_eq!(serde_json::to_value(element).unwrap()["hidden"], false);
    }

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
                InputReq::TYPE,
                serde_json::to_value(InputReq {
                    path: "scene:1/Canvas[0]/Input[0]".to_owned(),
                    text: "123".to_owned(),
                })
                .unwrap(),
            ),
            (InputRes::TYPE, serde_json::to_value(InputRes {}).unwrap()),
            (
                ScrollIntoViewReq::TYPE,
                serde_json::to_value(ScrollIntoViewReq {
                    path: "scene:1/Canvas[0]/Buy DTP & More[0]".to_owned(),
                })
                .unwrap(),
            ),
            (ScrollIntoViewRes::TYPE, serde_json::to_value(ScrollIntoViewRes {}).unwrap()),
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
                SlotReq::TYPE,
                serde_json::to_value(SlotReq { path: "scene:1/Canvas[0]/Inventory/3".to_owned() }).unwrap(),
            ),
            (SlotRes::TYPE, serde_json::to_value(SlotRes { value: serde_json::json!({ "level": 12 }) }).unwrap()),
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
