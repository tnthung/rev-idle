mod loader;
mod history;
mod bindings;
mod session;
mod ownership;
mod lifecycle;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    score: Option<String>,
    sequence: u64,
    received_at_ms: Option<u64>,
}

pub(crate) use lifecycle::run;
