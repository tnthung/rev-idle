mod state;
mod invoke;
mod input;
mod ui_path;
mod transfer;
mod connection;
mod packets;
mod control;
#[cfg(test)]
pub(crate) mod test_support;

pub(crate) use packets::{
    ClickCommand, DragCommand, InvokeReq, InvokeRes, LoadScript, LockScript, PauseScript, PressCommand, ReloadScript, RemoveScriptHistory,
    ReloadLockedScript, ResumeLockedScript, ResumeScript, ScrollCommand, StartCapture, StateReq, StateRes, StopCapture, StopScript, TransferReq,
    TransferRes, UiPathReq, UiPathRes, SlotReq,
};

pub(crate) use state::request_state;
pub(crate) use invoke::invoke;
pub(crate) use input::{click, drag, press, scroll};
pub(crate) use ui_path::{request_ui_path, UiPathTarget};
pub(crate) use transfer::transfer;
pub(crate) use connection::{ConnectionEvent, WsConnection};
pub(crate) use control::{publish_state, register_control_handlers};
