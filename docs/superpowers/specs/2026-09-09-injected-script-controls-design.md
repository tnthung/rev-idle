# Bidirectional Injected Script Controls Design

## Goal

Restore the three injected Unity controls on top of the raw bidirectional WebSocket transport. Unity sends best-effort control notifications to the client, and the client pushes one complete `StateUpdate` notification back to Unity whenever its state changes.

## Packet Semantics

Every WebSocket frame keeps the existing `{ uuid, type, payload }` envelope. Correlation is local behavior, not a wire-level kind.

- Unity sends `ReloadScript`, `StopScript`, `PauseScript`, `ResumeScript`, `StartCapture`, or `StopCapture` to request a local client transition.
- The client sends `StateUpdate { phase, capture }` to replace Unity's complete control-state projection.
- These packets are independent notifications. They use `connection.Send`/`connection.send`, receive fresh UUIDs, and their handlers never call `context.Send`.
- Notification send or handling failure is non-fatal. Notifications are not queued or replayed across WebSocket generations.

Existing correlated game-data, UI-path, invoke, and transfer operations are outside this control-state protocol and remain unchanged.

## State Model

The client owns the real QuickJS lifecycle. Its script phase is exactly one of:

```text
unloaded  no script path is known
stopped   a path is known but no session is running
running   a session is running
paused    a session exists and is paused
```

Capture mode is an orthogonal boolean. It may be enabled only while the script phase is `stopped` or `paused`.

The plugin keeps only the latest `StateUpdate` for the current WebSocket generation. It discards that state on disconnect or generation replacement. On every connection generation, and after every lifecycle or capture-mode transition, the client pushes a complete `StateUpdate`. Unity never requests or polls for control state.

Unity button handlers only send notifications. They do not infer that the client applied an action or mutate the displayed projection optimistically. The projection changes only when the client independently publishes its actual state.

## Control Flow

Unity button actions are:

```text
Reload face  -> ReloadScript
Stop face    -> StopScript
Resume face  -> ResumeScript
Pause face   -> PauseScript
Capture      -> StartCapture
```

The client registers notification handlers that enqueue the corresponding existing lifecycle command. These handlers return successfully without sending a correlated response. The lifecycle remains the only code that loads or destroys `ScriptSession`, changes `ActionGate`, or validates whether capture is allowed.

Local client actions use the same lifecycle transitions. F8, console commands, Ctrl+C, self-stop, load failure, and invocation failure therefore push the same complete `StateUpdate` as Unity-originated actions.

## One-Shot Capture

`StartCapture` arms the existing low-level Windows hook. The first left-button-down atomically disarms capture, consumes that down and its matching up, and queues `CaptureConsumed` into the lifecycle. That state transition makes the client push `StateUpdate` with `capture: false`.

The captured coordinate is still resolved with correlated `UiPathReq -> UiPathRes` because the client needs the returned path. A valid button or slot path is printed and copied, followed by `Copied to clipboard`. No target leaves the clipboard unchanged and prints no capture-exit message.

## Reconnection

The Rust transport exposes its active connection generation as a watchable value. The control publisher sends the latest `StateUpdate` once when a non-zero generation appears. It sends nothing while disconnected and does not run a retry timer; the existing transport supervisor owns the one-second connection cadence.

The C# transport exposes the current generation as a read-only value. The plugin treats control state from an older generation as unavailable, so all buttons remain disabled until the current client publishes its state.

## Overlay

The bottom-right overlay contains three 30 by 30 pixel buttons with 10 pixel padding and spacing, a one-pixel black border, rounded corners, `#454A4F` normal background, and `#222222` disabled background.

The projection is:

| Script phase | Reload/Stop | Resume/Pause | Capture |
| --- | --- | --- | --- |
| `unloaded` | disabled | disabled | disabled |
| `stopped` | Reload | disabled | enabled |
| `running` | Stop | Pause | disabled |
| `paused` | Stop | Resume | enabled |
| any phase while capturing | disabled | disabled | disabled |

Stop uses an outlined square, Pause uses two narrow separated bars, and Resume uses a narrow right-pointing triangle so none rasterizes as a solid white block. Tooltips use the existing enlarged font and dark background.

## Path Compatibility

Captured UI paths retain their full `scene:<handle>/name[index]/...` form. Resolution ignores only the runtime-unstable scene handle after normal lookup misses; every escaped name and sibling index remains exact. Existing scripts are not rewritten to a newly observed handle.

## Error Handling

- A disconnected notification fails immediately and is ignored or logged once by its caller.
- A notification handler never returns a lifecycle failure to the remote sender.
- Correlated request failures continue to reject the awaiting caller.
- Unknown or malformed packets retain the transport's existing protocol diagnostics.
- Client shutdown cancels the `StateUpdate` publisher through the existing shutdown watch and never waits for a remote response.

## Scope

Remove the restored HTTP long-poll control implementation completely. Do not add another listener, heartbeat, command queue, replay buffer, transport-specific overlay logic, or persistent control state. Do not start or install the game, plugin, or client during implementation.
