# Window-Relative Automation Design

## Goal

Make JavaScript mouse automation stable relative to the Revolution Idle game window. Scripts can set the game client-area size, clicks use client-relative coordinates, every click focuses the game first, and a global F8 hotkey immediately gates further automation before the script lifecycle enters its normal paused state.

## Public JavaScript API

The existing `rev` object gains these behaviors:

```javascript
(rev) => {
    rev.resize(1280, 720);
    rev.click(100, 200, "left");
}
```

### `rev.resize(width, height)`

- `width` and `height` must be finite, positive, signed 32-bit integers.
- The requested dimensions describe the drawable client area, excluding the title bar and window borders.
- The function finds the game window at call time, restores it if minimized or maximized, and resizes it without intentionally changing its screen position or Z-order.
- After resizing, it reads the client rectangle again and throws if Windows did not produce the requested client size.
- It returns `undefined` on success.

### `rev.click(x, y, button = "left")`

- `x` and `y` must be finite signed 32-bit integers.
- Coordinates are relative to the drawable client area's top-left corner.
- Valid coordinates satisfy `0 <= x < clientWidth` and `0 <= y < clientHeight`. Coordinates on or beyond the right or bottom edge are rejected.
- The function finds the game window, brings it to the foreground, verifies that it became the foreground window, reads the current client rectangle, validates the relative coordinates, converts them to screen coordinates, and then uses Enigo for the mouse movement and click.
- The action gate is checked before window work and again immediately before Enigo input. A blocked operation throws without issuing subsequent window or mouse actions.

Existing button names and validation remain unchanged: `left`, `right`, and `middle` are supported, with `left` as the default.

## Game Window Discovery

The client uses Win32 APIs to enumerate visible top-level windows. For each candidate it obtains the owning process ID and queries the executable image name. Matching against `Revolution Idle.exe` is case-insensitive and does not depend on the window title.

Each JavaScript window action performs discovery again rather than caching an `HWND`, so closing and reopening the game cannot leave a stale handle. Exactly one matching window is required. No match or multiple matches produces a JavaScript exception and no action.

## Window Control

A dedicated `window.rs` module owns the production Win32 implementation and exposes a narrow interface to the script host.

- `EnumWindows` and `GetWindowThreadProcessId` identify top-level window owners.
- `OpenProcess` and `QueryFullProcessImageNameW` identify `Revolution Idle.exe`.
- `GetClientRect` reads client width and height.
- `ClientToScreen` converts the client origin to screen coordinates, including negative coordinates on monitors left of or above the primary display.
- `ShowWindow(..., SW_RESTORE)` restores a minimized or maximized game before resizing.
- `GetWindowRect`, `GetClientRect`, and `SetWindowPos` preserve the current non-client frame while producing the requested client dimensions. A post-resize client-rectangle check establishes the exact result.
- `SetForegroundWindow` requests focus before a click, and `GetForegroundWindow` verifies it. If Windows foreground-lock policy denies focus, clicking is blocked and JavaScript receives an exception.

Win32 failures are reported with operation-specific messages. Checked arithmetic prevents coordinate or outer-size overflow before values reach Win32 or Enigo.

## Immediate F8 Gate and Lifecycle Pause

A dedicated `hotkey.rs` module owns a standard OS thread with a Win32 message loop. It registers unmodified F8 through `RegisterHotKey` using `MOD_NOREPEAT`. F8 is not implemented through Enigo because Enigo 0.6.1 simulates input but does not listen for global physical input.

The hotkey worker shares an atomic action gate with the script host and owns a clone of the existing script-command sender. On each F8 event it:

1. Immediately toggles the atomic gate.
2. Sends `ScriptCommand::SetPaused(newState)` to the script task.

Closing the gate prevents the next Win32 or Enigo operation that has not already been issued. It cannot retract an operating-system call or click already in progress. A JavaScript call that reaches a gated host function receives an `automation paused` exception; unless caught, that exception ends the current invocation early.

The script task remains the only owner of lifecycle state. It consumes `SetPaused(bool)` at the next existing command boundary:

- `SetPaused(true)` changes Running to Paused and is a no-op when already Paused.
- `SetPaused(false)` changes Paused to Running and is a no-op when already Running.
- Stopped remains Stopped, resets the gate to open, and reports that no script is running.

The existing console `pause` and `resume` commands update the same atomic gate when the script task applies them. Load, reload, and stop reset the gate to open. Using an explicit requested state instead of a second toggle prevents F8 and console commands from becoming inconsistent when both are queued.

If the current invocation performs no further host action, it may continue until it returns or rejects; the queued lifecycle command is then applied. Resuming opens the action gate immediately, while script execution resumes when the queued command is processed.

Hotkey registration is required for startup. If F8 is already registered or registration otherwise fails, the client exits with a clear error rather than running without its safety control.

The hotkey thread unregisters F8 during shutdown. The main task requests message-loop termination and joins the worker, so no blocking thread or global registration survives normal client shutdown.

## Component Integration

- `client/src/window.rs`: game-window discovery, exact client sizing, focus verification, bounds checking, and relative-to-screen translation.
- `client/src/hotkey.rs`: F8 registration, immediate atomic gate update, `SetPaused(bool)` delivery, message-loop shutdown, and thread joining.
- `client/src/script.rs`: `rev.resize`, relative `rev.click`, injected window-control test seam, action-gate errors, and requested-pause lifecycle handling.
- `client/src/console.rs`: add the internal `SetPaused(bool)` command variant. Text console syntax remains unchanged.
- `client/src/main.rs`: create the shared gate, start the hotkey worker, pass dependencies to the script runner, and shut the worker down deterministically.
- `client/Cargo.toml`: add a direct `windows` 0.61.3 dependency with only the required Win32 feature sets. The client targets Windows only, so no target-specific dependency section or platform guard is needed.

The current-thread Tokio runtime continues to own QuickJS and Enigo. Synchronous window operations run only while servicing JavaScript host calls. The blocking Win32 hotkey message loop runs on its dedicated OS thread and communicates through the command channel and atomic gate.

## Error Behavior

The following conditions produce JavaScript exceptions and no mouse click:

- automation is gated by F8;
- zero or multiple matching game windows exist;
- process inspection or a required Win32 query fails;
- resize dimensions or click coordinates are invalid;
- requested client dimensions overflow the Win32 size calculation;
- Windows does not produce the requested client size;
- the relative click lies outside the current client area;
- the game cannot be confirmed as the foreground window;
- Enigo fails to move or click the mouse.

F8 registration failure is a client startup error rather than a JavaScript exception.

## Testing

Production Win32 actions are hidden behind a narrow injected window-control interface so automated tests do not discover, focus, resize, or click a real game window.

Focused tests cover:

- process-name matching, zero matches, one match, and multiple matches;
- client-size calculation and post-resize verification;
- valid relative-coordinate translation, including negative screen origins;
- rejection of negative, right-edge, bottom-edge, and overflowing coordinates;
- focus and validation occurring before Enigo movement and clicking;
- blocked or failed operations never reaching Enigo;
- `rev.resize` argument validation and host-call forwarding;
- the action gate closing immediately during an active invocation;
- lifecycle pause after that invocation reaches a command boundary;
- F8 resume behavior and stopped-state behavior;
- consistent action-gate state when F8 and console lifecycle commands are interleaved;
- hotkey registration failure, no-repeat toggle delivery, shutdown, and registration cleanup through isolated worker seams.

The full Rust test suite and compiler check must pass. A bounded Windows manual check will verify discovery against a running Revolution Idle instance, exact client resizing, relative clicking, focus behavior, immediate F8 gating, resume, and clean hotkey release after client exit.

## Out of Scope

- Selecting among multiple Revolution Idle instances.
- Changing the game window's screen position.
- Supporting a configurable process name or hotkey.
- Using a keyboard hook or Enigo as an input listener.
- Interrupting arbitrary JavaScript computation or cancelling an OS action already issued.
- Adding a textual console command for F8 toggle behavior.
