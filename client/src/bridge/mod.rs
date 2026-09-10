mod state;
mod invoke;
mod input;
mod capture;
mod transfer;
mod connection;
mod packets;
#[cfg(test)]
pub(crate) mod test_support;

pub(crate) use packets::{
    CaptureReq, CaptureRes, ClickCommand, DragCommand, InvokeReq, InvokeRes, PressCommand, ScrollCommand,
    StateReq, StateRes, TransferReq, TransferRes,
};

pub(crate) use state::request_state;
pub(crate) use invoke::invoke;
pub(crate) use input::{click, drag, press, scroll};
pub(crate) use capture::{request_capture, CaptureTarget};
pub(crate) use transfer::transfer;
pub(crate) use connection::WsConnection;
