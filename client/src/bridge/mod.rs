mod state;
mod invoke;
mod capture;
mod transfer;
mod connection;
mod packets;
#[cfg(test)]
mod test_support;

pub(crate) use packets::{
    CaptureReq, CaptureRes, InvokeReq, InvokeRes, StateReq, StateRes, TransferReq, TransferRes,
};

pub(crate) use state::request_state;
pub(crate) use invoke::invoke;
pub(crate) use capture::{request_capture, CaptureTarget};
pub(crate) use transfer::transfer;
pub(crate) use connection::WsConnection;

#[cfg(test)]
pub(crate) static TEST_SERVER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
