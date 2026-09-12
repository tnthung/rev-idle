use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScriptCommand {
    Load(PathBuf),
    LoadLocked(PathBuf),
    Reload,
    ReloadLocked,
    #[allow(dead_code)]
    Pause,
    Resume,
    ResumeLocked,
    Stop,
    Capture,
    StartCapture,
    StopCapture,
    CaptureConsumed,
    Lock,
    Exit,
    #[allow(dead_code)]
    SetPaused(bool),
}
