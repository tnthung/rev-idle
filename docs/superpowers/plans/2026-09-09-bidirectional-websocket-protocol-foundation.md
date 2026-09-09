# Bidirectional Raw WebSocket Protocol Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the symmetric typed packet protocol and raw WebSocket-framed TCP connection on Rust and C# without migrating any existing HTTP bridge request.

**Architecture:** The C# plugin accepts one raw loopback TCP client and wraps the stream directly as a server-role WebSocket; Rust connects with a plain Tokio `TcpStream` and wraps it directly as a client-role WebSocket. Each physical connection owns a bounded MPSC queue and one writer, while its reader correlates UUIDs or starts independent async handlers.

**Tech Stack:** Rust 2024, Tokio, tokio-tungstenite, futures-util, serde, serde_json, uuid; C#/.NET 6, `TcpListener`, `System.Net.WebSockets`, `System.Threading.Channels`, `System.Text.Json`.

**Spec:** `docs/superpowers/specs/2026-09-09-bidirectional-websocket-transport-design.md`

## Global Constraints

- Bind the production server only to `127.0.0.1:19841`.
- Do not perform an HTTP WebSocket handshake or use `reqwest` inside the new transport.
- Retry the Rust TCP connection once per second while disconnected.
- Make `request()` and `send()` fail immediately while disconnected; never queue or replay calls across reconnection.
- Use one 256-entry MPSC channel and one socket writer per physical connection.
- Dispatch packet handlers asynchronously without awaiting earlier handlers.
- The envelope contains only `uuid`, `type`, and `payload`; it has no request/response mode.
- Do not add bridge-specific packets or migrate `state`, `capture`, `invoke`, `transfer`, or JavaScript bindings.
- Do not delete `HttpScoreServer` or remove `reqwest` in this phase.
- Do not run the application or plugin in the game during this intermediate phase.
- Do not run a build or test that requires stopping, replacing, or freeing the port used by the currently running game or client.
- Do not run formatting tools.
- Do not stage, commit, reset, or otherwise mutate Git state.

## File Map

- Create `protocol/fixtures/test-request.json`: exact cross-language envelope fixture.
- Modify `client/Cargo.toml` and `client/Cargo.lock`: add direct protocol dependencies without handshake features.
- Create `client/src/bridge/connection.rs`: Rust protocol, connection supervisor, raw socket session, correlation, handlers, and focused tests.
- Modify `client/src/bridge/mod.rs`: expose the new logical connection internally.
- Modify `client/src/app/mod.rs`: retain and shut down the Rust connection supervisor alongside the legacy HTTP client.
- Create `plugin/src/WsConnection.cs`: C# protocol, raw socket server/session, correlation, handler pump, and lifecycle.
- Modify `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`: link the new source and copy the shared fixture.
- Modify `plugin/tests/Program.cs`: add protocol-only C# tests and raw peer helpers.
- Modify `plugin/src/Plugin.cs`: start, pump, and dispose the raw protocol server instead of starting the legacy HTTP server.

---

### Task 1: Rust wire contract and disconnected API

**Files:**
- Create: `protocol/fixtures/test-request.json`
- Modify: `client/Cargo.toml`
- Modify: `client/Cargo.lock`
- Create: `client/src/bridge/connection.rs`
- Modify: `client/src/bridge/mod.rs`

**Interfaces:**
- Produces: `Packet`, `Requestable`, `PacketContext`, `WsConnection`, and `WsError` in `client::bridge::connection`.
- Produces: exact envelope `{ uuid, type, payload }` with hyphenated lowercase UUID text.
- Preserves: every existing HTTP bridge module and re-export.

- [ ] **Step 1: Add the exact shared fixture**

Create `protocol/fixtures/test-request.json` with one trailing newline:

```json
{"uuid":"7747b71a-66bc-4fc6-bf85-e9828277addf","type":"TestReq","payload":{"value":"hello"}}
```

- [ ] **Step 2: Add direct Rust dependencies**

Add these aligned entries to `[dependencies]` in `client/Cargo.toml` without removing `reqwest`:

```toml
futures-util     = { version = "0.3.34", default-features = false, features = ["sink", "std"] }
tokio-tungstenite = { version = "0.30.0", default-features = false }
uuid             = { version = "1.26.0", features = ["serde", "v4"] }
```

Run from `client`:

```powershell
cargo check
```

Expected: dependency resolution updates `Cargo.lock`; compilation still succeeds before the new module is exposed.

- [ ] **Step 3: Write failing wire-contract tests**

Start `client/src/bridge/connection.rs` with a `#[cfg(test)] mod tests` containing private packets and these assertions:

```rust
#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
struct TestReq {
    value: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
struct TestRes {
    value: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
struct OtherRes {
    value: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct DelayReq {
    gate: u8,
}

impl Packet for TestReq {
    const TYPE: &'static str = "TestReq";
}

impl Packet for TestRes {
    const TYPE: &'static str = "TestRes";
}

impl Packet for OtherRes {
    const TYPE: &'static str = "OtherRes";
}

impl Packet for DelayReq {
    const TYPE: &'static str = "DelayReq";
}

impl Requestable for TestReq {
    type Response = TestRes;
}

#[test]
fn envelope_matches_shared_fixture() {
    let envelope = Envelope::new(
        uuid::Uuid::parse_str("7747b71a-66bc-4fc6-bf85-e9828277addf").unwrap(),
        TestReq::TYPE,
        TestReq { value: "hello".to_owned() },
    ).unwrap();

    assert_eq!(
        serde_json::to_string(&envelope).unwrap(),
        include_str!("../../../protocol/fixtures/test-request.json").trim_end(),
    );
}

#[tokio::test]
async fn disconnected_calls_fail_immediately() {
    let connection = WsConnection::disconnected_for_test();

    assert_eq!(
        connection.request(TestReq { value: "request".to_owned() }).await.unwrap_err(),
        WsError::NotConnected,
    );
    assert_eq!(
        connection.send(TestReq { value: "send".to_owned() }).await.unwrap_err(),
        WsError::NotConnected,
    );
}
```

Add `mod connection;` to `client/src/bridge/mod.rs` now so the new test module participates in the RED run. Do not add its production re-export yet.

- [ ] **Step 4: Run the focused tests and confirm they fail**

Run from `client`:

```powershell
cargo test bridge::connection -- --nocapture
```

Expected: compilation fails because the protocol types and methods are not implemented.

- [ ] **Step 5: Implement the minimal wire and public type surface**

Define the following in `connection.rs`; keep all implementation-only types private:

```rust
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};
use uuid::Uuid;

const OUTBOUND_CAPACITY: usize = 256;
const TIMED_OUT_CAPACITY: usize = 1_024;
const RESPONSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
const RECONNECT_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

pub(crate) trait Packet: Serialize + DeserializeOwned + Send + 'static {
    const TYPE: &'static str;
}

pub(crate) trait Requestable: Packet {
    type Response: Packet;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WsError {
    NotConnected,
    Closed,
    Timeout,
    Remote(String),
    UnexpectedResponse { expected: &'static str, actual: String },
    Protocol(String),
    AlreadyResponded,
}

#[derive(Deserialize, Serialize)]
struct Envelope {
    uuid: Uuid,
    #[serde(rename = "type")]
    packet_type: String,
    payload: Value,
}
```

Implement `Display` and `std::error::Error` directly for `WsError`. `Envelope::new` must use `serde_json::to_value` and preserve the declaration order shown in the fixture.

Create an `Arc<Inner>`-backed `WsConnection`. Its initial active session is `None`; therefore both API methods return `WsError::NotConnected` before serializing or queueing. Add only the `#[cfg(test)] disconnected_for_test()` constructor needed by the test.

- [ ] **Step 6: Expose the module without changing legacy exports**

Update `client/src/bridge/mod.rs` to include:

```rust
mod connection;

pub(crate) use connection::WsConnection;
```

Leave `request_state`, `invoke`, `request_capture`, and `transfer` unchanged.

- [ ] **Step 7: Run the focused tests**

Run from `client`:

```powershell
cargo test bridge::connection -- --nocapture
```

Expected: the fixture and immediate-disconnection tests pass.

---

### Task 2: Rust raw connection, single writer, correlation, handlers, and retry

**Files:**
- Modify: `client/src/bridge/connection.rs`

**Interfaces:**
- Consumes: `Packet`, `Requestable`, `Envelope`, `WsError`, and constants from Task 1.
- Produces: `WsConnection::connect(SocketAddr)`, `shutdown()`, `handler()`, functional `request()` and `send()`, and `PacketContext::send()`/cancellation access.
- Produces: raw WebSocket framing through `WebSocketStream::from_raw_socket`; no HTTP handshake code.

- [ ] **Step 1: Write failing raw-session and concurrency tests**

Add a local test helper that binds `127.0.0.1:0`, accepts one Tokio stream, and wraps it as the opposite role:

```rust
async fn raw_server(
) -> (
    std::net::SocketAddr,
    tokio::sync::oneshot::Receiver<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>,
) {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        tx.send(tokio_tungstenite::WebSocketStream::from_raw_socket(
            stream,
            tokio_tungstenite::tungstenite::protocol::Role::Server,
            None,
        ).await).ok();
    });
    (address, rx)
}
```

Add these tests with the stated setup and assertions:

| Test | Setup, action, and required assertions |
| --- | --- |
| `concurrent_sends_share_one_raw_connection` | Connect one peer, join 32 `send(TestReq)` futures, read exactly 32 text messages from that peer, assert every payload is present once, all UUIDs are unique, and the listener accepted only one TCP stream. |
| `request_resolves_only_matching_uuid_and_type` | Start one request, read its envelope at the peer, reply with `TestRes` using that UUID, and assert the returned `TestRes.value` equals the peer's value. |
| `response_type_mismatch_fails_the_waiter` | Reply to a pending `TestReq` using the same UUID and type `OtherRes`; assert `WsError::UnexpectedResponse { expected: "TestRes", actual: "OtherRes" }`. |
| `handlers_run_concurrently_and_reply_out_of_order` | Register `DelayReq`; make request A wait on a test gate and request B reply immediately; assert B's UUID is received before releasing A, then release A and assert its UUID. |
| `disconnect_fails_pending_and_queued_work_without_replay` | Start a request and enough sends to leave queued work, close the peer, assert every unfinished call returns `Closed`, accept the supervisor's next connection, and assert no old UUID is received within 100 milliseconds. |
| `supervisor_retries_until_server_appears` | Reserve then release an ephemeral address, start with a 20 millisecond test retry, wait 45 milliseconds before binding the listener, and assert it accepts the connection within the next 40 milliseconds without an application call. |
| `timeout_sends_cancel_and_late_response_is_discarded` | Read a request without replying, assert it returns `Timeout` after the injected 20 millisecond test timeout, read `Cancel` with the same UUID, send a late `TestRes`, and assert the registered `TestRes` handler remains uncalled. |
| `remote_error_fails_only_its_matching_waiter` | Start two requests, reply to the first with `RemoteError { message: "failed" }`, reply normally to the second, and assert the first returns `WsError::Remote("failed")` while the second succeeds. |
| `malformed_and_unknown_packets_do_not_stop_the_reader` | Send invalid JSON, then an envelope with a valid UUID and unknown type, then a valid `TestReq`; assert the unknown UUID receives `RemoteError` and the valid handler still runs. |

Follow the table in order. For each row, add only that failing test, run it and record the expected RED output, implement the minimum behavior that turns it green, and rerun it before adding the next test. The implementation steps below define the required interfaces but do not permit writing later behavior before its failing test.

Use Tokio timeouts around every peer read so a failure cannot hang the test process. Do not bind fixed port `19841` in tests.

- [ ] **Step 2: Run the new tests and confirm failure**

Run from `client`:

```powershell
cargo test bridge::connection -- --nocapture
```

Expected: compilation fails on missing connection, handler, and shutdown methods.

- [ ] **Step 3: Implement one physical raw session and its writer**

Use these private records:

```rust
#[derive(Clone)]
struct Active {
    generation: u64,
    outbound: tokio::sync::mpsc::Sender<Outbound>,
}

struct Outbound {
    envelope: Envelope,
    written: tokio::sync::oneshot::Sender<Result<(), WsError>>,
}

struct Pending {
    generation: u64,
    expected: &'static str,
    response: tokio::sync::oneshot::Sender<Result<Value, WsError>>,
}
```

`run_session` must:

1. Call `TcpStream::connect(address)`.
2. Call `WebSocketStream::from_raw_socket(stream, Role::Client, None).await`.
3. Split the stream with `futures_util::StreamExt::split`.
4. Create a new 256-entry `mpsc` channel and publish an `Active` with a new generation.
5. Give the sink only to the writer loop and the stream only to the reader loop.
6. Clear only the matching active generation when either loop exits.
7. Fail its pending waiters and queued write completions with `WsError::Closed`.

The writer serializes each envelope once and calls `SinkExt::send(Message::Text(json.into()))`. It completes `written` only after `send` returns. No caller locks or accesses the sink.

- [ ] **Step 4: Implement local request/send semantics**

`send<P: Packet>` creates a UUID and calls one internal `send_with_uuid`. `request<P: Requestable>` must use this exact ordering:

1. Snapshot the active generation or return `NotConnected` immediately.
2. Create a UUID and insert `Pending { generation, expected: P::Response::TYPE, response }`.
3. Submit the envelope to that active generation and await its writer completion.
4. Remove the pending entry if writing fails.
5. Await the response for two seconds.
6. On timeout, remove the waiter, retain the UUID in the 1,024-entry FIFO/set, and best-effort send `Cancel` with the same UUID.

The reader checks `Cancel`, pending UUIDs, unmatched `RemoteError`, timed-out UUIDs, then handlers in that order. A matching non-error response must equal the waiter's expected packet type before its payload is delivered.

- [ ] **Step 5: Implement typed async handlers**

Store handlers as type-erased `Arc` closures:

```rust
type HandlerFuture = Pin<Box<dyn Future<Output = Result<(), WsError>> + Send>>;
type Handler = Arc<dyn Fn(PacketContext, Value) -> HandlerFuture + Send + Sync>;
```

`handler<P, F, Fut>` registers by `P::TYPE`, deserializes the payload inside the returned future, and invokes the typed closure. Reject duplicate type registration.

For every unmatched packet, create a cancellation watch channel, store its sender by `(generation, uuid)`, and spawn the handler without awaiting it in the reader. On completion:

- remove the active inbound record;
- send `RemoteError` with the inbound UUID if the handler failed before replying;
- report an error locally if it failed after replying.

`PacketContext::send` uses an atomic responded flag, the inbound UUID, and the inbound generation. It rejects a second response and never redirects the response to a newer connection. Expose `is_cancelled()` and an async `cancelled()` backed by the watch receiver.

- [ ] **Step 6: Implement supervision and shutdown**

`WsConnection::connect(address)` starts a supervisor using `RECONNECT_DELAY` and `RESPONSE_TIMEOUT`. Add a `#[cfg(test)]` constructor accepting both durations so reconnect and timeout tests use 20 milliseconds without changing production behavior. The supervisor attempts immediately, waits exactly its retry duration after connection failure or session closure, then tries again until shutdown.

Use a shutdown watch channel owned by `WsConnection`. `shutdown().await` signals it, closes the active generation, fails remaining work, and waits for the supervisor task. Application calls made after shutdown return `NotConnected`.

- [ ] **Step 7: Run Rust protocol tests and check the crate**

Run from `client`:

```powershell
cargo test bridge::connection -- --nocapture
cargo check --all-targets
```

Expected: all connection tests pass and all targets compile.

---

### Task 3: C# raw server, single writer, correlation, and handlers

**Files:**
- Create: `plugin/src/WsConnection.cs`
- Modify: `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`
- Modify: `plugin/tests/Program.cs`

**Interfaces:**
- Consumes: the shared envelope fixture and wire names from Tasks 1-2.
- Produces: C# `IRequest<TResponse>`, `PacketContext`, `WsConnection.Create`, `Handler`, `Request`, `Send`, `Pump`, and `Dispose`.
- Preserves: existing HTTP server and all existing C# tests.

- [ ] **Step 1: Link the source and fixture into the test executable**

Add to `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`:

```xml
<Compile Include="..\src\WsConnection.cs" Link="WsConnection.cs" />
<None Include="..\..\protocol\fixtures\test-request.json"
      Link="protocol\test-request.json"
      CopyToOutputDirectory="PreserveNewest" />
```

- [ ] **Step 2: Write failing C# fixture and disconnected tests**

Add these private test records at the end of `plugin/tests/Program.cs`:

```csharp
sealed record TestReq(string Value) : IRequest<TestRes>;
sealed record TestRes(string Value);
sealed record OtherRes(string Value);
```

Add calls near the existing top-level test list and implement:

```csharp
WsEnvelopeMatchesSharedFixture();
await WsDisconnectedCallsFailImmediately();

static void WsEnvelopeMatchesSharedFixture()
{
    Guid uuid = Guid.Parse("7747b71a-66bc-4fc6-bf85-e9828277addf");
    string actual = WsConnection.SerializeForTest(uuid, new TestReq("hello"));
    string expected = File.ReadAllText(Path.Combine(
        AppContext.BaseDirectory,
        "protocol",
        "test-request.json")).TrimEnd();
    Equal(expected, actual, nameof(WsEnvelopeMatchesSharedFixture));
}

static async Task WsDisconnectedCallsFailImmediately()
{
    using WsConnection connection = WsConnection.DisconnectedForTest();
    await ThrowsAsync<WsNotConnectedException>(() => connection.Request(new TestReq("request")), nameof(WsDisconnectedCallsFailImmediately));
    await ThrowsAsync<WsNotConnectedException>(() => connection.Send(new TestReq("send")), nameof(WsDisconnectedCallsFailImmediately));
}
```

For the remaining C# behaviors below, add one test at a time, run and record its expected RED result, implement only enough to make it green, then continue to the next behavior.

Add a local generic `ThrowsAsync<TException>` assertion beside the existing test helpers; it must fail when the action succeeds or throws another exception type.

- [ ] **Step 3: Run the C# harness and confirm failure**

From the repository root, using the installed game path:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0 -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
```

Expected: compilation fails because the C# protocol types are missing.

- [ ] **Step 4: Implement C# contracts and envelope serialization**

Start `plugin/src/WsConnection.cs` with file-scoped `RevIdle.ScoreTelemetry` namespace and these internal types:

```csharp
using System.Collections.Concurrent;
using System.Net;
using System.Net.Sockets;
using System.Net.WebSockets;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Channels;

namespace RevIdle.ScoreTelemetry;

internal interface IRequest<TResponse> { }

internal sealed class WsNotConnectedException : Exception
{
    public WsNotConnectedException() : base("WebSocket connection is not connected.") { }
}

internal sealed class PacketContext
{
    public CancellationToken CancellationToken { get; }
    public Task Send<TPacket>(TPacket packet);
}

internal sealed class WsConnection : IDisposable
{
    public static WsConnection? Create(string? configuredPort, Action<string>? log = null);
    public void Handler<TPacket>(Func<PacketContext, TPacket, Task> handler);
    public Task<TResponse> Request<TResponse>(IRequest<TResponse> packet);
    public Task Send<TPacket>(TPacket packet);
    public void Pump();
    public void Dispose();
}
```

Use private envelope properties decorated with `JsonPropertyName("uuid")`, `JsonPropertyName("type")`, and `JsonPropertyName("payload")`. Use one private `JsonSerializerOptions` with `PropertyNamingPolicy = JsonNamingPolicy.CamelCase`. Packet type names are `packet.GetType().Name` and registered types are `typeof(TPacket).Name`.

Define private reserved DTOs equivalent to `Cancel {}` and `RemoteError { string Message }`, with exact type names `Cancel` and `RemoteError`. Route local protocol reports through the optional `Action<string>`; production supplies the BepInEx logger and tests may omit it.

- [ ] **Step 5: Write failing raw-session behavior tests**

Add protocol tests for these exact cases:

```csharp
await WsRequestCorrelatesResponseByUuidAndType();
await WsConcurrentSendsShareOneWriter();
await WsHandlersStartWithoutWaitingForEarlierHandlers();
await WsResponseTypeMismatchFailsRequest();
await WsRemoteErrorFailsRequest();
await WsTimeoutSendsCancelAndDropsLateResponse();
await WsDisconnectFailsPendingAndAllowsNewClient();
await WsRejectsSecondActiveClient();
await WsMalformedPacketsDoNotStopReader();
```

Create test clients with raw TCP and no HTTP request:

```csharp
static async Task<(TcpClient Client, WebSocket Socket)> ConnectRawClient(WsConnection server)
{
    TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    return (
        client,
        WebSocket.CreateFromStream(
            client.GetStream(),
            isServer: false,
            subProtocol: null,
            keepAliveInterval: Timeout.InfiniteTimeSpan));
}
```

Each test uses an ephemeral server port, pumps test handlers explicitly, and applies a finite cancellation token to every receive. Add an internal `CreateForTest(port, responseTimeout)` constructor so the timeout test uses 20 milliseconds while production remains fixed at two seconds.

- [ ] **Step 6: Implement the C# physical session and MPSC writer**

`Create` validates `1..65535`, binds `TcpListener` to `IPAddress.Loopback`, records the actual `Port`, and starts one accept loop. A session immediately calls:

```csharp
WebSocket.CreateFromStream(
    client.GetStream(),
    isServer: true,
    subProtocol: null,
    keepAliveInterval: Timeout.InfiniteTimeSpan)
```

Each accepted physical session owns `Channel.CreateBounded<Outbound>(256)`. Exactly one writer task reads that channel and calls `WebSocket.SendAsync`; its reader task alone calls `ReceiveAsync` and reassembles fragmented text messages. Reject a second `TcpClient` by closing it while the active session remains open.

An outbound record contains generation, serialized UTF-8 bytes, and `TaskCompletionSource` configured with `RunContinuationsAsynchronously`. Complete it only after `SendAsync` succeeds. Session teardown closes the channel and fails all queued entries and pending requests for its generation.

- [ ] **Step 7: Implement C# request correlation and async handlers**

Use `ConcurrentDictionary<Guid, Pending>` for outbound requests, `ConcurrentDictionary<string, HandlerRegistration>` for handlers, and `ConcurrentQueue<Inbound>` for work that must start in `Pump()`.

`Request<TResponse>` installs its pending UUID before enqueueing, expects `typeof(TResponse).Name`, waits two seconds, then sends best-effort `Cancel` and retains the timed-out UUID in the bounded FIFO/set. `Send<TPacket>` skips only the pending waiter; it uses the same envelope and writer path.

The reader applies the same routing order as Rust. `Pump()` dequeues all currently available inbound packets, starts each registered `Task`, records its cancellation source, and does not await earlier handler tasks. Observe each task separately so failure before `PacketContext.Send` sends `RemoteError`; failure afterward is logged.

`PacketContext.Send` uses `Interlocked.Exchange` to allow one response, preserves UUID and generation, and submits through the session's MPSC writer. Its cancellation token is canceled by `Cancel`, timeout/disconnect, or disposal.

- [ ] **Step 8: Run all C# tests**

Run from the repository root:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0 -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
```

Expected: existing tests plus all protocol tests pass; the final printed count is updated to the actual total.

---

### Task 4: Production connection ownership without bridge migration

**Files:**
- Modify: `client/src/app/mod.rs`
- Modify: `plugin/src/Plugin.cs`

**Interfaces:**
- Consumes: Rust and C# `WsConnection` from Tasks 2-3.
- Produces: one Rust reconnect supervisor and one C# raw server on production port `19841`.
- Preserves: the existing Rust `reqwest::Client`, bridge helpers, script/capture ownership, and HTTP implementation source.

- [ ] **Step 1: Write a Rust application-lifetime test seam**

Keep connection behavior tests in `connection.rs`; add a focused test that calls `shutdown()` while the supervisor is retrying and asserts completion within 100 milliseconds. This establishes that application shutdown cannot be held open by the one-second retry delay.

- [ ] **Step 2: Retain the Rust supervisor in `app::run`**

Import `SocketAddr` and construct the connection before entering `LocalSet`:

```rust
let connection = crate::bridge::WsConnection::connect(std::net::SocketAddr::from((
    [127, 0, 0, 1],
    19841,
)));
```

Keep the existing `reqwest::Client` construction and every place it is cloned or moved. Change the existing leading `local` expression to `let result = local`, keep its complete `run_until` body byte-for-byte, and replace the final `.await` tail with:

```rust
        .await;
    connection.shutdown().await;
    result
}
```

- [ ] **Step 3: Switch C# production ownership to the raw server**

In `Plugin.cs`, replace the production `HttpScoreServer` field with:

```csharp
private static WsConnection? _connection;
```

Keep the existing config key and default port, but change its description to `"Raw packet server port on 127.0.0.1. Set to 0 to disable."`. Start `_connection = WsConnection.Create(configuredPort)`.

Remove only the parameterless production `CompletePending(nint window)` wrapper. Preserve `CompletePending(HttpScoreServer server, Func<object?> getData, nint window = 0)` because existing HTTP tests and the later migration still use it.

Add:

```csharp
internal static void PumpPackets()
{
    try { _connection?.Pump(); }
    catch (Exception exception) { _logger?.LogError($"Packet dispatch failed: {exception}"); }
}
```

Pass `message => _logger?.LogError($"[WebSocket] {message}")` as the logging callback. Change `ScoreTicker.Update()` from `Plugin.CompletePending(_bridge?.Window ?? 0)` to `Plugin.PumpPackets()`. Change `StopServer()` to dispose `_connection`. Do not register production handlers in this phase.

- [ ] **Step 4: Run focused lifecycle checks**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml bridge::connection -- --nocapture
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0 -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
```

Expected: both protocol suites and all retained C# legacy tests pass without running the application or game.

---

### Task 5: Final verification and boundary audit

**Files:**
- Verify only; no new files.

**Interfaces:**
- Verifies: raw framing, symmetric packet API, MPSC single-writer ownership, retry/disconnect semantics, async handlers, production lifetime, and deferred HTTP migration.

- [ ] **Step 1: Prove no bridge migration entered the diff**

Run from the repository root:

```powershell
git diff -- client/src/bridge/state.rs client/src/bridge/capture.rs client/src/bridge/invoke.rs client/src/bridge/transfer.rs client/src/capture.rs client/src/script plugin/src/HttpScoreServer.cs plugin/src/StatePayload.cs
```

Expected: no diff for these deferred files.

- [ ] **Step 2: Prove the new Rust transport has no handshake or reqwest path**

Run:

```powershell
rg -n "reqwest|connect_async|handshake|Sec-WebSocket|Upgrade" client/src/bridge/connection.rs plugin/src/WsConnection.cs
```

Expected: no matches.

- [ ] **Step 3: Run Rust verification**

From `client`, run:

```powershell
cargo test bridge::connection -- --nocapture
cargo check --all-targets
```

Expected: focused protocol tests and checks pass. Do not run the full suite while the current game and client own their live resources.

- [ ] **Step 4: Run C# verification**

From the repository root, run:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0 -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release -p:GameDir='E:\SteamLibrary\steamapps\common\Revolution Idle'
```

Expected: all test-harness checks pass and the plugin build completes with zero errors. Do not install or launch the plugin.

- [ ] **Step 5: Check whitespace and Git boundaries**

Run:

```powershell
git diff --check
git status --short
```

Expected: `git diff --check` is clean. Only the planned files plus the user's pre-existing `docs/todo.md` and `scripts/unity_loop2_config.js` changes appear. Do not stage or commit anything.
