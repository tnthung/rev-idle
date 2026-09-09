# Bidirectional Raw WebSocket Protocol Foundation Design

## Goal

Establish one persistent, symmetric packet connection using WebSocket framing directly over TCP. The C# game plugin is the server on `127.0.0.1:19841`; the Rust client reconnects once per second whenever no connection is active.

Either peer can concurrently send packets, request a typed response, and register asynchronous packet handlers. Application callers never write to the socket directly; one multi-producer, single-consumer writer owns each physical connection.

## Phase Boundary

This phase implements the generic protocol, both connection runtimes, production connection ownership, and protocol-only tests. It does not migrate `state`, `capture`, `invoke`, or `transfer` from HTTP.

The C# plugin starts the raw framed server on the existing configured port instead of starting `HttpScoreServer`. The Rust application starts and retains the new connection supervisor while its existing bridge calls continue to hold and use `reqwest::Client`. Consequently, the legacy calls are intentionally unavailable in this intermediate revision. The application will not be run as an automation client until the later migration is complete.

This phase does not delete `HttpScoreServer`, remove `reqwest`, define bridge-specific WebSocket packets, or change JavaScript APIs. Those changes belong to the later migration.

## Public API

Rust exposes one cloneable logical connection handle:

```rust
trait Packet: Serialize + DeserializeOwned + Send + 'static {
    const TYPE: &'static str;
}

trait Requestable: Packet {
    type Response: Packet;
}

impl WsConnection {
    fn handler<P, F, Fut>(&self, handler: F)
    where
        P: Packet,
        F: Fn(PacketContext, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static;

    async fn request<P: Requestable>(&self, packet: P) -> Result<P::Response>;
    async fn send<P: Packet>(&self, packet: P) -> Result<()>;
}
```

C# exposes the equivalent logical connection:

```csharp
interface IRequest<TResponse> { }

sealed class WsConnection
{
    public void Handler<TPacket>(Func<PacketContext, TPacket, Task> handler);
    public Task<TResponse> Request<TResponse>(IRequest<TResponse> packet);
    public Task Send<TPacket>(TPacket packet);
}
```

Packet DTO names are the wire type names. Rust implementations use an explicit matching `Packet.TYPE`; this keeps cross-language names stable when Rust module or CLR namespace names change.

`request()` and `send()` are local caller behaviors, not protocol concepts. `request()` installs a UUID waiter before sending and awaits the first correlated response. `send()` sends the same kind of packet without installing a waiter.

## Wire Protocol

Every text frame contains one JSON envelope:

```json
{
  "uuid": "7747b71a-66bc-4fc6-bf85-e9828277addf",
  "type": "CaptureReq",
  "payload": {
    "x": 123,
    "y": 456
  }
}
```

There is no request, notification, or response marker. On receipt, a peer:

1. Applies a reserved `Cancel` packet to the active inbound handler with that UUID.
2. Resolves an outbound waiter when the UUID matches a pending request.
3. Reports an unmatched reserved `RemoteError` locally.
4. Discards the UUID when it belongs to a recently timed-out request.
5. Otherwise dispatches the packet to the asynchronous handler registered for its type.

`PacketContext.Send(packet)` sends the response with the inbound UUID automatically. It may be called once. The response may finish out of order because UUID correlation, not arrival order, identifies it.

`RemoteError` and `Cancel` are reserved packet types, not envelope modes. A handler failure before a response sends a correlated `RemoteError` regardless of whether the origin installed a waiter. If a waiter exists, `RemoteError` fails that request; otherwise it is reported locally. A failure after `PacketContext.Send` is reported locally because the UUID has already received its response. A request timeout sends a best-effort `Cancel` packet so an inbound handler that has not started can be skipped and a running handler can observe cancellation.

Each physical connection retains up to 1,024 timed-out request UUIDs until it closes. This bounded set discards late responses rather than misinterpreting them as new inbound packets; the oldest UUID is evicted when the bound is reached.

## Connection Lifecycle

The C# plugin listens only on `127.0.0.1:19841` and accepts one active Rust client. The server continues listening after a disconnect. A second connection is rejected while the first remains active.

The Rust connection supervisor attempts to connect immediately. If the attempt fails, or an established connection later closes, it waits one second and tries again. Reconnection is independent of application calls.

While disconnected:

- `request()` returns `NotConnected` immediately.
- `send()` returns `NotConnected` immediately.
- calls are not queued for a later connection;
- packets are never replayed after reconnection.

When a physical connection closes, all pending requests and queued writes belonging to that connection fail immediately. A new connection receives a new generation, so stale work cannot cross the reconnect boundary.

Connected requests retain the existing two-second response timeout. Timeout cancellation is best effort: it prevents work that has not begun, but cannot undo a handler's completed side effects.

## Outbound MPSC Model

Each physical connection owns one 256-entry outbound channel and one writer task. Every clone of `WsConnection`, every handler context, and every protocol path submits complete envelopes to that channel.

```text
request / send / context.send
              |
              v
       bounded MPSC channel
              |
              v
       single writer task
              |
              v
       physical WebSocket
```

An outbound entry includes the serialized envelope, connection generation, and a completion sender. The public send operation completes only after the writer has written the WebSocket frame. A closed connection fails the entry rather than retaining it for the next writer.

The Rust implementation uses Tokio `mpsc`. The C# implementation uses an equivalent channel with many producers and one reader. No socket mutex is exposed to application code, and no other task calls the WebSocket send operation.

## Inbound Dispatch

One reader task owns the receive half of each physical connection. It reassembles complete text messages, parses envelopes, resolves correlated waiters, and schedules unmatched packets without awaiting their handlers.

Rust starts each unmatched packet handler as an independent Tokio task. A long-running handler therefore does not delay reading or dispatching later packets.

C# receives and parses frames on a background task. Unmatched packets enter the existing Unity-thread queue. `ScoreTicker.Update()` starts queued asynchronous handlers without awaiting previously started handlers. Production registers no application handlers in this phase; protocol tests pump their test handlers explicitly. During the later bridge migration, handlers must finish Unity-dependent work before their first `await` or explicitly enqueue later Unity work onto the Unity-thread queue.

Handler tasks are tracked until completion so exceptions are observed. A failed handler sends `RemoteError` with the same UUID. `PacketContext` exposes cancellation caused by a `Cancel` packet or connection loss.

## Raw WebSocket Framing

The transport uses WebSocket data framing but deliberately omits the RFC 6455 HTTP opening handshake. It is a private protocol between these two processes and is not compatible with browser WebSocket clients.

The C# server accepts a raw stream from `TcpListener` and immediately wraps it with `.NET`'s `WebSocket.CreateFromStream(stream, isServer: true, subProtocol: null, keepAliveInterval: Timeout.InfiniteTimeSpan)`. The framework handles WebSocket framing, fragmentation, ping/pong, and close frames.

The Rust supervisor establishes a plain Tokio `TcpStream`, immediately wraps it with `tokio_tungstenite::WebSocketStream::from_raw_socket(stream, Role::Client, None)`, and splits the stream. The reader owns the receive half and the MPSC writer owns the send half. It does not enable or call `tokio-tungstenite` handshake helpers.

No third-party C# server dependency, HTTP upgrade implementation, or custom WebSocket frame implementation is introduced. `reqwest` is not used anywhere in the new connection module.

## Deferred Bridge Migration

No `StateReq`, `CaptureReq`, `InvokeReq`, or `TransferReq` types are added in this phase. The existing HTTP routes, HTTP serializers, `reqwest` helpers, and JavaScript bindings are not rewritten.

Protocol tests define private test packets to exercise sending, requesting, correlated replies, errors, cancellation, and asynchronous handlers in both directions. Production registers no application packet handlers until the migration phase.

## Error Handling

- Malformed JSON and invalid UUIDs are reported locally. Missing or unknown packet types produce `RemoteError` with the same UUID. These packet errors do not stop later valid packets unless the WebSocket itself becomes unusable.
- A payload that cannot deserialize to its registered type produces `RemoteError` with the same UUID.
- A response packet whose type differs from `Requestable.Response` fails the local waiter.
- A missing response times out after two seconds.
- A disconnect fails pending requests and queued writes for that connection generation.
- A handler response attempted after cancellation or disconnect fails without being replayed.

## File Boundaries

Rust transport protocol and connection ownership live in `client/src/bridge/connection.rs`. `client/src/app/mod.rs` starts and retains the connection supervisor. Operation-specific bridge modules and every existing `reqwest::Client` parameter remain unchanged.

C# transport and handler registration live in `plugin/src/WsConnection.cs`. `Plugin.cs` starts it on the currently configured port, and `ScoreTicker.Update()` pumps inbound handler starts. `HttpScoreServer.cs` and its tests remain unchanged but the production plugin no longer starts it.

Existing state serialization, UI dispatch, HTTP bridge modules, capture worker, script lifecycle, session, bindings, and JavaScript remain unchanged.

## Validation

Protocol tests on both sides cover envelope serialization, UUID echoing, response-type validation, remote errors, cancellation, and malformed input.

Protocol loopback tests use ephemeral ports and cover:

- the Rust endpoint both requesting from and handling packets sent by a raw test peer;
- the C# endpoint both requesting from and handling packets sent by a raw test peer;
- concurrent callers sharing one physical writer;
- handlers completing out of order without blocking later packets;
- immediate `NotConnected` failures;
- connection attempts occurring once per second until the server appears;
- pending and queued work failing on disconnect;
- reconnection without replaying old packets;
- late timed-out responses not becoming new inbound requests.

One exact shared JSON fixture verifies that Rust and C# use the same envelope field names, UUID representation, packet type placement, and payload shape. Language-specific protocol tests verify the reserved packet names. Existing HTTP, state, capture, invoke, transfer, QuickJS, Unity-thread, plugin build, and installer tests remain present; this phase does not claim the intermediate production application can serve those HTTP calls.

No formatting tools are run. Git state is not staged or committed unless explicitly requested.
