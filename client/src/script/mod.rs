mod loader;
mod transfer;
mod background;
mod history;
mod bindings;
mod session;
mod ownership;
mod mutex;
mod lifecycle;
mod ui;
mod control;
mod worker;

pub(crate) use control::{ScriptControl, SessionControl};
pub(crate) use worker::ScriptWorker;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod worker_tests;

#[cfg(test)]
mod migration_tests;

#[cfg(test)]
mod transfer_tests;

#[cfg(test)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    score: Option<String>,
    sequence: u64,
    received_at_ms: Option<u64>,
}
