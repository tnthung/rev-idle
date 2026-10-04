# JavaScript API guide

This guide documents the JavaScript surface available to scripts: the QuickJS runtime, the host-provided `rev` and `console` globals, entry-module lifecycle hooks, and every export from `scripts/lib`.

Scripts may be JavaScript (`.js`) or TypeScript (`.ts`) ES modules. Signatures in this guide use TypeScript notation. Expected returned state fields and paths are cataloged separately in [STATE_KEYS.md](../plugin/STATE_KEYS.md) and the searchable [state graph](../plugin/STATE_GRAPH.html), using a best-effort static model.

## Script shape and scope

An entry script must export a default function. The host invokes it with no arguments, awaits its result, waits 50 real milliseconds, and invokes it again.

```javascript
export default async function() {
  console.log(await rev.state("IP"));
  await rev.sleep(500);
}
```

Returning ends only the current invocation. Module-level state survives later invocations. `rev.stop()` terminates the entire session, including pending invocations and background work. Stop also interrupts synchronous JavaScript loops.

```javascript
export default async function() {
  if (Number(await rev.state("DTP")) >= 41) {
    rev.stop();
    return;
  }
  await rev.sleep(500);
}
```

`console` exists while the entry module and its static imports are evaluated. `rev` is installed only while an entry function or lifecycle hook is running. Module initialization may define functions that refer to `rev`, but must not call `rev` directly.

The runtime supplies the standard JavaScript objects built into a full QuickJS context, including `Object`, `Array`, `Promise`, `JSON`, `Date`, `Math`, `RegExp`, collections, typed arrays, `ArrayBuffer`, `BigInt`, errors, symbols, generators, proxies, `WeakRef`, `FinalizationRegistry`, `performance`, `eval`, and `globalThis`.

It does not install browser or Node.js host APIs. In particular, scripts cannot access the DOM, `window`, `document`, `fetch`, `WebSocket`, `setTimeout`, `require`, `process`, `Buffer`, or Node built-in modules. Use `rev.sleep`, the `rev` file methods, and `rev.shell` where needed.

## Imports

Static imports, re-exports, and dynamic `import()` share the same resolver. The session has separate main and background runtimes. Main runs the default export and ordinary detached work; the background runtime runs registered daemons and UI callbacks. Static imports are replayed from the defining module's retained snapshot in each runtime, while explicit dynamic imports keep their existing fresh-source behavior.

- A specifier must start with `./` or `../` and is resolved relative to the importing module.
- `.js` is appended when the specifier has no extension.
- Package names, URLs, and absolute specifiers are rejected.
- Files are read as UTF-8. `.ts` files are transpiled to JavaScript in memory; use explicit `.ts` extensions when importing them. JavaScript and TypeScript modules can import each other. JSON modules and import attributes are not supported.
- Module identity includes a hash of the current source. A repeated dynamic import observes an edited file; unchanged source reuses the existing module instance.

TypeScript compilation does not emit JavaScript, source-map, or cache files. Generated code and source maps live for the script session, including older imported versions. Runtime stacks map back to the original TypeScript file, line, and column through `Error.prepareStackTrace`; replacing that hook overrides this behavior. Compilation reports syntax errors but does not perform type checking or read `tsconfig.json`. Use editor tooling or `tsc --noEmit` for type checking, with `isolatedModules` enabled for per-file transpilation. The runtime still provides the same host APIs listed above.

```javascript
import { States } from "./lib/states.js";

export default async function() {
  const { Action } = await import("./lib/action.js");
  console.log(await States.totalDTP());
  await Action.dismiss();
}
```

For frequently changing JSON configuration, use `JSON.parse(rev.read_file(path))` inside the invoked function.

## Lifecycle hooks

Only named exports of the entry module are discovered. Hooks take no arguments, may be synchronous or return a promise, and run with `rev` available. The host awaits each hook. A hook failure is logged and does not cancel the requested lifecycle transition.

| Export | When it runs |
| --- | --- |
| `default()` | Repeatedly while the script is running and connected. Required. |
| `afterLoad()` | Once after a new session has loaded, including a reload, before normal invocations. |
| `onConnect()` | After a connection transition to connected. |
| `onDisconnect()` | After a connection transition to disconnected. |
| `beforePause()` | Immediately before a requested pause is applied. |
| `afterResume()` | Immediately after a paused session resumes. |
| `beforeStop()` | No longer invoked. Terminal cleanup is owned by the host. |

```javascript
export function afterLoad() {
  console.log("loaded");
}

export function onConnect() {
  console.log("game connected");
}

export function onDisconnect() {
  console.error("game disconnected");
}

export function beforePause() {
  console.log("pausing");
}

export function afterResume() {
  console.log("resumed");
}

export default async function() {
  await rev.sleep(1000);
}
```

Pause prevents new default invocations and stops polling the current invocation and ordinary detached main work. Background daemons and UI callbacks continue. Connection notifications also preserve the current invocation; the existing connection rule still controls when another default call may start. There is at most one default invocation in flight.

Read `rev.paused` for the current state. Add `await rev.ensureRunning()` before each automatic loop iteration to wait during pause, especially before reading data used for the next action. Monitoring loops can omit the checkpoint. Game/window controls and ordinary `Action` calls remain available while paused. `Action.loopDetached()` checks for resume at each repetition.

`rev.pause()` and `rev.resume()` synchronously request the same transitions as the host controls. The lifecycle applies them when JavaScript yields, including the existing hooks and input-lock handling. Repeating the current request is harmless; the latest request wins before a transition is applied. `rev.paused` reflects the applied state. Daemons and UI callbacks can call `rev.resume()` while the main invocation is paused. Both methods throw `script session stopped` after termination.

`rev.stop()` works from defaults, hooks, daemons, and UI callbacks. Stop, reload, replacement, exit, and uncaught default failures cancel both runtimes and skip `beforeStop`. Host cleanup removes custom UI and releases input locks and ownership. It does not depend on JavaScript `finally`; perform script-specific cleanup explicitly before requesting Stop. Already-dispatched game actions may finish. Engine interruption cannot preempt a blocking native call such as synchronous `rev.shell` until that call returns.

## `rev`

`rev` is a shallow-frozen object. Its members cannot be replaced. Synchronous methods return `undefined` unless another result is documented.

### Complete member list

| Signature | Result | Contract |
| --- | --- | --- |
| `rev.state(...keys: string[])` | `Promise<RevValue>` | Requests fresh game state. One key unwraps that value; multiple keys return a flat object. No keys are rejected by the plugin. |
| `rev.invoke(path: string)` | `Promise<void>` | Invokes a button or checkbox at an exact Unity hierarchy path. |
| `rev.input(path: string, text: string)` | `Promise<void>` | Sets an editable Unity input field's text and fires its end-edit callback. |
| `rev.scrollIntoView(path: string)` | `Promise<void>` | Scrolls containing Unity scroll views until an exact hierarchy path is visible. |
| `rev.transfer(source: string, destination: string)` | `Promise<void>` | Dispatches drag/drop between two exact Unity slot paths. |
| `rev.slot(path: string)` | `Promise<unknown>` | Reads the contained item's data, or `null` when the slot is empty. |
| `rev.click(x: number, y: number, button = "left")` | `void` | Sends one background client-area click. |
| `rev.clickn(x: number, y: number, count: number, button = "left")` | `Promise<void>` | Sends `count` clicks with 10 ms between them. Zero is allowed. |
| `rev.scroll(x: number, y: number, length: number, axis = "vertical")` | `void` | Sends a background scroll. |
| `rev.drag(x1: number, y1: number, x2: number, y2: number)` | `void` | Sends a background drag. |
| `rev.press(key: string)` | `void` | Sends one supported key to the game. Input is case-insensitive. |
| `rev.resize(width: number, height: number)` | `void` | Sets the game client-area dimensions. |
| `rev.screenOwnership(label?: string)` | `Promise<ScreenOwnership>` | Waits for and acquires this script session's cooperative screen-input ownership. |
| `rev.mutex(channel: string)` | `Promise<MutexGuard>` | Waits for and acquires a named cooperative mutex shared by the session's main and background runtimes. |
| `rev.read_clipboard()` | `string` | Reads Windows Unicode text. |
| `rev.write_clipboard(text: string)` | `void` | Replaces Windows clipboard text. |
| `rev.read_file(path: string)` | `string \| null` | Synchronously reads UTF-8 text; returns `null` only when missing. |
| `rev.write_file(path: string, content: string)` | `void` | Synchronously creates or overwrites a file. |
| `rev.delete_file(path: string)` | `boolean` | Synchronously deletes a file; false means it did not exist. |
| `rev.shell(command: string)` | `{ stdout: string, stderr: string }` | Synchronously runs `cmd.exe /D /S /C command` and captures both streams. |
| `rev.sleep(milliseconds: number)` | `Promise<void>` | Waits using real elapsed time. |
| `rev.paused` | `boolean` | Live, read-only pause flag. |
| `rev.pause()` | `void` | Requests a pause of the main invocation and ordinary detached main work. |
| `rev.resume()` | `void` | Requests resumption of the paused session. |
| `rev.ensureRunning()` | `Promise<void>` | Resolves while running; waits while paused; rejects with `script session stopped` on termination. |
| `rev.stop()` | `void` | Terminates the entire session and interrupts JavaScript. |
| `rev.daemon(name: string, fn: RevDaemon \| null)` | `void` | Registers or replaces a background daemon; `null` retires it. Use `name in rev.daemon` for presence checks. |
| `rev.ui` | callable element registry | `rev.ui(name, attrs)` creates or patches session-owned UI; `rev.ui(name, null)` removes it. Named entries are read-only; element fields remain assignable. |
| `rev.global` | property proxy | Stores process-local values, including BigNum, shared by all script sessions. |

### Custom UI

Create or patch elements with `rev.ui(name, attrs)` in `afterLoad`. The call returns the live element, so callback setters can be chained:

```javascript
export function afterLoad() {
  rev.ui("button", {
    text: "Click Me", posX: 100, posY: -100,
    alignX: "center", alignY: "center",
    lenX: { min: 100 }, lenY: 28,
    color: [40, 40, 40],
    border: { thickness: 2, color: [255, 0, 0, 180] },
    corner: { radius: 5, topLeft: 0, bottomLeft: 0 },
    padding: { thickness: 4, left: 0, right: 8 },
    states: { clicks: 0 },
  }).setOnHover(function() { this.color = [0, 120, 0]; })
    .setOnLeave(function() { this.color = [40, 40, 40]; })
    .setOnClick(async function() {
      this.states.clicks = typeof this.states.clicks === "number" ? this.states.clicks + 1 : 1;
      this.text = "You clicked me!";
      await rev.sleep(1000);
      if (rev.ui.button === this) this.text = "Click Me";
    });
  rev.ui("label", { text: "Ready", font: "Consolas", size: 18, posX: 100, posY: -140 });
  rev.ui("signal", { posX: 80, posY: -140, lenX: 12, lenY: 12, color: [0, 255, 0] });
}

export default function() {}
```

Use `rev.ui("button", null)` to remove an element. Removal returns `undefined` and is a no-op for an absent name. Calling `rev.ui` for an existing name patches only the supplied fields, preserving its identity, callbacks, and omitted fields. Compound attributes such as `border`, `corner`, `padding`, and `states` replace the whole value when supplied. All supplied attributes are validated before any patch is applied. Named entries are read-only: assigning or deleting `rev.ui.button` throws. Element fields remain assignable, for example `rev.ui.button.text = "Ready"`. Check identity after an `await` before updating a captured element. A write through a removed proxy throws. Configure callbacks only with `setOnClick`, `setOnHover`, `setOnLeave`, and `setOnStateUpdate`; each returns the same element and accepts `null` to clear one. Callback properties are absent, and definitions cannot supply or override host methods. Equal field writes are no-ops, including structurally equal colors/styles and identical handler functions. Changes publish snapshots asynchronously, independently of the 50 ms default-call delay; unchanged UI is not periodically resent.

Colors use RGB or RGBA bytes. Lengths are fixed numbers or automatic `{ min, max }` bounds. Padding contributes to automatic size and is included in fixed sizes. Its `thickness` is the fallback for `top`, `right`, `bottom`, and `left`. Border is an external outline: it adds no size or hit area. Corner `radius` is the fallback for each named corner; explicit zero stays square. Replace compound fields as a whole, for example `rev.ui.button.border = { thickness: 1, color: [0, 255, 0] }`; nested writes throw. Text is plain and clipped inside the padded rounded box.

Use `alignX: "left" | "center" | "right"` and `alignY: "top" | "center" | "bottom"` to place text within its padded content area. Defaults are `"left"` and `"center"`. Assign either field to update an existing element, or delete it to restore the default. Alignment does not change automatic size or the element's position.

Set `font` to an installed system font family name, for example `font: "Consolas"` or `rev.ui.label.font = "Arial"`. Names are case-insensitive. Empty, omitted, or deleted `font` uses the default overlay font; unavailable fonts fall back with a warning. Set `size` to an integer pixel size from `1` through `2,147,483,647`; omitted or deleted `size` defaults to `14`. Font and size changes recalculate automatic width and height.

Use `await rev.ui.label.width()` and `await rev.ui.label.height()` to read the calculated box size in pixels, including padding and excluding border. Position reads return `[major, minor]` in pixels: `globalXPos(relativeTo?)` returns `[elementLeft - referenceLeft, elementRight - referenceRight]`, and `globalYPos(relativeTo?)` returns `[elementTop - referenceTop, elementBottom - referenceBottom]`. Supply an exact Unity hierarchy path as `relativeTo` to measure against that RectTransform's screen-space rectangle; omitted or empty uses the viewport. Minor offsets are negative when the element's right/bottom edge is inside the reference rectangle, matching negative `posX`/`posY` positioning. For example, an element 50 pixels wide starting 20 pixels from the left of a 300-pixel-wide reference returns `[20, -230]`. Each call waits for Unity to apply the changes made before the call; later changes may also be included. Reads work while paused or hidden by F6. They reject if the element is deleted before the read completes, the session stops, or the bridge disconnects or times out. Position reads also reject while `basedOn` or the supplied `relativeTo` is inaccessible; width and height remain available. These host-provided methods are required on live elements and are absent only from definition attributes; guard a possibly missing registry read or use `!` after a presence check.

Set `hidden` to `true` to hide one element without deleting it. Hidden elements keep accepting script updates and retain their calculated `width()` and `height()`, but do not receive pointer events. Other elements are not repositioned automatically; scripts can inspect `hidden` and manage dependent positions explicitly. Omitted or deleted `hidden` defaults to `false`.

Positive positions measure from the left/top; negative positions measure from the right/bottom to the element's far edge. Set `basedOn` to an exact Unity hierarchy path to mount the element to that RectTransform; its current screen-space rectangle becomes the position anchor and is recalculated each overlay update. An inaccessible mount hides the element and disables its interaction until the path resolves again. Elements with handlers receive pointer entry, exit, and matching press/release clicks. Async handlers overlap after yielding. Handler errors are logged without unloading the script. Elements without handlers allow pointer hits through.

Press F6 while the game is focused to hide or show all script-created UI. Hidden widgets keep their state and receive script updates, but do not receive pointer events. The control bar stays visible.

UI remains visible and callbacks remain usable while paused. Register a monitor as a daemon when it should keep running; ordinary detached main maintenance waits:

```javascript
rev.daemon("monitor", async function() {
  while (rev.ui.label) {
    rev.ui.label.text = rev.paused ? "Paused" : "Running";
    await rev.sleep(250);
  }
});

async function maintenance() {
  while (true) {
    await rev.ensureRunning();
    // Read fresh state and perform the next automatic action here.
    await rev.sleep(1000);
  }
}
```

Register the daemon from `afterLoad` and start maintenance with `maintenance().catch(console.error)`. Stop/reload removes all session UI. Disconnect retains visuals but disables interaction until a fresh snapshot arrives. Capture also disables custom interaction. The [original UI design](superpowers/specs/2026-09-28-script-ui-design.md) provides implementation background; this guide describes the current API. `scripts/ui_demo.ts` provides manual checks without automatic game actions.

Load [two_runtime_demo.ts](../scripts/two_runtime_demo.ts) for a focused runtime demo: Pause freezes a default invocation and its detached counter while daemon ticks and asynchronous buttons continue. Send requests while paused, then Resume to see the same invocation token handle them. The demo also exercises imported callback code, `setOnClick(null)`, and daemon replacement waiting for the current run to finish. The file's opening comments give the walkthrough; it only updates its own UI.

### Daemons, states, and transfer limits

`rev.daemon("name", fn)` registers or replaces a background function. A daemon runs once after installation and does not restart automatically when it returns or rejects. `rev.daemon("name", null)` retires the name; a running invocation may finish, and a replacement waits for its returned promise. Removing an absent name is a no-op. Assignment and deletion of daemon properties throw. Use `"name" in rev.daemon` for presence checks or `Object.keys(rev.daemon)` to list names; reading a daemon property throws. Daemons have `this === undefined`, so use `rev.global` or element `states` for communication. A daemon failure is reported with its name and does not stop the session.

Element definitions may include `states` containing JSON values and native `BigNum` instances. Live elements always expose a state map, and callback `this` is the element with that map. State values are copied through the host; nested objects need reassignment after mutation. Patching an existing name preserves its state map unless `states` is supplied, in which case the map is replaced as a whole. Deleting an element and creating it again produces a new element and state map. Use `rev.global` for other shared values. Native `BigNum` instances are reconstructed in the receiving runtime; functions, promises, other class prototypes, and original lexical closures do not cross runtimes.

`setOnStateUpdate(fn)` queues one background callback for each changed direct state assignment or deletion, including replacing or clearing the whole map and patching `states` through `rev.ui(name, attrs)`. Structurally equal writes and deleting absent keys do nothing. Call `element.update()` to queue one callback regardless of state changes; it returns the same element for chaining and does nothing when no state update handler is registered. Creating an element or registering the callback does not invoke it. Changes from either runtime are queued even while the handler is installing. Each invocation reads the current state through `this.states`; rapid changes can therefore produce several calls that see the same latest values. Callbacks may overlap after an `await`, and writing a changed state inside the callback queues another call. Clearing or replacing the handler, or removing the element, discards its queued calls; invocations already running may finish. Errors are reported without stopping the session.

```javascript
rev.ui("counter", { text: "Count: 0", states: { count: 0 } })
  .setOnStateUpdate(function() { this.text = `Count: ${this.states.count}`; });
rev.ui.counter.states.count = 1;
```

Load [state_update_demo.ts](../scripts/state_update_demo.ts) to try direct writes, deletion, whole-map replacement, equal-value writes, nested reassignment, and disabling notifications. The demo only updates its own UI; its buttons also work while paused.

Transferred daemons and callbacks replay their defining module's retained static imports and exported bindings in the background runtime. Exported bindings are imported from the same retained defining-module snapshot, while nonexported outer locals remain ordinary missing-name errors when reached. The defining module's top-level code evaluates normally in the background runtime, whose state is isolated from the main runtime. Callback installation is asynchronous and can fail naturally; the original main function is never invoked as a fallback.

### State

Paths are case-sensitive and use dot-separated properties. Numeric segments index arrays/lists. Compatibility aliases such as `IP`, `EP`, `DTP`, `dtpFree`, and `dtpSpent` are also accepted. There are no wildcard queries.

```javascript
const ep = await rev.state("EP");
const values = await rev.state("IP", "gameData.eternity.dtpSpent");
console.log(ep, values.IP, values["gameData.eternity.dtpSpent"]);
```

One key returns the selected scalar, object, array, or `null` directly. Two or more keys return a flat object keyed by the exact requested paths. Each request is a fresh WebSocket request to the plugin and rejects when disconnected, after the two-second response timeout, for an invalid path, or on serialization/protocol failure.

Nested values use [earlier-layer reference filtering](../plugin/STATE_KEYS.md#nested-object-references). For example, a dilation tree response exposes `center.level` directly, and branch upgrades omit `prev` references to that center. Explicit request paths and aliases remain valid; the state manual describes which references are omitted from broader responses.

Returned snapshots represent game values as follows:

| Game value | JavaScript value |
| --- | --- |
| BigDouble | Native `BigNum` instance, including inside objects and collections. |
| Integer outside JavaScript's safe range | Decimal string. |
| Safe integer or finite floating-point value | Number. |
| Non-finite floating-point value | `"NaN"`, `"Infinity"`, or `"-Infinity"`. |
| Boolean, string, or null | Matching JSON primitive. |
| Collection or gameplay object | Array or object. |

BigDouble values with a non-finite mantissa or a non-finite/non-integer exponent reject serialization. Ordinary numeric-looking strings remain strings.

The outer response object is shallow-frozen before one-key unwrapping. Nested objects and a one-key object value are not recursively frozen. A response is only a snapshot; mutating it never changes the game.

### Unity and window input

Coordinates are signed client-area pixels: `(0, 0)` is the top left. All coordinates and scroll lengths must be finite 32-bit integers. Width and height must be positive finite 32-bit integers. Fractional values, `NaN`, and infinities throw.

`click` and `clickn` accept `"left"`, `"right"`, or `"middle"`; omitted or `undefined` means left. The current bridge transmits coordinate clicks without the selected button, so production behavior is left-click regardless of this argument.

`scroll` accepts `"vertical"`, `"v"`, `"horizontal"`, or `"h"`. The signed length is passed directly to Unity's scroll delta.

`press` accepts one ASCII letter/digit or one of: `left`, `right`, `up`, `down`, `enter`, `escape`, `space`, `tab`, `backspace`, and `f1` through `f12`.

`invoke` rejects an empty path and asks the plugin to run a button's click handler or a checkbox's pointer-click handler. Checkboxes must be active and interactable; open the Automation tab before invoking its checkboxes. Capture recognizes checkbox paths and copies them to the clipboard. `scrollIntoView` rejects an empty path and asks the plugin to reveal the target through its containing Unity scroll views. It resolves after the plugin completes the request. `transfer` rejects empty paths and asks the plugin to execute the source drag and destination drop handlers. Successful dispatch does not guarantee the game accepted the resulting action; query state when confirmation matters.

`input` uses a captured `input` path to set a TextMeshPro or legacy Unity input field. It requires a nonempty path and an active, interactable field with `readOnly` disabled. Setting text fires the value-change callback when changed, then `onEndEdit` is invoked with the field's resulting text. Empty text is allowed. Open the Minerals page before using this for mineral spawn level:

```ts
await rev.input(capturedInputPath, String(level));
console.log(await rev.state("gameData.minerals.curMineralLevel"));
```

The promise resolves after the callbacks run. Check the state to see the value accepted by the game.

`slot` accepts the same exact hierarchy paths as `transfer`. It returns the slot's item data using the state serializer, or `null` when empty. Empty, missing, or unsupported paths reject. Slots must already be instantiated and initialized; inactive slots can be read. Like `state`, slot reads remain available while actions are paused. Narrow the `unknown` result to the item type expected by your script.

### Screen ownership

`rev.screenOwnership(label?: string)` waits in FIFO order for this session's cooperative, non-reentrant screen mutex and resolves to a `ScreenOwnership` token. The optional label appears in a tooltip when hovering the lock icon while that caller owns the screen. Use the token with JavaScript's explicit resource management so scope cleanup is automatic:

```typescript
import { Action } from "./lib/action.ts";

export default async function() {
  using so = await rev.screenOwnership("Opening main screen");
  await Action.main();
  so.rename("Infinity upgrades");
  await Action.infinity();
}
```

`so.rename(label: string)` synchronously updates the token's label while retaining ownership. It returns `void` and does nothing after release or session stop. While paused, the new label is retained and appears when ownership re-locks on resume.

While an ownership token is alive and unreleased, and the script is active rather than paused, lock mode is forced for that session. Ownership is opt-in, and nested helpers should not reacquire it. `release()` is idempotent, and `Symbol.dispose` performs the same release. Native Rust drop or garbage collection can release a forgotten token, but the timing is not guaranteed. Pausing unlocks player input while retaining the reservation; resuming re-locks it. A holder waiting at `ensureRunning()` retains its token and keeps other ownership callers waiting. Releasing a token preserves any existing manual lock. Stop or reload invalidates ownership and cancels waiters through host cleanup.

### Named mutexes

`rev.mutex(channel)` waits in FIFO order for a non-reentrant mutex named by a nonempty channel string. The registry is scoped to the script session, and the same named channel is shared by the main and background runtimes; different channels proceed independently. Use a `MutexGuard` with explicit resource management:

```typescript
export default async function() {
  using _global = await rev.mutex("global-update");
  const value = Number(rev.global.counter ?? 0);
  await rev.sleep(10);
  rev.global.counter = value + 1;
}
```

`release()` is idempotent, and `Symbol.dispose` and native Rust drop release the guard as well. Pausing retains a held guard, and stopping or reloading the session cancels queued acquisitions and releases held guards. Every cooperating read-modify-write must use the same channel name.

### Files, shell, and clipboard

File paths and shell commands use the client process's working directory, not the importing module's directory. Absolute paths are accepted.

`write_file` truncates an existing file and does not create parent directories. `delete_file` deletes files, not directories. Other read/write/delete failures throw. `shell` waits for the process to exit, decodes stdout/stderr lossily as UTF-8, and does not expose the exit code; inspect `stderr` or command output when the command must report failure.

```javascript
const result = rev.shell("where dotnet");
console.log(result.stdout);
if (result.stderr) console.error(result.stderr);
```

### `rev.global`

`rev.global` is a property proxy backed by a process-wide string-key map. Values survive stop, load, and reload, but disappear when the client exits. All scripts share the same namespace.

```javascript
rev.global.runs = (rev.global.runs ?? 0) + 1;
console.log(rev.global.runs, Object.keys(rev.global));
delete rev.global.runs;
```

Reads return a fresh value, with native `BigNum` instances reconstructed at any depth. Missing keys return `undefined`. Writes preserve JSON values and native `BigNum` instances; assigning `undefined` stores `null`. Cycles and `BigInt` cannot be stored, and other prototypes or methods are not preserved. The `in` operator, `Object.keys`, and `delete` work. Assign a nested object back after editing it.

## `console`

Only these methods are installed:

| Signature | Behavior |
| --- | --- |
| `console.log(...values)` | Writes one line to stdout. |
| `console.error(...values)` | Writes one line to stderr. |
| `console.clear()` | Clears the terminal and moves the cursor to the top left. When launched by the plugin, clears the BepInEx console. |

Arguments are separated by spaces. Strings print directly, objects use JSON serialization, errors include their name/message/available stack, and other values use string conversion. Formatting placeholders such as `%s` are not interpreted. Cyclic object formatting can throw. Methods such as `warn`, `info`, `debug`, `table`, and `time` do not exist.

## `scripts/lib/action.js`

```javascript
import { Action } from "./lib/action.js";
```

`Action` is the module's only export. An instance is both a mutable sequence and a callable function: `await action()` and `await action.execute()` run the same steps.

| Member | Contract |
| --- | --- |
| `steps` | Public mutable step array. |
| `click(x, y, delayMs = 100)` | Appends a `rev.click` step. |
| `scroll(x, y, length, axis = "vertical", delayMs = 100)` | Appends a `rev.scroll` step. |
| `drag(x1, y1, x2, y2, delayMs = 100)` | Appends a `rev.drag` step. |
| `press(key, delayMs = 100)` | Appends a `rev.press` step. |
| `invoke(path, delayMs = 100)` | Appends an awaited `rev.invoke` step. |
| `scrollIntoView(path, delayMs = 10)` | Appends an awaited `rev.scrollIntoView` step. |
| `silentInvoke(path, delayMs = 100)` | Appends an awaited `rev.invoke` step. Despite its name, it does not catch rejection. |
| `transfer(source, destination, delayMs = 100)` | Appends an awaited `rev.transfer` step. |
| `wait(ms)` | Appends an explicit `rev.sleep` step. |
| `chain(action)` | Appends the referenced action's current step objects and returns this action. |
| `clone()` | Returns a new Action with a shallow copy of `steps`. |
| `execute()` | Runs all steps sequentially and resolves `undefined`. |

After a non-wait operation, execution sleeps for the following operation's `delayMs`. There is no initial or final implicit delay. A following explicit `wait` supplies its own delay. Clones and chained actions have separate arrays but share their step objects.

### Static Action members

Every member below is reachable as `Action.member`.

| Group | Members |
| --- | --- |
| Notifications | `dismiss`; async `dismissLoop()` retries it forever and ignores invocation errors. |
| Revolution/unity reset | `gotoRevolution`, `claimIP`, `claimEP`, `openUnitOption`, `selectLeftZodiac`, `selectTopZodiac`, `selectBottomZodiac`, `selectRightZodiac`, `unit`, async `unitWith(position = "Left")`. |
| Infinity | `gotoInfinity`. |
| Eternity | `gotoEternity`, `gotoEternityChallenge`, `selectEternityChallenge1` through `selectEternityChallenge10`, `toggleEternityChallenge`. |
| Dilation | `gotoDilation`, `toggleDilation`, `gotoDilationTree`, `initDilationTreeLoadout`, `buyDilationTree`. |
| Dilation nodes | `selectDilationTreeC`, `selectDilationTreeT1` through `T4`, `selectDilationTreeM1` through `M4`, `selectDilationTreeB1` through `B4`. |
| Dilation loadout | `loadDilationLoadOut`, `applyDilationLoadOut`. |
| Unity navigation | `gotoUnity`, `gotoAstrology`, `gotoPlanet`, `gotoPlanetShop`, `gotoZodiacMerge`, `gotoZodiacEnhance`, `gotoZodiacReforge`, `gotoZodiacSacrifice`, `sortZodiacByRarity`, `gotoUnityTrial`, `resetUnity`. |
| Zodiac operations | `ZODIAC_INV_SLOT(n)`, `ZODIAC_MERGE_SLOT(n)`, async `sellZodiac(n)`, `sacrificeZodiac(n)`, `mergeZodiac(a, b, c)`, `equipZodiac(src, dest)`, `takeOffZodiac(from)`. |
| Attack | async `upgradeAttackRings()`, `buyRelic(n)`. |
| Other navigation | `gotoAutomation`, `gotoTimeFlow`. |

Static path strings are `ZODIAC_PLANET_SLOT_SUN`, `MERCURY`, `VENUS`, `MOON`, `MARS`, `JUPITER`, `SATURN`, `URANUS`, `NEPTUNE`, `PLUTO`, `CHIRON`, and `FORTUNE`, plus `ZODIAC_SELL_SLOT`, `ZODIAC_CELL_BUTTON`, `ZODIAC_MERGE_BUTTON`, `ZODIAC_MERGE_RESULT_SLOT`, `ZODIAC_SACRIFICE_SLOT`, and `ZODIAC_SACRIFICE_BUTTON`. `CELL` is the actual exported spelling.

`ZODIAC_INV_SLOT(n)` converts `n` with `Number` and does not bounds-check it. `ZODIAC_MERGE_SLOT(n)` accepts values from 0 through 2 after conversion, but does not enforce integrality. `takeOffZodiac` resolves `true` after moving to the first empty inventory slot and `false` when none is empty. Static helpers that use `this` must be called through `Action`.

The static Action instances and their paths are mutable conveniences tied to the captured Unity hierarchy. Clone before extending a preset and recapture paths after hierarchy changes.

## `scripts/lib/states.js`

```javascript
import {
  States,
  UnityZodiac,
} from "./lib/states.js";
```

`states.js` exports `States`, six data classes, and eight enum objects. `BigNum` is supplied globally by the host and requires no import.

### `States`

| Method | Result |
| --- | --- |
| `currentIP()` | `Promise<BigNum>` from `IP`. |
| `currentEP()` | `Promise<BigNum>` from `EP`. |
| `nextIP()` | `Promise<BigNum>` from `nextIP`. |
| `nextEP()` | `Promise<BigNum>` from `nextEP`. |
| `supernovaLevel()` | Raw value from `gameData.eternity.supernovaLv`. |
| `eternities()` | `Promise<BigNum>` from `gameData.eternity.eters`. |
| `totalAP()` | `Promise<BigNum>` from `gameData.eternity.APbought`. |
| `DilationMaxScore()` | `Promise<BigNum>` from `dilationMaxScoreCurrent`. |
| `inDilation()` | Raw value from `gameData.eternity.inDilation`. |
| `totalDTP()` | Raw value from `DTP`. |
| `unusedDTP()` | Raw value from `dtpFree`. |
| `spentDTP()` | Raw value from `dtpSpent`. |
| `unityZodiacInventory()` | Object whose values are `UnityZodiac` instances. |
| `planetZodiacInventory()` | Object whose values are `UnityZodiac` instances. |
| `gold()` | `Promise<BigNum>` from `gameData.attacks.gold`. |
| `nextGold()` | `Promise<BigNum>` from `gameData.attacks.goldOnUnity`. |
| `attackRelics()` | Array of `AttackRelic` instances. |
| `zodiacInventorySlotCount()` | Raw value from `gameController.inventory.SlotZodiac.CurrentValue`. |
| `currentAttackDamage()` | `Promise<BigNum>` from `gameData.attacks.totalAtkMult`. |
| `eternalChallenge(n)` | `EternalChallenge` for zero-based challenge index `n`. |
| `nextUnityZodiacs()` | Array of `UnityZodiac` instances. |
| `sacrificeState()` | Array of `ZodiacStat` instances built from the state object's entries. |
| `attackLevel()` | `AttackLevel` instance. |
| `maxAttackLevelReached()` | `Number` from `gameData.attacks.maxLevelReached`. |

All methods return promises. “Raw value” means the JSON type returned by `rev.state` is not converted by the library.

### Data classes

All constructors are public exports and accept one raw object argument.

| Class | Instance fields and getters |
| --- | --- |
| `EternalChallenge` | `challengeLevel` from `num`, `completeDiff`, `inChallenge`, `Unlocked`. |
| `UnityZodiac` | `Element`, `IsEmpty`, `RangeOffset`, `Season`, `hasPlanet`, `level`, `locked`, raw `planet`, `quality`, `rarity`, `rarityPlus`, `score`, `sign`, `stats`; getter `mergeKey`. |
| `UnityPlanet` | `bonusType`, `bonusValue`, `type`, `unlocked`. |
| `ZodiacStat` | `type`, `value`. |
| `AttackLevel` | `currentHP`, `goldGain`, numeric `level`, `maxHP`, `unlocked`. |
| `AttackRelic` | `ReqLevel`, `amount`, `baseCost`, `buyAmount`, `costInc`, `effect`, `effect_next`, numeric `num`, `regainedLevelsEst`, `sacriEffect`, `sacriLevel`, `totalCost`, `unlocked`. |

BigDouble fields in Data types use `BigNum` and arrive as native instances from the host. Enum-backed fields are converted through the corresponding enum object. `UnityZodiac.planet` remains the raw supplied value; the constructor does not create a `UnityPlanet`.

### Enum exports

Each enum is a mutable bidirectional object made by `Enum`. For example, `ZodiacSign.Aries === 0`, `ZodiacSign[0] === "Aries"`, and `ZodiacSign._Aries === "Aries"`.

| Export | Ordered names |
| --- | --- |
| `ZodiacElement` | Fire, Water, Earth, Wind. |
| `ZodiacSeason` | Spring, Summer, Autumn, Winter. |
| `ZodiacSign` | Aries, Taurus, Gemini, Cancer, Leo, Virgo, Libra, Scorpio, Sagittarius, Capricorn, Aquarius, Pisces. |
| `ZodiacRarity` | Garbage, Common, Uncommon, Rare, Epic, Legendary, Mythic, Godly, Divine, Immortal. |
| `ZodiacStatType` | MultsGain, CommonExponent, AscensionPower, PromPower, LapsSpeed, SlowdownPower, IPGain, GenExponent, MultPerBoughtGen, InfinityGain, StarBase, StardustExponent, LabMultPower, SupernovaReq, EPGain, EternityGain, DPGain, FreeLabLevels, GameSpeed, LuckAdd, Ach29Reward, DTPCost, CenterDTUEffect, ZodiacQualityMult. |
| `PlanetStatType` | SupernovaRewards. |
| `Planet` | Sun, Mercury, Venus, Moon, Mars, Jupiter, Saturn, Uranus, Neptune, Pluto, Chiron, Fortune. |
| `PlanetUpper` | SUN, MERCURY, VENUS, MOON, MARS, JUPITER, SATURN, URANUS, NEPTUNE, PLUTO, CHIRON, FORTUNE. |

## `scripts/lib/dilation_tree.js`

```javascript
import { DilationTree, DT_STAGES, DT_EXTRAS } from "./lib/dilation_tree.js";
```

`new DilationTree()` exposes mutable fields `c`, `t1`–`t4`, `m1`–`m4`, and `b1`–`b4`, initially zero.

| Member | Contract |
| --- | --- |
| `DilationTree.current()` | Reads the current game tree and returns a `DilationTree`. |
| `DilationTree.validatePoint(p)` | Throws only when `p < 0` or `p > 5`. |
| `validateChain(type, p1, p2, p3, p4)` | Validates chain name, point ranges, center, and preceding nodes. |
| `ctr(c)` | Sets center and returns this tree. |
| `top(t1, t2, t3, t4)` | Sets the top chain and returns this tree. |
| `mid(m1, m2, m3, m4)` | Sets the middle chain and returns this tree. |
| `bot(b1, b2, b3, b4)` | Sets the bottom chain and returns this tree. |
| `total` | Getter summing all 13 levels. |
| `string` | Getter producing the loadout format `C1;T1,0,0,0;M0,0,0,0;B0,0,0,0`. |
| `clone()` | Returns a separately mutable tree with the same levels. |
| `match()` | Resolves whether this tree's string equals the current game tree. |
| `apply()` | If not matching, writes the string to the clipboard and runs the two loadout Actions. |

`apply()` restores the old clipboard only after both actions succeed; restoration is not protected by `finally`. It does not initialize/open the loadout UI itself. Its console message precedes the UI work and is not success confirmation.

Static tree objects are `DTP1` through `DTP40`, `SN5`, `SN8`, `ETN13`, `SN16`, `SN18`, `SN22`, `SN35`, `SN40`, and `AP40`. There is no `DTP41`. They are shared mutable objects; clone before editing. `DTP24` references the same object as `DTP23`.

`DT_STAGES` is an ordered mutable array of `{ dtp, state, target, loadout }`:

| dtp | state | target | loadout |
| --- | --- | --- | --- |
| 5 | `States.supernovaLevel` | 80 | `SN5` |
| 8 | `States.supernovaLevel` | 105 | `SN8` |
| 13 | `States.eternities` | 1e8 | `ETN13` |
| 16 | `States.supernovaLevel` | 120 | `SN16` |
| 18 | `States.supernovaLevel` | 128 | `SN18` |
| 22 | `States.supernovaLevel` | 149 | `SN22` |
| 35 | `States.supernovaLevel` | 152 | `SN35` |
| 40 | `States.supernovaLevel` | 154 | `SN40` |

`DT_EXTRAS` is an ordered mutable array of `{ key, target, node }`. Keys are `t3`, `m2`, `b3`, `t2`, `b1`, `b2`, `t1`, `c`, and `m1`; every target is 5 and every node is its matching `Action.selectDilationTree...` action.

## `scripts/lib/utils.js`

```javascript
import {
  Enum,
  mantissa,
  exponent,
  wait_for,
  wait_for_exponent,
  print_state,
  isStringNumeric,
} from "./lib/utils.js";
```

### Functions

| Export | Contract |
| --- | --- |
| `Enum(...keys)` | Returns a mutable object mapping names to indexes, indexes to names, and `_Name` to `"Name"`. |
| `mantissa(value)` | Returns `Number` of the substring before lowercase `e`. |
| `exponent(value)` | Returns `BigInt` of the substring after lowercase `e`. |
| `wait_for(conditionFn, interval = 500, timeout = 5000)` | Sleeps first, then checks an awaited condition until true or its interval budget expires; resolves boolean. |
| `wait_for_exponent(key, target, interval = 500)` | Waits until the selected scientific string's exponent reaches `target`; discards the boolean result. |
| `print_state(key)` | Logs one selected state value as indented JSON. |
| `isStringNumeric(value)` | Async function resolving whether a string passes both `isNaN` and `parseFloat` checks. |

`wait_for` never checks immediately. It subtracts `interval` before checking and returns false when the remaining budget reaches zero. A zero or negative interval can prevent useful timeout behavior. Errors propagate.

### `BigNum`

`BigNum` is a Rust-backed QuickJS class available globally before module evaluation in both runtimes. It stores a normalized integer mantissa scaled by `10^15` and an arbitrary-size integer scientific exponent in Rust. The constructor accepts another `BigNum`, a number, a scientific/plain numeric string, or a bigint. Unsupported types and invalid strings throw. Values retain one integer mantissa digit and up to 15 fractional digits; excess digits are truncated toward zero.

| Member | Contract |
| --- | --- |
| `BigNum.ZERO` | Shared zero instance. Treat it as read-only. |
| `BigNum.ONE` | Shared one instance. Treat it as read-only. |
| `BigNum.NEGLIGIBLE_THRESHOLD` | Exponent gap above which addition/subtraction discard the smaller term; defaults to 15 per runtime. |
| `mantissa` | Numeric mantissa getter. |
| `exponent` | `BigInt` exponent getter. |
| `sign()` | Returns `-1`, `0`, or `1`. |
| `isZero` | Whether the mantissa is zero. |
| `isNeg`, `isPos` | Whether the value is negative or nonnegative, respectively. |
| `cmp(other)` | Returns `-1`, `0`, or `1`. Accepts `BigNum`, number, or bigint. |
| `lt`, `lte`, `gt`, `gte`, `eq`, `neq` | Boolean comparisons with the same operand types as `cmp`. |
| `neg()`, `abs()` | Return a new sign-flipped or absolute value. |
| `add(other)`, `sub(other)` | Return a new sum or difference. Accept the same operand types as `cmp`; exponent gaps above 15 discard the smaller term. |
| `mul(other)`, `div(other)` | Return a new product or quotient. Accept the same operand types as `cmp`; dividing by zero throws. |
| `min(other)`, `max(other)` | Return a copy of the smaller or larger value. |
| `BigNum.min(...values)`, `BigNum.max(...values)`, `BigNum.sum(...values)` | Select the minimum/maximum or sum `BigNum` values. Require at least one value. |
| `toString(manLen?)` | Returns the normalized scientific string, optionally truncating or padding the mantissa to the specified character count. |
| `toJSON()` | Returns the scientific string for ordinary JSON serialization. Host state/global/UI transport preserves the native type separately. |
| `toNumber()` | Converts the complete scientific string to a JavaScript number; large values can overflow to infinity. |
| `toInt()` | Floors the stored decimal value before converting to a JavaScript number. |
| `toBigInt()` | Floors the stored decimal value directly to a bigint. |

Mantissa arithmetic and comparisons use integer operations. Precision is bounded to 16 significant decimal digits, and division truncates excess digits. Number inputs already carry JavaScript's floating-point precision limits; use strings or bigints when the supplied digits must be preserved. The numeric `mantissa`, `toNumber()`, and `toInt()` results remain subject to JavaScript number precision; `toBigInt()` preserves the integer represented by the stored decimal value.

## Errors and cancellation

Synchronous host failures throw and asynchronous failures reject. An uncaught default-invocation error logs the failure and destroys the session through host cleanup, skipping `beforeStop`. UI callback errors are isolated unless the session has been stopped. Library methods generally allow errors to propagate unless their implementation explicitly catches them.

Pause preserves awaited work; use `rev.ensureRunning()` where a loop should wait for resume. Keep cross-invocation data at module scope or in `rev.global`.

Implementation references: [bindings](../client/src/script/bindings.rs), [session and hooks](../client/src/script/session.rs), [module loader](../client/src/script/loader.rs), [lifecycle](../client/src/script/lifecycle.rs), [state serializer](../plugin/src/StatePayload.cs), and [library modules](../scripts/lib).
