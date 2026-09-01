# Client Telemetry and Script Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Build a Rust client that publishes the latest valid UDP telemetry through a watch channel and runs one console-controlled QuickJS automation function with explicit (rev, memory) arguments.

**Architecture:** A UDP producer task deserializes score packets and replaces the value in tokio::sync::watch, while a console producer sends ordered lifecycle commands through tokio::sync::mpsc. One local JavaScript task owns both receivers and the entire QuickJS session; it takes one State snapshot per invocation, constructs a fresh rev object, reuses the same memory object, invokes the retained function, awaits it, and then sleeps for 50 ms.

**Tech Stack:** Rust 2024, Tokio 1.53.1, rquickjs 0.12.2 with full-async, serde 1.0.229, serde_json 1.0.151, Enigo 0.6.1, Windows.

**Spec:** docs/superpowers/specs/2026-09-01-client-telemetry-script-runtime-design.md

## Global Constraints

- Bind telemetry only to 127.0.0.1:19841.
- Preserve score as a String; never convert it through f64.
- Use tokio::sync::watch<State> for latest-state delivery. Do not add Arc<RwLock<State>> or another application-level state lock.
- Use tokio::sync::mpsc<ScriptCommand> for console commands because every command must retain FIFO order.
- Evaluate the script file once per load or reload. Repeatedly call the retained function as script(rev, memory); do not evaluate the source on every loop.
- Construct a fresh frozen rev object for each invocation. rev.state is a frozen snapshot and does not change during that invocation.
- Preserve the QuickJS context, globals, loaded function, and memory through successful calls, call errors, pause, and resume.
- Drop all persistent QuickJS roots before dropping AsyncContext and AsyncRuntime on load, reload, and stop.
- Apply console commands only at the top of the main script loop. Never interrupt an active JavaScript call.
- After every successful or failed script invocation, await tokio::time::sleep(Duration::from_millis(50)) before returning to the top of the main loop.
- Keep the QuickJS session on a Tokio current-thread runtime and LocalSet. Do not enable rquickjs parallel support.
- Do not add game-window discovery, focus, resize, or move operations.
- Do not create a function for a one-off sequence merely to name it. Extract only for reuse, focused tests, a required callback/task entry point, independent ownership, or a necessary borrow/lifetime boundary.
- The MouseInput trait and run_with_mouse test seam are allowed because automated tests must never move the real cursor.
- Do not run cargo fmt. Use targeted edits and non-mutating compiler/test checks.
- Do not edit or delete client/src/__tmp.rs; it is scratch pseudocode, not production code.
- Preserve the user's unrelated plugin/README.md, plugin/tests/InstallPlugin.Tests.ps1, and plugin/install.cmd changes.

## File Map

- Modify client/src/main.rs: remove the print-only listener and coordinate the UDP, console, and local JavaScript tasks.
- Create client/src/udp.rs: own State, UDP wire deserialization, loopback receive loop, sequence/timestamp generation, and watch sends.
- Create client/src/console.rs: own ScriptCommand, command parsing, and asynchronous stdin forwarding.
- Create client/src/script.rs: own QuickJS persistent roots, per-invocation rev construction, Enigo host bindings, lifecycle state, channel consumption, and the 50 ms loop delay.
- No Cargo.toml dependency change is expected; all required crates and features are already present.

---

### Task 1: Publish latest UDP telemetry through a watch channel

**Files:**
- Create: client/src/udp.rs
- Modify: client/src/main.rs
- Test: inline tests in client/src/udp.rs

**Interfaces:**
- Consumes: UDP datagrams containing {"score":"<exact string>"} and tokio::sync::watch::Sender<State>.
- Produces:

~~~rust
pub const LISTEN_ADDRESS: &str = "127.0.0.1:19841";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub score: Option<String>,
    pub sequence: u64,
    pub received_at_ms: Option<u64>,
}

pub async fn run(state_tx: tokio::sync::watch::Sender<State>)
    -> std::io::Result<()>;
~~~

- The private receive_one function is justified because the production loop calls it repeatedly and real-socket tests invoke it directly.

- [ ] **Step 1: Add failing watch-channel UDP tests**

Add mod udp; at the top of client/src/main.rs. Create client/src/udp.rs with this test module before adding the production definitions:

~~~rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{net::UdpSocket, sync::watch};

    async fn deliver(payload: &[u8], initial: State) -> State {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let (state_tx, state_rx) = watch::channel(initial.clone());
        let mut sequence = initial.sequence;
        let mut buffer = [0_u8; 65_535];

        sender
            .send_to(payload, receiver.local_addr().unwrap())
            .await
            .unwrap();

        receive_one(
            &receiver,
            &state_tx,
            &mut sequence,
            &mut buffer,
        )
        .await
        .unwrap();

        state_rx.borrow().clone()
    }

    #[test]
    fn default_state_is_empty() {
        assert_eq!(State::default().score, None);
        assert_eq!(State::default().sequence, 0);
        assert_eq!(State::default().received_at_ms, None);
    }

    #[tokio::test]
    async fn valid_packet_replaces_state_without_losing_score_precision() {
        let state = deliver(
            br#"{"score":"1.2345678901234567e123","extra":true}"#,
            State::default(),
        )
        .await;

        assert_eq!(
            state.score.as_deref(),
            Some("1.2345678901234567e123")
        );
        assert_eq!(state.sequence, 1);
        assert!(state.received_at_ms.is_some());
    }

    #[tokio::test]
    async fn malformed_packets_preserve_previous_state() {
        let previous = State {
            score: Some("9.5e42".to_owned()),
            sequence: 7,
            received_at_ms: Some(123),
        };

        for payload in [
            b"not-json".as_slice(),
            br#"{"score":42}"#.as_slice(),
            &[b'f', 0xff, b'o'],
        ] {
            assert_eq!(deliver(payload, previous.clone()).await, previous);
        }
    }

    #[tokio::test]
    async fn latest_valid_packet_wins_and_sequence_increments() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let (state_tx, state_rx) = watch::channel(State::default());
        let mut sequence = 0;
        let mut buffer = [0_u8; 65_535];

        for payload in [
            br#"{"score":"1e1"}"#.as_slice(),
            br#"{"score":"2e2"}"#.as_slice(),
        ] {
            sender
                .send_to(payload, receiver.local_addr().unwrap())
                .await
                .unwrap();
            receive_one(
                &receiver,
                &state_tx,
                &mut sequence,
                &mut buffer,
            )
            .await
            .unwrap();
        }

        let state = state_rx.borrow().clone();
        assert_eq!(state.score.as_deref(), Some("2e2"));
        assert_eq!(state.sequence, 2);
    }
}
~~~

- [ ] **Step 2: Run the tests and verify RED**

Run:

~~~powershell
cargo test --manifest-path client/Cargo.toml udp::tests
~~~

Expected: exit code 101 because State and receive_one do not exist. Correct syntax or module-loading failures until those missing production definitions are the cause.

- [ ] **Step 3: Implement State and the UDP producer**

Place this production code above the test module in client/src/udp.rs:

~~~rust
use serde::Deserialize;
use std::{
    io,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{net::UdpSocket, sync::watch};

pub const LISTEN_ADDRESS: &str = "127.0.0.1:19841";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub score: Option<String>,
    pub sequence: u64,
    pub received_at_ms: Option<u64>,
}

#[derive(Deserialize)]
struct ScorePayload {
    score: String,
}

async fn receive_one(
    socket: &UdpSocket,
    state_tx: &watch::Sender<State>,
    sequence: &mut u64,
    buffer: &mut [u8],
) -> io::Result<()> {
    let (length, source) = socket.recv_from(buffer).await?;
    let payload = match serde_json::from_slice::<ScorePayload>(&buffer[..length]) {
        Ok(payload) => payload,
        Err(error) => {
            eprintln!("ignored invalid telemetry from {source}: {error}");
            return Ok(());
        }
    };

    *sequence = (*sequence).saturating_add(1);
    let received_at_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX);

    state_tx
        .send(State {
            score: Some(payload.score),
            sequence: *sequence,
            received_at_ms: Some(received_at_ms),
        })
        .map_err(|_| io::Error::other("script state receiver closed"))?;

    Ok(())
}

pub async fn run(state_tx: watch::Sender<State>) -> io::Result<()> {
    let socket = UdpSocket::bind(LISTEN_ADDRESS).await?;
    let mut sequence = 0;
    let mut buffer = [0_u8; 65_535];

    loop {
        receive_one(
            &socket,
            &state_tx,
            &mut sequence,
            &mut buffer,
        )
        .await?;
    }
}
~~~

Keep parsing, timestamp creation, and watch sending inline inside receive_one. Do not add separate one-use parsing or timestamp wrapper functions.

- [ ] **Step 4: Replace the print-only main with a watch-channel intermediate**

Replace client/src/main.rs with:

~~~rust
mod udp;

use std::io;
use tokio::sync::watch;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (state_tx, mut state_rx) = watch::channel(udp::State::default());
    let receiver = tokio::spawn(udp::run(state_tx));

    while state_rx.changed().await.is_ok() {
        println!("{:?}", state_rx.borrow_and_update());
    }

    receiver
        .await
        .map_err(|error| io::Error::other(error.to_string()))?
}
~~~

This is an independently runnable Task 1 deliverable. Task 4 will replace only its orchestration body; the UDP behavior remains unchanged.

- [ ] **Step 5: Run Task 1 validation**

Run:

~~~powershell
cargo test --manifest-path client/Cargo.toml udp::tests
cargo check --manifest-path client/Cargo.toml
~~~

Expected: four UDP tests pass and cargo check exits 0. Compiler warnings in the new production module must be resolved without source-wide formatting.

- [ ] **Step 6: Commit Task 1**

Run:

~~~powershell
git add -- client/src/main.rs client/src/udp.rs
git commit -m "feat: publish latest telemetry state"
~~~

Do not stage plugin files or client/src/__tmp.rs.

---

### Task 2: Parse and forward console lifecycle commands

**Files:**
- Create: client/src/console.rs
- Modify: client/src/main.rs
- Test: inline tests in client/src/console.rs

**Interfaces:**
- Consumes: newline-delimited stdin text.
- Produces:

~~~rust
#[derive(Debug, PartialEq, Eq)]
pub enum ScriptCommand {
    Load(std::path::PathBuf),
    Reload,
    Pause,
    Resume,
    Stop,
}

pub fn parse_command(line: &str) -> Result<ScriptCommand, String>;

pub async fn run(
    command_tx: tokio::sync::mpsc::Sender<ScriptCommand>,
) -> std::io::Result<()>;
~~~

- parse_command is extracted because it has focused deterministic tests.
- run is extracted because it is the console task entry point.

- [ ] **Step 1: Write failing command parser tests**

Add mod console; beside mod udp; in client/src/main.rs. Create client/src/console.rs with:

~~~rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_lifecycle_commands() {
        assert_eq!(parse_command("reload"), Ok(ScriptCommand::Reload));
        assert_eq!(parse_command(" pause "), Ok(ScriptCommand::Pause));
        assert_eq!(parse_command("resume"), Ok(ScriptCommand::Resume));
        assert_eq!(parse_command("stop"), Ok(ScriptCommand::Stop));
    }

    #[test]
    fn load_uses_the_entire_remaining_path() {
        assert_eq!(
            parse_command(r#"load C:\My Scripts\farm.js"#),
            Ok(ScriptCommand::Load(PathBuf::from(
                r#"C:\My Scripts\farm.js"#
            )))
        );
        assert_eq!(
            parse_command(r#"load "C:\My Scripts\farm.js""#),
            Ok(ScriptCommand::Load(PathBuf::from(
                r#"C:\My Scripts\farm.js"#
            )))
        );
    }

    #[test]
    fn rejects_unknown_commands_and_empty_load_paths() {
        assert!(parse_command("").is_err());
        assert!(parse_command("start").is_err());
        assert!(parse_command("load").is_err());
        assert!(parse_command(r#"load """#).is_err());
        assert!(parse_command("pause now").is_err());
    }
}
~~~

- [ ] **Step 2: Run the parser tests and verify RED**

Run:

~~~powershell
cargo test --manifest-path client/Cargo.toml console::tests
~~~

Expected: exit code 101 because ScriptCommand and parse_command do not exist.

- [ ] **Step 3: Implement command parsing and stdin forwarding**

Place this production code above the test module:

~~~rust
use std::{io, path::PathBuf};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc,
};

const USAGE: &str = "commands: load <script-path> | reload | pause | resume | stop";

#[derive(Debug, PartialEq, Eq)]
pub enum ScriptCommand {
    Load(PathBuf),
    Reload,
    Pause,
    Resume,
    Stop,
}

pub fn parse_command(line: &str) -> Result<ScriptCommand, String> {
    let input = line.trim();
    let command_end = input.find(char::is_whitespace).unwrap_or(input.len());
    let (name, remainder) = input.split_at(command_end);
    let argument = remainder.trim();

    match name {
        "reload" if argument.is_empty() => Ok(ScriptCommand::Reload),
        "pause" if argument.is_empty() => Ok(ScriptCommand::Pause),
        "resume" if argument.is_empty() => Ok(ScriptCommand::Resume),
        "stop" if argument.is_empty() => Ok(ScriptCommand::Stop),
        "load" => {
            let path = if argument.len() >= 2
                && argument.starts_with('"')
                && argument.ends_with('"')
            {
                &argument[1..argument.len() - 1]
            } else {
                argument
            };

            if path.is_empty() {
                Err(USAGE.to_owned())
            } else {
                Ok(ScriptCommand::Load(PathBuf::from(path)))
            }
        }
        _ => Err(USAGE.to_owned()),
    }
}

pub async fn run(
    command_tx: mpsc::Sender<ScriptCommand>,
) -> io::Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    while let Some(line) = lines.next_line().await? {
        match parse_command(&line) {
            Ok(command) => {
                if command_tx.send(command).await.is_err() {
                    return Ok(());
                }
            }
            Err(message) => eprintln!("{message}"),
        }
    }

    Ok(())
}
~~~

Do not add a separate quote-stripping function; it has only this call site and is clearer inline.

- [ ] **Step 4: Run Task 2 validation**

Run:

~~~powershell
cargo test --manifest-path client/Cargo.toml console::tests
cargo test --manifest-path client/Cargo.toml
~~~

Expected: all console and UDP tests pass. console::run may remain unused until Task 4; do not suppress that temporary warning with an allow attribute.

- [ ] **Step 5: Commit Task 2**

Run:

~~~powershell
git add -- client/src/main.rs client/src/console.rs
git commit -m "feat: parse script console commands"
~~~

---

### Task 3: Create the persistent QuickJS session and explicit host arguments

**Files:**
- Create: client/src/script.rs
- Modify: client/src/main.rs
- Test: inline tests in client/src/script.rs

**Interfaces:**
- Consumes: JavaScript source, one State per invocation, and a MouseInput test seam.
- Produces:

~~~rust
trait MouseInput {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: enigo::Button,
    ) -> Result<(), String>;
}

struct ScriptSession {
    script: rquickjs::Persistent<rquickjs::Function<'static>>,
    memory: rquickjs::Persistent<rquickjs::Object<'static>>,
    context: rquickjs::AsyncContext,
    _runtime: rquickjs::AsyncRuntime,
}

impl ScriptSession {
    async fn new(source: &str) -> rquickjs::Result<Self>;

    async fn invoke(
        &self,
        state: crate::udp::State,
        mouse: std::rc::Rc<std::cell::RefCell<dyn MouseInput>>,
    ) -> rquickjs::Result<()>;
}
~~~

- ScriptSession is justified by ownership and drop-order requirements.
- MouseInput is justified solely by safe automated testing.
- new is reused by initial load, load, reload, and tests.
- invoke is called for every script iteration and directly tested.

- [ ] **Step 1: Add failing session tests with a fake mouse**

Add mod script; to client/src/main.rs. Create client/src/script.rs with this initial test module:

~~~rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::udp::State;
    use enigo::Button;
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Click {
        x: i32,
        y: i32,
        button: Button,
    }

    struct FakeMouse {
        clicks: Rc<RefCell<Vec<Click>>>,
    }

    impl MouseInput for FakeMouse {
        fn click_at(
            &mut self,
            x: i32,
            y: i32,
            button: Button,
        ) -> Result<(), String> {
            self.clicks.borrow_mut().push(Click { x, y, button });
            Ok(())
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn passes_fresh_state_and_preserves_memory_and_globals() {
        let source = r#"
            (async (rev, memory) => {
                if (!Object.isFrozen(rev) || !Object.isFrozen(rev.state)) {
                    throw new Error("rev and state must be frozen");
                }
                memory.count = (memory.count ?? 0) + 1;
                globalThis.count = (globalThis.count ?? 0) + 1;
                if (memory.count !== globalThis.count) {
                    throw new Error("persistent state mismatch");
                }
                if (rev.state.sequence !== memory.count) {
                    throw new Error("stale invocation state");
                }
                if (memory.count === 2) {
                    rev.click(-12, 34, "right");
                }
                const sequence = rev.state.sequence;
                await rev.sleep(1);
                if (rev.state.sequence !== sequence) {
                    throw new Error("state changed during invocation");
                }
            })
        "#;

        let session = ScriptSession::new(source).await.unwrap();
        let clicks = Rc::new(RefCell::new(Vec::new()));
        let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
            clicks: clicks.clone(),
        }));

        session
            .invoke(
                State {
                    score: Some("1e1".to_owned()),
                    sequence: 1,
                    received_at_ms: Some(10),
                },
                mouse.clone(),
            )
            .await
            .unwrap();
        session
            .invoke(
                State {
                    score: Some("2e2".to_owned()),
                    sequence: 2,
                    received_at_ms: Some(20),
                },
                mouse,
            )
            .await
            .unwrap();

        assert_eq!(
            clicks.borrow().as_slice(),
            &[Click {
                x: -12,
                y: 34,
                button: Button::Right,
            }]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invocation_error_does_not_reset_memory() {
        let session = ScriptSession::new(
            r#"
                ((rev, memory) => {
                    memory.count = (memory.count ?? 0) + 1;
                    if (memory.count === 1) {
                        throw new Error("first call");
                    }
                    if (memory.count !== 2) {
                        throw new Error("memory was reset");
                    }
                })
            "#,
        )
        .await
        .unwrap();
        let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
            clicks: Rc::new(RefCell::new(Vec::new())),
        }));

        assert!(
            session
                .invoke(State::default(), mouse.clone())
                .await
                .is_err()
        );
        session.invoke(State::default(), mouse).await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sleep_is_awaited_and_non_callable_source_is_rejected() {
        let session = ScriptSession::new(
            "(async (rev, memory) => { await rev.sleep(20); })",
        )
        .await
        .unwrap();
        let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
            clicks: Rc::new(RefCell::new(Vec::new())),
        }));
        let started = Instant::now();

        let invocation = session.invoke(State::default(), mouse);
        tokio::pin!(invocation);
        tokio::select! {
            result = &mut invocation => {
                panic!("sleep completed before yielding: {result:?}")
            }
            _ = tokio::time::sleep(Duration::from_millis(1)) => {}
        }
        invocation.await.unwrap();

        assert!(started.elapsed() >= Duration::from_millis(20));
        assert!(ScriptSession::new("42").await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invalid_host_arguments_throw_without_clicking() {
        let clicks = Rc::new(RefCell::new(Vec::new()));

        for source in [
            r#"((rev) => rev.click(1, 2, "side"))"#,
            r#"((rev) => rev.click(1.5, 2, "left"))"#,
            r#"((rev) => rev.sleep(-1))"#,
            r#"((rev) => rev.sleep(1.5))"#,
        ] {
            let session = ScriptSession::new(source).await.unwrap();
            let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
                clicks: clicks.clone(),
            }));
            assert!(session.invoke(State::default(), mouse).await.is_err());
        }

        assert!(clicks.borrow().is_empty());
    }
}
~~~

- [ ] **Step 2: Run the session tests and verify RED**

Run:

~~~powershell
cargo test --manifest-path client/Cargo.toml script::tests
~~~

Expected: exit code 101 because MouseInput and ScriptSession do not exist.

- [ ] **Step 3: Implement the mouse seam and persistent session roots**

Add these imports and definitions above the tests:

~~~rust
use crate::udp::State;
use enigo::{Button, Coordinate, Direction, Enigo, Mouse};
use rquickjs::{
    function::Async,
    promise::MaybePromise,
    AsyncContext,
    AsyncRuntime,
    Error,
    Function,
    Object,
    Persistent,
    Value,
};
use std::{cell::RefCell, rc::Rc, time::Duration};

trait MouseInput {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), String>;
}

impl MouseInput for Enigo {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), String> {
        self.move_mouse(x, y, Coordinate::Abs)
            .map_err(|error| error.to_string())?;
        self.button(button, Direction::Click)
            .map_err(|error| error.to_string())
    }
}

type SharedMouse = Rc<RefCell<dyn MouseInput>>;

struct ScriptSession {
    // Rust drops fields in declaration order. Persistent roots must be gone
    // before their context and runtime.
    script: Persistent<Function<'static>>,
    memory: Persistent<Object<'static>>,
    context: AsyncContext,
    _runtime: AsyncRuntime,
}
~~~

Do not wrap individual Enigo calls in additional functions. The trait method is the required test boundary.

- [ ] **Step 4: Implement session creation**

Add:

~~~rust
impl ScriptSession {
    async fn new(source: &str) -> rquickjs::Result<Self> {
        let runtime = AsyncRuntime::new()?;
        let context = AsyncContext::full(&runtime).await?;
        let source = source.to_owned();

        let (script, memory) = context
            .async_with(async move |ctx| {
                let script: Function = ctx.eval(source)?;
                let memory = Object::new(ctx.clone())?;

                Ok((
                    Persistent::save(&ctx, script),
                    Persistent::save(&ctx, memory),
                ))
            })
            .await?;

        Ok(Self {
            script,
            memory,
            context,
            _runtime: runtime,
        })
    }
}
~~~

Keep source reading outside this method. This method owns only QuickJS session construction and is reusable from tests and all load paths.

- [ ] **Step 5: Implement one explicit (rev, memory) invocation**

Add invoke to the same impl block:

~~~rust
async fn invoke(
    &self,
    state: State,
    mouse: SharedMouse,
) -> rquickjs::Result<()> {
    let script = self.script.clone();
    let memory = self.memory.clone();

    self.context
        .async_with(async move |ctx| {
            let script: Function = script.restore(&ctx)?;
            let memory: Object = memory.restore(&ctx)?;

            let state_object = Object::new(ctx.clone())?;
            state_object.set("score", state.score)?;
            state_object.set("sequence", state.sequence)?;
            state_object.set("receivedAtMs", state.received_at_ms)?;

            let rev = Object::new(ctx.clone())?;
            rev.set("state", state_object.clone())?;

            let click_mouse = mouse.clone();
            rev.set(
                "click",
                Function::new(
                    ctx.clone(),
                    move |x: f64, y: f64, button: Option<String>| {
                        if !x.is_finite()
                            || x.fract() != 0.0
                            || x < i32::MIN as f64
                            || x > i32::MAX as f64
                            || !y.is_finite()
                            || y.fract() != 0.0
                            || y < i32::MIN as f64
                            || y > i32::MAX as f64
                        {
                            return Err(Error::new_from_js_message(
                                "number",
                                "finite 32-bit integer coordinates",
                                "invalid mouse coordinates",
                            ));
                        }

                        let button = match button.as_deref().unwrap_or("left") {
                            "left" => Button::Left,
                            "right" => Button::Right,
                            "middle" => Button::Middle,
                            value => {
                                return Err(Error::new_from_js_message(
                                    "string",
                                    "mouse button",
                                    format!("unsupported button: {value}"),
                                ));
                            }
                        };

                        click_mouse
                            .borrow_mut()
                            .click_at(x as i32, y as i32, button)
                            .map_err(|message| {
                                Error::new_from_js_message(
                                    "mouse input",
                                    "JavaScript",
                                    message,
                                )
                            })
                    },
                )?,
            )?;

            rev.set(
                "sleep",
                Function::new(
                    ctx.clone(),
                    Async(|milliseconds: f64| async move {
                        if !milliseconds.is_finite()
                            || milliseconds < 0.0
                            || milliseconds.fract() != 0.0
                            || milliseconds >= u64::MAX as f64
                        {
                            return Err(Error::new_from_js_message(
                                "number",
                                "non-negative integer milliseconds",
                                "invalid sleep duration",
                            ));
                        }

                        tokio::time::sleep(Duration::from_millis(
                            milliseconds as u64,
                        ))
                        .await;
                        Ok::<(), Error>(())
                    }),
                )?,
            )?;

            let freeze: Function = ctx.eval("Object.freeze")?;
            let _: Object = freeze.call((state_object.clone(),))?;
            let _: Object = freeze.call((rev.clone(),))?;

            let result: MaybePromise = script.call((rev, memory))?;
            let _: Value = result.into_future().await?;
            Ok(())
        })
        .await
}
~~~

If the compiler requires explicit closure return annotations or a different Function::call generic form, adjust only those rquickjs type annotations. Preserve the ownership, persistence, and MaybePromise::into_future behavior shown here.

- [ ] **Step 6: Run Task 3 validation**

Run:

~~~powershell
cargo test --manifest-path client/Cargo.toml script::tests
cargo test --manifest-path client/Cargo.toml
cargo check --manifest-path client/Cargo.toml
~~~

Expected: all session, console, and UDP tests pass. The sleep test must complete without blocking other Tokio futures, and no automated test may move the real cursor.

- [ ] **Step 7: Commit Task 3**

Run:

~~~powershell
git add -- client/src/main.rs client/src/script.rs
git commit -m "feat: add persistent JavaScript session"
~~~

---

### Task 4: Add lifecycle control, the 50 ms main loop, and final task wiring

**Files:**
- Modify: client/src/script.rs
- Modify: client/src/main.rs
- Test: inline lifecycle tests in client/src/script.rs

**Interfaces:**
- Consumes:

~~~rust
pub async fn run(
    commands: tokio::sync::mpsc::Receiver<crate::console::ScriptCommand>,
    states: tokio::sync::watch::Receiver<crate::udp::State>,
    initial_path: std::path::PathBuf,
) -> Result<(), String>;
~~~

- Internal test seam:

~~~rust
async fn run_with_mouse(
    commands: tokio::sync::mpsc::Receiver<ScriptCommand>,
    states: tokio::sync::watch::Receiver<State>,
    initial_path: std::path::PathBuf,
    mouse: SharedMouse,
    loop_delay: std::time::Duration,
) -> Result<(), String>;
~~~

- run_with_mouse is justified because tests must inject a fake mouse and a short deterministic delay.
- The runner deliberately uses Option<ScriptSession> plus a paused bool instead of a larger state-machine abstraction:
  - Some session + paused false = Running.
  - Some session + paused true = Paused.
  - None = Stopped.

- [ ] **Step 1: Add a failing channel-driven lifecycle test**

Extend script.rs tests. The fake below sends observable clicks without exposing QuickJS memory to Rust:

~~~rust
#[tokio::test(flavor = "current_thread")]
async fn console_commands_control_context_lifetime() {
    use crate::console::ScriptCommand;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    use tokio::sync::{mpsc, watch};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "rev-idle-script-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir_all(&root).unwrap();
    let first = root.join("first.js");
    let second = root.join("second.js");

    fs::write(
        &first,
        r#"
            ((rev, memory) => {
                memory.count = (memory.count ?? 0) + 1;
                rev.click(memory.count, rev.state.sequence);
            })
        "#,
    )
    .unwrap();
    fs::write(
        &second,
        r#"
            ((rev, memory) => {
                memory.count = (memory.count ?? 0) + 1;
                rev.click(memory.count, 0, "right");
            })
        "#,
    )
    .unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct ChannelMouse(mpsc::UnboundedSender<(i32, i32, Button)>);
    impl MouseInput for ChannelMouse {
        fn click_at(
            &mut self,
            x: i32,
            y: i32,
            button: Button,
        ) -> Result<(), String> {
            self.0.send((x, y, button)).map_err(|error| error.to_string())
        }
    }

    let mouse: SharedMouse = Rc::new(RefCell::new(ChannelMouse(event_tx)));
    let (command_tx, command_rx) = mpsc::channel(32);
    let (state_tx, state_rx) = watch::channel(State {
        score: Some("1e1".to_owned()),
        sequence: 7,
        received_at_ms: Some(10),
    });

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                first.clone(),
                mouse,
                Duration::from_millis(5),
            ));

            assert_eq!(
                event_rx.recv().await,
                Some((1, 7, Button::Left))
            );

            state_tx
                .send(State {
                    score: Some("2e2".to_owned()),
                    sequence: 8,
                    received_at_ms: Some(20),
                })
                .unwrap();
            loop {
                if event_rx.recv().await.unwrap().1 == 8 {
                    break;
                }
            }

            command_tx.send(ScriptCommand::Resume).await.unwrap();
            assert!(event_rx.recv().await.unwrap().0 >= 2);

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            while event_rx.try_recv().is_ok() {}
            assert!(
                tokio::time::timeout(
                    Duration::from_millis(15),
                    event_rx.recv(),
                )
                .await
                .is_err()
            );

            command_tx.send(ScriptCommand::Pause).await.unwrap();
            command_tx.send(ScriptCommand::Resume).await.unwrap();
            let resumed = event_rx.recv().await.unwrap();
            assert!(resumed.0 >= 2);

            command_tx.send(ScriptCommand::Reload).await.unwrap();
            loop {
                if event_rx.recv().await == Some((1, 8, Button::Left)) {
                    break;
                }
            }

            command_tx
                .send(ScriptCommand::Load(second.clone()))
                .await
                .unwrap();
            loop {
                if event_rx.recv().await == Some((1, 0, Button::Right)) {
                    break;
                }
            }

            command_tx.send(ScriptCommand::Stop).await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            while event_rx.try_recv().is_ok() {}
            assert!(
                tokio::time::timeout(
                    Duration::from_millis(15),
                    event_rx.recv(),
                )
                .await
                .is_err()
            );

            command_tx.send(ScriptCommand::Stop).await.unwrap();
            command_tx.send(ScriptCommand::Pause).await.unwrap();
            command_tx.send(ScriptCommand::Resume).await.unwrap();
            command_tx.send(ScriptCommand::Reload).await.unwrap();
            loop {
                if event_rx.recv().await == Some((1, 0, Button::Right)) {
                    break;
                }
            }

            runner.abort();
            let _ = runner.await;
        })
        .await;

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn waits_after_failed_and_successful_invocations() {
    use std::{fs, time::Instant};
    use tokio::sync::{mpsc, watch};

    let path = std::env::temp_dir().join(format!(
        "rev-idle-delay-test-{}.js",
        std::process::id(),
    ));
    fs::write(
        &path,
        r#"
            ((rev) => {
                globalThis.calls = (globalThis.calls ?? 0) + 1;
                rev.click(globalThis.calls, 0, "left");
                if (globalThis.calls === 1) {
                    throw new Error("first call fails");
                }
            })
        "#,
    )
    .unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct TimedMouse(mpsc::UnboundedSender<Instant>);
    impl MouseInput for TimedMouse {
        fn click_at(
            &mut self,
            _x: i32,
            _y: i32,
            _button: Button,
        ) -> Result<(), String> {
            self.0
                .send(Instant::now())
                .map_err(|error| error.to_string())
        }
    }

    let mouse: SharedMouse = Rc::new(RefCell::new(TimedMouse(event_tx)));
    let (_command_tx, command_rx) = mpsc::channel(32);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let delay = Duration::from_millis(20);
    let local = tokio::task::LocalSet::new();

    let runner_path = path.clone();
    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                runner_path,
                mouse,
                delay,
            ));

            let first = tokio::time::timeout(
                Duration::from_secs(1),
                event_rx.recv(),
            )
            .await
            .unwrap()
            .unwrap();
            let second = tokio::time::timeout(
                Duration::from_secs(1),
                event_rx.recv(),
            )
            .await
            .unwrap()
            .unwrap();
            let third = tokio::time::timeout(
                Duration::from_secs(1),
                event_rx.recv(),
            )
            .await
            .unwrap()
            .unwrap();
            assert!(second.duration_since(first) >= delay);
            assert!(third.duration_since(second) >= delay);
            runner.abort();
            let _ = runner.await;
        })
        .await;

    fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn failed_load_stays_stopped_and_reload_retries_that_path() {
    use crate::console::ScriptCommand;
    use std::fs;
    use tokio::sync::{mpsc, watch};

    let missing = std::env::temp_dir().join(format!(
        "rev-idle-retry-test-{}.js",
        std::process::id(),
    ));
    let _ = fs::remove_file(&missing);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    struct RetryMouse(mpsc::UnboundedSender<()>);
    impl MouseInput for RetryMouse {
        fn click_at(
            &mut self,
            _x: i32,
            _y: i32,
            _button: Button,
        ) -> Result<(), String> {
            self.0.send(()).map_err(|error| error.to_string())
        }
    }

    let mouse: SharedMouse = Rc::new(RefCell::new(RetryMouse(event_tx)));
    let (command_tx, command_rx) = mpsc::channel(32);
    let (_state_tx, state_rx) = watch::channel(State::default());
    let local = tokio::task::LocalSet::new();

    local
        .run_until(async move {
            let runner = tokio::task::spawn_local(run_with_mouse(
                command_rx,
                state_rx,
                missing.clone(),
                mouse,
                Duration::from_millis(5),
            ));

            assert!(
                tokio::time::timeout(
                    Duration::from_millis(15),
                    event_rx.recv(),
                )
                .await
                .is_err()
            );

            fs::write(
                &missing,
                r#"((rev) => rev.click(1, 1, "left"))"#,
            )
            .unwrap();
            command_tx.send(ScriptCommand::Reload).await.unwrap();
            tokio::time::timeout(
                Duration::from_secs(1),
                event_rx.recv(),
            )
            .await
            .unwrap()
            .unwrap();

            drop(command_tx);
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            fs::remove_file(missing).unwrap();
        })
        .await;
}
~~~

- [ ] **Step 2: Run the lifecycle test and verify RED**

Run:

~~~powershell
cargo test --manifest-path client/Cargo.toml script::tests::console_commands_control_context_lifetime
~~~

Expected: exit code 101 because run_with_mouse does not exist.

- [ ] **Step 3: Implement reused script-file loading**

Add imports for ScriptCommand, Enigo settings, Path/PathBuf, mpsc, and watch. Add this private loader:

~~~rust
async fn load_path(path: &std::path::Path) -> Result<ScriptSession, String> {
    let source = tokio::fs::read_to_string(path)
        .await
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    ScriptSession::new(&source)
        .await
        .map_err(|error| format!("failed to load {}: {error}", path.display()))
}
~~~

This function is reused for startup, reload, and load. Keep current-path assignment and console messages in the runner loop.

- [ ] **Step 4: Implement the simple channel-owned runner**

Add:

~~~rust
pub async fn run(
    commands: tokio::sync::mpsc::Receiver<ScriptCommand>,
    states: tokio::sync::watch::Receiver<State>,
    initial_path: std::path::PathBuf,
) -> Result<(), String> {
    let mouse: SharedMouse = Rc::new(RefCell::new(
        Enigo::new(&enigo::Settings::default())
            .map_err(|error| format!("failed to initialize mouse input: {error}"))?,
    ));

    run_with_mouse(
        commands,
        states,
        initial_path,
        mouse,
        Duration::from_millis(50),
    )
    .await
}

async fn run_with_mouse(
    mut commands: tokio::sync::mpsc::Receiver<ScriptCommand>,
    mut states: tokio::sync::watch::Receiver<State>,
    initial_path: std::path::PathBuf,
    mouse: SharedMouse,
    loop_delay: Duration,
) -> Result<(), String> {
    let mut current_path = initial_path;
    let mut session = match load_path(&current_path).await {
        Ok(session) => {
            println!("running {}", current_path.display());
            Some(session)
        }
        Err(error) => {
            eprintln!("{error}");
            None
        }
    };
    let mut paused = false;

    loop {
        let command = if session.is_some() && !paused {
            match commands.try_recv() {
                Ok(command) => Some(command),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    return Ok(());
                }
            }
        } else {
            match commands.recv().await {
                Some(command) => Some(command),
                None => return Ok(()),
            }
        };

        if let Some(command) = command {
            match command {
                ScriptCommand::Load(path) => {
                    session = None;
                    paused = false;
                    current_path = path;
                    match load_path(&current_path).await {
                        Ok(loaded) => {
                            session = Some(loaded);
                            println!("running {}", current_path.display());
                        }
                        Err(error) => eprintln!("{error}"),
                    }
                }
                ScriptCommand::Reload => {
                    session = None;
                    paused = false;
                    match load_path(&current_path).await {
                        Ok(loaded) => {
                            session = Some(loaded);
                            println!("reloaded {}", current_path.display());
                        }
                        Err(error) => eprintln!("{error}"),
                    }
                }
                ScriptCommand::Pause => {
                    if session.is_none() {
                        println!("no script is running");
                    } else if paused {
                        println!("script is already paused");
                    } else {
                        paused = true;
                        println!("script paused");
                    }
                }
                ScriptCommand::Resume => {
                    if session.is_none() {
                        println!("script is stopped; use reload or load");
                    } else if paused {
                        paused = false;
                        println!("script resumed");
                    } else {
                        println!("script is already running");
                    }
                }
                ScriptCommand::Stop => {
                    if session.take().is_some() {
                        paused = false;
                        println!("script stopped");
                    } else {
                        println!("script is already stopped");
                    }
                }
            }

            continue;
        }

        let Some(active) = session.as_ref() else {
            continue;
        };
        let snapshot = states.borrow_and_update().clone();

        if let Err(error) = active.invoke(snapshot, mouse.clone()).await {
            eprintln!("script invocation failed: {error}");
        }

        tokio::time::sleep(loop_delay).await;
    }
}
~~~

The assignments session = None and session.take() are the reset boundary: persistent roots, context, and runtime are dropped before a replacement session is built. Keep lifecycle matching inline; it has one owning loop and no reuse requirement.

- [ ] **Step 5: Replace main with final LocalSet orchestration**

Replace client/src/main.rs with:

~~~rust
mod console;
mod script;
mod udp;

use std::{io, path::PathBuf};
use tokio::{
    sync::{mpsc, watch},
    task::LocalSet,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let initial_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("script.js"));
    let (state_tx, state_rx) = watch::channel(udp::State::default());
    let (command_tx, command_rx) = mpsc::channel(32);
    let local = LocalSet::new();

    local
        .run_until(async move {
            let mut udp_task = tokio::task::spawn_local(udp::run(state_tx));
            let mut console_task =
                tokio::task::spawn_local(console::run(command_tx));
            let mut script_task = tokio::task::spawn_local(script::run(
                command_rx,
                state_rx,
                initial_path,
            ));

            let result = tokio::select! {
                result = &mut udp_task => {
                    result
                        .map_err(|error| io::Error::other(error.to_string()))?
                }
                result = &mut console_task => {
                    result
                        .map_err(|error| io::Error::other(error.to_string()))?
                }
                result = &mut script_task => {
                    result
                        .map_err(|error| io::Error::other(error.to_string()))?
                        .map_err(io::Error::other)
                }
                result = tokio::signal::ctrl_c() => result,
            };

            udp_task.abort();
            console_task.abort();
            script_task.abort();
            result
        })
        .await
}
~~~

Keep initial argument parsing and task coordination inline in main. Neither sequence has another call site or a testing boundary that justifies extraction.

- [ ] **Step 6: Run automated final validation**

Run:

~~~powershell
cargo test --manifest-path client/Cargo.toml
cargo check --manifest-path client/Cargo.toml
git diff --check
~~~

Expected:

- all UDP, console, session, and lifecycle tests pass;
- cargo check exits 0 without warnings;
- git diff --check prints nothing;
- client/src/__tmp.rs and all plugin paths remain untouched.

- [ ] **Step 7: Run a bounded manual console and mouse check**

Create a temporary script outside tracked source files:

~~~javascript
(async (rev, memory) => {
    memory.count = (memory.count ?? 0) + 1;
    if (memory.count === 2) {
        rev.click(100, 100, "left");
    }
    await rev.sleep(10);
})
~~~

Run:

~~~powershell
cargo run --manifest-path client/Cargo.toml -- C:\path\to\manual-script.js
~~~

Enter these commands individually:

~~~text
pause
resume
reload
load C:\path\to\another-script.js
stop
~~~

Expected:

- pause takes effect only after the active invocation and its 50 ms inter-loop delay;
- resume preserves memory and QuickJS globals;
- reload resets memory and globals while keeping the same path;
- load resets memory and globals and adopts the new path;
- stop drops the JavaScript session while UDP reception continues;
- only the deliberate manual click moves the real cursor.

- [ ] **Step 8: Review scope and commit Task 4**

Run:

~~~powershell
git status --short
git diff --check
~~~

Confirm the implementation commits include only:

~~~text
client/src/main.rs
client/src/udp.rs
client/src/console.rs
client/src/script.rs
~~~

Then run:

~~~powershell
git add -- client/src/main.rs client/src/udp.rs client/src/console.rs client/src/script.rs
git commit -m "feat: run controllable JavaScript automation"
~~~

Do not stage the design/plan documents unless the user separately asks to commit them.

---

## Final Review Checklist

- UDP owns deserialization and publishes complete State values over watch.
- Invalid packets preserve the last valid watch value.
- The JavaScript task owns both receivers and all QuickJS state.
- The script source is evaluated once per load or reload.
- rev.state is a fresh immutable snapshot for each call.
- memory and JavaScript globals persist through invocation errors, pause, and resume.
- load, reload, and stop drop persistent roots before context/runtime.
- Every completed or failed invocation is followed by a 50 ms Tokio sleep.
- Console commands retain FIFO order and are processed only at loop boundaries.
- Closing the command channel exits the runner instead of spinning in a stopped or paused loop.
- Automated tests use a fake mouse; only the bounded manual check uses Enigo.
- No window-control API or crate is introduced.
- No single-use helper exists without a concrete reuse, testing, ownership, callback, or borrow/lifetime justification.
- Unrelated plugin work and client/src/__tmp.rs are unchanged.
