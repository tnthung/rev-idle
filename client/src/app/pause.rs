use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

const PAUSED_BIT: u64 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PauseUpdate {
    state: u64,
}

impl PauseUpdate {
    pub(crate) fn initial() -> Self {
        Self::default()
    }

    pub(crate) fn paused(self) -> bool {
        self.state & PAUSED_BIT != 0
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActionGate {
    state: Arc<AtomicU64>,
}

impl ActionGate {
    pub(crate) fn is_paused(&self) -> bool {
        self.state.load(Ordering::Acquire) & PAUSED_BIT != 0
    }

    pub(crate) fn set_paused(&self, paused: bool) -> PauseUpdate {
        self.update(|_| paused)
    }

    pub(crate) fn toggle(&self) -> PauseUpdate {
        self.update(|paused| !paused)
    }

    pub(crate) fn is_current(&self, update: PauseUpdate) -> bool {
        self.state.load(Ordering::Acquire) == update.state
    }

    pub(crate) fn current_update(&self) -> PauseUpdate {
        PauseUpdate {
            state: self.state.load(Ordering::Acquire),
        }
    }

    pub(crate) fn reset_if_current(&self, update: PauseUpdate) -> bool {
        if !update.paused() {
            return self.is_current(update);
        }
        let next = update.state.wrapping_add(1) & !PAUSED_BIT;
        self.state
            .compare_exchange(
                update.state,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn update(&self, requested: impl Fn(bool) -> bool) -> PauseUpdate {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let paused = requested(current & PAUSED_BIT != 0);
            let next = (current.wrapping_add(2) & !PAUSED_BIT) | u64::from(paused);
            match self.state.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return PauseUpdate { state: next },
                Err(observed) => current = observed,
            }
        }
    }
}
