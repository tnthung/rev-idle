# Background Unity UI Input Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Send one client-relative left-click request to Revolution Idle without focusing it and dispatch that request to the top Unity uGUI handler from the installed BepInEx plugin.

**Architecture:** The Rust probe posts one private WM_APP message containing a request ID and 32-bit x/y coordinates. A plugin-side SetWindowSubclass callback enqueues the request, and the existing Unity ScoreTicker.Update() drains the queue and uses EventSystem.RaycastAll plus ExecuteEvents on Unity's main thread.

**Tech Stack:** Rust 1.97.1, windows 0.61.3, C#/.NET 6 plugin, BepInEx 6 IL2CPP, Unity 2022.3 uGUI, Win32 User32/Comctl32.

**Spec:** docs/superpowers/specs/2026-09-02-background-input-bridge-design.md

## Global Constraints

- The no-focus path must not call SetForegroundWindow, move the cursor, or activate the game.
- Preserve the existing explicit --focus behavior, including its 100 ms foreground verification delay.
- Use message ID WM_APP + 0x417, a nonzero request ID in wParam, x in the low 32 bits of lParam, and y in the high 32 bits.
- The plugin window procedure may only decode and enqueue; all Unity APIs run from ScoreTicker.Update().
- The pending queue capacity is 32. Record duplicate IDs only after successful enqueue.
- Dispatch pointer-down, pointer-up, and pointer-click to the first valid uGUI raycast hierarchy.
- Do not make routine telemetry failures noisy and do not make the input bridge depend on UDP being enabled.
- Use Rust 1.97.1. Do not run cargo fmt.
- Preserve unrelated working-tree changes, including scripts/test1.js and the user-authored focus delay.
- Do not create commits or otherwise mutate git history unless the user explicitly requests it.

---

### Task 1: Tested plugin protocol and bounded queue

**Files:**
- Create: plugin/src/InputBridgeProtocol.cs
- Create: plugin/src/ClickCommandQueue.cs
- Modify: plugin/tests/Program.cs
- Modify: plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj

**Interfaces:**
- Produces: ClickCommand(ulong RequestId, uint X, uint Y).
- Produces: InputBridgeProtocol.MessageId, TryDecode, and TryMapToUnity.
- Produces: ClickCommandQueue.TryEnqueue, TryDequeue, and Clear.
- Task 2 consumes all three types without changing their signatures.

- [ ] **Step 1: Add failing protocol and queue tests**

Append calls in plugin/tests/Program.cs, then define tests with literal expectations:

~~~csharp
BridgeDecodesFullWidthCoordinates();
BridgeRejectsZeroRequestId();
BridgeMapsTopLeftClientCoordinatesToUnityCoordinates();
BridgeQueueRejectsDuplicatesAndOverflow();
BridgeQueueDequeuesInOrderAndClears();

static void BridgeDecodesFullWidthCoordinates()
{
    nint packed = unchecked((nint)(long)0x12345678ABCDEF01UL);
    Equal(true, InputBridgeProtocol.TryDecode(42, packed, out ClickCommand command), nameof(BridgeDecodesFullWidthCoordinates));
    Equal(42UL, command.RequestId, nameof(BridgeDecodesFullWidthCoordinates));
    Equal(0xABCDEF01U, command.X, nameof(BridgeDecodesFullWidthCoordinates));
    Equal(0x12345678U, command.Y, nameof(BridgeDecodesFullWidthCoordinates));
}

static void BridgeRejectsZeroRequestId()
{
    Equal(false, InputBridgeProtocol.TryDecode(0, 0, out _), nameof(BridgeRejectsZeroRequestId));
}

static void BridgeMapsTopLeftClientCoordinatesToUnityCoordinates()
{
    var command = new ClickCommand(1, 1200, 80);
    Equal(true, InputBridgeProtocol.TryMapToUnity(command, 1920, 1080, 1920, 1080, out float x, out float y), nameof(BridgeMapsTopLeftClientCoordinatesToUnityCoordinates));
    Near(1200f, x, nameof(BridgeMapsTopLeftClientCoordinatesToUnityCoordinates));
    Near(999f, y, nameof(BridgeMapsTopLeftClientCoordinatesToUnityCoordinates));
    Equal(false, InputBridgeProtocol.TryMapToUnity(command, 1200, 1080, 1920, 1080, out _, out _), nameof(BridgeMapsTopLeftClientCoordinatesToUnityCoordinates));
}

static void BridgeQueueRejectsDuplicatesAndOverflow()
{
    var queue = new ClickCommandQueue();
    Equal(true, queue.TryEnqueue(new ClickCommand(1, 1, 1)), nameof(BridgeQueueRejectsDuplicatesAndOverflow));
    Equal(false, queue.TryEnqueue(new ClickCommand(1, 2, 2)), nameof(BridgeQueueRejectsDuplicatesAndOverflow));
    for (ulong id = 2; id <= 32; id++)
        Equal(true, queue.TryEnqueue(new ClickCommand(id, 1, 1)), nameof(BridgeQueueRejectsDuplicatesAndOverflow));
    Equal(false, queue.TryEnqueue(new ClickCommand(33, 1, 1)), nameof(BridgeQueueRejectsDuplicatesAndOverflow));
}

static void BridgeQueueDequeuesInOrderAndClears()
{
    var queue = new ClickCommandQueue();
    queue.TryEnqueue(new ClickCommand(10, 1, 2));
    queue.TryEnqueue(new ClickCommand(11, 3, 4));
    Equal(true, queue.TryDequeue(out ClickCommand first), nameof(BridgeQueueDequeuesInOrderAndClears));
    Equal(10UL, first.RequestId, nameof(BridgeQueueDequeuesInOrderAndClears));
    queue.Clear();
    Equal(false, queue.TryDequeue(out _), nameof(BridgeQueueDequeuesInOrderAndClears));
}
~~~

Add the two new production files as linked compile items in the test project.

- [ ] **Step 2: Run the test executable and verify RED**

Run:

~~~powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
~~~

Expected: compilation fails because InputBridgeProtocol, ClickCommand, and ClickCommandQueue do not exist.

- [ ] **Step 3: Implement the protocol and queue minimally**

Create InputBridgeProtocol.cs with this public shape and exact mapping behavior:

~~~csharp
namespace RevIdle.ScoreTelemetry;

internal readonly record struct ClickCommand(ulong RequestId, uint X, uint Y);

internal static class InputBridgeProtocol
{
    internal const uint MessageId = 0x8000 + 0x417;

    internal static bool TryDecode(nuint requestId, nint packedCoordinates, out ClickCommand command)
    {
        if (requestId == 0)
        {
            command = default;
            return false;
        }

        ulong packed = unchecked((ulong)(long)packedCoordinates);
        command = new ClickCommand((ulong)requestId, (uint)packed, (uint)(packed >> 32));
        return true;
    }

    internal static bool TryMapToUnity(
        ClickCommand command,
        int clientWidth,
        int clientHeight,
        int screenWidth,
        int screenHeight,
        out float unityX,
        out float unityY)
    {
        if (clientWidth <= 0 || clientHeight <= 0 || screenWidth <= 0 || screenHeight <= 0 ||
            command.X >= (uint)clientWidth || command.Y >= (uint)clientHeight)
        {
            unityX = 0;
            unityY = 0;
            return false;
        }

        unityX = command.X * (float)screenWidth / clientWidth;
        unityY = screenHeight - 1f - command.Y * (float)screenHeight / clientHeight;
        return true;
    }
}
~~~

Create ClickCommandQueue.cs using a Queue<ClickCommand>, HashSet<ulong>, and a FIFO of at most 256 recent accepted IDs. TryEnqueue returns false for request ID zero, a duplicate, or when 32 requests are pending. Add the ID to dedupe state only after the pending item is accepted. TryDequeue preserves FIFO order. Clear empties pending and dedupe state.

- [ ] **Step 4: Run the test executable and verify GREEN**

Run the same dotnet run command. Expected: 9 tests passed with a zero exit code.

- [ ] **Step 5: Mutation-check the tests**

Confirm an assertion would fail for each mutation: swap x/y extraction, accept request ID zero, omit duplicate rejection, accept a 33rd pending command, or omit the y-axis flip. Do not leave mutations in the tree.

---

### Task 2: Win32 receiver and Unity uGUI dispatcher

**Files:**
- Create: plugin/src/Win32InputBridge.cs
- Create: plugin/src/UnityUiClickDispatcher.cs
- Modify: plugin/src/Plugin.cs
- Modify: plugin/src/RevolutionIdle.ScoreTelemetry.csproj
- Modify: plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
- Modify: plugin/README.md

**Interfaces:**
- Consumes: the exact Task 1 ClickCommand, InputBridgeProtocol, and ClickCommandQueue types.
- Produces: Win32InputBridge.TryAttach(), Win32InputBridge.Dispose(), and UnityUiClickDispatcher.TryDispatch().
- ScoreTicker owns one queue and bridge and invokes both on Unity's main thread.

- [ ] **Step 1: Establish the RED build**

Add linked compile entries for Win32InputBridge.cs and UnityUiClickDispatcher.cs to the test project before creating those files. Add a reference to $(GameDir)\BepInEx\interop\UnityEngine.UI.dll with Private=false in the plugin project and Private=true in the test project. Add bridge references to Plugin.cs, then run:

~~~powershell
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release
~~~

Expected: build fails because the bridge types do not exist.

- [ ] **Step 2: Implement the Win32 subclass receiver**

Create Win32InputBridge.cs with this shape:

~~~csharp
internal sealed class Win32InputBridge : IDisposable
{
    private static readonly SubclassProc SubclassCallback = WindowProcedure;
    private static Win32InputBridge? _active;
    private readonly ClickCommandQueue _queue;
    private nint _window;
    private bool _disposed;

    internal Win32InputBridge(ClickCommandQueue queue) => _queue = queue;
    internal nint Window => _window;
    internal bool IsAttached => _window != 0;
    internal bool TryAttach();
    public void Dispose();

    private static nint WindowProcedure(
        nint window,
        uint message,
        nuint wParam,
        nint lParam,
        nuint subclassId,
        nuint referenceData);
}
~~~

Use EnumWindows to collect visible windows owned by Environment.ProcessId, reject ConsoleWindowClass, require exactly one candidate with a non-empty client area, and verify its owner thread equals GetCurrentThreadId(). Install with SetWindowSubclass and a fixed nonzero subclass ID.

The rooted callback performs this custom-message branch:

~~~csharp
if (message == InputBridgeProtocol.MessageId &&
    _active is { _disposed: false } bridge &&
    InputBridgeProtocol.TryDecode(wParam, lParam, out ClickCommand command))
{
    bridge._queue.TryEnqueue(command);
    return 0;
}
~~~

Forward every other message through DefSubclassProc. On WM_NCDESTROY, clear the stored handle and active owner after forwarding. Dispose is idempotent, stops acceptance first, removes the subclass when the handle is still valid and owned by the current thread, clears the queue, and releases _active last. No callback path logs, blocks, calls Unity, or throws across the native boundary; catch any callback exception and fall back to DefSubclassProc.

- [ ] **Step 3: Implement main-thread uGUI dispatch**

Create UnityUiClickDispatcher.cs with:

~~~csharp
internal static class UnityUiClickDispatcher
{
    internal static bool TryDispatch(
        nint window,
        ClickCommand command,
        out string result);
}
~~~

Read the current client rectangle, call InputBridgeProtocol.TryMapToUnity with Screen.width and Screen.height, require EventSystem.current, create left-button PointerEventData with position, clickCount = 1, and clickTime = Time.unscaledTime, then use an Il2CppSystem.Collections.Generic.List<RaycastResult> with EventSystem.RaycastAll. Select the first result whose gameObject is non-null. Set pointerCurrentRaycast and pointerPressRaycast, then dispatch in this order:

~~~csharp
ExecuteEvents.ExecuteHierarchy(hit.gameObject, pointerData, ExecuteEvents.pointerDownHandler);
ExecuteEvents.ExecuteHierarchy(hit.gameObject, pointerData, ExecuteEvents.pointerUpHandler);
GameObject? clickTarget = ExecuteEvents.GetEventHandler<IPointerClickHandler>(hit.gameObject);
if (clickTarget is null)
{
    result = $"no click handler at ({command.X}, {command.Y})";
    return false;
}
ExecuteEvents.Execute(clickTarget, pointerData, ExecuteEvents.pointerClickHandler);
result = $"clicked '{clickTarget.name}' at ({command.X}, {command.Y})";
return true;
~~~

Return concise failure strings for invalid bounds, missing EventSystem, an empty raycast, and a missing click handler. Catch exceptions at the ScoreTicker call site so later frames continue.

- [ ] **Step 4: Integrate bridge ownership into ScoreTicker**

Change Plugin.Load() so AddComponent<ScoreTicker>() runs even when UDP publishing is disabled. Store the BepInEx logger in a static field and expose internal one-line bridge information/error helpers.

Add queue and bridge fields to ScoreTicker. During Update(), attempt attachment at most once per second until successful. After attachment, drain at most 32 queued commands and dispatch each. Log one result per command. Keep the 50 ms telemetry accumulator unchanged.

Add both lifecycle callbacks through one idempotent method:

~~~csharp
public void OnDestroy() => StopBridge();
public void OnApplicationQuit() => StopBridge();

private void StopBridge()
{
    _bridge?.Dispose();
    _bridge = null;
    _clicks.Clear();
}
~~~

If WM_NCDESTROY removes the handle, a later retry may attach to a recreated Unity window.

- [ ] **Step 5: Build and run plugin tests**

Run:

~~~powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release
~~~

Expected: 9 tests passed and a successful Release build with no new warnings.

- [ ] **Step 6: Document the bridge**

Add a Background UI click probe section to plugin/README.md. State that the bridge accepts only the fixed message, targets uGUI handlers rather than Input.GetMouseButtonDown(), does not focus or move the cursor by default, and can be tested with:

~~~powershell
cargo +1.97.1 run --manifest-path client/Cargo.toml --bin window_message_probe
~~~

Document --focus as explicit opt-in.

---

### Task 3: Rust custom-message sender

**Files:**
- Modify: client/bin/window_message_probe.rs
- Preserve: client/Cargo.toml

**Interfaces:**
- Consumes: Task 1's exact message ID and bit layout.
- Produces: one PostMessageW request for (1200, 80) and optional focus behavior.

- [ ] **Step 1: Replace mouse-sequence tests with failing bridge tests**

Keep the focus-flag test and replace the packing/sequence tests with:

~~~rust
#[test]
fn packs_full_width_coordinates_for_bridge_message() {
    assert_eq!(
        super::pack_bridge_coordinates(0xabcdef01, 0x12345678) as u64,
        0x12345678abcdef01,
    );
}

#[test]
fn bridge_message_uses_reserved_wm_app_offset() {
    assert_eq!(super::INPUT_BRIDGE_MESSAGE, 0x8417);
}

#[test]
fn generated_request_id_is_never_zero() {
    assert_ne!(super::request_id_from(0, 0), 0);
    assert_ne!(super::request_id_from(123, 456), 0);
}
~~~

- [ ] **Step 2: Run the probe tests and verify RED**

~~~powershell
cargo +1.97.1 test --manifest-path client/Cargo.toml --bin window_message_probe
~~~

Expected: compilation fails because the bridge packing, message, and request-ID functions do not exist.

- [ ] **Step 3: Implement the one-message sender**

Remove SendMessageW, the standard mouse-message constants, and MK_LBUTTON. Add:

~~~rust
const WM_APP: u32 = 0x8000;
const INPUT_BRIDGE_MESSAGE: u32 = WM_APP + 0x417;

fn pack_bridge_coordinates(x: u32, y: u32) -> isize {
    (((y as u64) << 32) | x as u64) as isize
}

fn request_id_from(timestamp_nanos: u128, process_id: u32) -> usize {
    let mixed = timestamp_nanos as u64 ^ ((process_id as u64) << 32);
    (mixed as usize) | 1
}
~~~

Generate the request ID from SystemTime::now().duration_since(UNIX_EPOCH) and std::process::id(). Record GetForegroundWindow() immediately before posting. If --focus is present, preserve the existing SetForegroundWindow, 100 ms delay, and foreground verification. Post exactly once:

~~~rust
unsafe {
    PostMessageW(
        Some(hwnd),
        INPUT_BRIDGE_MESSAGE,
        WPARAM(request_id),
        LPARAM(pack_bridge_coordinates(CLICK_X as u32, CLICK_Y as u32)),
    )
}
~~~

For the default path, wait 100 ms, record the foreground window again, and fail only if the game was not foreground before but became foreground after. Print the request ID, coordinate, target handle, and before/after foreground handles.

- [ ] **Step 4: Run focused and full client tests**

~~~powershell
cargo +1.97.1 test --manifest-path client/Cargo.toml --bin window_message_probe
cargo +1.97.1 test --manifest-path client/Cargo.toml
~~~

Expected: all tests pass. Do not run cargo fmt.

---

### Task 4: Install and perform the authorized live test

**Files:**
- Install source: plugin/src/bin/Release/net6.0/RevIdle.ScoreTelemetry.dll
- Install target: C:\Program Files (x86)\Steam\steamapps\common\Revolution Idle\BepInEx\plugins\RevIdle.ScoreTelemetry\RevIdle.ScoreTelemetry.dll
- Inspect: C:\Program Files (x86)\Steam\steamapps\common\Revolution Idle\BepInEx\LogOutput.log

**Interfaces:**
- Consumes: the built plugin and sender.
- Produces: live evidence of plugin load, hook installation, exactly one dispatch attempt, and unchanged foreground state.

- [ ] **Step 1: Run the existing non-destructive installer**

~~~powershell
powershell -NoProfile -ExecutionPolicy Bypass -File plugin/install.ps1
~~~

Expected: the DLL is installed atomically under the existing plugin directory. If the running process locks it, pause rather than terminating the game.

- [ ] **Step 2: Restart safely**

Do not force-terminate the game. If it remains running, ask the user to close and reopen it. After restart, inspect LogOutput.log for plugin load and bridge installation.

- [ ] **Step 3: Send exactly one no-focus click**

~~~powershell
cargo +1.97.1 run --manifest-path client/Cargo.toml --bin window_message_probe
~~~

Expected: one posted request at (1200, 80), and the sender's before/after foreground evidence shows the game did not become foreground.

- [ ] **Step 4: Verify plugin and visible behavior**

Inspect only the new log tail. Require exactly one bridge result for the request ID, with either a named target or an explicit no-target reason. Report visible game behavior separately; message receipt alone is not a successful uGUI click.

- [ ] **Step 5: Run final regression checks**

Re-run the plugin tests, plugin Release build, focused probe tests, and full client tests. Record exact outcomes and any live limitation.

