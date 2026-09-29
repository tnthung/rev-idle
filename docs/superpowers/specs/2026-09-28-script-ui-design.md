# Script UI and Cooperative Pause Design

Status: implemented and automatically verified on 2026-09-29; installed-game validation remains outstanding. The cooperative pause, screen-ownership, and interrupting stop rules below reflect the agreed behavior. See the [implementation plan](../plans/2026-09-28-script-ui.md) for verification results.

## Goal

Allow a loaded script to create named, mutable UI elements inside the game through `rev.ui`. Elements support text, background and border colors, corner radii, padding, sizing, positioning, and asynchronous pointer handlers. Monitoring and UI handlers continue while the main automation is paused.

## Constraints

- Preserve global `rev`, including its use inside imported `Action` helpers.
- Use the existing QuickJS runtime and WebSocket bridge; add no runtime fork or dependency.
- Preserve screen-ownership exclusivity and its existing pause/unlock behavior.
- Keep the first UI version to a flat collection of rectangles with optional borders, rounded corners, padding, text, and pointer handlers.
- Preserve the existing `{ uuid, type, payload }` packet envelope.
- Follow existing coding style and never run formatting tools.
- Do not stage, commit, reset, push, or otherwise mutate Git state without an explicit request.
- Implementation and source/build checks are authorized. Installation and live-game actions remain separate.

## 1. Cooperative pause

### Scheduling

Pause prevents the lifecycle from starting another call to the default export. An invocation already in flight continues, including its promises and local variables. There is at most one default-export invocation in flight. Resume does not start a second invocation while the first is still pending.

The session continues processing JavaScript jobs and asynchronous host work while paused, even when no default invocation is active. Detached promises, monitoring loops, and UI handlers keep running unless they explicitly wait for resume. Existing connection conditions for starting default invocations remain in effect; a connection-state notification must not accidentally discard an in-flight invocation.

Game-control methods such as `rev.click`, `rev.invoke`, `rev.input`, and calls through `Action` remain available while paused. Remove the old automatic pause no-ops from host bindings, including clipboard and window controls. Argument validation and connection errors retain their existing behavior.

### Script API

```typescript
readonly paused: boolean;
ensureRunning(): Promise<void>;
```

`rev.paused` reads the current pause flag each time. It is not a snapshot captured when a wrapper or callback was created.

`rev.ensureRunning()` resolves without waiting for a transition when already running. While paused, it waits for a running state using notifications, then rechecks the state. Multiple callers may wait. A rapid resume/pause sequence must not be lost or incorrectly treated as a durable permission to run.

This is a cooperative checkpoint, not a reservation: pause may occur after it resolves, and work continues until its next checkpoint. `rev.sleep` continues to measure real elapsed time.

Waiters belong to one loaded session. Once stopping begins, a pending or new `ensureRunning()` rejects with `script session stopped`; it cannot be released by a replacement session's resume. Session teardown cancels the remaining runtime work rather than waiting for infinite daemon loops to finish.

### Lifecycle hooks and stop

Keep the existing hook roles. `beforePause` runs once per transition; command-driven pause invokes it before publishing the paused transition. F8 can expose its immediate pause flag before that hook, as it does today. Both paths retain the in-flight default invocation. Hooks must not wait for resume from within `beforePause`.

Resume updates ownership/lock state and publishes running before `afterResume`, so `ensureRunning()` in that hook can complete. Finish the resume hook before scheduling another default call. Existing background jobs may progress while asynchronous hooks are awaiting.

Stop terminates the loaded session, including an in-flight default invocation, background work, UI callbacks, and lifecycle hooks. It must interrupt synchronous JavaScript such as `while (true) {}` without requiring a client or game restart. It does not wait for a return, an `await`, or an `ensureRunning()` checkpoint. Reload, replacement, exit, and application shutdown use the same termination path for the old session.

`rev.stop()` requests that same termination from any script context. Its signal belongs to the session, including wrappers captured by older invocations; it is never an invocation-local flag. Once requested, cancellation stays set until that runtime is destroyed. Throw from the binding to leave the current call promptly and let the engine interrupt enforce termination even if script code catches that error or loops in `finally`. The interrupt takes effect at an engine checkpoint; do not promise that the very next bytecode is skipped. All side-effecting host bindings, including `rev.global` and UI writes, reject further work once cancellation is set. Repeated requests are harmless and an obsolete session's request cannot stop its replacement.

### Interrupt delivery and teardown

Run the script lifecycle and QuickJS on a dedicated OS thread with its own current-thread Tokio runtime and `LocalSet`. Create, use, and drop all QuickJS values, persistent roots, `Rc` host controls, context, and runtime on that thread. Keep bridge/control reception, Ctrl+C handling, and publishers on the application's control runtime; only thread-safe channels, cancellation state, and plain data cross the boundary. A synchronous script loop must not starve the code receiving Stop.

Install the existing `AsyncRuntime::set_interrupt_handler` before evaluating any user module. Its callback checks a thread-safe terminal signal and raises QuickJS's uncatchable interruption when set. This covers module initialization, hooks, default calls, and promise jobs. Returning control from one interrupted job must lead straight to teardown; do not keep draining queued jobs from that runtime.

All host terminal-command producers must set cancellation before waiting to enqueue their command. Publish a cancellable loading session before module evaluation. Coordinate session registration and terminal commands with monotonically increasing request generations carried by terminal command envelopes. Keep interruption active until the corresponding command is handled and the old session is dropped; acknowledging one request must not clear a newer request. Stop cannot miss a pending load, superseded loads must not enter JavaScript, and a delayed request must not kill a later replacement. Wake the lifecycle separately for idle/awaiting sessions; a flag read only by the engine is insufficient when no JavaScript is running. Pause never sets this terminal signal.

Terminal signaling is irreversible. If the newest terminal command cannot be enqueued or its sender is canceled, it becomes a host Stop and still supersedes older requests. Keep that Stop pending until the worker performs cleanup and acknowledges it; producer cancellation must not allow initial module evaluation or restore an older Load. Closing the last command producer supersedes queued terminal commands so a buffered Load cannot start another session during shutdown.

Every terminal path skips `beforeStop` and does not rely on JavaScript `finally` or disposal callbacks. This changes the existing hook guarantee: Stop, self-stop, reload, replacement, exit, shutdown/channel closure, and an uncaught default-export failure all use host-owned cleanup. An existing `beforeStop` export may remain in a script but is no longer invoked. Do not reset cancellation to re-enter the old runtime for cleanup or add a separate graceful-stop mode in this feature. Scripts that need their own cleanup must perform it explicitly before requesting Stop.

On termination, reject new calls/events, make pause waiters terminal, cancel retained invocation/callback/driver futures, close screen ownership and its waiters, clear manual/ownership input locks and console lock, publish an empty UI snapshot, and drop callback roots before the context/runtime. Teardown must not await an infinite loop or hook. Session checks prevent old completions from mutating global/UI state or issuing fresh host actions. Requests already dispatched to the game may complete; stopping does not roll back their effects.

This guarantees recovery from runaway JavaScript. QuickJS interrupts cannot preempt a synchronous native host call that has not returned, such as the current blocking `rev.shell`; hard termination of arbitrary native code or subprocesses would require separate process isolation and is outside this feature. Do not report infinite-loop recovery as verified until the control-to-interrupt path and loading another script have both been exercised.

## 2. Screen ownership

`screenOwnership()` remains the existing cooperative, FIFO, non-reentrant mutex. Holding its token preserves exclusivity until release/disposal or session teardown.

| State | Ownership token and queue | Player-input lock |
| --- | --- | --- |
| Running with an owner | Retained | Enabled |
| Paused with an owner | Retained | Disabled |
| Resumed with the same owner | Retained | Enabled again |
| Session stopped | Invalidated; waiters canceled | Disabled |

An owner may call `await rev.ensureRunning()` while holding its token. Do not release/reacquire the token merely because of pause. Other callers of `screenOwnership()` continue waiting. Monitoring or UI handlers that do not request ownership continue normally; ownership remains opt-in for game-control calls.

Keep the current manual-lock rule: ordinary pause clears the manual lock, and plain resume does not restore it. A retained ownership token can still restore the ownership-driven lock. Preserve `ResumeLocked` behavior.

The reservation preserves execution and queue position, not the game's visible page or data. Place maintenance checkpoints before reading the state used to choose the next action, and re-establish the required game view through the existing `Action` navigation when execution resumes.

## 3. UI API

The additions to `scripts/rev.d.ts` are:

```typescript
type RevUiColor = readonly [number, number, number]
  | readonly [number, number, number, number];

type RevUiLength = number | Readonly<{ min?: number; max?: number }>;

type RevUiBorder = Readonly<{
  thickness?: number;
  color?: RevUiColor;
}>;

type RevUiCorner = Readonly<{
  radius?: number;
  topLeft?: number;
  topRight?: number;
  bottomLeft?: number;
  bottomRight?: number;
}>;

type RevUiPadding = Readonly<{
  thickness?: number;
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;
}>;

interface RevUiElement {
  text?: string;
  font?: string;
  size?: number;
  alignX?: "left" | "center" | "right";
  alignY?: "top" | "center" | "bottom";
  posX?: number;
  posY?: number;
  lenX?: RevUiLength;
  lenY?: RevUiLength;
  color?: RevUiColor;
  textColor?: RevUiColor;
  border?: RevUiBorder;
  corner?: RevUiCorner;
  padding?: RevUiPadding;
  onHover?: () => void | Promise<void>;
  onLeave?: () => void | Promise<void>;
  onClick?: () => void | Promise<void>;
}

// Additions to Rev; the existing Readonly<Rev> makes the registry reference readonly.
interface Rev {
  readonly paused: boolean;
  ensureRunning(): Promise<void>;
  ui: Record<string, RevUiElement | undefined>;
}
```

`border`, `corner`, and `padding` are ordinary optional fields of `RevUiElement`. The same definition supports initial creation and replacement:

```javascript
rev.ui.ele = {
  text: "Click Me",
  posX: 100,
  posY: -100,
  lenX: { min: 100 },
  lenY: 20,
  color: [40, 40, 40],
  border: { thickness: 2, color: [255, 0, 0] },
  corner: { radius: 5, topLeft: 0, bottomLeft: 0 },
  padding: { thickness: 4, left: 0, right: 8 },
};

// Update the same field on the existing element.
rev.ui.ele.border = { thickness: 1, color: [0, 255, 0] };
```

Initial definitions and later property assignments use the same field types, defaults, copying, and validation rules. Every field in `RevUiElement`, including handlers, can be supplied in the initial object. A valid creation publishes the complete styled element in its first snapshot.

### Registry and mutation

- `rev.ui.name = definition` creates or replaces one element. Replacement gets a new instance identity and keeps that key's stacking position.
- `rev.ui.name.text = value` updates an existing instance. Reads reflect the change immediately; transport/rendering follow asynchronously.
- `delete rev.ui.name` removes it and unregisters its handlers. Deleting an absent name succeeds. Missing names read as `undefined`.
- The registry is stable for the loaded session, including repeated default invocations and hooks. It is separate from process-wide JSON-only `rev.global`.
- Names are nonempty strings stored in a prototype-free dictionary; names such as `__proto__` must work as ordinary keys.
- Assignments validate before committing any change. Unknown fields (including nested border/corner/padding fields), invalid field types, non-finite coordinates, invalid lengths/colors/thickness/radii/padding, and an undefined/null whole definition throw synchronously without changing the previous instance.
- Assigning the same value to an existing element field is a no-op after validation. Compare scalar values directly, arrays/objects structurally by their requested values (ignoring object key order), and handlers by function identity. A no-op preserves instance identity, handler version, and snapshot revision, creates no new dirty state, and sends no update. It must not clear an earlier pending change. Compare requested values before layout clamping, so distinct sizes/radii that happen to render alike still retain their new settings.
- Assigning a whole element definition remains replacement with a new instance identity, even when its fields match the previous definition. The same-value rule applies to field assignments. Deleting an absent element or an already-absent property creates no dirty state.
- Definitions are copied into the managed element. Mutating the original input object later does not mutate the element.
- Compound values are copied and returned deeply read-only: replace `color`, `textColor`, `lenX`, `lenY`, `border`, `corner`, or `padding` as a whole. This includes the nested `border.color` array. Nested mutation must throw rather than silently diverging from the renderer.
- Deleting an element property resets it to its default; deleting a handler unregisters it. Arbitrary property descriptors/prototypes are unsupported and must not bypass validation.
- A captured element proxy becomes invalid after deletion or replacement. Writes through it throw `UI element no longer exists`; it never edits a later element with the same name.

Ordinary callbacks take no arguments and retain normal JavaScript closures. Initialize UI in `afterLoad()` or guard creation explicitly when using the repeating default export.

### Visual defaults and layout

These defaults complete the proposed first-version contract:

| Field | Default and interpretation |
| --- | --- |
| `text` | Empty string; plain text, rich-text markup disabled |
| `font` | Empty string uses the existing overlay font fallback; otherwise an installed system font family name |
| `size` | `14`; integer pixel size from 1 through 2,147,483,647 |
| `alignX` | `"left"`; one of `"left"`, `"center"`, `"right"` |
| `alignY` | `"center"`; one of `"top"`, `"center"`, `"bottom"` |
| `posX`, `posY` | `0`; finite viewport-pixel offsets |
| `lenX`, `lenY` | `{ min: 0 }`; automatic text size plus padding |
| Numeric length | Fixed non-negative finite pixels |
| Object length | Preferred text size plus padding clamped between `min` (default 0) and optional `max`; both finite, non-negative, and `max >= min` |
| `color` | Transparent `[0, 0, 0, 0]`; background color |
| `textColor` | White `[255, 255, 255, 255]` |
| `border` | `{ thickness: 0, color: [255, 255, 255, 255] }`; no visible border by default |
| `border.thickness` | Non-negative finite pixels, including fractional values; omitted value is 0 |
| `corner` | `{ radius: 0 }`; square corners by default |
| Corner radii | Non-negative finite pixels, including fractional values; each named corner overrides `radius`, which defaults to 0 |
| `padding` | `{ thickness: 0 }`; no padding by default |
| Padding values | Non-negative finite pixels, including fractional values; each named edge overrides `thickness`, which defaults to 0 |
| Color channels | Integer bytes `0..255`; omitted alpha is 255 |
| Text layout | Selected font and `size`, aligned by `alignX`/`alignY` within the content area after padding, no automatic wrapping; explicit newlines supported |

Font names are matched case-insensitively against Unity's installed system font names. Unavailable or unloadable names use the default font and log a warning once while that name remains requested. The requested string remains readable from the element. `font` supports initial definitions and later assignments; deleting it restores the default. Non-string values throw without changing the element. Measure and render with the same resolved font, and recalculate automatic size when the font changes. Share dynamically created fonts across elements and release them when no longer used or when the UI is cleared; never destroy the game's default font.

`size` supports initial definitions and later assignments; deleting it restores 14. Values must be integer pixels from 1 through 2,147,483,647. Measure and render with the same size, and recalculate automatic size when it changes.

Measure text with Unity's text metrics. Preferred width is text width plus left/right padding; preferred height is text height plus top/bottom padding. Apply automatic min/max constraints to those preferred sizes. Numeric lengths and min/max bounds describe the element's box including padding. Border thickness contributes nothing to fixed or automatic size, text layout, or anchoring. An empty string has zero preferred text size; padding can still give it a nonzero box. A zero-width or zero-height element renders nothing, including its border, and has no pointer hit area.

Text uses `alignX` and `alignY` in the content rectangle starting at `(left, top)`, with width `max(0, w - left - right)` and height `max(0, h - top - bottom)`. Alignment changes placement within that rectangle without changing preferred size, padding, anchors, or hit area. Both fields support initial definitions, later assignments, and deletion back to their defaults; other values throw without changing the element. Clip text to the intersection of that rectangle and the rounded element shape. If padding consumes a fixed/max-constrained dimension, the content area is empty; preserve the requested padding and box size without drawing text outside them.

### Borders, corners, and padding

Resolve each corner as its named value when supplied, otherwise `radius`, otherwise 0. Resolve each padding edge as its named value when supplied, otherwise `thickness`, otherwise 0. Explicit zero overrides either fallback. The corner field is `radius`; `radias` is an unknown field. Assigning `border`, `corner`, or `padding` replaces that whole object and omitted members use defaults. Deleting one of those element properties restores its default.

Draw `border` as a solid outline entirely outside the element box, with its inner edge touching the box boundary and no gap. It reserves no space, reduces no content area, and never moves the element when changed. Its visual bounds extend by `border.thickness` on each side and may overlap other elements or be clipped at the viewport edge. Keep the element's existing stacking order; the outline receives no separate stacking priority.

Clamp each effective corner radius to half the smaller resolved box dimension. The outline follows that contour: a rounded corner's outer radius is its effective radius plus border thickness; a zero-radius corner keeps a square join. Border thickness is not clamped to the box size. Retain requested values in the registry/snapshot and recompute effective radii after content growth or resize. Corner radii do not affect preferred size.

Border color uses the same RGB/RGBA byte rules as the other colors, with white as its omitted default. The outline ring and background occupy separate regions so their alpha is applied independently. Interaction covers the rounded element box, including its padding, even when its fill is transparent; the outline adds no hit area. Native and relayed hits use the same box/corner geometry. Clipped corners and outline-only pixels do not receive hover/click events. Unlocked misses may reach UI underneath; lock-consumed misses remain consumed under section 5.

### Positioning and stacking

Non-negative X measures from viewport left to element left; negative X measures from viewport right to element right. Non-negative Y measures from viewport top to element top; negative Y measures from viewport bottom to element bottom. All anchors use the box including padding and exclude the outline. Zero, including negative zero, uses the near edge. For viewport `(W, H)` and measured element size `(w, h)`, the top-left point is `(posX, posY)` on non-negative axes and `(W + posX - w, H + posY - h)` on negative axes. Resizing and content growth preserve those anchors.

Use a separate persistent `ScreenSpaceOverlay` canvas at sorting order `32766`, below the existing controls at `32767`. Later-created keys stack above earlier keys. Recreating a deleted key appends it. Keep the elements visible through game scene changes and pause; dispose the root with the plugin.

### Pointer and callback behavior

`onHover` fires on pointer entry, `onLeave` on exit, and `onClick` on left-button release over the same instance that received the press. They do not fire continuously every frame. Presence of any handler makes the element consume pointer hits; decorative elements allow them through. The child text never independently intercepts input.

Dispatch events in arrival order, starting each handler on the session's JavaScript runtime. Track returned promises without waiting for one handler to finish before starting another. A rejected handler is logged with the element key, handler name, and stack; it does not unload the script or disable the element.

Recheck the session, instance, and handler registration immediately before invoking a queued event. In-place visual edits preserve handlers. Changing a handler invalidates events targeting its previous registration. Deletion prevents future dispatch; an already-started handler may finish and should check identity before updating the element after an await.

All callbacks work while paused, including calls through `Action`. A callback that calls `ensureRunning()` deliberately opts into waiting. Offloading the session cancels the remaining handler work.

## 4. Transport and renderer

Keep callbacks inside QuickJS; send only normalized rendering data over the existing bridge. Batch dirty registry state into complete snapshots using a latest-value channel. Only effective registry changes mark it dirty; same-value field assignments follow section 3 and do not trigger publication. Mutation never waits for Unity. Coalesce pending updates rather than queuing one packet for every property write; sending must not starve lifecycle commands. There is no periodic resend of unchanged UI; startup/unload clears and reconnect resynchronization still publish as defined below.

Use three notification packet types with camelCase JSON fields:

```text
ScriptUiSnapshot {
  sessionId: UUID string | null,
  revision: u64,
  elements: ScriptUiElementState[]
}
ScriptUiElementState {
  id: string, instanceId: UUID string, eventsVersion: u64,
  text: string, font: string, size: i32, posX: number, posY: number,
  alignX: "left" | "center" | "right", alignY: "top" | "center" | "bottom",
  lenX: ScriptUiLengthState, lenY: ScriptUiLengthState,
  color: [u8, u8, u8, u8], textColor: [u8, u8, u8, u8],
  border: ScriptUiBorderState, corner: ScriptUiCornerState, padding: ScriptUiPaddingState,
  events: ("hover" | "leave" | "click")[]
}
ScriptUiLengthState { fixed: number | null, min: number, max: number | null }
ScriptUiBorderState { thickness: number, color: [u8, u8, u8, u8] }
ScriptUiCornerState { topLeft: number, topRight: number, bottomLeft: number, bottomRight: number }
ScriptUiPaddingState { top: number, right: number, bottom: number, left: number }
ScriptUiEvent {
  sessionId: UUID string, elementId: string, instanceId: UUID string,
  eventsVersion: u64, event: "hover" | "leave" | "click"
}
ScriptUiPointer {
  sessionId: UUID string, pressId: u64, phase: "down" | "up",
  x: i32, y: i32, width: i32, height: i32
}
```

All are independent notifications with fresh envelope UUIDs; handlers do not send a correlated response. A resulting `ScriptUiEvent` is sent independently through the connection. Keep the existing packets unchanged.

Snapshot revision increases across the client publisher's lifetime, including session replacement and clear operations. A null session ID requires an empty element list and clears the renderer. Element order is authoritative. `eventsVersion` increments only when handler registration changes, so frequent label updates do not discard valid clicks.

Normalize border defaults and RGB to RGBA, resolve the `radius` fallback into four explicit corner values, and resolve padding's `thickness` fallback into four explicit edge values before transport. The plugin computes content layout and clamps corner radii after sizing, following section 3. Border/corner/padding edits preserve instance identity and handler registration.

The publisher lives with the client lifecycle and retains the latest complete snapshot even with no script loaded. Publish an empty snapshot at startup/unload, and resend the current snapshot on each connection generation. Accept snapshots only from the current connection generation and in increasing revision order within that generation. Apply/validate a snapshot atomically; malformed data retains the last valid rendered state and logs an error.

On disconnect, retain the last visible UI but disable its raycasts/events. On reconnect, keep it disabled until a current-generation snapshot is applied. Clear old event/pointer queues. Never replay old events. Every queued notification retains its originating connection identity and is revalidated at dispatch.

Unity owns object creation, layout, reconciliation, event listeners, and disposal on its main thread. Apply snapshots from `ScoreTicker.Update()` after the bounded packet pump. Reuse objects by instance identity; ordinary text/color edits do not recreate them. Text measurement failure or unavailable font is reported and retried through the normal renderer update, without falsely treating the snapshot as successfully rendered.

## 5. Lock mode and capture

Keep the existing native pass-through for the built-in script controls. For custom UI, relay consumed locked left-button presses/releases to Unity using `ScriptUiPointer`. Unity hit-tests only the custom overlay at the supplied coordinates, using its actual current layout. It records the pressed session/instance under `pressId` and emits a click only on a matching release over that same live instance.

Do not grant native pass-through based on cached custom-element rectangles: deleted, moved, or resized widgets could otherwise expose game controls underneath. A relayed click that misses custom UI remains consumed. Retain press ownership through its matching release even if lock mode changes or the pointer leaves the game; an outside release cancels the custom press. Mark pending presses invalid on disconnect, capture, unload, replacement, or invalid viewport dimensions.

Keep the mouse hook nonblocking. Forward its events through the capture worker, preserving press order and connection/session identity. Unlocked input uses Unity's normal pointer path and is not relayed. Mouse movement continues reaching Unity for hover.

The existing `FOCUS_MESSAGE` synthesizes a click at `(1, 1)`. Do not use that click-based focus path for relayed custom input, where it could activate another custom element. Use non-click foreground activation for that path; custom dispatch must work even if Windows declines foreground activation. Preserve the separate capture workflow.

Capture takes priority over custom interaction. Keep custom UI visible but disable its raycasts/events while capturing so game-path capture sees the underlying game UI. Custom elements must never be reported as game hierarchy paths. Neither relay nor event dispatch bypasses this check.

## 6. Existing-script migration

Add explicit checkpoints to automation loops in `scripts/unity_loop.ts`: zodiac, attack, and mineral maintenance, the inner zodiac action loop, and the longer automation loops before they read state for their next action. Preserve policies, sell/sacrifice behavior, configuration, and action ordering. Retain held screen-ownership tokens across a checkpoint.

Make `Action.loopDetached()` wait at the start of each automatic repetition. Keep `Action.execute()` and ordinary Action calls available while paused; placing an unconditional checkpoint in that shared execution path would block manual UI handlers too.

Finite action sequences already in progress may finish before reaching a checkpoint. Monitoring/UI loops omit checkpoints when continued operation during pause is intended. Existing `rev.sleep` is sufficient for the original one-second label-reset example; adding browser-style timers is outside this feature.

## 7. Acceptance

- Pause prevents new default calls without canceling the current call or detached work; resume never duplicates an in-flight call.
- `paused` stays current; checkpoint waits release on resume and cannot cross a session boundary.
- Direct host controls and imported Action calls work while paused.
- A paused ownership holder keeps the same token and FIFO position while player input is unlocked, and resumes without releasing it.
- Named UI creation, replacement, field edits, deletion, validation, and stale proxies follow section 3. Initial objects accept border, corner, and padding fields using the same rules as later assignments, including copied/frozen values.
- Borders draw outside the box without affecting size, anchors, content layout, or hit area. Corner and padding overrides honor zero; padding contributes to automatic size and reduces the content area of fixed boxes. Corner radii clamp after layout, and native/relayed hit tests agree with the rounded box including padding.
- Async handlers overlap after yielding, failures are isolated, and UI remains useful while the main loop is paused.
- Stop interrupts infinite JavaScript in initialization, default calls, background jobs, hooks, and UI callbacks; control reception remains responsive and another script can load without restarting the client or game.
- Self-stop is session-wide and cannot be defeated by catching its binding error. All terminal paths skip script cleanup hooks and complete host-owned cleanup without waiting for script cooperation.
- Stop/reload clears UI, releases input locks and ownership, and prevents old continuations from issuing new work against the next session. Already-dispatched game actions are not rolled back.
- Reconnect restores the current complete UI without replaying events.
- Locked custom clicks never reach underlying game controls; capture targets game UI and built-in controls remain accessible.
- Pixel anchors, growth, clipping, scene changes, and input behavior are verified in the installed game separately from source/build tests.
