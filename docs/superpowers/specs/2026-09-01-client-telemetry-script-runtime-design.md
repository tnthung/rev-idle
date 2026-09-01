# Client Telemetry and Script Runtime Design

## Goal

Turn the Rust client into a small local automation host with two concurrent responsibilities:

1. Receive Revolution Idle score telemetry over UDP and keep the latest valid game state in Rust.
2. Run one reloadable JavaScript automation function repeatedly in QuickJS.

The JavaScript function receives explicit `(rev, memory)` arguments. `rev.state` is the latest telemetry snapshot captured immediately before that invocation. The script can also click at absolute screen coordinates and sleep asynchronously, while `memory` retains script-owned state between invocations. Console commands control which script is loaded and whether it is running, paused, or stopped.

## Scope

The client listens only on `127.0.0.1:19841`. The existing plugin sends one UTF-8 JSON datagram every 50 ms with a score represented as a string, for example:

```json
{"score":"1.2345678901234567e123"}
```

The client runs one script at a time. It supports the console commands `load`, `reload`, `pause`, `resume`, and `stop`.

This design supersedes the print-only client listener described in `2026-09-01-client-listener-installer-design.md`. It does not change that document's installer behavior.

## Non-goals

This version does not:

- find, focus, resize, or move the game window;
- accept telemetry from non-loopback interfaces;
- run multiple scripts concurrently;
- persist JavaScript memory across client process restarts;
- watch script files or reload them automatically;
- forcibly interrupt a JavaScript invocation that has not finished;
- provide a graphical control interface.

## Runtime Architecture

The client uses a Tokio current-thread runtime and `LocalSet`. This keeps the QuickJS runtime and context on one executor thread without enabling rquickjs's experimental parallel support. UDP reception and console input remain asynchronous and can make progress while JavaScript awaits `rev.sleep`.

The runtime consists of:

- a Tokio watch channel carrying the latest `State` snapshot;
- a UDP receiver task that parses valid telemetry and replaces the watch channel value;
- a console input task that parses lines into `ScriptCommand` values and sends them through an MPSC channel;
- a script runner that owns both channel receivers and one Enigo input backend, and exclusively owns the current `AsyncRuntime`, `AsyncContext`, script function, and `memory` object.

The watch channel directly represents the required latest-valid-state semantics: a newer UDP update replaces the previous value instead of building a queue. Immediately before an invocation, the script runner clones `state_rx.borrow_and_update()` and builds that invocation's `rev` object from the clone. The script never owns a lock or channel handle.

## Telemetry State Channel

The Rust state is:

```rust
struct State {
    score: Option<String>,
    sequence: u64,
    received_at_ms: Option<u64>,
}
```

The initial watch value has score and receive time set to `None` and sequence set to `0`. The UDP task owns the sequence counter. Every valid datagram constructs and sends a complete `State`, recording the current Unix time in milliseconds and incrementing the sequence with saturating arithmetic. The score stays a string so the client never loses precision by converting it through `f64`.

Malformed UTF-8, malformed JSON, and payloads without a string `score` field do not send a new value, so the previous valid watch value remains available. Unknown JSON fields are ignored. The receiver prints a concise warning for an invalid packet and continues. Failure to bind the socket or an unrecoverable receive error terminates the client so automation cannot continue indefinitely against permanently stale telemetry.

## Script Contract

A script file is a JavaScript expression that evaluates to one callable function. The normal form is:

```javascript
(async (rev, memory) => {
    memory.ticks ??= 0;
    memory.ticks += 1;

    if (rev.state.score !== null && memory.ticks === 10) {
        rev.click(1200, 800, "left");
        await rev.sleep(250);
    }
})
```

The evaluated function is retained by the script runner and invoked repeatedly as `script(rev, memory)`. Immediately before each call, the runner clones the latest watch value and constructs a fresh frozen `rev` object containing that snapshot and the host functions. The return value may be synchronous or a promise; the runner awaits promise settlement before beginning the next invocation. After every completed or failed invocation, the runner waits 50 ms before returning to the top of the main loop. Commands that arrive during the invocation or delay remain queued for that next safe boundary.

The client does not install `rev`, `memory`, or the script function on `globalThis`. A script may use ordinary JavaScript globals deliberately. Those globals follow the same lifetime as the QuickJS context.

The default initial script path is `script.js` in the client's working directory. A first command-line argument replaces that path. Failure to load the initial script leaves the script runner stopped while the UDP receiver and console remain available, allowing the user to retry with `reload` or choose another file with `load`.

## JavaScript Host API

### `rev`

`rev` is a fresh frozen host-owned object created for each invocation. Its state snapshot and API properties cannot be replaced by the script.

#### `rev.state`

Contains the watch-channel snapshot captured immediately before the invocation:

```javascript
{
    score: "1.2345678901234567e123", // null before the first valid packet
    sequence: 42,
    receivedAtMs: 1788240000000       // null before the first valid packet
}
```

The state object is frozen. New UDP packets do not change it during the active invocation; the next invocation receives the newest available snapshot.

#### `rev.click(x, y, button = "left")`

Moves the mouse to absolute screen coordinates `(x, y)` and performs one click. Coordinates must be finite 32-bit integers; negative coordinates are allowed for multi-monitor layouts. Supported buttons are `"left"`, `"right"`, and `"middle"`.

The Enigo call is synchronous because it is short. Invalid arguments and operating-system input failures throw a catchable JavaScript exception.

#### `rev.sleep(milliseconds)`

Returns a JavaScript promise backed by `tokio::time::sleep`. The value must be a finite, non-negative integer representable as `u64` milliseconds. Invalid values throw a catchable JavaScript exception. Sleeping does not block UDP reception or console input.

### `memory`

`memory` is a mutable plain JavaScript object created with each fresh QuickJS context. The same object is passed to every invocation in that context. It may contain arbitrary JavaScript values and is not serialized to Rust or disk.

`memory` and ordinary JavaScript globals survive normal invocations, script exceptions, pause, and resume. They are destroyed by load, reload, stop, or process exit. The per-invocation `rev` object is not retained by Rust after that invocation settles.

## Script Lifecycle

The externally visible script states are `Running`, `Paused`, and `Stopped`.

All lifecycle commands are applied at the top of the main loop. If a command arrives while the script function is active, including while it awaits `rev.sleep`, or during the required 50 ms inter-loop delay, the command remains queued until the next loop iteration. There is no forced cancellation or execution timeout in this version. A script that never settles also prevents pending lifecycle commands from completing.

Commands are processed in arrival order. The console prints confirmation after a transition has actually completed, not merely after the command is queued.

### Console Commands

#### `pause`

- From `Running`, wait for the active invocation, then enter `Paused`.
- Preserve the QuickJS runtime, context, script function, `memory`, and JavaScript globals. The next invocation still receives a fresh `rev`.
- From `Paused`, do nothing and report that the script is already paused.
- From `Stopped`, do nothing and report that no script is running.

#### `resume`

- From `Paused`, enter `Running` using the preserved context and memory.
- From `Running`, do nothing and report that the script is already running.
- From `Stopped`, do nothing and instruct the user to use `reload` or `load`.

#### `reload`

- Wait for the active invocation when necessary.
- Destroy the current QuickJS runtime, context, globals, `rev`, and `memory`.
- Read the current script path again, create a fresh runtime and context, and start in `Running`.
- Work from `Running`, `Paused`, or `Stopped`.
- On failure, report the error and remain `Stopped` with the current path retained so another `reload` retries it.

#### `stop`

- Wait for the active invocation when necessary.
- Destroy the current QuickJS runtime, context, globals, `rev`, and `memory`.
- Enter `Stopped` while retaining the current script path for a later `reload`.
- From `Stopped`, do nothing and report that the script is already stopped.

#### `load <script-path>`

- Wait for the active invocation when necessary.
- Destroy the current QuickJS runtime, context, globals, `rev`, and `memory`.
- Adopt `<script-path>` as the current script path, create a fresh runtime and context, and start in `Running`.
- Work from `Running`, `Paused`, or `Stopped`.
- Treat the remainder of the console line as the path and strip one matching pair of surrounding quotes, allowing Windows paths with spaces.
- On failure, report the error and remain `Stopped` with the requested path retained so `reload` retries it.

The UDP receiver and watch channel continue unchanged across pause, resume, reload, stop, and load. Resetting a JavaScript context never resets the latest telemetry state.

## Error Handling

- UDP bind and unrecoverable receive failures terminate the client with the operating-system error visible.
- Invalid telemetry is reported and ignored without clearing the latest valid state.
- Enigo initialization failure prevents the script runner from starting. Script file read errors, syntax errors, non-callable evaluation results, and QuickJS initialization errors fail that load operation and leave the runner `Stopped`.
- An exception from a normal script invocation is printed, but the same context and memory remain alive and the loop continues after 50 ms.
- A click or sleep argument error becomes a JavaScript exception that the script may catch.
- Unknown or malformed console commands print a short usage line and do not change state.
- Ctrl+C terminates the whole client. The `stop` console command stops only the script runtime.

## File Responsibilities

- `client/src/main.rs`: current-thread Tokio setup, `LocalSet`, task startup, fatal task coordination, initial script path, and Ctrl+C handling.
- `client/src/udp.rs`: `State`, the private UDP payload type, loopback socket binding, payload deserialization, local sequence ownership, and watch-channel sends.
- `client/src/console.rs`: `ScriptCommand`, console-line parsing, and asynchronous stdin command forwarding.
- `client/src/script.rs`: watch and command receivers, QuickJS ownership, lifecycle state machine, script loading, per-invocation `rev` construction, persistent `memory`, repeated invocation, host API bindings, and Enigo integration.

No window-control module or direct Windows window API dependency is added.

## Implementation Style

Keep logic inline in its owning loop when it has only one call site. Do not create a function merely to name a short one-off sequence. Extract a function only when at least one concrete need applies: reuse, focused unit testing, a required callback or async task entry point, independent ownership, or a borrow/lifetime boundary that would otherwise make the code unclear. Do not add wrapper functions around single library calls.

## Implementation Tasks

### Task 1: Receive UDP telemetry and publish the latest state

Create the UDP module, wire a Tokio watch sender into the receiver task, and verify that valid packets replace the latest snapshot while invalid packets preserve it. This task produces `watch::Receiver<State>` for the script runner.

### Task 2: Parse and forward console lifecycle commands

Create the console module, parse the five supported commands, and forward valid commands through an ordered channel while reporting invalid input without stopping stdin processing.

### Task 3: Build the persistent JavaScript session

Create the QuickJS session in the script module, expose a fresh `(rev, memory)` call where `rev.state` is the per-invocation snapshot, retain the script function and memory object, await synchronous and asynchronous results, and bind sleep and mouse input.

### Task 4: Run the lifecycle loop and wire the client

Consume the watch and command receivers in the JavaScript task, implement all lifecycle transitions and the required 50 ms post-invocation delay, and wire the three tasks into a current-thread Tokio `LocalSet` in `main`.

## Verification

Task 1 automated tests cover:

- exact preservation of the score string;
- initial empty state;
- sequence and receive-time updates;
- latest-valid-packet-wins behavior;
- malformed UTF-8, JSON, and payload shapes preserving the previous state;
- real loopback UDP delivery.

Tasks 2 through 4 automated tests cover:

- a script expression must evaluate to a callable function;
- repeated invocation with explicit `(rev, memory)` arguments;
- `memory` and JavaScript globals persisting across invocations and pause/resume;
- `rev.state` remaining stable during one invocation and observing a newer watch value on the next invocation;
- script exceptions not stopping later invocations or resetting memory;
- sleep yielding without blocking UDP progress;
- a 50 ms delay after both successful and failed script invocations;
- console command parsing, including quoted and unquoted paths with spaces;
- running, paused, and stopped lifecycle transitions, including representative no-op commands;
- load, reload, and stop destroying the previous context and memory;
- failed load and reload leaving the runner stopped with the expected current path;
- mouse validation and calls through a fake input backend so tests never move the real cursor.

A bounded Windows manual check verifies one real mouse click, pausing after an active invocation, resuming with preserved memory, reloading with reset memory, loading a different script, and stopping without stopping UDP telemetry.
