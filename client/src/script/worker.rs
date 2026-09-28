use super::{lifecycle, ScriptControl};
use crate::{
    app::{ActionGate, PauseUpdate, ScriptCommand, StateUpdate},
    bridge::{QueuedScriptUiEvent, ScriptUiPublisher, WsConnection},
    capture::{CaptureState, LockState},
};
use std::{
    path::PathBuf,
    sync::{
        atomic::AtomicBool,
        mpsc::{self, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
};
use tokio::sync::{mpsc as tokio_mpsc, watch};

pub(crate) struct ScriptWorker {
    completion: Option<Receiver<Result<(), String>>>,
    thread: Option<JoinHandle<()>>,
    control: ScriptControl,
    shutdown: watch::Sender<bool>,
}

struct WaitCancellation {
    control: ScriptControl,
    shutdown: watch::Sender<bool>,
    armed: bool,
}

impl Drop for WaitCancellation {
    fn drop(&mut self) {
        if self.armed {
            self.control.request_internal_terminal();
            self.shutdown.send_replace(true);
        }
    }
}

impl ScriptWorker {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start(
        commands: tokio_mpsc::Receiver<ScriptCommand>,
        ui_events: tokio_mpsc::Receiver<QueuedScriptUiEvent>,
        connection: WsConnection,
        hotkey_pauses: watch::Receiver<PauseUpdate>,
        initial_path: Option<PathBuf>,
        actions_paused: ActionGate,
        shutdown: watch::Receiver<bool>,
        script_running: Arc<AtomicBool>,
        console_locked: Arc<AtomicBool>,
        capture_state: CaptureState,
        lock_state: LockState,
        state_updates: watch::Sender<StateUpdate>,
        script_control: ScriptControl,
        ui_publisher: ScriptUiPublisher,
    ) -> Result<Self, String> {
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let (worker_shutdown, worker_shutdown_rx) = watch::channel(*shutdown.borrow());
        let (lifecycle_shutdown, lifecycle_shutdown_rx) = watch::channel(*shutdown.borrow());
        tokio::spawn(async move {
            let mut shutdown = shutdown;
            let mut worker_shutdown_rx = worker_shutdown_rx;
            tokio::select! {
                _ = async {
                    while !*shutdown.borrow() && shutdown.changed().await.is_ok() {}
                } => {}
                _ = async {
                    while !*worker_shutdown_rx.borrow() && worker_shutdown_rx.changed().await.is_ok() {}
                } => {}
            }
            lifecycle_shutdown.send_replace(true);
        });
        let worker_control = script_control.clone();
        let thread = thread::Builder::new()
            .name("rev-idle-script".to_owned())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = completion_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let local = tokio::task::LocalSet::new();
                let result = runtime.block_on(local.run_until(lifecycle::run_with_control(
                    commands,
                    ui_events,
                    connection,
                    hotkey_pauses,
                    initial_path,
                    actions_paused,
                    lifecycle_shutdown_rx,
                    script_running,
                    console_locked,
                    capture_state,
                    lock_state,
                    state_updates,
                    worker_control,
                    ui_publisher,
                )));
                let _ = completion_tx.send(result);
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            completion: Some(completion_rx),
            thread: Some(thread),
            control: script_control,
            shutdown: worker_shutdown,
        })
    }

    pub(crate) async fn wait(mut self) -> Result<(), String> {
        let completion = self.completion.take().expect("script worker completion must exist");
        let thread = self.thread.take().expect("script worker thread must exist");
        let mut cancellation = WaitCancellation {
            control: self.control.clone(),
            shutdown: self.shutdown.clone(),
            armed: true,
        };
        let result = tokio::task::spawn_blocking(move || {
            let completion = completion.recv().map_err(|error| error.to_string());
            let joined = thread
                .join()
                .map_err(|_| "script worker thread panicked".to_owned());
            match (completion, joined) {
                (_, Err(error)) => Err(error),
                (Err(error), Ok(())) => Err(error),
                (Ok(result), Ok(())) => result,
            }
        })
        .await;
        if result.is_ok() {
            cancellation.armed = false;
        }
        result.map_err(|error| error.to_string())?
    }
}

impl Drop for ScriptWorker {
    fn drop(&mut self) {
        let Some(thread) = self.thread.take() else { return };
        self.control.request_internal_terminal();
        self.shutdown.send_replace(true);
        if thread.is_finished() {
            let _ = thread.join();
        } else {
            let _ = thread::Builder::new()
                .name("rev-idle-script-reaper".to_owned())
                .spawn(move || {
                    let _ = thread.join();
                });
        }
    }
}
