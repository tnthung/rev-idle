use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex, Weak,
};
use tokio::sync::watch;

struct ControlState {
    next_generation: AtomicU64,
    requested_generation: AtomicU64,
    acknowledged_generation: AtomicU64,
    pending_command_generation: AtomicU64,
    terminal_updates: watch::Sender<u64>,
    active: Mutex<Option<Weak<SessionControlState>>>,
    request_order: Mutex<()>,
}

#[derive(Clone)]
pub(crate) struct ScriptControl {
    state: Arc<ControlState>,
}

impl Default for ScriptControl {
    fn default() -> Self {
        Self {
            state: Arc::new(ControlState {
                next_generation: AtomicU64::new(0),
                requested_generation: AtomicU64::new(0),
                acknowledged_generation: AtomicU64::new(0),
                pending_command_generation: AtomicU64::new(0),
                terminal_updates: watch::channel(0).0,
                active: Mutex::new(None),
                request_order: Mutex::new(()),
            }),
        }
    }
}

impl ScriptControl {
    pub(crate) fn request_terminal(&self) -> u64 {
        let _order = self
            .state
            .request_order
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.request_terminal_locked(true)
    }

    pub(crate) fn request_internal_terminal(&self) -> u64 {
        let _order = self
            .state
            .request_order
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.pending_command_generation() != 0 {
            self.requested_generation()
        } else {
            self.request_terminal_locked(false)
        }
    }

    pub(crate) fn request_forced_terminal(&self) -> u64 {
        let _order = self
            .state
            .request_order
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.state
            .pending_command_generation
            .store(0, Ordering::Release);
        self.request_terminal_locked(false)
    }

    fn request_terminal_locked(&self, command: bool) -> u64 {
        let generation = self
            .state
            .next_generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        self.state.requested_generation.store(generation, Ordering::Release);
        if command {
            self.state
                .pending_command_generation
                .store(generation, Ordering::Release);
        }
        if let Some(active) = self
            .state
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .and_then(Weak::upgrade)
        {
            active.stop();
        }
        self.state.terminal_updates.send_replace(generation);
        generation
    }

    pub(crate) fn abandon_terminal_envelope(&self, generation: u64) {
        let _order = self
            .state
            .request_order
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.pending_command_generation() == generation
            && self.requested_generation() == generation
        {
            self.state
                .pending_command_generation
                .store(0, Ordering::Release);
            // The interrupt is irreversible once requested. Reclassify a
            // canceled enqueue as an internal Stop and let the worker clean
            // up and acknowledge it.
            self.state.terminal_updates.send_replace(generation);
        }
    }

    pub(crate) fn subscribe_terminal(&self) -> watch::Receiver<u64> {
        self.state.terminal_updates.subscribe()
    }

    pub(crate) fn requested_generation(&self) -> u64 {
        self.state.requested_generation.load(Ordering::Acquire)
    }

    pub(crate) fn acknowledged_generation(&self) -> u64 {
        self.state.acknowledged_generation.load(Ordering::Acquire)
    }

    pub(crate) fn pending_command_generation(&self) -> u64 {
        self.state
            .pending_command_generation
            .load(Ordering::Acquire)
    }

    pub(crate) fn terminal_pending(&self) -> bool {
        self.requested_generation() > self.acknowledged_generation()
    }

    pub(crate) fn acknowledge_terminal(&self, generation: u64) {
        let mut acknowledged = self
            .state
            .acknowledged_generation
            .load(Ordering::Acquire);
        while generation > acknowledged {
            match self.state.acknowledged_generation.compare_exchange_weak(
                acknowledged,
                generation,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(observed) => acknowledged = observed,
            }
        }
        let pending = self.pending_command_generation();
        if pending != 0 && generation >= pending {
            let _ = self.state.pending_command_generation.compare_exchange(
                pending,
                0,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }

    pub(crate) fn acknowledge_unpaired_terminal(&self) -> bool {
        let _order = self
            .state
            .request_order
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.pending_command_generation() != 0 || !self.terminal_pending() {
            return false;
        }
        self.acknowledge_terminal(self.requested_generation());
        true
    }

    pub(crate) fn register_session(&self) -> SessionControl {
        let state = Arc::new(SessionControlState::new());
        let session = SessionControl { state: state.clone() };
        *self
            .state
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Arc::downgrade(&state));
        if self.terminal_pending() {
            state.stop();
        }
        session
    }
}

struct SessionControlState {
    stopped: AtomicBool,
    stopped_updates: watch::Sender<bool>,
    paused: AtomicBool,
    paused_updates: watch::Sender<bool>,
}

impl SessionControlState {
    fn new() -> Self {
        let (stopped_updates, _) = watch::channel(false);
        let (paused_updates, _) = watch::channel(false);
        Self {
            stopped: AtomicBool::new(false),
            stopped_updates,
            paused: AtomicBool::new(false),
            paused_updates,
        }
    }

    fn stop(&self) {
        if !self.stopped.swap(true, Ordering::AcqRel) {
            self.stopped_updates.send_replace(true);
        }
    }
}

#[derive(Clone)]
pub(crate) struct SessionControl {
    state: Arc<SessionControlState>,
}

impl SessionControl {
    #[cfg(test)]
    pub(crate) fn standalone() -> Self {
        Self { state: Arc::new(SessionControlState::new()) }
    }

    pub(super) fn is_stopped(&self) -> bool {
        self.state.stopped.load(Ordering::Acquire)
    }

    pub(super) fn stop(&self) {
        self.state.stop();
    }

    pub(super) fn subscribe_stopped(&self) -> watch::Receiver<bool> {
        self.state.stopped_updates.subscribe()
    }

    pub(super) fn is_paused(&self) -> bool {
        self.state.paused.load(Ordering::Acquire)
    }

    pub(super) fn subscribe_paused(&self) -> watch::Receiver<bool> {
        self.state.paused_updates.subscribe()
    }

    pub(super) fn acknowledge_pause(&self, paused: bool) {
        if self.state.paused.swap(paused, Ordering::AcqRel) != paused {
            self.state.paused_updates.send_replace(paused);
        }
    }

    pub(super) fn error_message() -> &'static str {
        "script session stopped"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_generations_cannot_clear_newer_stop() {
        let control = ScriptControl::default();
        let first = control.request_terminal();
        let second = control.request_terminal();
        control.acknowledge_terminal(first);
        assert!(control.terminal_pending());
        assert_eq!(control.requested_generation(), second);
        control.acknowledge_terminal(second);
        assert!(!control.terminal_pending());
    }

    #[test]
    fn obsolete_session_stop_does_not_stop_replacement() {
        let control = ScriptControl::default();
        let first = control.register_session();
        let replacement = control.register_session();
        first.stop();
        assert!(first.is_stopped());
        assert!(!replacement.is_stopped());
    }

    #[test]
    fn internal_terminal_does_not_overtake_a_pending_envelope() {
        let control = ScriptControl::default();
        let external = control.request_terminal();
        assert_eq!(control.request_internal_terminal(), external);
        assert_eq!(control.requested_generation(), external);
        assert_eq!(control.pending_command_generation(), external);
        control.acknowledge_terminal(external);
        assert!(!control.terminal_pending());
    }

    #[test]
    fn unpaired_ack_cannot_acknowledge_an_external_envelope() {
        let control = ScriptControl::default();
        let internal = control.request_internal_terminal();
        assert!(control.acknowledge_unpaired_terminal());
        assert_eq!(control.acknowledged_generation(), internal);

        let external = control.request_terminal();
        assert!(!control.acknowledge_unpaired_terminal());
        assert_eq!(control.pending_command_generation(), external);
        assert!(control.acknowledged_generation() < external);
    }

    #[test]
    fn forced_terminal_supersedes_a_queued_envelope() {
        let control = ScriptControl::default();
        let external = control.request_terminal();
        let forced = control.request_forced_terminal();
        assert!(forced > external);
        assert_eq!(control.pending_command_generation(), 0);
        assert!(control.acknowledge_unpaired_terminal());
        assert!(external <= control.acknowledged_generation());
    }

    #[test]
    fn pause_acknowledgment_is_session_local_and_published() {
        let control = ScriptControl::default();
        let first = control.register_session();
        let mut first_updates = first.subscribe_paused();
        let replacement = control.register_session();

        assert!(!first.is_paused());
        assert!(!replacement.is_paused());
        first.acknowledge_pause(true);
        assert!(first.is_paused());
        assert!(!replacement.is_paused());
        assert!(first_updates.has_changed().unwrap());
        assert!(*first_updates.borrow_and_update());

        first.acknowledge_pause(false);
        assert!(!first.is_paused());
        assert!(first_updates.has_changed().unwrap());
        assert!(!*first_updates.borrow_and_update());
    }
}
