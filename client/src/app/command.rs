use std::path::PathBuf;

use crate::script::ScriptControl;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::sync::mpsc;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScriptCommand {
    Load(PathBuf),
    LoadLocked(PathBuf),
    RemoveFromHistory(PathBuf),
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
    Terminal {
        command: Box<ScriptCommand>,
        generation: u64,
    },
}

#[derive(Clone)]
pub(crate) struct ScriptCommandSender {
    sender: mpsc::Sender<ScriptCommand>,
    control: ScriptControl,
    _producer: Arc<ScriptCommandProducer>,
}

struct ScriptCommandProducer {
    control: ScriptControl,
}

impl Drop for ScriptCommandProducer {
    fn drop(&mut self) {
        self.control.request_forced_terminal();
    }
}

struct TerminalRequest {
    control: ScriptControl,
    generation: u64,
    delivered: bool,
}

impl Drop for TerminalRequest {
    fn drop(&mut self) {
        if !self.delivered {
            self.control.abandon_terminal_envelope(self.generation);
        }
    }
}

pub(crate) trait ScriptCommandSink: Clone + Send + Sync + 'static {
    fn send(&self, command: ScriptCommand) -> Pin<Box<dyn Future<Output = ()> + Send>>;

    fn blocking_send(&self, command: ScriptCommand);
}

impl ScriptCommandSender {
    pub(crate) fn new(sender: mpsc::Sender<ScriptCommand>, control: ScriptControl) -> Self {
        Self {
            sender,
            control: control.clone(),
            _producer: Arc::new(ScriptCommandProducer { control }),
        }
    }

    pub(crate) async fn send(&self, command: ScriptCommand) -> Result<(), mpsc::error::SendError<ScriptCommand>> {
        let mut terminal = None;
        let command = if matches!(
            &command,
            ScriptCommand::Load(_)
                | ScriptCommand::LoadLocked(_)
                | ScriptCommand::Reload
                | ScriptCommand::ReloadLocked
                | ScriptCommand::Stop
                | ScriptCommand::Exit
        ) {
            let generation = self.control.request_terminal();
            terminal = Some(TerminalRequest {
                control: self.control.clone(),
                generation,
                delivered: false,
            });
            ScriptCommand::Terminal {
                command: Box::new(command),
                generation,
            }
        } else {
            command
        };
        let result = self.sender.send(command).await;
        if result.is_ok()
            && let Some(terminal) = terminal.as_mut()
        {
            terminal.delivered = true;
        }
        result
    }

    pub(crate) fn blocking_send(&self, command: ScriptCommand) -> Result<(), mpsc::error::SendError<ScriptCommand>> {
        let mut terminal = None;
        let command = if matches!(
            &command,
            ScriptCommand::Load(_)
                | ScriptCommand::LoadLocked(_)
                | ScriptCommand::Reload
                | ScriptCommand::ReloadLocked
                | ScriptCommand::Stop
                | ScriptCommand::Exit
        ) {
            let generation = self.control.request_terminal();
            terminal = Some(TerminalRequest {
                control: self.control.clone(),
                generation,
                delivered: false,
            });
            ScriptCommand::Terminal {
                command: Box::new(command),
                generation,
            }
        } else {
            command
        };
        let result = self.sender.blocking_send(command);
        if result.is_ok()
            && let Some(terminal) = terminal.as_mut()
        {
            terminal.delivered = true;
        }
        result
    }
}

impl ScriptCommandSink for ScriptCommandSender {
    fn send(&self, command: ScriptCommand) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let sender = self.clone();
        Box::pin(async move {
            let _ = ScriptCommandSender::send(&sender, command).await;
        })
    }

    fn blocking_send(&self, command: ScriptCommand) {
        let _ = ScriptCommandSender::blocking_send(self, command);
    }
}

impl ScriptCommandSink for mpsc::Sender<ScriptCommand> {
    fn send(&self, command: ScriptCommand) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let sender = self.clone();
        Box::pin(async move {
            let _ = sender.send(command).await;
        })
    }

    fn blocking_send(&self, command: ScriptCommand) {
        let _ = mpsc::Sender::blocking_send(self, command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn terminal_signal_precedes_a_blocked_enqueue() {
        let control = ScriptControl::default();
        let (tx, mut rx) = mpsc::channel(1);
        tx.send(ScriptCommand::Load(PathBuf::from("occupied"))).await.unwrap();
        let sender = ScriptCommandSender::new(tx, control.clone());
        let mut blocked = Box::pin(tokio::spawn(async move {
            sender.send(ScriptCommand::Stop).await
        }));
        tokio::task::yield_now().await;
        assert!(control.terminal_pending());
        assert!(timeout(Duration::from_millis(10), &mut blocked).await.is_err());
        assert_eq!(rx.recv().await, Some(ScriptCommand::Load(PathBuf::from("occupied"))));
        assert!(blocked.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn canceled_terminal_enqueue_becomes_an_internal_stop() {
        let control = ScriptControl::default();
        let (tx, _rx) = mpsc::channel(1);
        tx.send(ScriptCommand::Pause).await.unwrap();
        let sender = ScriptCommandSender::new(tx, control.clone());
        let keepalive = sender.clone();
        let blocked = tokio::spawn(async move { sender.send(ScriptCommand::Stop).await });
        tokio::task::yield_now().await;
        assert!(control.terminal_pending());
        blocked.abort();
        assert!(blocked.await.unwrap_err().is_cancelled());
        let generation = control.requested_generation();
        assert!(control.terminal_pending());
        assert_eq!(control.pending_command_generation(), 0);
        control.acknowledge_terminal(generation);
        assert!(!control.terminal_pending());
        drop(keepalive);
    }

    #[tokio::test]
    async fn canceled_newer_terminal_supersedes_an_older_envelope_without_wedging() {
        let control = ScriptControl::default();
        let (tx, mut rx) = mpsc::channel(1);
        tx.send(ScriptCommand::Pause).await.unwrap();
        let sender = ScriptCommandSender::new(tx, control.clone());
        let keepalive = sender.clone();
        let first_sender = sender.clone();
        let first = tokio::spawn(async move {
            first_sender.send(ScriptCommand::Load(PathBuf::from("older.js"))).await
        });
        tokio::task::yield_now().await;
        let older_generation = control.requested_generation();
        let second = tokio::spawn(async move { sender.send(ScriptCommand::Stop).await });
        tokio::task::yield_now().await;
        let stop_generation = control.requested_generation();
        assert!(stop_generation > older_generation);

        second.abort();
        assert!(second.await.unwrap_err().is_cancelled());
        assert!(control.terminal_pending());
        assert_eq!(control.pending_command_generation(), 0);
        assert_eq!(rx.recv().await, Some(ScriptCommand::Pause));
        assert!(first.await.unwrap().is_ok());
        assert_eq!(
            rx.recv().await,
            Some(ScriptCommand::Terminal {
                command: Box::new(ScriptCommand::Load(PathBuf::from("older.js"))),
                generation: older_generation,
            }),
        );

        control.acknowledge_terminal(stop_generation);
        assert!(!control.terminal_pending());
        drop(keepalive);
    }

    #[test]
    fn dropping_the_last_command_producer_signals_an_internal_terminal() {
        let control = ScriptControl::default();
        let (tx, _rx) = mpsc::channel(1);
        let sender = ScriptCommandSender::new(tx, control.clone());
        drop(sender);
        assert!(control.terminal_pending());
        assert_eq!(control.pending_command_generation(), 0);
    }
}
