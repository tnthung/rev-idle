# Window-Relative Automation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add process-based Revolution Idle window discovery, client-relative resize and click APIs, verified focus-before-click, and an immediate global F8 pause gate.

**Architecture:** A focused `window.rs` module owns Win32 process/window discovery and client-coordinate operations. A dedicated `hotkey.rs` OS thread owns `RegisterHotKey` and its message loop, closes an atomic action gate immediately, and queues an idempotent requested pause state for the existing script lifecycle. QuickJS and Enigo remain on the current-thread Tokio script task behind narrow test seams.

**Tech Stack:** Rust 2024, Tokio 1.53.1 current-thread runtime, rquickjs 0.12.2, Enigo 0.6.1, windows 0.61.3 Win32 bindings.

**Spec:** `docs/superpowers/specs/2026-09-01-window-relative-automation-design.md`

## Global Constraints

- Match visible top-level windows by the case-insensitive executable filename `Revolution Idle.exe`; never depend on the window title.
- Require exactly one matching window. Zero or multiple matches must block window and mouse actions.
- `rev.resize(width, height)` targets the drawable client area and accepts only finite, positive, signed 32-bit integer dimensions.
- `rev.click(x, y, button)` uses client-relative coordinates and permits only `0 <= x < clientWidth` and `0 <= y < clientHeight`.
- Focus and verify the game window before every click. Focus failure must block the click.
- F8 uses `RegisterHotKey` with `MOD_NOREPEAT`; do not add a keyboard hook or Enigo listener.
- F8 toggles the atomic Rust action gate before sending the requested lifecycle state. Host operations check the gate before Win32 work and again before Enigo input.
- Console pause/resume and load/reload/stop keep the same gate synchronized with lifecycle state.
- Do not run source-rewriting formatters. Use targeted edits, `cargo test`, `cargo check`, and `git diff --check`.
- Do not mutate Git state or create commits unless the user explicitly authorizes it. Commit steps below are approval gates, not automatic actions.

## File Structure

- Create `client/src/window.rs`: pure bounds/size helpers, process-based HWND discovery, client resizing, focus verification, and client-to-screen translation.
- Create `client/src/hotkey.rs`: F8 registration, message-loop worker, immediate gate toggle, requested-pause delivery, and deterministic shutdown.
- Modify `client/src/script.rs`: shared host controls, `rev.resize`, relative `rev.click`, gate checks, and requested-pause lifecycle handling.
- Modify `client/src/console.rs`: add the internal `ScriptCommand::SetPaused(bool)` variant without changing textual console syntax.
- Modify `client/src/main.rs`: create the shared gate, start the hotkey worker, pass the gate to the script runner, and shut down the worker.
- Modify `client/Cargo.toml` and `client/Cargo.lock`: add the direct Windows-only `windows` 0.61.3 feature set.

---

### Task 1: Win32 Game Window Controller

**Files:**
- Create: `client/src/window.rs`
- Modify: `client/src/main.rs:1-3`
- Modify: `client/Cargo.toml:6-12`
- Modify: `client/Cargo.lock`
- Test: inline tests in `client/src/window.rs`

**Interfaces:**
- Consumes: Win32 APIs from `windows` 0.61.3.
- Produces: `pub trait WindowControl`, `pub struct Win32WindowControl`, `WindowControl::resize_client(width: i32, height: i32) -> Result<(), String>`, and `WindowControl::focus_and_translate(x: i32, y: i32) -> Result<(i32, i32), String>`.

- [ ] **Step 1: Add failing pure-helper tests before production definitions**

Create `client/src/window.rs` with a `#[cfg(test)]` module that states the desired selection, geometry, boundary, and overflow behavior. Add `mod window;` to `client/src/main.rs` so Cargo compiles the new tests.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn matches_only_the_game_executable_name_case_insensitively() {
        assert!(is_game_executable(Path::new(
            r#"C:\Games\Revolution Idle.exe"#,
        )));
        assert!(is_game_executable(Path::new(
            r#"C:\Games\REVOLUTION IDLE.EXE"#,
        )));
        assert!(!is_game_executable(Path::new(
            r#"C:\Games\Revolution Idle Launcher.exe"#,
        )));
    }

    #[test]
    fn requires_exactly_one_matching_window() {
        assert_eq!(require_exactly_one(Vec::<u32>::new()).unwrap_err(),
            "Revolution Idle window not found");
        assert_eq!(require_exactly_one(vec![17]).unwrap(), 17);
        assert_eq!(require_exactly_one(vec![17, 23]).unwrap_err(),
            "multiple Revolution Idle windows found");
    }

    #[test]
    fn translates_only_points_inside_the_client_area() {
        let geometry = ClientGeometry {
            origin_x: -1200,
            origin_y: 40,
            width: 1280,
            height: 720,
        };
        assert_eq!(geometry.translate(0, 0).unwrap(), (-1200, 40));
        assert_eq!(geometry.translate(1279, 719).unwrap(), (79, 759));
        for point in [(-1, 0), (0, -1), (1280, 0), (0, 720)] {
            assert!(geometry.translate(point.0, point.1).is_err());
        }
    }

    #[test]
    fn calculates_outer_size_from_the_current_frame_and_checks_overflow() {
        assert_eq!(outer_size_for_client(1296, 759, 1280, 720, 800, 600).unwrap(),
            (816, 639));
        assert!(outer_size_for_client(
            i32::MAX, i32::MAX, 1, 1, i32::MAX, i32::MAX,
        ).is_err());
    }
}
```

- [ ] **Step 2: Run the focused tests and verify the expected red state**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml window::tests
```

Expected: compilation fails because `is_game_executable`, `require_exactly_one`, `ClientGeometry`, and `outer_size_for_client` do not exist.

- [ ] **Step 3: Add the direct Windows dependency and minimal pure helpers**

Add this target dependency so non-Windows dependency resolution remains isolated:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.61.3", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_System_Threading",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
] }
```

Define these helper shapes in `window.rs` and implement them with checked arithmetic and exact exclusive-edge validation:

```rust
const GAME_EXECUTABLE: &str = "Revolution Idle.exe";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ClientGeometry {
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
}

impl ClientGeometry {
    fn translate(self, x: i32, y: i32) -> Result<(i32, i32), String>;
}

fn is_game_executable(path: &std::path::Path) -> bool;
fn require_exactly_one<T>(matches: Vec<T>) -> Result<T, String>;
fn outer_size_for_client(
    outer_width: i32,
    outer_height: i32,
    client_width: i32,
    client_height: i32,
    target_width: i32,
    target_height: i32,
) -> Result<(i32, i32), String>;
```

Use `Path::file_name()` plus `eq_ignore_ascii_case`, `checked_sub`, and `checked_add`. Error messages must distinguish missing windows, multiple windows, out-of-client coordinates, and arithmetic overflow.

- [ ] **Step 4: Run the pure-helper tests and verify green**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml window::tests
```

Expected: all four new tests pass.

- [ ] **Step 5: Add failing contract tests for the window-control seam**

Extend the test module with a compile-time contract check and geometry assertions that the production controller must use:

```rust
fn assert_window_control<T: WindowControl>() {}

#[test]
fn win32_controller_implements_window_control() {
    assert_window_control::<Win32WindowControl>();
}
```

Run the same focused test command. Expected: compilation fails because the trait and controller are not defined.

- [ ] **Step 6: Implement process discovery and Win32 window operations**

Add the exact public seam:

```rust
pub trait WindowControl {
    fn resize_client(&self, width: i32, height: i32) -> Result<(), String>;
    fn focus_and_translate(&self, x: i32, y: i32)
        -> Result<(i32, i32), String>;
}

#[derive(Default)]
pub struct Win32WindowControl;
```

Implement `Win32WindowControl` using these operations in this order:

1. `EnumWindows` collects only `IsWindowVisible(hwnd).as_bool()` handles.
2. `GetWindowThreadProcessId` yields each owner PID.
3. `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)` and `QueryFullProcessImageNameW` yield the executable path; always close successful process handles with `CloseHandle`.
4. Filter with `is_game_executable`, then call `require_exactly_one`.
5. For resize, reject non-positive dimensions, restore with `ShowWindow(hwnd, SW_RESTORE)`, read `GetWindowRect` and `GetClientRect`, calculate the new outer size with `outer_size_for_client`, and call `SetWindowPos` with `SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE`.
6. Read `GetClientRect` again and require exact requested dimensions.
7. For click translation, require `SetForegroundWindow(hwnd).as_bool()`, then require `GetForegroundWindow() == hwnd`, read `GetClientRect`, call `ClientToScreen` for `(0, 0)`, and pass the resulting `ClientGeometry` to `translate`.

Wrap each failing Win32 result in an operation-specific `String`; do not let raw `windows::core::Error` values cross the trait boundary.

- [ ] **Step 7: Run the window tests and compiler check**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml window::tests
cargo check --manifest-path client/Cargo.toml
```

Expected: focused tests pass and the client compiles with the direct Win32 dependency.

- [ ] **Step 8: Commit only if Git mutation is explicitly authorized**

```powershell
git add -- client/Cargo.toml client/Cargo.lock client/src/main.rs client/src/window.rs
git commit -m "feat: control Revolution Idle window"
```

If authorization is absent, leave the verified changes uncommitted and continue.

---

### Task 2: Relative Click and Resize JavaScript Host APIs

**Files:**
- Modify: `client/src/script.rs:18-239`
- Test: `client/src/script.rs:348-665`

**Interfaces:**
- Consumes: `WindowControl` and `Win32WindowControl` from Task 1.
- Produces: `HostControls`, `SharedWindow`, `ScriptSession::invoke(state, controls)`, `rev.resize(width, height)`, and relative `rev.click(x, y, button)`.

- [ ] **Step 1: Refactor test setup without changing behavior**

Introduce a shared host-control container while keeping all existing tests green:

```rust
type SharedMouse = Rc<RefCell<dyn MouseInput>>;
type SharedWindow = Rc<dyn WindowControl>;

#[derive(Clone)]
struct HostControls {
    mouse: SharedMouse,
    window: SharedWindow,
    actions_paused: Arc<AtomicBool>,
}
```

Add an identity `FakeWindow` to tests whose `focus_and_translate(x, y)` returns `(x, y)` and whose resize succeeds. Add a helper that constructs `HostControls` from the existing fake mouse. Update `ScriptSession::invoke` and `run_with_mouse` call sites to accept controls without changing click results.

Run:

```powershell
cargo test --manifest-path client/Cargo.toml script::tests
```

Expected: all pre-existing script tests pass before new behavior tests are added.

- [ ] **Step 2: Write failing JavaScript host tests**

Add a fake window and shared ordered event log:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
enum HostEvent {
    Resize(i32, i32),
    FocusAndTranslate(i32, i32),
    Click(i32, i32, Button),
}
```

Add tests with these required assertions:

```rust
#[tokio::test(flavor = "current_thread")]
async fn resize_and_relative_click_use_the_window_controller_in_order() {
    let session = ScriptSession::new(
        r#"((rev) => { rev.resize(1280, 720); rev.click(10, 20, "right"); })"#,
    ).await.unwrap();
    let controls = recording_controls((1010, 2020));

    session.invoke(State::default(), controls.clone()).await.unwrap();

    assert_eq!(recorded_events(&controls), vec![
        HostEvent::Resize(1280, 720),
        HostEvent::FocusAndTranslate(10, 20),
        HostEvent::Click(1010, 2020, Button::Right),
    ]);
}

#[tokio::test(flavor = "current_thread")]
async fn resize_rejects_invalid_dimensions_without_window_work() {
    for source in [
        r#"((rev) => rev.resize(0, 720))"#,
        r#"((rev) => rev.resize(-1, 720))"#,
        r#"((rev) => rev.resize(1.5, 720))"#,
        r#"((rev) => rev.resize(Infinity, 720))"#,
    ] {
        let controls = recording_controls((0, 0));
        let session = ScriptSession::new(source).await.unwrap();
        assert!(session.invoke(State::default(), controls.clone()).await.is_err());
        assert!(recorded_events(&controls).is_empty());
    }
}
```

Also add one test where `focus_and_translate` returns an outside-coordinate error and assert that no `Click` event is recorded.

- [ ] **Step 3: Run the focused tests and verify red**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml script::tests::resize_and_relative_click_use_the_window_controller_in_order
cargo test --manifest-path client/Cargo.toml script::tests::resize_rejects_invalid_dimensions_without_window_work
```

Expected: the first test fails because `rev.resize` is absent; the second fails for the same missing host API rather than a test setup error.

- [ ] **Step 4: Implement minimal resize and relative-click bindings**

Change `ScriptSession::invoke` to receive one cloned `HostControls`. Bind `rev.resize` with the same finite-integer style already used by `rev.click`, plus strict `width > 0` and `height > 0` checks. Call `controls.window.resize_client(width, height)` and convert any `String` into an rquickjs host error.

Change `rev.click` so it:

```rust
ensure_actions_running(&controls.actions_paused)?;
let (screen_x, screen_y) = controls
    .window
    .focus_and_translate(x as i32, y as i32)
    .map_err(host_error)?;
ensure_actions_running(&controls.actions_paused)?;
controls
    .mouse
    .borrow_mut()
    .click_at(screen_x, screen_y, button)
    .map_err(host_error)
```

Preserve existing button parsing exactly. At this stage, construct production controls in `script::run` from `Enigo`, `Win32WindowControl`, and a local `Arc::new(AtomicBool::new(false))`. Tests and lifecycle code use `run_with_controls` to inject a shared gate. Task 5 changes the public `script::run` signature and replaces this temporary local gate with the one shared with the hotkey worker.

- [ ] **Step 5: Verify the new APIs and all script regressions**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml script::tests
```

Expected: all old and new script tests pass; existing fake-window identity behavior preserves tests unrelated to window positioning.

- [ ] **Step 6: Commit only if Git mutation is explicitly authorized**

```powershell
git add -- client/src/script.rs
git commit -m "feat: add window-relative script actions"
```

If authorization is absent, leave the verified changes uncommitted and continue.

---

### Task 3: Immediate Action Gate and Lifecycle Synchronization

**Files:**
- Modify: `client/src/console.rs:7-16`
- Modify: `client/src/script.rs:241-346`
- Test: inline lifecycle tests in `client/src/script.rs`

**Interfaces:**
- Consumes: `HostControls.actions_paused: Arc<AtomicBool>` from Task 2.
- Produces: `ScriptCommand::SetPaused(bool)` and lifecycle handling that keeps console and F8 state idempotently synchronized.

- [ ] **Step 1: Write a failing active-invocation gate test**

Add a test script that sleeps before attempting a window action. While it sleeps, close the atomic gate and send the requested paused state:

```rust
#[tokio::test(flavor = "current_thread")]
async fn requested_pause_gates_the_active_turn_then_pauses_the_loop() {
    // Write a temporary script equivalent to:
    // async (rev) => { await rev.sleep(30); rev.click(1, 1); }
    // Start run_with_controls, wait 5 ms, then:
    controls.actions_paused.store(true, Ordering::Release);
    command_tx.send(ScriptCommand::SetPaused(true)).await.unwrap();

    // The click after sleep must be rejected, and no later invocation may run.
    assert!(tokio::time::timeout(
        Duration::from_millis(80),
        event_rx.recv(),
    ).await.is_err());

    // Request resume and observe the next invocation's click.
    controls.actions_paused.store(false, Ordering::Release);
    command_tx.send(ScriptCommand::SetPaused(false)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
        .await.unwrap().unwrap();
}
```

Add a second test that interleaves console `Pause`, F8-style `SetPaused(false)`, console `Resume`, load/reload, and stop. Assert after each applied command that the gate equals the lifecycle's requested state; stopped, load, and reload leave the gate open.

- [ ] **Step 2: Run the lifecycle tests and verify red**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml script::tests::requested_pause_gates_the_active_turn_then_pauses_the_loop
```

Expected: compilation fails because `ScriptCommand::SetPaused(bool)` does not exist.

- [ ] **Step 3: Add idempotent requested-pause commands and gate synchronization**

Add this internal-only enum variant without adding it to `parse_command`:

```rust
pub enum ScriptCommand {
    Load(PathBuf),
    Reload,
    Pause,
    Resume,
    Stop,
    SetPaused(bool),
}
```

At command boundaries in `run_with_controls`:

- `Pause` and `SetPaused(true)` store `true` into the gate and set `paused = true` when a session exists.
- `Resume` and `SetPaused(false)` store `false` into the gate and set `paused = false` when a session exists.
- A redundant requested state is a no-op with the same existing-style status message.
- `Load`, successful or failed `Reload`, and `Stop` store `false` into the gate because those commands reset or remove the active lifecycle.
- `SetPaused(_)` while stopped leaves the session stopped, restores the gate to `false`, and reports that no script is running.

Use `Ordering::Acquire` for host gate reads and `Ordering::Release` for lifecycle writes. The hotkey toggle in Task 4 uses `Ordering::AcqRel`.

- [ ] **Step 4: Run lifecycle and console tests**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml script::tests::requested_pause_gates_the_active_turn_then_pauses_the_loop
cargo test --manifest-path client/Cargo.toml script::tests::console_commands_control_context_lifetime
cargo test --manifest-path client/Cargo.toml console::tests
```

Expected: all focused tests pass, and `SetPaused` remains unavailable as typed console syntax.

- [ ] **Step 5: Commit only if Git mutation is explicitly authorized**

```powershell
git add -- client/src/console.rs client/src/script.rs
git commit -m "feat: synchronize immediate automation pause"
```

If authorization is absent, leave the verified changes uncommitted and continue.

---

### Task 4: Global F8 Hotkey Worker

**Files:**
- Create: `client/src/hotkey.rs`
- Modify: `client/src/main.rs:1-3`
- Test: inline tests in `client/src/hotkey.rs`

**Interfaces:**
- Consumes: `ScriptCommand::SetPaused(bool)` and `Arc<AtomicBool>`.
- Produces: `pub struct HotkeyWorker`, `HotkeyWorker::start(gate, command_tx) -> Result<HotkeyWorker, String>`, and `HotkeyWorker::shutdown(self) -> Result<(), String>`.

- [ ] **Step 1: Write failing worker-core tests**

Define test expectations through a fake platform event source:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HotkeyEvent {
    Pressed,
    Continue,
    Quit,
}

#[test]
fn f8_closes_the_gate_before_delivering_paused_state() {
    let gate = Arc::new(AtomicBool::new(false));
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let platform = FakeHotkeyPlatform::events([
        HotkeyEvent::Pressed,
        HotkeyEvent::Quit,
    ]);

    run_hotkey_loop(platform, gate.clone(), command_tx).unwrap();

    assert!(gate.load(Ordering::Acquire));
    assert_eq!(command_rx.blocking_recv(), Some(ScriptCommand::SetPaused(true)));
}

#[test]
fn second_f8_opens_the_gate_and_delivers_resume() {
    let gate = Arc::new(AtomicBool::new(false));
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let platform = FakeHotkeyPlatform::events([
        HotkeyEvent::Pressed,
        HotkeyEvent::Pressed,
        HotkeyEvent::Quit,
    ]);

    run_hotkey_loop(platform, gate.clone(), command_tx).unwrap();

    assert!(!gate.load(Ordering::Acquire));
    assert_eq!(command_rx.blocking_recv(), Some(ScriptCommand::SetPaused(true)));
    assert_eq!(command_rx.blocking_recv(), Some(ScriptCommand::SetPaused(false)));
}
```

Add fake-platform assertions that registration failure returns an error and that successful loops call unregister exactly once on quit.

- [ ] **Step 2: Run the hotkey tests and verify red**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml hotkey::tests
```

Expected: compilation fails because `hotkey.rs`, `run_hotkey_loop`, and the platform seam are not implemented.

- [ ] **Step 3: Implement the testable hotkey loop**

Use this internal seam:

```rust
trait HotkeyPlatform {
    fn register(&mut self) -> Result<(), String>;
    fn next_event(&mut self) -> Result<HotkeyEvent, String>;
    fn unregister(&mut self) -> Result<(), String>;
}

fn run_hotkey_loop<P: HotkeyPlatform>(
    mut platform: P,
    gate: Arc<AtomicBool>,
    command_tx: mpsc::Sender<ScriptCommand>,
) -> Result<(), String>;
```

After `register`, process events until `Quit`. On `Pressed`, calculate the requested state with `let paused = !gate.fetch_xor(true, Ordering::AcqRel);`, then call `command_tx.blocking_send(ScriptCommand::SetPaused(paused))`. Always attempt `unregister` after successful registration, including message-loop or channel errors; return the first operational error while appending cleanup context when unregister also fails.

- [ ] **Step 4: Run worker-core tests and verify green**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml hotkey::tests
```

Expected: gate ordering, two-way toggle, registration failure, and cleanup tests pass.

- [ ] **Step 5: Implement the Win32 platform and worker lifecycle**

Use constants with application-range ID and unmodified F8:

```rust
const HOTKEY_ID: i32 = 1;
```

The production platform must:

- call `RegisterHotKey(None, HOTKEY_ID, MOD_NOREPEAT, VK_F8.0 as u32)` on the worker thread;
- call `GetMessageW(&mut message, None, 0, 0)` and distinguish positive, zero (`WM_QUIT`), and negative return values;
- emit `Pressed` only for `WM_HOTKEY` with `wParam == HOTKEY_ID`, and `Continue` for unrelated messages;
- call `UnregisterHotKey(None, HOTKEY_ID)` before the thread exits.

`HotkeyWorker::start` must spawn one `std::thread`, use a `std::sync::mpsc` startup handshake to return registration failure synchronously, and retain the Win32 thread ID from `GetCurrentThreadId`. `shutdown` must call `PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0))`, join the thread, and return either a post or worker-loop error. `Drop` performs the same stop request and join as best-effort cleanup without panicking.

- [ ] **Step 6: Run hotkey tests and compiler check**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml hotkey::tests
cargo check --manifest-path client/Cargo.toml
```

Expected: unit tests pass and all Win32 imports/type signatures compile.

- [ ] **Step 7: Commit only if Git mutation is explicitly authorized**

```powershell
git add -- client/src/hotkey.rs client/src/main.rs
git commit -m "feat: add global F8 automation toggle"
```

If authorization is absent, leave the verified changes uncommitted and continue.

---

### Task 5: Application Wiring and Deterministic Shutdown

**Files:**
- Modify: `client/src/main.rs:1-61`
- Modify: `client/src/script.rs:221-239`
- Test: existing unit tests plus compiler checks

**Interfaces:**
- Consumes: `HotkeyWorker::start`, `HotkeyWorker::shutdown`, `Win32WindowControl`, and the action-gate-aware `script::run`.
- Produces: one client process whose console and F8 commands share lifecycle state and whose hotkey registration is always released on normal shutdown.

- [ ] **Step 1: Make the desired wiring fail to compile**

Change the `script::run` signature first so it requires `actions_paused: Arc<AtomicBool>` without updating `main.rs` yet:

```rust
pub async fn run(
    commands: mpsc::Receiver<ScriptCommand>,
    states: watch::Receiver<State>,
    initial_path: std::path::PathBuf,
    actions_paused: Arc<AtomicBool>,
) -> Result<(), String>;
```

Run:

```powershell
cargo check --manifest-path client/Cargo.toml
```

Expected: compilation fails at the `script::run` call because the new gate argument is missing.

- [ ] **Step 2: Wire startup, shared state, and shutdown**

In `main.rs`:

```rust
let actions_paused = Arc::new(AtomicBool::new(false));
let (command_tx, command_rx) = mpsc::channel(32);
let hotkey = HotkeyWorker::start(actions_paused.clone(), command_tx.clone())
    .map_err(io::Error::other)?;
```

Pass `command_tx` to the console, `actions_paused` to `script::run`, and keep `hotkey` owned outside the `tokio::select!`. After the select resolves, abort the three Tokio tasks, call `hotkey.shutdown()`, and combine errors so hotkey cleanup failure is returned if the selected task otherwise succeeded. Startup must return before spawning normal tasks when F8 registration fails.

- [ ] **Step 3: Compile and run the full automated suite**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml
cargo check --manifest-path client/Cargo.toml
```

Expected: every Rust test passes and the client compiles without errors or warnings introduced by this feature.

- [ ] **Step 4: Check the working-tree patch**

Run:

```powershell
git diff --check
git status --short
git diff -- client/Cargo.toml client/Cargo.lock client/src/main.rs client/src/console.rs client/src/script.rs client/src/window.rs client/src/hotkey.rs docs/superpowers/specs/2026-09-01-window-relative-automation-design.md docs/superpowers/plans/2026-09-01-window-relative-automation.md
```

Expected: `git diff --check` emits no output; status contains only intended project changes plus any pre-existing user changes, which must remain untouched.

- [ ] **Step 5: Commit only if Git mutation is explicitly authorized**

```powershell
git add -- client/Cargo.toml client/Cargo.lock client/src/main.rs client/src/console.rs client/src/script.rs client/src/window.rs client/src/hotkey.rs docs/superpowers/specs/2026-09-01-window-relative-automation-design.md docs/superpowers/plans/2026-09-01-window-relative-automation.md
git commit -m "feat: add window-relative game automation"
```

If authorization is absent, leave the fully verified patch uncommitted.

---

### Task 6: Bounded Windows Manual Verification

**Files:**
- Create temporarily, then delete: `client/manual-window-check.js`
- Verify: built client and one running Revolution Idle instance

**Interfaces:**
- Consumes: the complete implementation from Tasks 1-5.
- Produces: direct evidence for real Win32 discovery, client sizing, focus, input, F8 safety, and registration cleanup.

- [ ] **Step 1: Prepare a temporary script outside tracked source files**

Create `client/manual-window-check.js` with this content using `apply_patch`:

```javascript
(async (rev, memory) => {
    if (!memory.initialized) {
        rev.resize(1280, 720);
        memory.initialized = true;
    }
    rev.click(100, 100, "left");
    await rev.sleep(500);
})
```

- [ ] **Step 2: Run only with the user present and explicit approval for real input**

Run:

```powershell
cargo run --manifest-path client/Cargo.toml -- .\client\manual-window-check.js
```

Verify all of the following manually:

- the one visible `Revolution Idle.exe` window is found by process even if its title differs;
- its drawable client area becomes exactly 1280 by 720;
- `(100, 100)` clicks 100 pixels from the client area's top-left corner;
- another foreground application is displaced only immediately before a click;
- pressing F8 during the script prevents the next resize, focus, or click and the turn ends or settles before lifecycle pause;
- pressing F8 again resumes subsequent invocations;
- a click at `(-1, 0)`, `(1280, 0)`, or `(0, 720)` throws without moving the cursor;
- starting a second matching game window makes actions fail rather than selecting one;
- after exiting the client, a new client process can register F8, proving cleanup released it.

Delete `client/manual-window-check.js` with `apply_patch` after the manual run and confirm it does not appear in `git status --short`.

- [ ] **Step 3: Re-run automated verification after the manual check**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml
cargo check --manifest-path client/Cargo.toml
git diff --check
```

Expected: all tests pass, compilation succeeds, and the diff check emits no output.
