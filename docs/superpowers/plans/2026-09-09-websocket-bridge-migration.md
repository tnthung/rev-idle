# WebSocket Bridge Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move state, capture, invoke, and transfer onto the existing bidirectional raw-WebSocket connection while preserving every JavaScript-facing behavior and removing the obsolete HTTP transport.

**Architecture:** Define the eight operation packets in focused Rust and C# modules and prove their camel-case payloads against one shared fixture. Rust operation modules call the existing `WsConnection::request` MPSC path; C# registers four async handlers that perform Unity work in their synchronous prefix when `ScoreTicker.Update()` pumps inbound packets. Then replace `reqwest::Client` throughout the Rust ownership chain and remove all HTTP-only code and dependencies atomically.

**Tech Stack:** Rust 2024, Tokio, tokio-tungstenite raw WebSocket streams, serde/serde_json, rquickjs, C#/.NET 8 and 9, `System.Net.WebSockets`, `System.Text.Json`, BepInEx IL2CPP.

**Spec:** `docs/superpowers/specs/2026-09-09-websocket-bridge-migration-design.md`

## Global Constraints

- Preserve the JavaScript APIs and behavior of `rev.state`, `rev.invoke`, `rev.transfer`, and capture lookup.
- Packets use only the neutral envelope `{ uuid, type, payload }`; request versus send is never represented on the wire.
- All outbound operation packets use the existing connection MPSC writer; no operation adds a queue, retry, fallback, or second connection.
- Calls while disconnected fail immediately with `WsError::NotConnected`; nothing is queued or replayed.
- The game remains the server at `127.0.0.1:19841`; the client retries connection once per second through the existing supervisor.
- C# handlers are async and independently scheduled; slow prior handlers do not block later packets.
- State access and Unity UI work execute on the `ScoreTicker.Update()` thread before the handler's first incomplete `await`.
- `StatePayload.Encode` remains the only state encoding authority.
- Production must contain no temporary HTTP fallback after this plan completes.
- Tests use ephemeral loopback ports only. Never launch, replace, stop, install into, or bind against the currently running game/client, and never run a test that binds `19841`.
- Do not run a formatter. Follow the existing coding style and run `git diff --check`.
- Preserve unrelated changes in `scripts/test.js` and `scripts/unity_loop2_config.js` and exclude them from every commit.

---

### Task 1: Shared Packet Contract and Rust Operations

**Files:**
- Create: `protocol/fixtures/bridge-packets.json`
- Create: `client/src/bridge/packets.rs`
- Create: `client/src/bridge/test_support.rs`
- Modify: `client/src/bridge/mod.rs`
- Modify: `client/src/bridge/connection.rs`
- Modify: `client/src/bridge/state.rs`
- Modify: `client/src/bridge/capture.rs`
- Modify: `client/src/bridge/invoke.rs`
- Modify: `client/src/bridge/transfer.rs`

**Interfaces:**
- Consumes: `Packet`, `Requestable`, `WsConnection::request`, and test-only `WsConnection::connect_for_test` from `client/src/bridge/connection.rs`.
- Produces: `StateReq { keys } -> StateRes { value }`, `CaptureReq { x, y } -> CaptureRes { target_type, path }`, `InvokeReq { path } -> InvokeRes {}`, and `TransferReq { source, destination } -> TransferRes {}` plus WebSocket-backed operation functions with the signatures required by the spec.
- Produces: a test-only ephemeral raw-WebSocket peer reusable by Task 3 tests without exporting test machinery in production builds.

- [ ] **Step 1: Add the literal shared payload fixture**

Create `protocol/fixtures/bridge-packets.json` with these independently derived expected payloads:

```json
{
  "StateReq": { "keys": ["score", "eternity.dtpSpent"] },
  "StateRes": { "value": { "score": "1e3", "enabled": true, "nested": { "value": null }, "items": [1, "two", false] } },
  "CaptureReq": { "x": 123, "y": -45 },
  "CaptureRes": { "type": "slot", "path": "scene:1/Canvas[0]/Inventory/3" },
  "InvokeReq": { "path": "scene:1/Canvas[0]/Buy DTP & More[0]" },
  "InvokeRes": {},
  "TransferReq": { "source": "scene:1/Canvas[0]/Inventory/3", "destination": "scene:1/Canvas[0]/Combine/0" },
  "TransferRes": {}
}
```

- [ ] **Step 2: Write failing Rust packet contract tests**

In `client/src/bridge/packets.rs`, add a `#[cfg(test)]` table that loads `../../../protocol/fixtures/bridge-packets.json`, serializes one value of each concrete packet, and compares it to the literal object indexed by `P::TYPE`. The break caught is any wrong packet type, wrong field name, or wrong payload shape. Include nullable capture fields in a separate assertion:

```rust
assert_eq!(serde_json::to_value(CaptureRes {
    target_type: None,
    path: None,
}).unwrap(), serde_json::json!({ "type": null, "path": null }));
```

Run: `cargo test bridge::packets -- --nocapture`

Expected: FAIL because `bridge::packets` and the packet types do not exist.

- [ ] **Step 3: Implement the minimal Rust packet types**

Define serde structs with `Packet::TYPE` matching their Rust type names and these request mappings:

```rust
impl Requestable for StateReq { type Response = StateRes; }
impl Requestable for CaptureReq { type Response = CaptureRes; }
impl Requestable for InvokeReq { type Response = InvokeRes; }
impl Requestable for TransferReq { type Response = TransferRes; }
```

Use `#[serde(rename = "type")]` only for `CaptureRes::target_type`; do not introduce a protocol direction field or a second packet trait.

Run: `cargo test bridge::packets -- --nocapture`

Expected: PASS.

- [ ] **Step 4: Write failing operation tests over an ephemeral raw WebSocket**

Add operation tests that connect `WsConnection` to a listener bound to `127.0.0.1:0`, inspect the real envelope, and send the correlated response. Use literal assertions to catch wrong operation routing:

```rust
assert_eq!(request.get("type"), Some(&serde_json::json!("StateReq")));
assert_eq!(request.get("payload"), Some(&serde_json::json!({
    "keys": ["score", "eternity.dtpSpent"]
})));
assert_eq!(request_state(&connection, &keys).await.unwrap(),
    r#"{"score":"1e3","enabled":true,"nested":{"value":null},"items":[1,"two",false]}"#);
```

Cover `CaptureRes` field mapping, both empty success responses, a correlated `RemoteError`, malformed/non-object state values, and an immediately disconnected call. The test peer must wrap accepted `TcpStream` directly with `WebSocketStream::from_raw_socket`; it must not perform an HTTP upgrade.

Run: `cargo test bridge::state bridge::capture bridge::invoke bridge::transfer -- --nocapture`

Expected: FAIL because the operation functions still require `reqwest::Client` and issue HTTP requests.

- [ ] **Step 5: Route all four Rust operations through `WsConnection::request`**

Implement exactly these public bridge boundaries:

```rust
pub(crate) async fn request_state(connection: &WsConnection, keys: &[String]) -> Result<String, String>
pub(crate) async fn request_capture(connection: &WsConnection, x: i32, y: i32) -> Result<CaptureTarget, String>
pub(crate) async fn invoke(connection: &WsConnection, path: String) -> Result<(), String>
pub(crate) async fn transfer(connection: &WsConnection, source: String, destination: String) -> Result<(), String>
```

Each function constructs its request packet, awaits `connection.request`, and maps the error with `error.to_string()`. `request_state` must reject a non-object `StateRes.value` with `"state response must be a JSON object"` and return `serde_json::to_string(&value)` on success. Do not bypass or wrap the connection writer.

Run: `cargo test bridge::state bridge::capture bridge::invoke bridge::transfer -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Verify and commit Task 1**

Run:

```text
cargo test bridge -- --nocapture
cargo check --all-targets
git diff --check
```

Expected: the bridge tests and check pass; remaining `reqwest` references are expected until Task 3; no test listens on `19841`.

Commit only the files listed in this task with message `[Chg] route bridge operations through websocket`.

### Task 2: C# Packets and Unity-Thread Handlers

**Files:**
- Create: `plugin/src/BridgePackets.cs`
- Modify: `plugin/src/Plugin.cs`
- Modify: `plugin/tests/Program.cs`
- Modify: `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`

**Interfaces:**
- Consumes: the Task 1 fixture and exact packet names/payloads, `WsConnection.Handler<T>`, `PacketContext.Send`, `StatePayload.Encode`, and `UnityUiClickDispatcher`.
- Produces: `Plugin.RegisterHandlers(WsConnection, Func<object?>, Func<nint>)`, `Plugin.PumpPackets(nint)`, and four production packet handlers registered once from `Plugin.Load()`.

- [ ] **Step 1: Link the shared fixture and write failing C# packet tests**

Link `protocol/fixtures/bridge-packets.json` into the test output as `protocol/bridge-packets.json`. Add a table-driven test that serializes each record with `WsConnection.SerializeForTest(Guid.Empty, packet)`, extracts `type` and `payload`, and compares the payload to the fixture object keyed by the exact record name. Add this nullable assertion:

```csharp
Equal("{\"type\":null,\"path\":null}",
    JsonSerializer.Serialize(new CaptureRes(null, null), new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase }),
    nameof(BridgePacketPayloadsMatchSharedFixture));
```

Run: `dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0`

Expected: FAIL because the eight packet records do not exist.

- [ ] **Step 2: Implement the eight focused C# packet records**

Create exactly:

```csharp
internal sealed record StateReq(IReadOnlyList<string> Keys) : IRequest<StateRes>;
internal sealed record StateRes(JsonElement Value);
internal sealed record CaptureReq(int X, int Y) : IRequest<CaptureRes>;
internal sealed record CaptureRes(string? Type, string? Path);
internal sealed record InvokeReq(string Path) : IRequest<InvokeRes>;
internal sealed record InvokeRes;
internal sealed record TransferReq(string Source, string Destination) : IRequest<TransferRes>;
internal sealed record TransferRes;
```

Run the same harness command.

Expected: the packet fixture test passes.

- [ ] **Step 3: Write failing raw-socket handler tests**

Using `WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2))`, connect a `TcpClient` to its ephemeral port and wrap the stream directly with `WebSocket.CreateFromStream(stream, false, null, TimeSpan.FromSeconds(5))`. Send one literal request envelope, call `Plugin.PumpPackets(window)` from the test thread, and read the correlated response. Tests must prove:

```text
StateReq -> StateRes preserves mixed JSON and selected keys
missing state data -> RemoteError
invalid state path -> RemoteError
CaptureReq -> CaptureRes or correlated RemoteError
InvokeReq -> InvokeRes or correlated RemoteError
TransferReq -> TransferRes or correlated RemoteError
getData/getWindow and every UI dispatcher call occur on the Pump caller thread
one handler awaiting its response send does not prevent a later queued handler from starting
```

The break caught is missing registration, wrong request dispatch, Unity access after an incomplete await, or an uncorrelated error. Use the existing real `WsConnection`; test seams may inject `getData` and `getWindow`, but assertions must inspect real wire responses rather than mock call counts.

Run the same harness command.

Expected: FAIL because `Plugin.RegisterHandlers` does not exist and `PumpPackets` does not receive the current window.

- [ ] **Step 4: Register handlers with Unity work in the synchronous prefix**

Add the internal seam:

```csharp
internal static void RegisterHandlers(
    WsConnection connection,
    Func<object?> getData,
    Func<nint> getWindow)
```

For state, call `StatePayload.Encode`, throw `InvalidOperationException` for null data, invalid path, or serialization failure, parse successful bytes with `JsonDocument.Parse`, clone `RootElement`, then `await context.Send(new StateRes(value))`. For capture/invoke/transfer, call the corresponding dispatcher and throw `InvalidOperationException(error)` on failure before `await context.Send(...)`. Do not call Unity APIs after an incomplete await.

In `Plugin.Load()`, call `RegisterHandlers(_connection, () => GameController.data, () => _window)` when connection creation succeeds. Store the current window in `PumpPackets(nint window)` before calling `_connection?.Pump()`, and change `ScoreTicker.Update()` to call `Plugin.PumpPackets(_bridge?.Window ?? 0)`.

Run the same harness command.

Expected: all C# tests pass on ephemeral ports.

- [ ] **Step 5: Verify and commit Task 2**

Run:

```text
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release
git diff --check
```

Expected: harness and Release build pass without launching or installing the plugin; no test listens on `19841`.

Commit only the files listed in this task with message `[Add] handle bridge packets in plugin`.

### Task 3: Rust Ownership Chain and JavaScript Behavior

**Files:**
- Modify: `client/src/app/mod.rs`
- Modify: `client/src/capture.rs`
- Modify: `client/src/script/bindings.rs`
- Modify: `client/src/script/lifecycle.rs`
- Modify: `client/src/script/session.rs`
- Modify: `client/src/script/tests.rs`
- Modify: `client/Cargo.toml`
- Modify: `client/Cargo.lock`
- Modify: `client/src/bridge/mod.rs`

**Interfaces:**
- Consumes: the WebSocket-backed operation signatures from Task 1 and cloneable `WsConnection`.
- Produces: a single application-owned `WsConnection` cloned through `CaptureWorker` and `ScriptSession`, with existing JavaScript validation, pause, object-freezing, and capture clipboard behavior unchanged.

- [ ] **Step 1: Rewrite the existing bridge-facing tests first**

Replace only HTTP listener/proxy setup in capture and script tests with the Task 1 ephemeral raw-WebSocket peer. Preserve the existing JavaScript assertions and add literal wire assertions for the requested packet. The core state assertion remains:

```javascript
const state = await rev.state("score");
if (state.score !== "1e3" || !Object.isFrozen(state)) throw new Error("state contract changed");
```

Keep the existing `rev.invoke`/`rev.transfer` validation and pause tests; their successful wire tests must now observe `InvokeReq`/`TransferReq`, and remote errors must still reject the JavaScript promise with the plugin message. Update capture lookup to observe `CaptureReq` and retain its clipboard/result assertions.

Run:

```text
cargo test capture::tests::describe_capture -- --nocapture
cargo test script::tests::rev_state -- --nocapture
cargo test script::tests::rev_invoke -- --nocapture
cargo test script::tests::rev_transfer -- --nocapture
```

Expected: FAIL because production ownership still requires `reqwest::Client`.

- [ ] **Step 2: Replace `reqwest::Client` through the ownership chain**

Change these values and parameters to `crate::bridge::WsConnection` without adding a wrapper type:

```text
app::run connection
CaptureWorker::start(connection: WsConnection)
run_capture_loop(..., connection: WsConnection, ...)
describe_capture(connection: &WsConnection, ...)
script::run(..., connection: WsConnection, ...)
ScriptSession { connection: WsConnection }
ScriptSession::new_with_connection(source, name, connection)
create_rev(ctx, connection: WsConnection, ...)
```

`ScriptSession::new` must use `WsConnection::disconnected_for_test()` so non-bridge unit tests fail bridge calls immediately without opening a socket. Add that test-only constructor to `WsConnection` only if it does not already exist. In `app::run`, construct the connection before `CaptureWorker::start`, pass clones to capture and script, retain the original outside `LocalSet::run_until`, stop capture/script consumers through the existing shutdown sequence, then call `connection.shutdown().await` before returning.

Run the four focused commands from Step 1.

Expected: PASS.

- [ ] **Step 3: Remove the Rust HTTP dependency and fixed-port lock**

Delete `reqwest` from `client/Cargo.toml`, update `client/Cargo.lock` through normal Cargo dependency resolution, remove `TEST_SERVER_LOCK`, and remove all HTTP URLs/proxy helpers. Do not remove `tokio-tungstenite` or introduce `fastwebsockets`; the already-proven raw-WebSocket foundation remains authoritative.

Run:

```text
rg -n "reqwest|http://127\.0\.0\.1:19841|TEST_SERVER_LOCK" client/src client/Cargo.toml client/Cargo.lock
cargo check --all-targets
cargo test bridge -- --nocapture
cargo test capture::tests -- --nocapture
cargo test script::tests -- --nocapture
git diff --check
```

Expected: the search returns no matches and all commands pass without using the live port.

- [ ] **Step 4: Commit Task 3**

Commit only the files listed in this task with message `[Chg] migrate client bridge consumers to websocket`.

### Task 4: Remove HTTP Server and Verify the Atomic Migration

**Files:**
- Delete: `plugin/src/HttpScoreServer.cs`
- Modify: `plugin/src/Plugin.cs`
- Modify: `plugin/tests/Program.cs`
- Modify: `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`
- Modify if it contains HTTP bridge documentation: `plugin/README.md`

**Interfaces:**
- Consumes: Task 2 packet registration and Task 3's HTTP-free Rust client.
- Produces: one production transport with no HTTP server, HTTP test helpers, or stale HTTP bridge documentation.

- [ ] **Step 1: Run removal-sensitive tests before deleting HTTP code**

Run the C# harness once and record the passing baseline. Then identify the HTTP-only top-level calls and methods by exact symbol:

```text
ServerQueuesUiRequests
ServerRejectsInvalidUiRequests
ServerDropsTimedOutInvocations
CreateServer
RequestWithServer
CompletePendingUntil
CompletePluginPendingUntil
Plugin.CompletePending(HttpScoreServer, ...)
```

The tests that must remain are all generic `WsConnection`, packet handler, `StatePayload`, `UnityUiClickDispatcher`, input bridge, and install tests.

- [ ] **Step 2: Delete the HTTP implementation and HTTP-only harness code**

Delete `plugin/src/HttpScoreServer.cs`, its project link, `Plugin.CompletePending(HttpScoreServer, ...)`, and only the HTTP-specific top-level calls/methods/helpers. Remove `System.Net.Http`, raw HTTP request builders, and HTTP parser imports only when unused. Do not delete packet handler error tests that replace those semantics.

Run: `dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0`

Expected: PASS.

- [ ] **Step 3: Prove the repository transport boundary**

Run these read-only searches:

```text
rg -n "reqwest|HttpScoreServer|http://127\.0\.0\.1:19841|GET /state|POST /invoke|POST /transfer" client plugin protocol
rg -n "19841" client/src plugin/tests
```

Expected: the first search has no matches; the second may show only the production endpoint/configuration and must show no test listener binding.

- [ ] **Step 4: Run final non-invasive verification**

Run:

```text
cargo test bridge -- --nocapture
cargo test capture::tests -- --nocapture
cargo test script::tests -- --nocapture
cargo check --all-targets
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release
powershell -NoProfile -ExecutionPolicy Bypass -File plugin/tests/InstallPlugin.Tests.ps1
git diff --check
```

Expected: all commands pass, no fixed-port listener starts, no game/client process is stopped or replaced, and no plugin is installed.

- [ ] **Step 5: Commit Task 4**

Commit only the files listed in this task with message `[Rmv] remove legacy HTTP bridge`.
