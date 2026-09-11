use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ScriptPhase { Unloaded, Stopped, Running, Paused }

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct StateUpdate { pub(crate) phase: ScriptPhase, pub(crate) capture: bool, pub(crate) locked: bool }

impl StateUpdate {
    pub(crate) fn new(has_path: bool, script_loaded: bool, paused: bool, capture: bool, locked: bool) -> Self {
        Self {
            phase: if !has_path { ScriptPhase::Unloaded } else if !script_loaded { ScriptPhase::Stopped } else if paused { ScriptPhase::Paused } else { ScriptPhase::Running },
            capture: capture && has_path && (!script_loaded || paused),
            locked: locked && has_path && script_loaded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_update_projects_lifecycle_and_capture() {
        assert_eq!(StateUpdate::new(false, false, false, false, false).phase, ScriptPhase::Unloaded);
        assert_eq!(StateUpdate::new(true, false, false, false, false).phase, ScriptPhase::Stopped);
        assert_eq!(StateUpdate::new(true, true, false, false, false).phase, ScriptPhase::Running);
        assert_eq!(StateUpdate::new(true, true, true, true, false).phase, ScriptPhase::Paused);
        assert!(StateUpdate::new(true, false, false, true, false).capture);
        assert!(!StateUpdate::new(false, false, false, true, false).capture);
        assert!(!StateUpdate::new(true, true, false, true, false).capture);
    }

    #[test]
    fn state_update_projects_locked_only_while_a_script_is_loaded() {
        assert!(StateUpdate::new(true, true, false, false, true).locked);
        assert!(StateUpdate::new(true, true, true, false, true).locked);
        assert!(!StateUpdate::new(true, false, false, false, true).locked);
        assert!(!StateUpdate::new(false, false, false, false, true).locked);
    }

    #[test]
    fn script_phase_serializes_as_lowercase() {
        assert_eq!(serde_json::to_string(&ScriptPhase::Paused).unwrap(), "\"paused\"");
    }
}
