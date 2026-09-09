# WebSocket Bridge Migration Design

## Goal

Migrate state, capture, invoke, and transfer from the unavailable HTTP bridge to the existing bidirectional raw-WebSocket connection without changing the JavaScript API.

The migration is atomic. Production has one transport, one connection, and no temporary HTTP fallback. After all callers and tests use packets, remove `reqwest`, `HttpScoreServer`, and their HTTP-only tests.

## Preserved behavior

- `rev.state(...keys)` returns the same frozen top-level JavaScript object and unwraps a single requested key.
- `rev.invoke(path)` and `rev.transfer(source, destination)` retain their current validation, pause behavior, and plugin error text.
- Capture lookup retains clipboard and console behavior.
- Calls made while disconnected fail immediately through `WsError::NotConnected`; they are not queued or replayed.
- State access and Unity UI operations execute on the Unity thread during `ScoreTicker.Update()`.

## Packet contract

Packets continue to use the neutral envelope `{ uuid, type, payload }`. Request versus send remains local waiter state and is not represented on the wire.

| Request | Payload | Response | Payload |
| --- | --- | --- | --- |
| `StateReq` | `{ "keys": string[] }` | `StateRes` | `{ "value": object }` |
| `CaptureReq` | `{ "x": int, "y": int }` | `CaptureRes` | `{ "type": string|null, "path": string|null }` |
| `InvokeReq` | `{ "path": string }` | `InvokeRes` | `{}` |
| `TransferReq` | `{ "source": string, "destination": string }` | `TransferRes` | `{}` |

Rust implements `Packet` for every request and response and `Requestable` for every request. C# record names match the packet type strings exactly. A shared fixture covers every payload shape in both languages.

`StatePayload.Encode` remains the state authority. C# parses its successful JSON bytes into a cloned `JsonElement` stored in `StateRes.Value`; Rust receives that as `serde_json::Value`, requires an object, and serializes it back to the raw JSON string consumed by the existing JavaScript wrapper. This avoids changing state value types or freezing behavior.

## Rust migration

The existing operation modules remain the public bridge boundary:

```rust
request_state(connection: &WsConnection, keys: &[String]) -> Result<String, String>
request_capture(connection: &WsConnection, x: i32, y: i32) -> Result<CaptureTarget, String>
invoke(connection: &WsConnection, path: String) -> Result<(), String>
transfer(connection: &WsConnection, source: String, destination: String) -> Result<(), String>
```

Each function constructs its request packet, calls `WsConnection::request`, and maps `WsError` to the existing string error boundary. No operation bypasses the connection's MPSC writer.

`WsConnection` replaces `reqwest::Client` through the current ownership chain:

```text
app::run
  +-- CaptureWorker
  `-- script::run
        `-- ScriptSession
              `-- create_rev bindings
```

The application constructs the connection before starting capture, passes clones to both consumers, and retains the original until shutdown. Test-only session constructors use a disconnected connection unless the test exercises a bridge packet.

After all references are gone, remove `reqwest` from `Cargo.toml` and `Cargo.lock`, remove the fixed-port HTTP test lock, and verify no `reqwest` or HTTP URL remains under `client/src`.

## C# migration

Add the eight packet records in one focused source file. `Plugin.Load()` creates the connection and registers all four handlers once.

An internal registration method accepts the connection plus `Func<object?> getData` and `Func<nint> getWindow`. Production supplies `GameController.data` and the current bridge window; tests supply fixtures. This retains the existing test seam without keeping an HTTP abstraction.

`ScoreTicker.Update()` passes the current bridge window to `Plugin.PumpPackets(window)`. `Pump()` invokes each handler on that thread and runs its synchronous prefix before the first incomplete `await`:

- `StateReq` reads game data, encodes the requested keys, and sends `StateRes`.
- `CaptureReq` calls `UnityUiClickDispatcher.TryCapture` and sends `CaptureRes`.
- `InvokeReq` calls `UnityUiClickDispatcher.TryInvoke` and sends `InvokeRes`.
- `TransferReq` calls `UnityUiClickDispatcher.TryTransfer` and sends `TransferRes`.

Invalid state paths, missing game data, serialization failures, and failed UI operations throw before a response. The connection converts them to correlated `RemoteError` packets. Successful UI side effects occur before awaiting `PacketContext.Send`, so Unity objects are never accessed from a continuation thread.

After packet handler tests pass, delete `HttpScoreServer.cs`, remove it from the test project, and remove only HTTP request/parser/completion tests and helpers. Retain all `StatePayload`, UI dispatcher, generic WebSocket, and input-bridge tests.

## Error and lifecycle behavior

- The connection layer continues to own UUID correlation, two-second timeout, cancellation, disconnect failure, generation isolation, and retry.
- Operation modules do not add retries or queues.
- Handler failures use correlated `RemoteError`; Rust exposes the message through the existing JavaScript error boundary.
- A response that cannot deserialize to its declared packet type fails only its matching request.
- Shutdown stops capture and script consumers before shutting down the retained connection.

## Verification

Tests use ephemeral loopback ports only and never require the running game or client to stop.

- Shared Rust/C# fixture tests cover all eight packet payloads and exact type names.
- Rust bridge tests cover state object preservation, capture fields, empty success responses, and correlated remote errors.
- QuickJS tests replace HTTP proxy/listener fixtures and prove unchanged `rev.state`, `rev.invoke`, and `rev.transfer` behavior over packets.
- Capture tests use a test connection and preserve clipboard behavior.
- C# handler tests pump requests through a raw client and verify state, capture, invoke, transfer, and error replies on the Pump caller thread.
- Existing generic protocol suites remain green.
- Cargo check and the C# Release build pass.
- Searches prove `reqwest`, HTTP URLs, `HttpScoreServer`, and HTTP-only test helpers are removed.

No application, game, plugin installation, or fixed-port test is run during verification.
