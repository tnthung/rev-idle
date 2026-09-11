use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScriptCommand {
    Load(PathBuf),
    Reload,
    #[allow(dead_code)]
    Pause,
    Resume,
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
