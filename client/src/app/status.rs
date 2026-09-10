use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ScriptPhase { Unloaded, Stopped, Running, Paused }

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct StateUpdate { pub(crate) phase: ScriptPhase, pub(crate) capture: bool }

impl StateUpdate {
    pub(crate) fn new(has_path: bool, script_loaded: bool, paused: bool, capture: bool) -> Self {
        Self {
            phase: if !has_path { ScriptPhase::Unloaded } else if !script_loaded { ScriptPhase::Stopped } else if paused { ScriptPhase::Paused } else { ScriptPhase::Running },
            capture: capture && has_path && (!script_loaded || paused),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_update_projects_lifecycle_and_capture() {
        assert_eq!(StateUpdate::new(false, false, false, false).phase, ScriptPhase::Unloaded);
        assert_eq!(StateUpdate::new(true, false, false, false).phase, ScriptPhase::Stopped);
        assert_eq!(StateUpdate::new(true, true, false, false).phase, ScriptPhase::Running);
        assert_eq!(StateUpdate::new(true, true, true, true).phase, ScriptPhase::Paused);
        assert!(StateUpdate::new(true, false, false, true).capture);
        assert!(!StateUpdate::new(false, false, false, true).capture);
        assert!(!StateUpdate::new(true, true, false, true).capture);
    }

    #[test]
    fn script_phase_serializes_as_lowercase() {
        assert_eq!(serde_json::to_string(&ScriptPhase::Paused).unwrap(), "\"paused\"");
    }
}
