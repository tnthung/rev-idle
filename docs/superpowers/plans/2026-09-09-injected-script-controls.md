# Bidirectional Injected Script Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the Unity control overlay using independent WebSocket notifications and a simple reconnect-safe lifecycle projection.

**Architecture:** Keep the raw WebSocket connection as the only transport. Unity sends independent best-effort control notifications; the client pushes one complete `StateUpdate { phase, capture }` after state changes and reconnects. A narrow plugin control module caches only the current generation's latest update.

**Tech Stack:** Rust, Tokio watch/mpsc channels, tokio-tungstenite, C#, System.Net.WebSockets, BepInEx IL2CPP, Unity UI.

**Spec:** `docs/superpowers/specs/2026-09-09-injected-script-controls-design.md`

## Global Constraints

- Preserve the current `{ uuid, type, payload }` envelope and raw WebSocket transport.
- Leave existing correlated game-data, UI-path, invoke, and transfer packets unchanged.
- Notification handlers must not call `context.send`/`context.Send`.
- Notification failure must not terminate the client, plugin, or script lifecycle.
- Do not queue or replay notifications across reconnects.
- Keep the existing one-second WebSocket reconnection cadence; add no polling or heartbeat loop.
- The client pushes one complete `StateUpdate` after transitions and reconnects; Unity never requests or polls for control state.
- All three overlay buttons are disabled before a current-generation script state is known.
- Capture is one-shot and consumes the captured down plus its matching up.
- Normalize only the unstable `scene:<handle>/` prefix; keep the hierarchy exact.
- Do not run formatting tools.
- Do not start or install the game, plugin, or client.
- Preserve the user's edits in `docs/todo.md` and `scripts/unity_loop2_config.js`.
- Follow the current scoped task-commit workflow. Task 7 creates one scoped documentation commit containing this plan, the spec, and `plugin/README.md`.

---

### Task 1: Reconcile the Stash onto the WebSocket Baseline

**Files:**
- Resolve: `client/src/app/mod.rs`
- Resolve: `client/src/bridge/mod.rs`
- Resolve: `client/src/capture.rs`
- Resolve: `client/src/script/lifecycle.rs`
- Resolve: `plugin/src/Plugin.cs`
- Resolve: `plugin/tests/Program.cs`
- Resolve: `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`
- Delete: `plugin/src/HttpScoreServer.cs`
- Modify: `plugin/README.md`
- Modify: `plugin/src/RevolutionIdle.ScoreTelemetry.csproj`

**Interfaces:**
- Consumes: the current `WsConnection`, packet handlers, WebSocket input packets, and lifecycle command channel from `main`.
- Produces: a conflict-free WebSocket baseline with no HTTP control route, waiter, reqwest control client, or duplicate server.

- [ ] **Step 1: Resolve existing files in favor of the new transport structure**

For each conflicted file, begin with the current `main` version introduced by `5ac14c1`, then reapply only behavior named by this plan. Do not retain `reqwest::Client` parameters, `/control/next`, `/control/status`, `HttpScoreServer`, `HasControlWaiter`, or `PublishControl`.

- [ ] **Step 2: Remove obsolete restored control files from compilation**

Delete `plugin/src/HttpScoreServer.cs`. Remove the old HTTP implementation from `client/src/bridge/control.rs`; the path remains and is rewritten in Task 4. Keep `client/src/app/status.rs` only if rewritten as the lifecycle state module in Task 2.

- [ ] **Step 3: Mark only the eight conflict resolutions**

Run:

```powershell
git add client/src/app/mod.rs client/src/bridge/mod.rs client/src/capture.rs client/src/script/lifecycle.rs plugin/src/Plugin.cs plugin/tests/Program.cs plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
git rm plugin/src/HttpScoreServer.cs
```

Do not stage `docs/todo.md`, `scripts/unity_loop2_config.js`, or other unrelated edits.

- [ ] **Step 4: Confirm no conflict markers or HTTP control remnants remain**

Run:

```powershell
rg -n "^(<<<<<<<|=======|>>>>>>>)|control/next|control/status|HttpScoreServer|HasControlWaiter" client plugin
git status --short
```

Expected: no conflict markers or old HTTP control symbols; the two user-edited files remain unstaged.

---

### Task 2: Define the Notification Vocabulary and Lifecycle State

**Files:**
- Rewrite: `client/src/app/status.rs`
- Modify: `client/src/app/mod.rs`
- Modify: `client/src/bridge/packets.rs`
- Modify: `client/src/bridge/mod.rs`
- Rename: `client/src/bridge/capture.rs` to `client/src/bridge/ui_path.rs`
- Modify: `plugin/src/BridgePackets.cs`
- Modify: `plugin/src/Plugin.cs`
- Modify: `plugin/src/UnityUiClickDispatcher.cs`
- Modify: `protocol/fixtures/bridge-packets.json`
- Test: `client/src/app/status.rs`
- Test: `client/src/bridge/packets.rs`
- Test: `plugin/tests/Program.cs`

**Interfaces:**
- Produces: `ScriptPhase::{Unloaded, Stopped, Running, Paused}` and the seven independent packet types `ReloadScript`, `StopScript`, `PauseScript`, `ResumeScript`, `StartCapture`, `StopCapture`, and `StateUpdate { phase, capture }`.
- Produces: the renamed correlated `UiPathReq -> UiPathRes` lookup without changing its behavior.
- Preserves: all existing correlated request/response relationships.

- [ ] **Step 1: Write failing Rust state-model tests**

Define tests that require these exact projections:

```rust
assert_eq!(StateUpdate::new(false, false, false, false).phase, ScriptPhase::Unloaded);
assert_eq!(StateUpdate::new(true, false, false, false).phase, ScriptPhase::Stopped);
assert_eq!(StateUpdate::new(true, true, false, false).phase, ScriptPhase::Running);
assert_eq!(StateUpdate::new(true, true, true, true).phase, ScriptPhase::Paused);
assert!(!StateUpdate::new(false, false, false, true).capture);
assert!(!StateUpdate::new(true, true, false, true).capture);
```

- [ ] **Step 2: Extend the shared fixture and packet tests**

Add these payloads to `protocol/fixtures/bridge-packets.json` and assert them from both languages:

```json
"ReloadScript": {},
"StopScript": {},
"PauseScript": {},
"ResumeScript": {},
"StartCapture": {},
"StopCapture": {},
"StateUpdate": { "phase": "paused", "capture": true }
```

Rename `CaptureReq`/`CaptureRes` to `UiPathReq`/`UiPathRes`, `request_capture` to `request_ui_path`, `CaptureTarget` to `UiPathTarget`, and `UnityUiClickDispatcher.TryCapture` to `TryFindUiPath`. Keep their payload and correlation behavior unchanged. Do not add `Requestable` or `IRequest<TResponse>` to a notification record.

- [ ] **Step 3: Run the focused tests and observe failure**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml app::status::tests -- --test-threads=1
cargo test --manifest-path client/Cargo.toml bridge::packets::tests -- --test-threads=1
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0 -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
```

Expected: compile or assertion failures for the missing notification and state types.

- [ ] **Step 4: Implement the minimal shared vocabulary**

Use empty object structs/records so both languages serialize notifications identically:

```rust
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct StartCapture {}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct StateUpdate {
    pub(crate) phase: ScriptPhase,
    pub(crate) capture: bool,
}
```

```csharp
internal sealed record StartCapture;
internal sealed record StateUpdate(string Phase, bool Capture);
```

Repeat the empty type for the other five action notifications. Implement `Packet` for all seven, but do not implement `Requestable` or `IRequest<TResponse>`.

- [ ] **Step 5: Implement the closed client state projection**

`StateUpdate::new` maps the lifecycle facts into one phase and clamps invalid capture combinations to `false`. Serialize `ScriptPhase` as lowercase. Use this same value in the lifecycle watch channel and on the wire.

- [ ] **Step 6: Rerun the focused tests**

Expected: packet fixtures and state projection pass in Rust and C#.

---

### Task 3: Expose WebSocket Connection Generations

**Files:**
- Modify: `client/src/bridge/connection.rs`
- Modify: `plugin/src/WsConnection.cs`
- Test: `client/src/bridge/connection.rs`
- Test: `plugin/tests/Program.cs`

**Interfaces:**
- Produces: `WsConnection::connection_generation() -> watch::Receiver<u64>` and C# `WsConnection.ConnectionGeneration`.
- Consumes: the existing activation and close-generation paths; no new timer or reconnect loop.

- [ ] **Step 1: Write failing generation tests**

Rust must observe `0 -> 1 -> 0 -> 2` while a test listener accepts, closes, and accepts two raw WebSocket sessions. C# must expose `0` while disconnected, a positive generation during a raw client session, and `0` after that session closes.

- [ ] **Step 2: Run focused tests and observe failure**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml bridge::connection::tests::connection_generation_tracks_reconnects -- --exact --test-threads=1
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0 -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
```

Expected: failures for missing generation interfaces.

- [ ] **Step 3: Publish generation changes from existing lifecycle points**

Add one Tokio watch sender to the Rust connection implementation. Publish the generation in `Inner::activate`, publish `0` only when `close_generation` removes that active generation, and expose a subscribed receiver from `WsConnection`.

In C#, expose the current active session generation under `_gate`:

```csharp
public long ConnectionGeneration
{
    get
    {
        lock (_gate)
            return _active?.Generation ?? 0;
    }
}
```

Do not add callbacks, polling, delays, or packet replay.

- [ ] **Step 4: Rerun the focused tests**

Expected: both generation-transition tests pass without changing existing transport tests.

---

### Task 4: Replace HTTP Control with Rust Notification Handlers

**Files:**
- Rewrite: `client/src/bridge/control.rs`
- Modify: `client/src/bridge/mod.rs`
- Modify: `client/src/app/command.rs`
- Modify: `client/src/app/mod.rs`
- Modify: `client/src/script/lifecycle.rs`
- Modify: `client/src/capture.rs`
- Test: `client/src/bridge/control.rs`
- Test: `client/src/script/tests.rs`
- Test: `client/src/capture.rs`

**Interfaces:**
- Consumes: the notification packet types, `mpsc::Sender<ScriptCommand>`, lifecycle `watch::Receiver<StateUpdate>`, and connection generation receiver.
- Produces: notification handlers plus a reconnect-aware state publisher that never owns lifecycle state.

- [ ] **Step 1: Write failing notification-handler tests**

Using the existing ephemeral raw WebSocket test support, send each independent notification to the client and assert the corresponding lifecycle command arrives:

```text
ReloadScript -> Reload
StopScript -> Stop
PauseScript -> Pause
ResumeScript -> Resume
StartCapture -> StartCapture
StopCapture -> StopCapture
```

Assert the handler sends no response frame. Closing the command receiver must not terminate the WebSocket connection or produce a `RemoteError`.

- [ ] **Step 2: Write failing state-publication tests**

Assert that the publisher:

```text
sends one complete StateUpdate when generation becomes non-zero, without a Unity state request
sends the changed phase after load, reload, pause, resume, stop, F8, self-stop, and failure
sends StateUpdate with capture false after the lifecycle observes one-shot capture consumption
sends nothing while generation is zero
does not replay intermediate states after reconnect
stops promptly when shutdown becomes true
```

- [ ] **Step 3: Run focused tests and observe failure**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml bridge::control::tests -- --test-threads=1
cargo test --manifest-path client/Cargo.toml script::tests -- --test-threads=1
cargo test --manifest-path client/Cargo.toml capture::tests -- --test-threads=1
```

Expected: compile or assertion failures until the new handler and publication flow exists.

- [ ] **Step 4: Register best-effort notification handlers**

Register all six action handlers on the shared `WsConnection`. Each handler enqueues one explicit `ScriptCommand`, ignores a closed command channel, returns `Ok(())`, and never calls `PacketContext::send`.

Retain the local console `Capture` toggle. Add explicit `StartCapture`, `StopCapture`, and `CaptureConsumed` variants for notification and hook paths so duplicate packets are idempotent.

- [ ] **Step 5: Publish actual lifecycle state**

Pass a latest-value `watch::Sender<StateUpdate>` into lifecycle execution. Recompute and publish the closed state at the top of each loop iteration, where `current_path`, `session`, `paused`, and `capture_state` are simultaneously visible.

The control publisher selects between state changes, connection-generation changes, and shutdown. For a non-zero generation, send exactly the latest complete `StateUpdate`. Ignore `NotConnected`/`Closed`; let the next generation or state change trigger another attempt.

- [ ] **Step 6: Make one-shot capture push `StateUpdate` once**

Keep the atomic claim in the hook. After the click disarms capture, queue `CaptureConsumed`; the resulting lifecycle change makes the publisher send `StateUpdate` with `capture: false`. Do not also send from the hook thread. Keep the correlated `UiPathReq -> UiPathRes` lookup and clipboard behavior unchanged.

- [ ] **Step 7: Wire the module into app startup and shutdown**

Register handlers on the connection before starting normal application tasks. Spawn the state publisher beside console and lifecycle tasks. It must stop only from the existing shutdown watch; notification failures never become a task-supervisor error.

- [ ] **Step 8: Rerun focused Rust tests**

Expected: all control, lifecycle, and capture tests pass using ephemeral sockets.

---

### Task 5: Add a Generation-Scoped Unity Control Module

**Files:**
- Create: `plugin/src/ControlBridge.cs`
- Modify: `plugin/src/ControlOverlay.cs`
- Modify: `plugin/src/Plugin.cs`
- Modify: `plugin/src/RevolutionIdle.ScoreTelemetry.csproj`
- Modify: `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`
- Test: `plugin/tests/Program.cs`

**Interfaces:**
- Consumes: `WsConnection.ConnectionGeneration`, independent control packets, and client-pushed `StateUpdate`.
- Produces: `ControlBridge.State`, `ControlBridge.Connected`, and `ControlBridge.Send(ControlCommand)` for the overlay.

- [ ] **Step 1: Write failing bridge tests**

Cover these behaviors with an ephemeral raw peer:

```text
no connection -> state unavailable and send is a non-fatal no-op
StateUpdate on generation N replaces the complete projection for N
disconnect or generation N+1 -> old state is unavailable
Send(Pause) emits a new PauseScript UUID and waits only for the local socket write
outbound actions do not mutate the cached projection before a client notification arrives
malformed phase is ignored without a correlated RemoteError
```

- [ ] **Step 2: Write failing overlay projection tests**

Require:

```text
unloaded -> all three disabled
stopped -> Reload and Capture enabled
running -> Stop and Pause enabled
paused -> Stop, Resume, and Capture enabled
capturing -> all three disabled
unknown/disconnected generation -> all three disabled
```

- [ ] **Step 3: Run plugin tests and observe failure**

Run:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0 -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
```

Expected: compile or assertion failures for `ControlBridge` and the new projection.

- [ ] **Step 4: Implement the narrow control module**

`ControlBridge` registers one inbound `StateUpdate` handler, stores the generation with the complete projection, and maps overlay actions to independent packet instances. Sending an action does not mutate that projection. A disconnected synchronous exception or failed send task is observed and logged without propagating into `ScoreTicker.Update`.

Handlers update local projection and return `Task.CompletedTask`; they never call `PacketContext.Send`.

- [ ] **Step 5: Wire overlay creation and application**

Create `ControlBridge` after `WsConnection` in `Plugin.Load`. Lazily create `ControlOverlay` from `ScoreTicker.Update`, apply the bridge projection every frame, and dispose only the overlay from ticker teardown. The existing plugin shutdown remains responsible for disposing the WebSocket server.

- [ ] **Step 6: Rerun plugin tests**

Expected: all control module and presentation tests pass without a live game or fixed test port.

---

### Task 6: Preserve Capture UX and Fix the White-Block Icons

**Files:**
- Modify: `plugin/src/ControlOverlay.cs`
- Test: `plugin/tests/Program.cs`
- Verify: `plugin/src/UnityUiClickDispatcher.cs`

**Interfaces:**
- Produces: readable procedural Stop, Pause, and Resume icons plus exact scene-handle-normalized path resolution.
- Preserves: 30 by 30 rounded controls, one-pixel black border, enlarged tooltip background/text, and click-through behavior while capturing.

- [ ] **Step 1: Replace weak icon assertions with shape tests**

Assert negative space as well as visible pixels:

```text
Stop has an empty center and a one-pixel outline
Pause has two two-pixel bars separated by at least three transparent columns
Resume points right and has transparent interior pixels
button corners are transparent, border pixels black, and inner pixels white
```

Also assert every `Button.targetGraphic` is the rounded background image rather than its icon.

- [ ] **Step 2: Run plugin tests and observe icon failures**

Run the Task 5 plugin test command.

Expected: current filled Stop/Resume and thick Pause raster assertions fail.

- [ ] **Step 3: Implement only the required raster and target changes**

Change `IconPixel` to draw the three outlined/narrow shapes and set `button.targetGraphic = image` in the constructor-local button factory. Do not add image assets or a new rendering module.

- [ ] **Step 4: Add the scene-handle regression case**

Add a dispatcher test proving a stored path beginning with `scene:-148/` can resolve the exact hierarchy currently found under `scene:-280/`, while a different escaped name or sibling index does not match. The current dispatcher already ignores only the handle during its fallback; if the regression test passes, make no dispatcher code change.

- [ ] **Step 5: Rerun plugin tests**

Expected: icon, tooltip, presentation, and path regression tests pass.

---

### Task 7: Documentation and Final Verification

**Files:**
- Modify: `plugin/README.md`
- Modify: `docs/superpowers/specs/2026-09-09-injected-script-controls-design.md`
- Modify: `docs/superpowers/plans/2026-09-09-injected-script-controls.md`

**Interfaces:**
- Produces: an accurate WebSocket control handoff and fresh repository-local build artifacts.

- [ ] **Step 1: Replace HTTP documentation**

Document the three buttons, disabled-state table, one-shot capture, independent notification semantics, the single client-pushed `StateUpdate`, and the fact that `UiPathReq -> UiPathRes` remains correlated only for path lookup. Remove all mentions of long polling, `/control/next`, `/control/status`, HTTP control timeouts, and game-side control-state requests.

- [ ] **Step 2: Run the full Rust suite and Release build**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml -- --test-threads=1
cargo build --manifest-path client/Cargo.toml --release
```

Expected: all tests pass and `client/target/release/client.exe` builds. The suite uses ephemeral WebSocket listeners and does not require the game.

- [ ] **Step 3: Run plugin tests and Release build**

Run:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0 -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release -warnaserror -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
```

Expected: all plugin tests pass and the repository-local DLL builds without installation.

- [ ] **Step 4: Run static checks and inspect scope**

Run:

```powershell
rg -n "^(<<<<<<<|=======|>>>>>>>)|control/next|control/status|HttpScoreServer" client plugin/src plugin/tests plugin/README.md
git diff --check
git status --short
git diff -- client/src client/Cargo.toml client/Cargo.lock plugin/src plugin/tests plugin/README.md protocol/fixtures/bridge-packets.json docs/superpowers
```

Expected: no conflicts or obsolete HTTP control symbols in production code, tests, or the user-facing README; the plan/spec may mention removed symbols when documenting the migration. There must be no whitespace errors or changes to the user's unrelated files.

- [ ] **Step 5: Hand off live validation**

Report repository-local test/build results. Leave DLL installation, game restart, client launch, button clicks, tooltip appearance, and live one-shot capture validation to the user.
