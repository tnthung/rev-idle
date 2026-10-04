# Changelog

Detailed development history of the Revolution Idle plugin, Rust client, bridge protocol, installer, and automation code originally maintained in this repository.

## Coverage

- Covers all 414 commits from the first commit on **September 1, 2026** through **October 4, 2026**.
- Entries use commit author dates in UTC+08:00 and appear newest first. Related commits are grouped by behavior; merge commits and planning updates are not counted as separate feature deliveries.
- The repository has no release tags at this snapshot, so these are development dates rather than versioned releases. Commit links identify the supporting changes.
- Automation scripts are included through September 25, when `scripts/` was removed from version control. Later local script changes are outside this history.
- Dated entries record committed implementation and test coverage, not a claim that every historical change was verified in a running game. See the [plugin guide](plugin/README.md) and [JavaScript API guide](docs/js_api_guide.md) for usage.

## Major transitions

| Date | Change |
| --- | --- |
| September 1 | Initial score telemetry plugin, Rust listener, JavaScript runtime, and Windows input automation. |
| September 2 | On-demand HTTP state requests replaced periodic UDP telemetry. |
| September 3 | Broader game-state traversal and generated state references. |
| September 9 | A bidirectional WebSocket bridge replaced HTTP. |
| September 10–13 | In-game script controls, input locking, recent scripts, and automatic client startup. |
| September 16–17 | Native TypeScript entry support and migration of the automation library. |
| September 25 | Scoped screen ownership; automation scripts stopped being tracked here. |
| September 29 | Script-created UI, cooperative pause, and interruptible Stop. |
| September 30 | Separate main/background runtimes, script pause/resume, and callable UI updates. |
| October 1 | Callable daemon registration, removal through null arguments for daemon and UI entries, and named mutexes shared across runtimes. |
| October 2 | Asynchronous UI state-change callbacks and generic UI state declarations. |
| October 3 | Renamable screen-ownership labels, relative UI edge measurements, and same-file exports for transferred callbacks. |
| October 4 | Native BigNum values throughout the script API, callable BigNum construction, and explicit UI state-update callbacks. |

## 2026-10-04

### Native BigNum values

- Added Rust-backed `BigNum` globally in both QuickJS runtimes, available before module evaluation without an import. Arithmetic uses an integer mantissa with 16 significant decimal digits and an arbitrary-size integer exponent; excess precision is truncated toward zero. Added arithmetic, comparisons, conversions, shared `ZERO`/`ONE` constants, and a configurable per-runtime `NEGLIGIBLE_THRESHOLD`. Updated the numeric API documentation and added native regression coverage. ([4f18147](https://github.com/tnthung/rev-idle/commit/4f18147), [9dbf4e8](https://github.com/tnthung/rev-idle/commit/9dbf4e8))
- Changed `rev.state()` and `rev.slot()` to return native `BigNum` instances for game `BigDouble` values, including nested fields and collection entries. Bridge responses carry explicit type metadata so ordinary numeric-looking strings remain strings. Shared globals and UI state reconstruct native instances in the receiving runtime; ordinary JSON serialization still produces scientific strings. Replaced `RevJsonValue` with `RevValue` and added bridge, serialization, and cross-runtime regression coverage. ([4f18147](https://github.com/tnthung/rev-idle/commit/4f18147))
- Added TypeScript call signatures and regression coverage for both `BigNum(value)` and `new BigNum(value)`, including `.map(BigNum)`. Omitting the value or passing `undefined` now constructs zero; extra callback arguments are ignored. ([9af40ba](https://github.com/tnthung/rev-idle/commit/9af40ba))

### State reference viewer

- Changed **Copy struct** to emit `BigNum` for `BigDouble` fields and collection elements, replacing the previous `string` type and `// BigDouble` annotation. Regenerated the state reference and updated clipboard regression coverage. ([4f18147](https://github.com/tnthung/rev-idle/commit/4f18147))

### UI state callbacks

- Added chainable `element.update()` to queue a background state-update callback even when the state has not changed. It returns the same element and does nothing when no handler is registered. Updated declarations, usage documentation, and regression coverage. ([949a660](https://github.com/tnthung/rev-idle/commit/949a660))

## 2026-10-03

### Script execution

- Added automatic imports of same-file exported bindings for transferred daemons and UI callbacks, preserving original local names and export aliases alongside retained static imports. Imports use the defining module's retained source snapshot, including after source edits. The defining module evaluates in the isolated background runtime; nonexported outer locals remain unavailable. Added regression coverage for exported functions, classes, enums, aliases, nested transfer, and daemon/UI callbacks registered from another module, with accompanying behavior documentation. ([e8184e3](https://github.com/tnthung/rev-idle/commit/e8184e3), [9a6d6e3](https://github.com/tnthung/rev-idle/commit/9a6d6e3))

### UI positioning

- Changed `globalXPos(relativeTo?)` and `globalYPos(relativeTo?)` to return `[major, minor]` offsets for both element edges, relative to an optional Unity hierarchy path. Omitting the path or passing an empty string uses the viewport. Inaccessible anchors or reference targets reject position requests while width/height remain readable. Updated bridge packets, declarations, fixtures, and client/plugin regression coverage. ([0910c53](https://github.com/tnthung/rev-idle/commit/0910c53))

### Screen ownership

- Added synchronous `ScreenOwnership.rename(label)` to update a live token's label without releasing ownership. Renaming a released token or a stopped session is a no-op; labels are retained through pause and shown when ownership locks again. Added declarations, usage documentation, and regression coverage for ownership retention, pause/resume, and stale tokens. ([b7c9b51](https://github.com/tnthung/rev-idle/commit/b7c9b51))

## 2026-10-02

### UI state callbacks

- Added chainable `setOnStateUpdate(callback | null)` for asynchronous background notifications after changed direct state-key writes/deletes or whole-map replacement, including `rev.ui` state patches. Equal writes, deleting absent keys, element creation, and callback registration do not invoke it. Nested mutations require reassignment. ([f762a76](https://github.com/tnthung/rev-idle/commit/f762a76))
- Callbacks read the current state through `this.states`, continue while paused, and may overlap after an `await`. Clearing/replacing a handler or removing its element discards queued calls while running invocations may finish. Added regression coverage for shallow changes from both runtimes, handler lifecycle, paused execution, and isolated errors. ([f762a76](https://github.com/tnthung/rev-idle/commit/f762a76))

### Script declarations

- Refined `RevUiBaseStates`, `RevUiElementAttr`, `RevUiElement`, and `RevUiCallback` generics so attributes, elements, callback receivers, and chainable setters preserve the same state type. Stateful attributes require complete state maps, and `RevWithUi` provides a module-local typed UI registry. ([cfaf06a](https://github.com/tnthung/rev-idle/commit/cfaf06a))

## 2026-10-01

### Script synchronization

- Added Rust-backed `rev.mutex(channel): Promise<MutexGuard>` for cooperative exclusive access across a session's main and background runtimes, including updates to `rev.global`. Matching channel names share a FIFO, non-reentrant lock; different channels proceed independently without affecting screen locking. ([87089d5](https://github.com/tnthung/rev-idle/commit/87089d5))
- Guards support scoped `using` disposal and idempotent `release()`. Pause retains held locks; Stop/reload cancels queued acquisitions and cleans up guards. Added TypeScript declarations, usage documentation, and regression coverage for contention across runtimes, independent channels, disposal after exceptions, and cancellation of waiters. ([87089d5](https://github.com/tnthung/rev-idle/commit/87089d5))

### Daemon and UI APIs

- Changed daemon registration and replacement to `rev.daemon(name, fn)` and retirement to `rev.daemon(name, null)`. Assignment and deletion of daemon properties now throw. Removing an absent name is a no-op; active invocations may finish, and replacements still wait for the current invocation to settle. ([293db0c](https://github.com/tnthung/rev-idle/commit/293db0c))
- Replaced `delete rev.ui.name` with `rev.ui(name, null)`. Named UI entries are read-only while their fields remain assignable. Removal preserves callback cleanup and stale-element rejection. Updated API documentation and regression coverage for the callable registry behavior. ([293db0c](https://github.com/tnthung/rev-idle/commit/293db0c))

### Rust client

- Suppressed the Rust `unused` lint group at the crate level with `#![allow(unused)]`. ([293db0c](https://github.com/tnthung/rev-idle/commit/293db0c))

### Script declarations

- Added the shared script API declarations to the tracked repository at `protocol/rev.d.ts`, covering `rev`, disposable screen ownership, custom UI, daemons, and console methods. ([34fc4fa](https://github.com/tnthung/rev-idle/commit/34fc4fa))
- Made `RevUiElement<S>` accept a JSON-compatible type for its `states` property, with `Record<string, RevJsonValue>` as the default. ([faacc66](https://github.com/tnthung/rev-idle/commit/faacc66))

### State reference viewer

- Changed **Copy struct** to generate `export type Name = { ... };` declarations. Updated the tooltip, regenerated the manual, and adjusted the clipboard regression test to expect type aliases. ([241d153](https://github.com/tnthung/rev-idle/commit/241d153))
- Added `// BigDouble` annotations to copied fields whose source type contains `BigDouble`, including collections. BigDouble values still map to JSON strings; regenerated the viewer and updated clipboard regression coverage. ([f112d2c](https://github.com/tnthung/rev-idle/commit/f112d2c))

## 2026-09-30

### Script execution and lifecycle

- Split each script session into main and background QuickJS runtimes. The default export and ordinary detached work run in main; registered daemons and UI callbacks run in background. Pausing now suspends polling of the current main invocation and resumes that same invocation later, while background work continues. This superseded the September 29 behavior in which in-flight main work continued during pause. ([cde0f56](https://github.com/tnthung/rev-idle/commit/cde0f56))
- Added named background jobs through `rev.daemon.name = fn`. A daemon runs once, may keep itself alive asynchronously, and reports failures without stopping the session. Deleting a name retires it immediately; replacing a running daemon waits for its current invocation to finish. ([cde0f56](https://github.com/tnthung/rev-idle/commit/cde0f56))
- Added function transfer for daemons and UI callbacks, including replay of the defining module's retained static imports. Original lexical closures do not cross runtimes; JSON communication uses `rev.global` and element `states`. Stop and reload cancel both runtimes. ([cde0f56](https://github.com/tnthung/rev-idle/commit/cde0f56))
- Captured `rev.sleep()` deadlines when JavaScript calls the method, so time spent paused counts toward an already-started wait. Added migration, transfer, UI, and worker regression coverage for the two-runtime behavior. ([cde0f56](https://github.com/tnthung/rev-idle/commit/cde0f56))
- Added `rev.pause()` and `rev.resume()`. They request the same lifecycle transitions as host controls, apply when JavaScript yields, and preserve lifecycle hooks and lock handling. Background callbacks can resume paused main work; repeated requests are harmless and the latest pending request wins. ([31fec1f](https://github.com/tnthung/rev-idle/commit/31fec1f))

### UI and screen ownership

- Moved shared UI state into Rust so both runtimes can access the same elements. Added JSON-only element `states`, bound callback `this` to the element, and changed callback registration to `setOnClick`, `setOnHover`, and `setOnLeave`, with `null` clearing a handler. ([cde0f56](https://github.com/tnthung/rev-idle/commit/cde0f56))
- Changed creation and updates to `rev.ui(name, attributes)`. Updating an existing name preserves identity, callbacks, and omitted fields, and validates every supplied attribute before applying the patch. Compound fields, including `states`, replace their whole value when supplied. Whole-definition assignment through `rev.ui.name = {...}` now throws. Callback setters return the element for chaining. ([fe0f5de](https://github.com/tnthung/rev-idle/commit/fe0f5de))
- Added an optional label to `rev.screenOwnership(label)`, carried as `lockLabel` in `StateUpdate`. The overlay displays the current owner's label in the lock tooltip, with plain-text rendering and size adjustment for longer labels. ([cfb3505](https://github.com/tnthung/rev-idle/commit/cfb3505))

## 2026-09-29

### Custom script UI

- Added session-owned labels, buttons, and signals through `rev.ui`, rendered on a persistent Unity canvas below the built-in controls. Supported text, positive/negative pixel anchors, fixed or automatic dimensions, RGB/RGBA colors, external borders, per-corner radii, and padding. The initial API used property assignment; the September 30 callable API superseded it. ([e790c1e](https://github.com/tnthung/rev-idle/commit/e790c1e))
- Added asynchronous click, hover, and leave callbacks, with callback errors isolated from the main script. UI changes publish snapshots after mutations and on reconnect; unchanged elements do not require periodic resending. Stop/reload removes session UI, while disconnect keeps visuals and disables interaction. ([e790c1e](https://github.com/tnthung/rev-idle/commit/e790c1e))
- Routed clicks received during lock mode to custom UI while preserving capture priority. Custom clicks are consumed rather than replayed onto game controls, and capture disables custom UI raycasts so underlying game targets remain selectable. ([e790c1e](https://github.com/tnthung/rev-idle/commit/e790c1e))
- Added horizontal and vertical text alignment using `alignX` and `alignY`, defaulting to left/center. Corrected automatic text height through line-spacing handling. ([7997288](https://github.com/tnthung/rev-idle/commit/7997288))
- Added installed system-font selection with fallback and shared font caching. Included glyph bounds in sizing and placement to prevent clipped text. ([b96fb4d](https://github.com/tnthung/rev-idle/commit/b96fb4d))
- Added F6 to hide/show custom UI while the game is focused. Hidden widgets retain state and continue receiving updates; the control bar stays visible. ([93bcda2](https://github.com/tnthung/rev-idle/commit/93bcda2))
- Added asynchronous `width()` and `height()` measurements that wait for Unity to apply preceding changes and return calculated pixel dimensions. ([e391bb6](https://github.com/tnthung/rev-idle/commit/e391bb6))
- Added per-element `hidden` visibility. Hidden elements retain updates and measurements but stop receiving pointer events. ([d52f692](https://github.com/tnthung/rev-idle/commit/d52f692))
- Added `basedOn` anchoring to an exact Unity `RectTransform` path, plus `globalXPos()` and `globalYPos()` measurements. Anchors follow the target's current screen rectangle; an inaccessible target hides the attached element until it becomes available again. ([e9f4e34](https://github.com/tnthung/rev-idle/commit/e9f4e34))
- Extended Ctrl+click capture to return `rectTransformPath` alongside any recognized interactive target. When raycasts do not expose a suitable target, capture can use the top visible active `Graphic`, including graphics with raycasting disabled. The captured path can anchor custom UI. ([90b276c](https://github.com/tnthung/rev-idle/commit/90b276c))
- Added the `size` property for font size, defaulting to 14 pixels when omitted. Size changes update text rendering and automatic dimensions, and older packets without the field keep the default. ([cf03595](https://github.com/tnthung/rev-idle/commit/cf03595))

### Pause and Stop behavior

- Introduced cooperative pause with `rev.paused` and `rev.ensureRunning()`. At this stage, pause prevented new default calls while preserving in-flight work and callbacks; automatic loops could wait explicitly at checkpoints. Game/window actions became available while paused. Main-runtime suspension replaced this behavior on September 30. ([e790c1e](https://github.com/tnthung/rev-idle/commit/e790c1e))
- Moved script execution to a separate worker thread and made Stop interrupt runaway JavaScript. `rev.stop()` became effective from hooks and callbacks as well as the default export. Terminal cleanup became host-owned, removing UI and releasing locks/ownership without invoking `beforeStop`; blocking native calls remain interruptible only after they return. ([e790c1e](https://github.com/tnthung/rev-idle/commit/e790c1e))

## 2026-09-27

- Added input fields to UI path capture and introduced `rev.input(path, text)` across the client, protocol, and plugin. TextMeshPro and legacy Unity fields receive updated text and `onEndEdit`; changed text also invokes `onValueChanged`. Fields must be active, interactable, and editable. ([7243d97](https://github.com/tnthung/rev-idle/commit/7243d97))
- Added a state-viewer button to copy TypeScript enum declarations, including explicit numeric variant values. ([e398989](https://github.com/tnthung/rev-idle/commit/e398989))
- Added a state-viewer button to copy TypeScript interfaces for object fields and regression coverage for the generated declarations. Updated both the viewer template and generated HTML. ([b9cb086](https://github.com/tnthung/rev-idle/commit/b9cb086))

## 2026-09-26

- Added `rev.scrollIntoView(path)` across the Rust binding, WebSocket packets, and Unity dispatcher. It adjusts active containing scroll views from inner to outer, honors enabled axes and content bounds, and supports inactive buffered children without clicking the target. ([f40a8ca](https://github.com/tnthung/rev-idle/commit/f40a8ca))
- Enabled Capture before any script has been loaded, updating both the client lifecycle and overlay's unloaded-state controls. ([f40a8ca](https://github.com/tnthung/rev-idle/commit/f40a8ca))
- Added a temporary workaround to keep offscreen relic buy buttons active, then removed it in favor of scroll-to-visible behavior and the game's own activation logic later the same day. ([7b0aaf5](https://github.com/tnthung/rev-idle/commit/7b0aaf5), [f40a8ca](https://github.com/tnthung/rev-idle/commit/f40a8ca))

## 2026-09-25

### Screen ownership and state inspection

- Added `rev.screenOwnership()` as a FIFO, non-reentrant screen mutex for cooperating script tasks. The returned token supports explicit `release()` and scoped disposal. Ownership forces input locking while active; pause temporarily unlocks player input while retaining the reservation, and Stop/reload invalidates ownership and cancels waiters. ([a3db572](https://github.com/tnthung/rev-idle/commit/a3db572))
- Extended state encoding to report which requested path failed resolution or serialization through a `failedPath` output. ([e4c82fa](https://github.com/tnthung/rev-idle/commit/e4c82fa))
- Reworked nested serialization to filter earlier-layer references while retaining direct children. This prevents back-references from consuming fields that should appear directly in the response. Updated Dilation Tree reads to use `center.level` rather than reaching the center through a branch's `prev`. ([6dea1a1](https://github.com/tnthung/rev-idle/commit/6dea1a1))
- Updated generated state references to describe expected returned fields and enum variants. Added clickable enum types, Back navigation, and a default Fields tab to the viewer, along with serialization benchmarks and generator/viewer regression tests. ([6dea1a1](https://github.com/tnthung/rev-idle/commit/6dea1a1))

### Automation and repository scope

- Increased Unity reset timing, caught reset failures, and emitted the prepared reward summary only after the reset action succeeded. ([6ea6f4f](https://github.com/tnthung/rev-idle/commit/6ea6f4f))
- Split Dilation Tree loadout confirmation into its own action and started a detached confirmation loop. Added a delay after transferring a zodiac from a planet to inventory. ([cf9df91](https://github.com/tnthung/rev-idle/commit/cf9df91), [b81dd8f](https://github.com/tnthung/rev-idle/commit/b81dd8f))
- Changed attack maintenance to interleave ring upgrades with individual relic purchases, and restored parent navigation for attack actions. ([c26d9f4](https://github.com/tnthung/rev-idle/commit/c26d9f4))
- Removed `scripts/` from the tracked repository and added it to `.gitignore`. Script and helper-library changes before this commit remain part of the repository's history; subsequent local script development is not represented by this changelog. ([d1a00ef](https://github.com/tnthung/rev-idle/commit/d1a00ef))

## 2026-09-24

### Automation and helpers

- Cleared queued zodiac actions when their target became unavailable and reported that condition as an error, preventing stale work from remaining queued. ([270758e](https://github.com/tnthung/rev-idle/commit/270758e), [31c31d6](https://github.com/tnthung/rev-idle/commit/31c31d6))
- Renamed the gold accessor to `currentGold()` and expanded state helpers for current currencies, reset counts, and attack gold. Returned unlocked achievements as a `Set` of unique IDs and taught the debugging serializer to handle sets. ([6d6e2fe](https://github.com/tnthung/rev-idle/commit/6d6e2fe), [ff111e2](https://github.com/tnthung/rev-idle/commit/ff111e2), [4615e26](https://github.com/tnthung/rev-idle/commit/4615e26))
- Exported `planLoadout` and introduced `setup3.ts` for attack achievements 230–233, with farming/push phases, elemental loadouts, reserved zodiacs, retries, and achievement-aware maintenance. Follow-up work refined setup2/setup3 loadout planning and attack sampling, with accompanying analysis and implementation notes. ([305b0f9](https://github.com/tnthung/rev-idle/commit/305b0f9), [a71a0e8](https://github.com/tnthung/rev-idle/commit/a71a0e8))
- Tightened the eternity bootstrap threshold, added upgrade and DTP purchase actions, and adjusted ring/relic purchase pacing and spending limits. ([6493d8c](https://github.com/tnthung/rev-idle/commit/6493d8c), [46c2d21](https://github.com/tnthung/rev-idle/commit/46c2d21), [738d815](https://github.com/tnthung/rev-idle/commit/738d815), [bc7cf99](https://github.com/tnthung/rev-idle/commit/bc7cf99), [967aff1](https://github.com/tnthung/rev-idle/commit/967aff1))
- Moved attack maintenance into a continuously running detached loop started by `afterLoad`, with revised logging. This was script-level background work; the host `rev.daemon` API arrived on September 30. ([fb70961](https://github.com/tnthung/rev-idle/commit/fb70961), [6d4f87a](https://github.com/tnthung/rev-idle/commit/6d4f87a))
- Corrected eternal-challenge loop completion handling. ([dac7743](https://github.com/tnthung/rev-idle/commit/dac7743))

### Installation

- Integrated state-reference generation into the installer workflow and added game-directory validation coverage. Required `assembly-hash.txt` when determining whether BepInEx interop generation was ready. ([90c0049](https://github.com/tnthung/rev-idle/commit/90c0049), [747703e](https://github.com/tnthung/rev-idle/commit/747703e))

## 2026-09-23

- Started zodiac maintenance in a detached background loop after config loading, added pacing, and reworked loadout planning and phase management. Increased swap delays to allow equipment changes to settle. ([54114aa](https://github.com/tnthung/rev-idle/commit/54114aa), [b796066](https://github.com/tnthung/rev-idle/commit/b796066), [c65e1fa](https://github.com/tnthung/rev-idle/commit/c65e1fa), [7f60e3c](https://github.com/tnthung/rev-idle/commit/7f60e3c))
- Unified Unity level-cap logic, adjusted minimum zodiac quality, added `States.unities()`, and preserved the `canSkip` flag across reloads. ([f7bf454](https://github.com/tnthung/rev-idle/commit/f7bf454), [c662a2c](https://github.com/tnthung/rev-idle/commit/c662a2c), [db36f6c](https://github.com/tnthung/rev-idle/commit/db36f6c))
- Corrected relic affordability checks to consider total cost, increased the purchase cooldown, and added insufficient-gold diagnostics. ([d9c1c2b](https://github.com/tnthung/rev-idle/commit/d9c1c2b), [43f7d09](https://github.com/tnthung/rev-idle/commit/43f7d09), [62e526c](https://github.com/tnthung/rev-idle/commit/62e526c))
- Exited dilation before bootstrapping eternity and made the attack polling interval explicit. Corrected elapsed-time measurement and improved attack-level, zodiac-level, and stat output. ([8507041](https://github.com/tnthung/rev-idle/commit/8507041), [2112a60](https://github.com/tnthung/rev-idle/commit/2112a60), [22ae8fe](https://github.com/tnthung/rev-idle/commit/22ae8fe), [65bde13](https://github.com/tnthung/rev-idle/commit/65bde13))
- Added `States.attackRevolutionMults()`, then replaced broad revolution-object serialization with reads of scalar leaves. Gated zodiac phase changes on completed attack revolutions and adjusted attack ETA caps. ([96eb098](https://github.com/tnthung/rev-idle/commit/96eb098), [31b2a7e](https://github.com/tnthung/rev-idle/commit/31b2a7e), [a665a8d](https://github.com/tnthung/rev-idle/commit/a665a8d), [3920433](https://github.com/tnthung/rev-idle/commit/3920433))
- Wrapped existing BepInEx log listeners to suppress the repeated inactive-Dilation coroutine message. Added script debugging helpers through `dbg` and an `Object` extension. ([9856ea5](https://github.com/tnthung/rev-idle/commit/9856ea5), [0b49d3d](https://github.com/tnthung/rev-idle/commit/0b49d3d), [cdec6a7](https://github.com/tnthung/rev-idle/commit/cdec6a7))

## 2026-09-22

### Unity interaction

- Added `rev.slot(path)` to read an initialized slot's item through the state serializer, returning `null` when empty. Implemented matching Rust/C# request-response packets, shared fixtures, bindings, and tests. ([ddca4f4](https://github.com/tnthung/rev-idle/commit/ddca4f4))
- Extended capture and `rev.invoke()` to Unity checkboxes, including the game's `BundleToggle`. Invocation uses the pointer-click handler and requires an active, interactable toggle. ([ddca4f4](https://github.com/tnthung/rev-idle/commit/ddca4f4))

### Automation and numeric helpers

- Introduced `setup2.ts` and switched the Unity loop to load it. Revised Unity-level reset decisions, reward selection, Water-element preference, and loadout/equipping behavior. ([d6f80f9](https://github.com/tnthung/rev-idle/commit/d6f80f9), [f19d45b](https://github.com/tnthung/rev-idle/commit/f19d45b), [1e1d37c](https://github.com/tnthung/rev-idle/commit/1e1d37c), [14c559c](https://github.com/tnthung/rev-idle/commit/14c559c), [e197d41](https://github.com/tnthung/rev-idle/commit/e197d41))
- Added `UnityZodiac.statMap` for stat lookup and `UnityDirection` for reward directions. Added diagnostics for missing enum values and more detailed zodiac/Unity action logs. ([fe9d279](https://github.com/tnthung/rev-idle/commit/fe9d279), [97cf252](https://github.com/tnthung/rev-idle/commit/97cf252), [673f6e8](https://github.com/tnthung/rev-idle/commit/673f6e8), [eca7afe](https://github.com/tnthung/rev-idle/commit/eca7afe))
- Reworked relic purchasing and added selling of the lowest-scoring zodiac when inventory was too full. ([f0a346e](https://github.com/tnthung/rev-idle/commit/f0a346e), [1c36054](https://github.com/tnthung/rev-idle/commit/1c36054), [d662995](https://github.com/tnthung/rev-idle/commit/d662995))
- Changed merge, enhance, and reforge actions to close their dialogs and use clearer success checks. Corrected eternal-challenge completion bookkeeping. ([65ea80d](https://github.com/tnthung/rev-idle/commit/65ea80d), [3849027](https://github.com/tnthung/rev-idle/commit/3849027))
- Added configurable mantissa length to `BigNum.toString()`, support for BigInt in the script serializer, and consistent comparison return values. ([da53983](https://github.com/tnthung/rev-idle/commit/da53983), [f5399a0](https://github.com/tnthung/rev-idle/commit/f5399a0), [259eca9](https://github.com/tnthung/rev-idle/commit/259eca9))

## 2026-09-21

- Moved the Unity loop's configuration from `unity_loop_config.ts` into `setup1.ts`. Made configuration properties optional and added error handling around reward-choice and relic-purchase callbacks. ([88c2a0b](https://github.com/tnthung/rev-idle/commit/88c2a0b), [85e6975](https://github.com/tnthung/rev-idle/commit/85e6975))
- Added `BigNum.lt`, `lte`, `gt`, `gte`, `eq`, and `neq`. Updated `nextUnityZodiacs()` and added `UnityZodiac.hasStat()`. ([871cdc4](https://github.com/tnthung/rev-idle/commit/871cdc4), [f9a6269](https://github.com/tnthung/rev-idle/commit/f9a6269))
- Added a TypeScript test script for local automation work. ([86417a8](https://github.com/tnthung/rev-idle/commit/86417a8))

## 2026-09-19

- Updated the `Object.map` helper to handle null callback results. ([98234e1](https://github.com/tnthung/rev-idle/commit/98234e1))

## 2026-09-18

- Renamed the reward-choice configuration field from `unitWith` to `uniteWith`. ([a834dcc](https://github.com/tnthung/rev-idle/commit/a834dcc))
- Added `BigNum.toNumber()` for explicit conversion to a JavaScript number. ([6904578](https://github.com/tnthung/rev-idle/commit/6904578))
- Fixed zodiac inventory-slot paths to account for the different planet and shop views. ([0464eb2](https://github.com/tnthung/rev-idle/commit/0464eb2))

## 2026-09-17

- Ported the Unity automation loop and the action, state, Dilation Tree, and utility libraries to TypeScript. Removed the older JavaScript loop, helper, and configuration implementations after migration. ([4656c1a](https://github.com/tnthung/rev-idle/commit/4656c1a), [6e464c9](https://github.com/tnthung/rev-idle/commit/6e464c9))
- Added a `shouldReset` configuration hook to escape stalled progression. Moved config loading out of global initialization and renamed `shouldUnit` to `shouldUnite`. ([29bc4eb](https://github.com/tnthung/rev-idle/commit/29bc4eb), [e88c63e](https://github.com/tnthung/rev-idle/commit/e88c63e), [deab195](https://github.com/tnthung/rev-idle/commit/deab195))
- Improved `console.clear()` for the BepInEx console and redirected/non-terminal output, with matching client/plugin changes. ([6c77b0e](https://github.com/tnthung/rev-idle/commit/6c77b0e))
- Corrected dilation toggle placement and the `DilationMaxScore` state key, and added a message when the Dilation Tree was fully grown. ([d1ace39](https://github.com/tnthung/rev-idle/commit/d1ace39), [3e1de5d](https://github.com/tnthung/rev-idle/commit/3e1de5d), [85054f6](https://github.com/tnthung/rev-idle/commit/85054f6))
- Made zodiac enhance/reforge helpers return success so higher-level actions could react to the outcome. Improved state-dump serialization and error handling. ([d3f7abd](https://github.com/tnthung/rev-idle/commit/d3f7abd), [a19b81b](https://github.com/tnthung/rev-idle/commit/a19b81b))

## 2026-09-16

- Added direct `.ts` script execution with in-memory transpilation, source-mapped errors, and versioned module caching. Extended script selection and completion to TypeScript, and added `rev.d.ts` and `tsconfig.json` for editor support. ([1e44e0e](https://github.com/tnthung/rev-idle/commit/1e44e0e))
- Reorganized the `Rev` interface and console declarations. ([4cd308d](https://github.com/tnthung/rev-idle/commit/4cd308d))
- Corrected the Windows `ShowWindow` visibility parameter. ([9186344](https://github.com/tnthung/rev-idle/commit/9186344))

## 2026-09-13

- Added a script picker and removed missing or failed scripts from recent history. Added an explicit `RemoveFromHistory` command. ([3d63a68](https://github.com/tnthung/rev-idle/commit/3d63a68), [c5bd9a2](https://github.com/tnthung/rev-idle/commit/c5bd9a2))
- Integrated client installation and startup with the plugin. The installer copies `client.exe`, and the plugin launches it using the configured bridge port and `--non-interactable`, forwarding stdout/stderr to BepInEx logs. Added support for clipboard operations in that mode. ([46b07ba](https://github.com/tnthung/rev-idle/commit/46b07ba))
- Fixed sacrifice-state value extraction and improved errors for unsupported `BigNum` input types. ([b259473](https://github.com/tnthung/rev-idle/commit/b259473), [faaf1b4](https://github.com/tnthung/rev-idle/commit/faaf1b4))

## 2026-09-12

- Added connection and script lifecycle hooks, then made their ordering and behavior safe across reconnects. Hooks gained access to `rev`, and consumed mask/capture clicks could bring the game to the foreground. ([4d8a429](https://github.com/tnthung/rev-idle/commit/4d8a429), [c3581d0](https://github.com/tnthung/rev-idle/commit/c3581d0))
- Added persistent recent-script history and an in-game menu for launching recent entries with locked controls. ([b4aad2d](https://github.com/tnthung/rev-idle/commit/b4aad2d))
- Added attack-ring upgrades and relic purchasing to eternity bootstrapping. ([4dbcd27](https://github.com/tnthung/rev-idle/commit/4dbcd27))
- Added the JavaScript API guide covering script execution and host APIs. ([8f93145](https://github.com/tnthung/rev-idle/commit/8f93145))

## 2026-09-11

- Reworked state helpers around a dedicated `BigNum` class and migrated dependent automation calculations. Improved zodiac selection, Aries equipment handling, and elapsed-time output in damage calculations. ([315e96a](https://github.com/tnthung/rev-idle/commit/315e96a), [fa63c6f](https://github.com/tnthung/rev-idle/commit/fa63c6f), [f691394](https://github.com/tnthung/rev-idle/commit/f691394))
- Added lock mode to prevent accidental game clicks while automation runs. Pause and Stop release the lock; `ReloadLocked` and `ResumeLocked` provide explicit locked transitions. ([21645c8](https://github.com/tnthung/rev-idle/commit/21645c8), [2e875c0](https://github.com/tnthung/rev-idle/commit/2e875c0), [10a92ee](https://github.com/tnthung/rev-idle/commit/10a92ee))
- Kept Capture armed when clicking outside the game, corrected screen-to-client targeting, and improved capture-button color state. Limited hotkey processing to the focused game and added debounce handling. ([5c435dc](https://github.com/tnthung/rev-idle/commit/5c435dc), [5442e1c](https://github.com/tnthung/rev-idle/commit/5442e1c), [859ab2e](https://github.com/tnthung/rev-idle/commit/859ab2e), [342db97](https://github.com/tnthung/rev-idle/commit/342db97), [10a92ee](https://github.com/tnthung/rev-idle/commit/10a92ee))

## 2026-09-10

### WebSocket input and in-game controls

- Moved click, scroll, and drag delivery from Windows messages to WebSocket input packets. Removed the old window-message probe, queue, protocol, and Win32 bridge implementation. ([5ac14c1](https://github.com/tnthung/rev-idle/commit/5ac14c1))
- Added `rev.press()`, `rev.delete_file()`, and `rev.shell()`, including plugin keyboard dispatch and client bindings. ([deb2628](https://github.com/tnthung/rev-idle/commit/deb2628))
- Reconciled one-shot capture with the WebSocket bridge and introduced the UI-path request and control-notification vocabulary. Capture resolves a path through a correlated request while control actions and state updates use notifications. ([a928c82](https://github.com/tnthung/rev-idle/commit/a928c82), [9942869](https://github.com/tnthung/rev-idle/commit/9942869), [a92673e](https://github.com/tnthung/rev-idle/commit/a92673e))
- Added connection generations, retained the current generation for late subscribers, and reset it on shutdown. Used generation-scoped Unity controls to reject stale state and avoid reconnect races. ([2bbb82c](https://github.com/tnthung/rev-idle/commit/2bbb82c), [029b6e5](https://github.com/tnthung/rev-idle/commit/029b6e5), [ed51563](https://github.com/tnthung/rev-idle/commit/ed51563), [b557c84](https://github.com/tnthung/rev-idle/commit/b557c84), [98c1845](https://github.com/tnthung/rev-idle/commit/98c1845))
- Added the in-game Reload/Stop, Resume/Pause, and Capture overlay, driven by client-published state. Follow-up fixes preserved queued commands, suppressed duplicate state, corrected capture presentation, and retained icon assets. ([b557c84](https://github.com/tnthung/rev-idle/commit/b557c84), [091dbb2](https://github.com/tnthung/rev-idle/commit/091dbb2), [10645d2](https://github.com/tnthung/rev-idle/commit/10645d2), [b6a1fda](https://github.com/tnthung/rev-idle/commit/b6a1fda))
- Initially rejected Capture while unloaded; startup Capture was subsequently enabled on September 26. Improved persistent-scene UI path lookup and stopped relying on unstable scene handles. ([307c730](https://github.com/tnthung/rev-idle/commit/307c730), [4d1d48e](https://github.com/tnthung/rev-idle/commit/4d1d48e), [5d81520](https://github.com/tnthung/rev-idle/commit/5d81520))

### Automation

- Added `UnityZodiac`, reworked zodiac merge/sell decisions, expanded sacrifice/equipment handling, and fixed merge ordering so cascades could complete. Corrected equipment support for planet sources. ([9061356](https://github.com/tnthung/rev-idle/commit/9061356), [92bfd34](https://github.com/tnthung/rev-idle/commit/92bfd34), [75c80e7](https://github.com/tnthung/rev-idle/commit/75c80e7), [c44c2f9](https://github.com/tnthung/rev-idle/commit/c44c2f9))
- Planned Dilation Tree changes virtually before applying them, added attack ETA checks, and moved thresholds into configuration. Added sacrifice-state access and corrected run-start timing. ([a38728e](https://github.com/tnthung/rev-idle/commit/a38728e), [b3924db](https://github.com/tnthung/rev-idle/commit/b3924db), [06c52a0](https://github.com/tnthung/rev-idle/commit/06c52a0), [4d8d1ec](https://github.com/tnthung/rev-idle/commit/4d8d1ec), [82a9e96](https://github.com/tnthung/rev-idle/commit/82a9e96))
- Added a zodiac-state dump script, standardized its output as `__dump.json`, and stopped execution after the export. ([e8b28c5](https://github.com/tnthung/rev-idle/commit/e8b28c5), [fbad7cb](https://github.com/tnthung/rev-idle/commit/fbad7cb))

## 2026-09-09

### WebSocket bridge migration

- Designed and implemented the raw bidirectional WebSocket transport: a Rust client connection and a plugin-side server. Added shared JSON packet fixtures for client/plugin agreement. ([6d1ed25](https://github.com/tnthung/rev-idle/commit/6d1ed25), [efcccff](https://github.com/tnthung/rev-idle/commit/efcccff))
- Routed state, capture, invoke, and transfer operations through typed request/response packets, migrated client consumers, and added plugin dispatch. Removed the legacy HTTP server and its old tests after migration. Coordinate input moved off Windows messages the following day. ([a69e6bd](https://github.com/tnthung/rev-idle/commit/a69e6bd), [b770255](https://github.com/tnthung/rev-idle/commit/b770255), [3a3d7cb](https://github.com/tnthung/rev-idle/commit/3a3d7cb), [88d7ca3](https://github.com/tnthung/rev-idle/commit/88d7ca3))
- Fixed request-correlation races and teardown behavior, added packet-handler coverage, retained full bridge error messages, and bounded bridge work per Unity frame. ([f34abcb](https://github.com/tnthung/rev-idle/commit/f34abcb), [e828183](https://github.com/tnthung/rev-idle/commit/e828183), [3d434d2](https://github.com/tnthung/rev-idle/commit/3d434d2), [512e12d](https://github.com/tnthung/rev-idle/commit/512e12d), [b7dbef6](https://github.com/tnthung/rev-idle/commit/b7dbef6))
- Changed Unity target lookup to resolve hierarchy paths directly. ([3b1ebaf](https://github.com/tnthung/rev-idle/commit/3b1ebaf))

### Script loading and automation

- Refreshed dynamic imports when module source changes, allowing configuration/module edits to be picked up without retaining stale module contents. ([501acb6](https://github.com/tnthung/rev-idle/commit/501acb6))
- Corrected numeric zodiac-level comparisons. ([0524276](https://github.com/tnthung/rev-idle/commit/0524276))
- Combined eternal challenges into one run, added a DTP stop condition, initialized Dilation Tree loadouts explicitly, and reversed DTP-stage iteration. Adjusted default action delays and added `maxAttackLevelReached()` state access. ([460138b](https://github.com/tnthung/rev-idle/commit/460138b), [9a71d60](https://github.com/tnthung/rev-idle/commit/9a71d60), [b1456e9](https://github.com/tnthung/rev-idle/commit/b1456e9), [5cf7ed3](https://github.com/tnthung/rev-idle/commit/5cf7ed3), [d81e855](https://github.com/tnthung/rev-idle/commit/d81e855))

## 2026-09-08

- Introduced `unity_loop2.js` with staged eternity bootstrapping, eternal challenges, Dilation Tree progression, DTP40 completion, Unity timing, pause accounting, and reset thresholds. Extracted reusable Dilation Tree and utility modules and expanded `Action` and `States`. ([5848380](https://github.com/tnthung/rev-idle/commit/5848380))
- Made `Action` callable through a `Function` subclass and proxy, dispatching calls to action execution. Renamed action methods and updated callers. ([d3c7126](https://github.com/tnthung/rev-idle/commit/d3c7126), [0898e59](https://github.com/tnthung/rev-idle/commit/0898e59))
- Added file read/write APIs and console clearing, then placed clearing on `console.clear()` instead of `rev.clear()`. ([a0154d2](https://github.com/tnthung/rev-idle/commit/a0154d2), [acbae38](https://github.com/tnthung/rev-idle/commit/acbae38))
- Moved runtime configuration into `unity_loop2_config.js` and loaded it at run start. Added a dismiss loop, planet-zodiac inventory reads, and JSON inventory snapshots during eternity bootstrap. ([6c29d83](https://github.com/tnthung/rev-idle/commit/6c29d83), [66dd925](https://github.com/tnthung/rev-idle/commit/66dd925), [0491ad7](https://github.com/tnthung/rev-idle/commit/0491ad7), [9418a51](https://github.com/tnthung/rev-idle/commit/9418a51))
- Fixed numeric zodiac rarity comparisons, grouped merge/sell decisions by element, and protected locked zodiacs in hard-trial handling. ([234b56b](https://github.com/tnthung/rev-idle/commit/234b56b), [4cca61e](https://github.com/tnthung/rev-idle/commit/4cca61e), [a206ed3](https://github.com/tnthung/rev-idle/commit/a206ed3))
- Reduced waiting-loop work, improved extra Dilation Tree traversal, shortened sleeps, and used condition polling for completion of the first nine eternal challenges. ([130f149](https://github.com/tnthung/rev-idle/commit/130f149), [a0e9a4a](https://github.com/tnthung/rev-idle/commit/a0e9a4a), [4f845f0](https://github.com/tnthung/rev-idle/commit/4f845f0), [947b290](https://github.com/tnthung/rev-idle/commit/947b290))

## 2026-09-07

### Capture, invoke, and transfer

- Added Unity button invocation by name or path and enriched capture results with the target's hierarchy path. This established the first direct UI invocation flow through the HTTP bridge. ([921a143](https://github.com/tnthung/rev-idle/commit/921a143))
- Added slot-to-slot transfers across script bindings, the client bridge, and Unity dispatch, and extended the action library with invocation/transfer delays. ([97d4596](https://github.com/tnthung/rev-idle/commit/97d4596), [a10643d](https://github.com/tnthung/rev-idle/commit/a10643d), [4104212](https://github.com/tnthung/rev-idle/commit/4104212))
- Copied successfully captured paths to the clipboard, validated clipboard formats before reading Unicode text, and fixed consumption of the matching mouse release when capture stops. ([770ec55](https://github.com/tnthung/rev-idle/commit/770ec55), [cefd5b2](https://github.com/tnthung/rev-idle/commit/cefd5b2), [ed5eb80](https://github.com/tnthung/rev-idle/commit/ed5eb80))

### Client structure and console

- Split the Rust client into focused `app`, `console`, `script`, `bridge`, and `window` modules. Separated command parsing/history/completion, script bindings/lifecycle/loading, bridge operations, and Windows discovery/input/clipboard handling. ([0835bca](https://github.com/tnthung/rev-idle/commit/0835bca))
- Reworked console locking and command handling, removed the old direct pause/stop console commands, and added command/path completion with common-prefix expansion. Excluded `exit` from saved command history. ([4f27ed7](https://github.com/tnthung/rev-idle/commit/4f27ed7), [373b2b8](https://github.com/tnthung/rev-idle/commit/373b2b8), [cbc21a0](https://github.com/tnthung/rev-idle/commit/cbc21a0), [1594ffc](https://github.com/tnthung/rev-idle/commit/1594ffc))

### Automation

- Improved eternal-challenge execution, fixed stuck-loop conditions, and corrected elapsed time across stopped periods. Shifted Dilation Tree priorities toward T3 nodes and M2 passive eternals. ([036114e](https://github.com/tnthung/rev-idle/commit/036114e), [6ddab3f](https://github.com/tnthung/rev-idle/commit/6ddab3f), [23ef0c4](https://github.com/tnthung/rev-idle/commit/23ef0c4), [26b73d3](https://github.com/tnthung/rev-idle/commit/26b73d3), [710ca02](https://github.com/tnthung/rev-idle/commit/710ca02))
- Reworked zodiac selling and validated inventory positions before shop actions. Began the larger Unity-loop rewrite that became `unity_loop2.js` on September 8. ([7d2dd17](https://github.com/tnthung/rev-idle/commit/7d2dd17), [2d70108](https://github.com/tnthung/rev-idle/commit/2d70108), [49fe88a](https://github.com/tnthung/rev-idle/commit/49fe88a))

## 2026-09-06

- Added `beforePause` and `afterResume` lifecycle hooks. Used them in automation to track pause duration and keep elapsed-run accounting accurate. ([002cfe6](https://github.com/tnthung/rev-idle/commit/002cfe6), [d13d6a5](https://github.com/tnthung/rev-idle/commit/d13d6a5))
- Improved Unity waiting logic and automation for eternal challenges 1–9. ([3daaccc](https://github.com/tnthung/rev-idle/commit/3daaccc), [d6f312b](https://github.com/tnthung/rev-idle/commit/d6f312b))
- Restored the capture-stopped message when Ctrl+C ends capture. ([c55d191](https://github.com/tnthung/rev-idle/commit/c55d191))

## 2026-09-05

- Added script module resolution and loading for relative imports, allowing automation code to be split across modules. ([9c97807](https://github.com/tnthung/rev-idle/commit/9c97807))
- Updated Dilation Tree handling to work from an Eternal clone, added incremental center/top action sequences, and improved the EC10 dilation flow. ([d346255](https://github.com/tnthung/rev-idle/commit/d346255), [e152e89](https://github.com/tnthung/rev-idle/commit/e152e89), [8e06d02](https://github.com/tnthung/rev-idle/commit/8e06d02))
- Refined initialization/error handling, Unity wait timing, zodiac-shop thresholds, and insufficient-AP behavior. Corrected `dilationMaxScore` usage. ([89435b7](https://github.com/tnthung/rev-idle/commit/89435b7), [3c45486](https://github.com/tnthung/rev-idle/commit/3c45486), [8692b70](https://github.com/tnthung/rev-idle/commit/8692b70), [ef69280](https://github.com/tnthung/rev-idle/commit/ef69280), [8a99e6f](https://github.com/tnthung/rev-idle/commit/8a99e6f), [aa3f02c](https://github.com/tnthung/rev-idle/commit/aa3f02c))
- Added reset logic for Unity runs that exceeded the configured duration. ([88b21bb](https://github.com/tnthung/rev-idle/commit/88b21bb))

## 2026-09-04

### Script APIs

- Added `rev.read_clipboard()`, retry handling for clipboard access, and clipboard restoration around Dilation Tree loadout imports. ([d733b15](https://github.com/tnthung/rev-idle/commit/d733b15), [2e6c021](https://github.com/tnthung/rev-idle/commit/2e6c021), [9b42e2a](https://github.com/tnthung/rev-idle/commit/9b42e2a))
- Added `rev.drag(x1, y1, x2, y2)` across the client/plugin input bridge, dispatching Unity drag/drop lifecycle events at the source and destination. ([8ccc6bf](https://github.com/tnthung/rev-idle/commit/8ccc6bf))
- Added process-wide JSON-backed `rev.global`, allowing data to survive script reloads and new QuickJS contexts. Moved run-start timing into this storage. ([572c580](https://github.com/tnthung/rev-idle/commit/572c580), [7e28358](https://github.com/tnthung/rev-idle/commit/7e28358))
- Fixed an F8 pause race that could incorrectly stop an in-flight host call. ([9fe29e6](https://github.com/tnthung/rev-idle/commit/9fe29e6))

### Unity automation

- Added the first substantial game automation entry as `scripts/test1.js`, then promoted it to `unity_loop.js` with repeatable completion flow. It combined state reads, waits, UI actions, and Dilation Tree progression. ([1d3543e](https://github.com/tnthung/rev-idle/commit/1d3543e), [7a748a2](https://github.com/tnthung/rev-idle/commit/7a748a2))
- Corrected Dilation Tree state/property handling and loadout cycling, optimized the 36–38 loadout path, fixed matching/conditions, and prevented infinite loops. ([c6379c8](https://github.com/tnthung/rev-idle/commit/c6379c8), [792ccaa](https://github.com/tnthung/rev-idle/commit/792ccaa), [8a2e6ae](https://github.com/tnthung/rev-idle/commit/8a2e6ae), [a75f508](https://github.com/tnthung/rev-idle/commit/a75f508), [d6a6a91](https://github.com/tnthung/rev-idle/commit/d6a6a91))
- Refined action-step execution and drag timing, added run elapsed-time tracking, and expanded the state alias table with 65 high-value aliases. ([91b0daa](https://github.com/tnthung/rev-idle/commit/91b0daa), [ee44089](https://github.com/tnthung/rev-idle/commit/ee44089), [5013922](https://github.com/tnthung/rev-idle/commit/5013922), [6d014fa](https://github.com/tnthung/rev-idle/commit/6d014fa))

## 2026-09-03

### Complete state access

- Expanded the narrow state payload into a reflective game-state graph, with nested objects, collection traversal, full state paths, and explicit handling of eligible properties and delegates. Getter failures became errors instead of silently disappearing. ([dfb6846](https://github.com/tnthung/rev-idle/commit/dfb6846), [8c94865](https://github.com/tnthung/rev-idle/commit/8c94865), [b55bff4](https://github.com/tnthung/rev-idle/commit/b55bff4), [20a0798](https://github.com/tnthung/rev-idle/commit/20a0798), [2267db5](https://github.com/tnthung/rev-idle/commit/2267db5))
- Preserved complete JSON values in the Rust client and froze returned state through QuickJS. Added direct unwrapping for single-key requests so `rev.state(path)` returns that value. ([9dfc386](https://github.com/tnthung/rev-idle/commit/9dfc386), [fc542d7](https://github.com/tnthung/rev-idle/commit/fc542d7), [acafc8e](https://github.com/tnthung/rev-idle/commit/acafc8e))
- Added static controller types as additional state roots, then required every request path to name an explicit root key. Raw paths such as `eternity.EP` became `gameData.eternity.EP`; compatibility aliases retained their meanings. Keyless requests were rejected instead of implicitly dumping all game data. ([c13fe3a](https://github.com/tnthung/rev-idle/commit/c13fe3a), [554f005](https://github.com/tnthung/rev-idle/commit/554f005))
- Made `rev` a global in the script context and removed the previous memory argument from the script entry API. Added a clear-console command and made Ctrl+C cancel an outstanding `rev.sleep()` promptly. ([e6fab0a](https://github.com/tnthung/rev-idle/commit/e6fab0a), [acafc8e](https://github.com/tnthung/rev-idle/commit/acafc8e), [a1ebaf6](https://github.com/tnthung/rev-idle/commit/a1ebaf6))

### Generated state reference

- Added `generate-state-reference.ps1` and generated `STATE_KEYS.md`, including Unity-value paths. Added a `.cmd` launcher and multiple target-framework support for plugin tests. ([3949624](https://github.com/tnthung/rev-idle/commit/3949624), [1319194](https://github.com/tnthung/rev-idle/commit/1319194), [2cc5f5e](https://github.com/tnthung/rev-idle/commit/2cc5f5e), [09ecb9d](https://github.com/tnthung/rev-idle/commit/09ecb9d), [eb47df0](https://github.com/tnthung/rev-idle/commit/eb47df0))
- Added `STATE_GRAPH.json`, a reusable HTML template, and a standalone interactive state viewer for finding types, fields, aliases, and paths. ([8f72734](https://github.com/tnthung/rev-idle/commit/8f72734))
- Iterated on route sorting, fuzzy search, copyable paths/aliases/fields, and resizable navigation. The early DAG visualization was replaced by a text/path-oriented reference interface later that day. ([304b5c1](https://github.com/tnthung/rev-idle/commit/304b5c1), [2b49a31](https://github.com/tnthung/rev-idle/commit/2b49a31), [871055c](https://github.com/tnthung/rev-idle/commit/871055c), [0db45c8](https://github.com/tnthung/rev-idle/commit/0db45c8), [808c842](https://github.com/tnthung/rev-idle/commit/808c842))
- Added keyboard search navigation, click-to-copy, a full-height two-column layout, narrow-screen tabs, contained scrolling, sticky headers, and more reliable tab/result navigation. ([25010ca](https://github.com/tnthung/rev-idle/commit/25010ca), [1cd9df4](https://github.com/tnthung/rev-idle/commit/1cd9df4), [9c1915b](https://github.com/tnthung/rev-idle/commit/9c1915b), [eda20fb](https://github.com/tnthung/rev-idle/commit/eda20fb), [fcccf6c](https://github.com/tnthung/rev-idle/commit/fcccf6c), [0d9854e](https://github.com/tnthung/rev-idle/commit/0d9854e))

## 2026-09-02

### Background input and script controls

- Added repeated clicking and scrolling to the script API, script-visible console logging, and improved QuickJS error messages. Fixed starting a stopped runtime without an initial script. ([c36647a](https://github.com/tnthung/rev-idle/commit/c36647a), [ce8f10e](https://github.com/tnthung/rev-idle/commit/ce8f10e), [f5f7458](https://github.com/tnthung/rev-idle/commit/f5f7458), [4dd0e79](https://github.com/tnthung/rev-idle/commit/4dd0e79))
- Added mouse-position capture and excluded the BepInEx console from game-window discovery. ([3cf1dc0](https://github.com/tnthung/rev-idle/commit/3cf1dc0), [c57c330](https://github.com/tnthung/rev-idle/commit/c57c330))
- Implemented background Unity UI input through a Windows-message bridge, with a command queue, plugin-side raycast dispatch, and client transport. Routed clicks and scrolling through it, filtered non-clickable raycast results, and removed the temporary `enigo` dependency. ([0fa9a79](https://github.com/tnthung/rev-idle/commit/0fa9a79), [000695e](https://github.com/tnthung/rev-idle/commit/000695e), [761775c](https://github.com/tnthung/rev-idle/commit/761775c), [78ba516](https://github.com/tnthung/rev-idle/commit/78ba516), [798a62a](https://github.com/tnthung/rev-idle/commit/798a62a), [abcbc2c](https://github.com/tnthung/rev-idle/commit/abcbc2c))
- Added `rev.stop()` and clipboard writing, stopped scripts on unhandled errors, and added a tracked automation test script. ([e075255](https://github.com/tnthung/rev-idle/commit/e075255), [41ca1a2](https://github.com/tnthung/rev-idle/commit/41ca1a2), [f80177b](https://github.com/tnthung/rev-idle/commit/f80177b), [88c2b0f](https://github.com/tnthung/rev-idle/commit/88c2b0f))
- Hardened Ctrl+C handling and process cleanup so active modes terminate and the console can exit without waiting indefinitely for standard input. ([7cd2ae1](https://github.com/tnthung/rev-idle/commit/7cd2ae1), [f986073](https://github.com/tnthung/rev-idle/commit/f986073), [c6e89ac](https://github.com/tnthung/rev-idle/commit/c6e89ac))

### UDP-to-HTTP migration

- Replaced periodic UDP telemetry with on-demand HTTP state requests. Added `HttpScoreServer` and `StatePayload`, exposed `/state`, and removed the original score payload/publisher classes. Changed the client to fetch state when requested. ([2c69aae](https://github.com/tnthung/rev-idle/commit/2c69aae), [459823b](https://github.com/tnthung/rev-idle/commit/459823b))
- Bounded pending-request waits and refined request cleanup, disconnect handling, and client/server responsibilities. Updated the plugin guide for the HTTP API. ([f7b0f9f](https://github.com/tnthung/rev-idle/commit/f7b0f9f), [25e9024](https://github.com/tnthung/rev-idle/commit/25e9024), [46737ca](https://github.com/tnthung/rev-idle/commit/46737ca), [d6d5910](https://github.com/tnthung/rev-idle/commit/d6d5910), [9ce3c3e](https://github.com/tnthung/rev-idle/commit/9ce3c3e), [3f3b91e](https://github.com/tnthung/rev-idle/commit/3f3b91e), [089b688](https://github.com/tnthung/rev-idle/commit/089b688))
- Updated installation/test requirements to .NET SDK 8 or newer. ([e09f853](https://github.com/tnthung/rev-idle/commit/e09f853))

## 2026-09-01 — Repository inception

### Initial plugin

- Created the repository with the score-telemetry design and implementation plan at 10:05:54 UTC+08:00. The root commit was documentation and repository setup; the implementation followed the same morning. ([99c7d49](https://github.com/tnthung/rev-idle/commit/99c7d49))
- Added JSON encoding of the game's `BigDouble` score as mantissa/exponent values and a loopback UDP publisher with port validation. Added the initial C# test harness. ([9741d3d](https://github.com/tnthung/rev-idle/commit/9741d3d))
- Added the BepInEx IL2CPP plugin and a ticker reading the live score every 50 milliseconds using unscaled time. Network configuration defaulted to port `19841`; invalid ports disabled publishing. Added plugin metadata and build/install documentation. ([5652e95](https://github.com/tnthung/rev-idle/commit/5652e95))
- Corrected plugin references to use managed assemblies. Added an automated PowerShell installer, then hardened recovery and partial-install handling, preserved concurrent installer files, and added the process-scoped execution-policy launcher `install.cmd`. ([7ee04eb](https://github.com/tnthung/rev-idle/commit/7ee04eb), [a6b5c03](https://github.com/tnthung/rev-idle/commit/a6b5c03), [52fc0b5](https://github.com/tnthung/rev-idle/commit/52fc0b5), [24f1e4c](https://github.com/tnthung/rev-idle/commit/24f1e4c), [446ce90](https://github.com/tnthung/rev-idle/commit/446ce90), [1a8e148](https://github.com/tnthung/rev-idle/commit/1a8e148))

### Rust client and JavaScript execution

- Bootstrapped the Rust client and asynchronous QuickJS dependencies. Added UDP score reception and publication of the latest telemetry snapshot. ([deceea5](https://github.com/tnthung/rev-idle/commit/deceea5), [65b5196](https://github.com/tnthung/rev-idle/commit/65b5196), [e75c1e4](https://github.com/tnthung/rev-idle/commit/e75c1e4), [2237a22](https://github.com/tnthung/rev-idle/commit/2237a22))
- Added console command parsing, persistent JavaScript sessions, repeated script execution, and initial `rev` automation bindings for input, sizing, and waits. Retained script state between invocations and added lifecycle/error coverage. ([4a1f4fc](https://github.com/tnthung/rev-idle/commit/4a1f4fc), [6978b8a](https://github.com/tnthung/rev-idle/commit/6978b8a), [4078d68](https://github.com/tnthung/rev-idle/commit/4078d68), [91158cc](https://github.com/tnthung/rev-idle/commit/91158cc), [11e0d2f](https://github.com/tnthung/rev-idle/commit/11e0d2f))
- Added game-window discovery and window-relative actions, including the corrected implementation after an initial revert. ([b816130](https://github.com/tnthung/rev-idle/commit/b816130), [f4e67cc](https://github.com/tnthung/rev-idle/commit/f4e67cc))
- Added the F8 automation toggle, synchronized immediate pause behavior, and fixed hotkey shutdown deadlocks and cleanup after task errors. ([5037f2a](https://github.com/tnthung/rev-idle/commit/5037f2a), [efe1279](https://github.com/tnthung/rev-idle/commit/efe1279), [4884d88](https://github.com/tnthung/rev-idle/commit/4884d88), [8fda5c0](https://github.com/tnthung/rev-idle/commit/8fda5c0), [a111f74](https://github.com/tnthung/rev-idle/commit/a111f74), [e7aca07](https://github.com/tnthung/rev-idle/commit/e7aca07), [57e8c2d](https://github.com/tnthung/rev-idle/commit/57e8c2d))
