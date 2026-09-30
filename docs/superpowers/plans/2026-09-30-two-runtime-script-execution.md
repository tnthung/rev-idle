# Two-Runtime Script Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Suspend main script execution during Pause while daemons and UI callbacks continue in a separate runtime, with shared Rust UI state and automatic import replay.

**Architecture:** Each session retains one main and one background QuickJS runtime on the existing script worker. Rust owns shared registries; runtime-local proxies submit data and function descriptors. Generated background modules reconstruct functions using their defining module's retained imports, while the lifecycle retains and gates all main execution futures.

**Tech Stack:** Rust 2024, Tokio, rquickjs 0.12.2, Oxc 0.150.0, oxc_sourcemap 8.1.2, TypeScript, existing Unity WebSocket UI bridge.

**Spec:** [Two-runtime script execution](../../../__explain.md). Read it before this plan. The spec is currently ignored by Git; preserve it with the plan when sharing these files. Implementation was approved and completed on 2026-09-30. At the user's request, workers implemented disjoint files in parallel, followed by integrated tests and a runtime review.

## Execution results

Tasks 1–9 are complete. The original step checklists below describe the planned sequence; they are retained as written because parallel execution did not run every individual RED command in that sequence. The execution record is in `.superpowers/sdd/2026-09-30-two-runtime-script-execution/progress.md`.

| Tasks | Implemented and verified |
| --- | --- |
| 1–2 | Retained static imports/re-exports, fresh dynamic imports, native function provenance, generated transfer modules, imported classes/enums/helpers, and natural missing-local errors. |
| 3–6 | Rust-owned UI data and element states, a separately driven background runtime, daemon soft retirement, and asynchronous function-or-null UI setters. Existing UI regressions remain enabled. |
| 7–8 | Retained main invocation, acknowledged Pause gating, hook/connection ordering, call-time sleep deadlines, subprocess interruption cases, and session teardown with pending background work. |
| 9 | Public declarations, UI demo/history migrations, and API documentation. Concurrent user declaration edits were preserved. |

Final verification: `cargo test` passed **280 tests, 0 failed, 0 ignored**; `cargo check --all-targets` passed; Bun builds for `scripts/ui_demo.ts` and `scripts/lib/history.ts` and `node --check client/src/script/ui.js` passed. `git diff --check` passed. TypeScript type-checking was unavailable because `tsc` was not installed on PATH. The final runtime review has no remaining findings. No installation or live-game verification was performed.

## Global Constraints

- A script session owns exactly two independently driven QuickJS runtimes.
- The host polls the runtimes sequentially. It must not synchronously enter one runtime from the other.
- JavaScript objects, functions, promises, and persistent roots stay with their owning runtime.
- After Pause acknowledgment, do not poll or directly enter main; retain its invocation and resume it exactly once.
- Capture the sleep deadline at the JavaScript call. Time spent paused counts toward the duration.
- Use `rev.global` for cross-runtime JSON communication and per-element `states` for element data. Add no channel API.
- Configure UI callbacks only through the three setters in spec section 9: a function registers/replaces; `null` clears. Expose no `onClick`, `onHover`, or `onLeave` callback properties.
- Reconstruct functions from captured native source/metadata getters; add no custom syntax, compiler function tags, or lexical-capture checker.
- Install stable background `rev` before generated modules and their dependencies evaluate.
- Use the existing dependencies and packet shapes. Element states and transfer descriptors stay in Rust; Unity needs no new state protocol.
- Follow existing style. Never run formatting tools or mutate Git state without an explicit request. Task checkpoints below mean reviewable working files, not commits.
- Preserve unrelated changes, including untracked `README.md`. Do not alter ignore rules or force-add ignored scripts/specs.
- Use one owner per file. Disjoint implementation areas run in parallel with explicit shared interfaces; no worker edits another owner's files.

## Review Focus

1. A dependency changes after main linked it: background must replay the old static graph while a later explicit dynamic import sees the edit. Task 1 tests both paths, including re-exports.
2. A handler installation finishes after replacement/removal, or its setter receives a previously failed function again: stale work stays inactive and the retry can succeed. Task 6 tests both races.
3. Pause/Resume requests and connection changes arrive during an awaited hook: no stale acknowledgment, lost transition, extra default invocation, or hidden main poll. Task 7 tests these transitions.
4. A running daemon is deleted and immediately re-added: its old run still blocks the replacement, including when it never settles. Task 5 tests retained retirement bookkeeping and detached descendants.
5. A UI callback awaits a measurement while the element or session is replaced: old proxies cannot mutate the replacement and measurement results are rejected. Tasks 6 and 8 test element and session invalidation.

---

## Approved policies

Daemon retirement is confirmed: unregister immediately; let the active invocation finish; wait for its returned promise before starting the latest replacement. Detached descendants are outside this ordering. Hook timing is also confirmed: `beforePause` finishes before acknowledgment, and connection hooks received during Pause defer until Resume.

UI callback configuration is confirmed: `setOnClick`, `setOnHover`, and `setOnLeave` accept a function to register/replace or `null` to clear. The API exposes no callback properties, separate removal methods or manual-dispatch wrapper.

The approved plan also uses these defaults from spec sections 8 and 11:

- **Hook ordering detail:** `afterLoad` runs in main; Resume enables main, runs `afterResume`, then replays deferred connection hooks in receive order before starting a new default invocation. Existing pending main work may advance while hooks run. Hook errors retain the current log-and-continue policy.
- **Daemon reads:** daemon property reads throw `TypeError("Daemons are assignment-only")`; membership/enumeration report currently registered names. The selected UI methods do not change daemon registration syntax.
- **Whole-map states:** assigning `element.states = { ... }` replaces contents; deleting `element.states` clears contents. Existing state proxies continue to address the same element instance.

These defaults were adopted with plan approval. Record any necessary implementation correction with its reason and keep the spec consistent.

Pause has no promised millisecond latency. A single QuickJS/rquickjs poll can drain ready jobs. Task 8 measures that limitation safely; a stronger per-job scheduling mechanism would require a separate design decision.

## File map and task order

| Files | Responsibility |
| --- | --- |
| `client/src/script/loader.rs` | Retained module snapshots, static dependency tokens, import replay, generated module origin and source maps |
| New `client/src/script/transfer.rs`; `script/mod.rs` | Runtime-local captured function intrinsics; Rust-owned function descriptors |
| `client/src/script/ui.rs`, `ui.js` | One Rust registry and publisher; two local proxy sets; per-element JSON states and measurement checks |
| New `client/src/script/background.rs`; `script/mod.rs` | Rust registration queue, daemon generations, background runtime service, installed callback roots |
| `client/src/script/session.rs`, `bindings.rs` | Own both runtimes, wire local bindings, stable background `rev`, deadline-based sleep |
| `client/src/script/control.rs`, `lifecycle.rs` | Acknowledged pause state, retained main work, hook queue, background polling, teardown |
| `client/src/script/ownership.rs` | Preserve Rust ownership state; update checkpoint tests and verify tokens remain local to their runtime |
| `client/src/script/tests.rs`, `worker_tests.rs`, inline module tests | Import, transfer, lifecycle, UI and interruption coverage |
| `client/src/bridge/script_ui.rs`, `packets.rs` | Preserve publication/event/measurement contracts; extend regression coverage only as needed |
| `scripts/rev.d.ts`, `ui_demo.ts`, `lib/history.ts` | Public declarations and migration of callback captures/always-running work |
| `docs/js_api_guide.md`, `docs/superpowers/specs/2026-09-28-script-ui-design.md` | Current API guide and short supersession pointer in historical design |

Tasks 1–2 establish transfer independently of the session scheduler. Task 3 establishes shared UI data. Tasks 4–6 integrate background execution and both public registration surfaces. Task 7 switches Pause behavior, Task 8 verifies termination/fairness, and Task 9 migrates shipped scripts and documentation. Keep one plan because these pieces must agree on runtime ownership to deliver the feature.

### Test conventions

Run Cargo commands from `D:\Project\rev-idle\client`. Commands below use that directory. New tests use the existing current-thread Tokio, fake mouse/window, WebSocket server, and temporary-module patterns in `script/tests.rs`, `ui.rs`, and `worker_tests.rs`. Assertion blocks specify the collected observations; they are not new production helper APIs. Keep setup local to each test unless an existing harness already covers it. Use barriers/notifications for ordering and bounded timeouts for failures; do not wait through a real 1,000,000 ms timer.

Every filtered test command must execute at least one test. A zero-match result is a failed verification step. Do not install tools or dependencies just for documentation checks.

## Task 1: Retain the static module graph and emitted imports

**Files:** Modify `client/src/script/loader.rs`; test `client/src/script/tests.rs` and loader inline tests.

**Interfaces:**

- Preserve `ScriptModules::insert(&self, name: &str, source: String) -> Result<(), String>` and the existing `Resolver`/`Loader` trait signatures.
- Add `ModuleImports { module_id: String, resolution_base: String, declarations: Vec<String> }` and `ScriptModules::imports(&self, id: &str) -> Option<ModuleImports>`; returned declarations are emitted runtime imports with retained static tokens, in source order.
- Extend private `CompiledModule` with those imports and generated-origin metadata. Retain private static-edge records containing owner module ID, original specifier, token and `Option<String>` resolved ID.

- [ ] Write `two_runtime_loader_static_graph_keeps_versions` and `two_runtime_loader_import_forms_and_maps`. The first loads a function through a barrel and transitive dependency, edits both dependency files, then replays the recorded imports in a second runtime and performs a dynamic import from the same origin. The second covers JS/TS default, named, alias, namespace, side-effect and type-only imports, plus a Unicode source-map location.

  ```rust
  // two_runtime_loader_static_graph_keeps_versions
  assert_eq!(main_static_value, 1);
  assert_eq!(background_static_value, 1);
  assert_eq!(background_dynamic_value, 2);
  // two_runtime_loader_import_forms_and_maps
  assert_eq!(replayed_bindings, serde_json::json!(["default", "alias", "namespace", "enum"]));
  assert_eq!(side_effect_count, 1);
  assert!(!replayed_source.contains("TypeOnly"));
  assert!(error.contains("unicode.ts:4:"));
  ```

- [ ] Run `cargo test two_runtime_loader -- --nocapture`; expect failure because import metadata/version pinning is absent.
- [ ] Extend `ScriptModules::insert` to parse both JS and TS with Oxc. After TS lowering, rewrite only static import/re-export source literals into private tokens before code generation. Visit `ImportDeclaration`, `ExportFromDeclaration`, and `ExportAllDeclaration`; leave dynamic import expressions unchanged. Use allocator-owned literal values and keep codegen source maps for both languages. Record only ordinary runtime import declarations for replay; retain all static edges, including re-exports, for graph consistency.
- [ ] Extend `Resolver::resolve` to lazily pin each registered static token to its first resolved source-version ID, then reuse that ID in either runtime. Apply tokens to every compiled module, including transitive dependencies. Release registry borrows before recursively compiling/resolving. Unknown private tokens fail. Ordinary relative dynamic imports continue hashing current file bytes.
- [ ] Run `cargo test two_runtime_loader -- --nocapture`, `cargo test import`, and `cargo test typescript`; expect all matching tests to pass, including unchanged dynamic-import freshness and Unicode mapping tests. Review the loader diff without staging it.

## Task 2: Capture function provenance and build background modules

**Files:** Create `client/src/script/transfer.rs`; modify `loader.rs`, `mod.rs`; test transfer inline tests and `tests.rs`.

**Interfaces:**

```rust
// transfer.rs: owned data only; optional metadata supports natural install failures.
pub(super) struct FunctionDescriptor {
    pub(super) source: String,
    pub(super) module_id: Option<String>,
    pub(super) line: Option<i32>,
    pub(super) column: Option<i32>,
}
pub(super) struct FunctionTransfer { /* roots belonging to one runtime */ }
impl FunctionTransfer {
    pub(super) fn new<'js>(ctx: &Ctx<'js>) -> rquickjs::Result<Self>;
    pub(super) fn capture<'js>(&self, ctx: &Ctx<'js>, function: Function<'js>)
        -> rquickjs::Result<FunctionDescriptor>;
}
// loader.rs: inserts a uniquely named generated module and returns its identity.
impl ScriptModules {
    pub(super) fn transfer_module(&self, registration: uuid::Uuid, function: &FunctionDescriptor)
        -> Result<String, String>;
}
```

- [ ] Write `two_runtime_transfer_forms_origin_and_intrinsics`: define ordinary/async/arrow/method functions in an imported file, register from another, spoof own `fileName`/`toString`, and replace prototype methods after capture initialization. Write `two_runtime_transfer_imports_and_missing_locals`: replay imported classes/enums/helpers with dependency-private state, attempt an absent defining-module local, re-register a nested background function, and cover invalid/native/bound source and transformed code requiring a module-local helper.

  ```rust
  assert_eq!(forms_returned, serde_json::json!([1, 2, 3, 4]));
  assert_eq!(captured_module_id, defining_module_id);
  assert_eq!(background_import_side_effects, 1);
  assert_eq!(main_import_side_effects, 1);
  assert!(missing_local_error.contains("ReferenceError"));
  assert_eq!(nested_import_value, 42);
  assert!(unsupported_install_failed);
  assert_eq!(original_main_callback_calls, 0);
  assert_ne!(first_generated_id, second_generated_id);
  ```

- [ ] Run `cargo test two_runtime_transfer -- --nocapture`; expect failure because capture/reconstruction is absent.
- [ ] Implement `FunctionTransfer::new/capture` using captured native `Function.prototype.toString` and filename/line/column getter functions. Capture them before evaluating user code. Pass call-time `Ctx` to Rust callbacks; never restore roots in another runtime. Runtime adapters own the persistent roots outside the engine; Rust host callbacks use `Weak<FunctionTransfer>` when they need access, so a JS callback cannot keep its runtime alive through a strong persistent-root cycle. Keep unsupported origin/source data for installation diagnostics; reject non-functions synchronously at the registration surface.
- [ ] Implement `ScriptModules::transfer_module` using Task 1 imports. Parse source as a function expression first, then an ordinary object-method form; use a valid object wrapper for method shorthand. Do not use regex-based free-variable analysis. Generate an ES module exporting the function, with a unique registration UUID in its identity. Preserve origin/import metadata for nested/re-registered functions and resolve dynamic imports relative to the original module. Keep source-map fallback honest when exact transformed locations cannot be recovered.
- [ ] Run `cargo test two_runtime_transfer -- --nocapture` and `cargo test typescript`; expect pass. Confirm failed reconstruction cannot execute the original function, and diagnostics identify the original module/function plus generated location when needed. Review the transfer diff.

## Task 3: Move UI data into one Rust store

**Files:** Modify `client/src/script/ui.rs`, `ui.js`, `session.rs`, `bindings.rs`; test UI inline tests. Task 6 will switch handler execution; this task isolates storage first.

**Interfaces:**

- Keep the name `ScriptUiState` for the shared Rust-only owner; change construction to `ScriptUiState::new(publisher: ScriptUiPublisher, session: SessionControl) -> Self`.
- Introduce runtime-local `ScriptUiBindings::new<'js>(ctx: &Ctx<'js>, state: Rc<ScriptUiState>, connection: WsConnection, session: SessionControl) -> rquickjs::Result<Self>` and `registry<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<Object<'js>>`.
- Add `ScriptUiBindings::element<'js>(&self, ctx: &Ctx<'js>, name: &str, instance: Uuid) -> rquickjs::Result<Object<'js>>` for a checked local receiver.
- Store records as Rust visual attributes, UUID instance, JSON state map and handler metadata; local bindings own proxy/function identity caches. Keep the existing single `ScriptUiPublisher::replace/clear` contracts.

- [ ] Write `two_runtime_ui_shared_store_and_states`: create two contexts in different runtimes over one Rust owner and alternate synchronous operations. Write `two_runtime_ui_atomic_replacement_and_proxy_identity`: invalid replacement leaves the old record intact; valid replacement retains stacking position and invalidates both old proxies.

  ```rust
  assert_eq!(published_names, vec!["from_main", "from_background"]);
  assert_eq!(background_read_after_main_write, serde_json::json!(10));
  assert_eq!(state_only_revision_after, state_only_revision_before);
  assert_eq!(nested_value_before_reassignment, serde_json::json!(false));
  assert_eq!(nested_value_after_reassignment, serde_json::json!(true));
  assert!(same_runtime_lookup_identity);
  assert!(states_proxy_survives_clear_and_whole_map_replacement);
  assert_eq!(instance_after_invalid_definition, original_instance);
  assert_ne!(instance_after_valid_definition, original_instance);
  assert!(old_main_proxy_rejected && old_background_proxy_rejected);
  ```

- [ ] Run `cargo test two_runtime_ui_ -- --nocapture`; expect failure because the map currently belongs to one JS closure.
- [ ] Refactor the existing JS validation/proxy logic to read and mutate the Rust owner synchronously. Keep validation rules and compound-field read-only views; validate/serialize complete definitions before committing. Rust produces the only complete visual snapshot. Equal visual writes are no-ops; state mutations do not publish. Give all attribute/state operations a checked `(name, instance)` target and preserve JSON-copy semantics from `rev.global`, including serialization errors.
- [ ] Wire one main `ScriptUiBindings` into the current session and `create_rev`. Keep callback roots local during this intermediate task; never put them into shared records. Preserve existing UI behavior until Task 6 replaces dispatch. Ensure only the shared owner clears publication on teardown, not each local binding.
- [ ] Run `cargo test two_runtime_ui_ -- --nocapture`, `cargo test script::ui::tests`, and `cargo test script_ui_packet_payloads_match_shared_fixture`; expect pass. Preserve current defaults, layout, size, hidden behavior, measurement correlation and whole-field equality. Review the UI storage diff.

## Task 4: Add the background runtime service and registration queue

**Files:** Create `client/src/script/background.rs`; modify `mod.rs`, `session.rs`, `bindings.rs`, `lifecycle.rs`; test background inline tests.

**Interfaces:**

```rust
// background.rs: descriptors and queue records contain no QuickJS values.
pub(super) enum BackgroundTarget {
    Daemon(String),
    Ui { name: String, instance: Uuid, event: ScriptUiEventKind },
}
pub(super) struct BackgroundRegistry { /* shared Rust registrations and notifications */ }
impl BackgroundRegistry {
    pub(super) fn new(session: SessionControl) -> Self;
    pub(super) fn register(&self, target: BackgroundTarget, function: FunctionDescriptor) -> Uuid;
    pub(super) fn unregister(&self, target: &BackgroundTarget);
    pub(super) fn is_current(&self, registration: Uuid) -> bool;
}
pub(super) struct ScriptBackground { /* local roots, context, then runtime */ }
impl ScriptBackground {
    pub(super) async fn new(modules: ScriptModules, registry: Rc<BackgroundRegistry>,
        ui: Rc<ScriptUiState>, connection: WsConnection, session: SessionControl,
        ownership: Rc<ScreenOwnershipState>) -> Result<Self, String>;
    pub(super) async fn run(&self, controls: HostControls);
}
// session.rs: one retained future per session, polled by lifecycle.
impl ScriptSession {
    pub(super) fn drive_background(&self, controls: HostControls)
        -> impl std::future::Future<Output = ()> + '_;
}
// bindings.rs: update main and background callers together.
pub(super) fn create_rev<'js>(ctx: Ctx<'js>, connection: WsConnection,
    controls: HostControls, parse: Function<'js>, freeze: Function<'js>,
    session: SessionControl, screen_ownership: Rc<ScreenOwnershipState>,
    ui: Rc<ScriptUiBindings>, transfer: Rc<FunctionTransfer>,
    background: Rc<BackgroundRegistry>) -> rquickjs::Result<Object<'js>>;
```

- [ ] Write `two_runtime_background_environment_and_ownership`: evaluate a dependency initializer using `rev`, compare class/module state with main, and register more background work from an async continuation. Write `two_runtime_background_failure_isolation` for import, reconstruction, sync execution and rejected-promise errors.

  ```rust
  assert_eq!(runtime_count, 2);
  assert!(background_initializer_saw_rev);
  assert!(!main_initializer_saw_rev);
  assert!(background_rev_identity_stable);
  assert_eq!(background_dependency_initializations, 1);
  assert_eq!(main_dependency_mutation_seen_in_background, false);
  assert_eq!(error_reports_per_failed_registration, 1);
  assert!(unrelated_background_work_completed);
  ```

- [ ] Run `cargo test two_runtime_background -- --nocapture`; expect failure because no background runtime exists.
- [ ] Implement `ScriptBackground::new/run` and `BackgroundRegistry` with Rust queue/status records and notifications. The service owns installation futures, installed roots and in-flight invocations; poll them together with its runtime driver. Do not await one callback/daemon to completion before accepting other work. Recheck registration/session validity after every awaited installation before activation or start.
- [ ] Wire one background service future into lifecycle immediately after session creation, including while `afterLoad` or another main hook awaits. Create its local console/intrinsics/UI/ownership proxies and stable `rev` before processing installations. Update `create_rev` to consume local `ScriptUiBindings`, `FunctionTransfer`, and shared `BackgroundRegistry`; preserve its existing host services. Main bindings remain main-local. Host callbacks may strongly capture Rust-only stores; use weak access to adapters that own persistent JS roots. Shared ownership tokens contain Rust state, never cross-runtime JS roots.
- [ ] Make background-environment initialization failure log its cause and terminate the session through existing controls. Installation/execution failures remain isolated as specified. A completed background service must not leave lifecycle spinning on a ready future.
- [ ] Run `cargo test two_runtime_background -- --nocapture`, `cargo test ownership`, and `cargo test script::tests`; expect pass. Keep the service alive through cadence delays and Pause, and release it before session teardown. Review the runtime ownership diff.

## Task 5: Expose daemon registration and soft retirement

**Files:** Modify `client/src/script/background.rs`, `bindings.rs`; test background inline tests and `tests.rs`.

**Interfaces:**

- Add `BackgroundRegistry::contains_daemon(&self, name: &str) -> bool` and `daemon_names(&self) -> Vec<String>` for property membership/enumeration.
- Bind JS `rev.daemon[name] = function` and `delete rev.daemon[name]` in both runtimes through Task 4 registration operations. No special receiver or cancellation token is introduced.
- The service retains each active daemon's name and registration UUID until its own returned promise settles, even after registry deletion. Only the latest still-current replacement may start afterward.

- [ ] Write `two_runtime_daemon_runs_once_and_uses_shared_global`, `two_runtime_daemon_retirement_preserves_name_order`, and `two_runtime_daemon_pending_install_is_superseded`. Cover same-function reassignment, delete/re-add, multiple replacements, rejection, a never-settling old run, and a detached child outliving its parent.

  ```rust
  assert!(daemon_receiver_is_undefined);
  assert_eq!(normal_return_start_count, 1);
  assert!(!name_registered_immediately_after_delete);
  assert_eq!(starts_before_old_settlement, vec!["old"]);
  assert_eq!(starts_after_old_settlement, vec!["old", "latest"]);
  assert_eq!(starts_while_old_never_settles, vec!["old"]);
  assert_eq!(deleted_pending_generation_starts, 0);
  assert!(detached_child_continues_after_parent_settles);
  assert_eq!(cross_runtime_global_value, serde_json::json!(42));
  ```

- [ ] Run `cargo test two_runtime_daemon -- --nocapture`; expect failure because `rev.daemon` is absent.
- [ ] Add the daemon proxy to `create_rev`, using captured local function metadata and Rust operations only. Validate nonempty string names and callable assignments; preserve normal missing-name deletion. Property reads fail with the reviewed error. Assigning always creates a new registration, including the same function.
- [ ] Implement service start ordering: current descriptors may install while an old run is active, but invocation waits for old settlement. Keep retired-run bookkeeping across delete/re-add; coalesce superseded waiting replacements. Call with `this === undefined`; log rejection once, mark that run settled, and continue servicing others. Dropping an observer must never be presented as cancellation.
- [ ] Run `cargo test two_runtime_daemon -- --nocapture`; expect pass. Review the registry/start-order diff and confirm no silent overlap of daemon root invocations with the same name.

## Task 6: Install and dispatch every UI handler in background

**Files:** Modify `client/src/script/ui.rs`, `ui.js`, `background.rs`, `session.rs`, `bindings.rs`; test UI/background inline tests and bridge regression tests.

**Interfaces:**

- Extend `ScriptUiBindings::new` from Task 3 with trailing `transfer: Rc<FunctionTransfer>, background: Rc<BackgroundRegistry>` arguments.
- Expose `setOnClick(callback): void`, `setOnHover(callback): void`, and `setOnLeave(callback): void` on live element proxies. Each setter accepts `((this: element) => void | Promise<void>) | null` and returns no callback. Each method retains its element instance identity.
- Add `ScriptUiState::complete_handler_install(&self, name: &str, instance: Uuid, event: ScriptUiEventKind, registration: Uuid, installed: bool) -> bool`; return false for obsolete work, publish event activation only for a current successful installation.
- Add `BackgroundRegistry::dispatch_ui(&self, event: ScriptUiEvent, current: impl Fn() -> bool + 'static) -> impl Future<Output = Result<(), String>> + '_`; enqueue Rust work and return completion through a Rust response channel.
- Preserve `ScriptSession::dispatch_ui_event_if_current(&self, event: ScriptUiEvent, controls: HostControls, current: impl Fn() -> bool + 'static) -> Result<(), String>` as an async method, routing through the queue. It must never enter main. Keep `controls` for caller compatibility; the retained background service uses the session's controls.

- [ ] Write `two_runtime_ui_handler_install_races`, `two_runtime_ui_receiver_and_overlap`, `two_runtime_ui_measurement_invalidation`, and `two_runtime_ui_callback_methods_only`. Delay installation, replace/clear with the setter, release the old install, retry by passing the failed function again, and click through existing bridge test events. Cover all three event kinds, registration from both runtimes, `null` on empty/pending/active slots, rejection of other argument values, and stale cached setters.

  ```rust
  assert!(events_before_install.is_empty());
  assert_eq!(obsolete_handler_calls, 0);
  assert_eq!(same_function_pending_registration, same_function_active_registration);
  assert_ne!(failed_registration, retried_registration);
  assert!(retried_handler_is_active);
  assert!(receiver_equals_background_element);
  assert_eq!(overlapping_calls_started_before_release, 2);
  assert_eq!(main_original_function_calls, 0);
  assert!(old_measurement_rejected && old_proxy_write_rejected);
  assert_eq!(background_hidden_element_width, 120.0);
  assert!(callback_property_reads_are_undefined);
  assert!(!callback_properties_in_membership_or_enumeration);
  assert!(callback_property_descriptors_are_absent);
  assert!(legacy_callback_definition_and_assignment_rejected);
  assert!(setter_returns_undefined_for_function_and_null);
  assert!(clearing_empty_slot_is_noop);
  assert!(null_invalidates_pending_and_active_handlers);
  assert!(arguments_other_than_function_or_null_rejected);
  assert!(stale_setter_rejected);
  ```

- [ ] Run `cargo test two_runtime_ui_ -- --nocapture`; expect handler/race/receiver assertions to fail before rerouting.
- [ ] Replace callback fields with three read-only setters. Definitions contain only visual fields/states; reject legacy callback fields in definitions and assignments, and expose no handler through reads, membership, enumeration or descriptors. Function arguments transfer/queue callbacks from either runtime; `null` synchronously invalidates pending/active registration, with an empty slot a no-op. Reject other values, including omitted/undefined arguments. Deleting an absent legacy property does not unregister the hidden callback. Whole-definition replacement starts with no callbacks. All three setters reject stale instances/stopped sessions.
- [ ] Retain runtime-local function identity for repeated setter calls. A matching pending/active registration is a no-op; a failed matching function retries with a new UUID. Distinct functions with equal text stay distinct. Setter calls return immediately without a function or installation promise.
- [ ] Implement background installation/dispatch. Invalidate old versions immediately; activate/publish only after successful install. Recheck session, instance, event version, connection generation, capture epoch and the supplied `current` predicate immediately before calling the handler. Invoke ordinary functions with Task 3's background element proxy; arrows keep lexical `this`. Preserve overlap and existing errors; removing a handler does not cancel calls already running.
- [ ] Route both runtimes' measurements through the same authoritative visual revision and revalidate the instance after the response. Keep state writes outside visual revision changes. Replace old tests expecting callback `this === undefined` and callback main-local closures with the new contract; retain capture/input and stale-event regression tests.
- [ ] Run `cargo test two_runtime_ui_ -- --nocapture`, `cargo test script::ui::tests`, `cargo test script_ui`, and `cargo test capture`; expect pass. Review the event activation and stale-proxy paths.

## Task 7: Gate all main execution and acknowledge Pause precisely

**Files:** Modify `client/src/script/control.rs`, `lifecycle.rs`, `session.rs`, `bindings.rs`; test `tests.rs`, UI tests, `ownership.rs` and control inline tests. Inspect `app/pause.rs` as the request/input source; retain its monotonic request semantics.

**Interfaces:**

```rust
impl SessionControl {
    pub(super) fn is_paused(&self) -> bool;
    pub(super) fn subscribe_paused(&self) -> tokio::sync::watch::Receiver<bool>;
    pub(super) fn acknowledge_pause(&self, paused: bool);
}
```

Retain `ScriptSession::drive`, `invoke_classified`, and existing hook method signatures. `ActionGate` remains the immediate request/input gate; `SessionControl` exposes the acknowledged script state to `rev.paused` and `ensureRunning`. Lifecycle owns request-version checks before acknowledging a transition.

- [ ] Change `pause_preserves_in_flight_default_and_blocks_next_call` to require zero main progress after acknowledgment. Add `two_runtime_pause_detached_timer_and_promise`, `two_runtime_pause_hooks_and_connections`, and `two_runtime_sleep_deadline_begins_at_call`. Use ready/not-ready barriers, locals, a detached loop, rapidly superseded requests, multiple connection transitions and both clock-expired and delayed-first-poll sleeps.

  ```rust
  assert_eq!(main_steps_after_ack, main_steps_at_ack);
  assert!(background_steps_after_ack > background_steps_at_ack);
  assert_eq!(resumed_local_value, 42);
  assert_eq!(resumed_invocation_start_count, 1);
  assert_eq!(deferred_connection_hooks, vec!["disconnect", "connect", "disconnect"]);
  assert!(!stale_pause_request_acknowledged);
  assert!(before_pause_saw_running && after_resume_saw_running);
  assert!(expired_sleep_ready_on_first_resumed_poll);
  assert!(sleep_deadline_was_created_before_first_future_poll);
  ```

- [ ] Run `cargo test two_runtime_pause -- --nocapture`, `cargo test pause_preserves_in_flight_default_and_blocks_next_call`, and `cargo test two_runtime_sleep`; expect the new freeze/deadline assertions to fail.
- [ ] Add session acknowledgment state and wire `rev.paused`/`ensureRunning` to it. Preserve subscribe/check/wait/recheck and terminal rejection. Capture `tokio::time::Instant` deadline in the synchronous `rev.sleep` binding call and use `sleep_until` inside its future; preserve integer validation and existing overflow behavior.
- [ ] Refactor lifecycle's existing branches into one scheduling owner that retains main invocation, main driver and main hook futures. Poll none of them after acknowledgment. Handle background service, host connection state, UI events, commands and terminal signals while paused. Check requests before another main turn; do not gate only new default calls. Preserve the 50 ms default cadence and current connection eligibility rule. Update checkpoint/ownership tests that currently toggle only `ActionGate` to exercise acknowledged session state. Retain ownership reservations and FIFO ordering while Pause unlocks player input; confirm a token created in either context releases the shared Rust reservation without crossing runtime boundaries.
- [ ] Schedule hooks as retained futures alongside background work. Pause stops new default starts, finishes `beforePause`, rechecks the request version, then acknowledges/publishes Paused. Resume clears acknowledgment, runs `afterResume`, replays session-tagged connection transitions FIFO, then permits new defaults. Hook failures log and advance the transition; termination drops queued hooks. Audit every direct `run_*` and `async_with` call, including startup/reload, so no paused branch enters main accidentally.
- [ ] Run `cargo test two_runtime_pause -- --nocapture`, `cargo test two_runtime_sleep`, `cargo test pause_`, `cargo test connection_hooks`, `cargo test ensure_running`, and `cargo test ownership`; expect pass. Update old expectations that detached main work runs during Pause. Review all main polling and acknowledgment paths.

## Task 8: Verify interruption, teardown and actual poll responsiveness

**Files:** Modify `client/src/script/worker_tests.rs`; adjust `session.rs`, `background.rs`, `lifecycle.rs` only where the tests expose an ownership/termination defect.

**Interfaces:** Preserve `SessionControl::stop/is_stopped/subscribe_stopped`, `ScriptSession::terminate`, and the existing `ScriptWorker` control interface. Both runtimes install interrupt handlers using the same terminal state.

- [ ] Extend `stop_interrupts_infinite_javascript` / `interruption_child` with background dependency initialization, daemon synchronous loop, daemon ready-promise chain, callback ready-promise chain, background self-stop, and Stop during pending installation. Add `two_runtime_teardown_invalidates_old_work` for reload while callback measurement/daemon work is pending.

  ```rust
  assert!(child_status.success());
  assert!(output.contains("replacement ran on the same worker"));
  assert!(snapshots.borrow().session_id.is_none());
  assert_eq!(obsolete_session_host_writes, 0);
  assert!(new_session_states_are_empty);
  assert_eq!(global_value_after_reload, global_value_before_reload);
  ```

- [ ] Run `cargo test two_runtime_teardown -- --nocapture` and the existing exact interruption test command below; expect new cases to expose missing stop/cleanup paths before their fixes.
- [ ] Apply shared interrupt state before either runtime evaluates user code. On termination drop lifecycle invocation/hook/service futures, pending dispatch responses and installed function roots before their contexts/runtimes. Close shared UI/ownership once; reject further host writes. Verify both runtimes actually drop without retained-root assertions, including registered callbacks never invoked. Never rely on JS `finally` or run `beforeStop` on terminal paths that currently skip it.
- [ ] Add subprocess characterization cases `pause_pending_timer`, `pause_ready_chain`, and `pause_sync_segment` to the existing watchdog harness. Record request-to-ack time and prove no main progress after any acknowledgment. Bound ready-chain/sync experiments, then Stop from the independent control runtime. A case that cannot acknowledge before Stop records that limitation; it must not pretend Pause succeeded. Background/UI progress during a blocked worker is likewise not guaranteed. Keep the 10-second parent watchdog and kill/reap only the test child.
- [ ] Run `cargo test script::worker_tests::stop_interrupts_infinite_javascript -- --exact --nocapture` and `cargo test two_runtime_teardown -- --nocapture`; expect successful termination/replacement for all cases. Record actual characterization results in the plan's execution notes before claiming a latency guarantee. Review destruction order and both interrupt registrations.

## Task 9: Migrate public types, scripts and behavior documentation

**Files:** Modify `scripts/rev.d.ts`, `scripts/ui_demo.ts`, `scripts/lib/history.ts`, `docs/js_api_guide.md`, historical `docs/superpowers/specs/2026-09-28-script-ui-design.md`; add migration tests in `client/src/script/tests.rs`. Update `__explain.md` only to record reviewed policy choices and measured limitations.

**Interfaces:**

- Add `RevDaemon = (this: void) => void | Promise<void>` and `Rev.daemon: Record<string, RevDaemon | undefined>`; document assignment-only reads and retirement.
- Remove `onClick`, `onHover`, and `onLeave` from `RevUiElement`. Add `states?: Record<string, RevJsonValue>` and `RevUiCallback = (this: RevUiElement & { states: Record<string, RevJsonValue> }) => void | Promise<void>`.
- Add readonly optional host-method fields `setOnClick?: (callback: RevUiCallback | null) => void`, `setOnHover?: (callback: RevUiCallback | null) => void`, and `setOnLeave?: (callback: RevUiCallback | null) => void`. Live elements always provide them; optional declarations let definitions omit them, matching existing measurement method typing. Document that definitions cannot supply/override host methods. Live state maps likewise always exist.
- Keep `UnityHistory.updateUI()` and existing main callers. Add no cross-runtime class-instance transport or broader history-cache refactor.

- [ ] Add `two_runtime_migrated_history_ui_uses_element_states` and `two_runtime_migrated_demo_monitor_survives_pause`. Exercise actual migrated callback source through the transfer path with fake host services; isolate temporary fixture imports from live files. History receives two successive main updates around a click; demo runs main/detached work plus its daemon and handlers through Pause.

  ```rust
  assert_eq!(history_after_first_click, collapsed_latest_run);
  assert_eq!(history_after_next_main_update, collapsed_new_run);
  assert_eq!(history_after_second_click, expanded_new_run);
  assert_eq!(demo_main_count_after_pause, demo_main_count_at_pause);
  assert!(demo_monitor_count_after_pause > demo_monitor_count_at_pause);
  assert_eq!(demo_click_count_after_pause, demo_click_count_at_pause + 1);
  assert_eq!(automatic_game_actions, 0);
  ```

- [ ] Run `cargo test two_runtime_migrated -- --nocapture`; expect failure from existing captured locals and detached-main monitor behavior.
- [ ] Update declarations and migrate `ui_demo.ts`: create elements with visual attributes/states, then register ordinary functions through `setOnClick`, `setOnHover`, and `setOnLeave`. Counters shared with daemon go in `rev.global`, click count/control path go in element states, the intended monitor becomes `rev.daemon.monitor`, and maintenance remains ordinary main work. Keep the monitor bounded and game actions opt-in. Narrow JSON values before arithmetic or passing them to typed methods.
- [ ] Migrate `history.ts` without making background depend on its stale module cache: `updateUI()` writes preformatted expanded/collapsed text into the element's states and chooses using `states.expanded`; register a function through `setOnClick` after element creation to toggle that state and assign matching text through `this`. Remove the old module-level `expanded` flag. Persisted history and formatting remain in the existing main `UnityHistory` methods.
- [ ] Update the API guide's runtime availability, imports, pause/hooks, daemon, UI receiver/states, errors and examples. Replace callback definition/assignment examples with setter calls and callback deletion with setter calls passing `null`. Replace examples that imply ordinary detached work runs while paused. Document snapshot imports versus fresh dynamic imports, missing closures, natural transfer failures, soft retirement and the absence of callback properties. Put normative behavior in the guide/spec once; add a short supersession link to the historical UI design rather than rewriting its history.
- [ ] Run `cargo test two_runtime_migrated -- --nocapture`, then `cargo test` and `cargo check --all-targets` once after the integrated change. Run `tsc --project scripts/tsconfig.json --noEmit` from the repository root only if a local compiler is available; it was not on PATH during planning. Report compiler absence or pre-existing diagnostics accurately, without downloading tooling. Finish with `git diff --check` from the repository root and direct whitespace checks on ignored changed files.

## Completion and live verification

- [ ] Self-review spec sections 1–12 against Tasks 1–9, with particular attention to the five Review Focus cases. Record commands actually run and their results; do not mark future checks as completed.
- [ ] Present the resulting source changes and automated evidence. Keep installed-game proof separate: this plan does not authorize installation, restart, or game actions.
- [ ] When live verification is separately requested, use the migrated demo: pause a pending main invocation, click/measure/update UI while paused, observe the daemon, resume the same invocation, replace/delete an element, then Stop/reload. Report what was observed in the installed client/plugin, not just build success.

### Plan self-review

The planning pass mapped ownership/scheduling to Tasks 4, 7 and 8; transfer/imports/errors to Tasks 1, 2 and 4; daemons to Task 5; UI/state/measurement rules to Tasks 3 and 6; and declarations/migrations/docs to Task 9. Shared interfaces use one descriptor type, registration UUIDs, a Rust-only registry and context-local bindings. All five Review Focus cases have explicit tests. No implementation, build, test, installation or Git mutation was performed while writing this plan.
