# Revolution Idle Score Telemetry

This BepInEx plugin exposes selected live Revolution Idle values through a loopback WebSocket bridge. Values are fetched on demand; the plugin does not push periodic telemetry.

## Requirements

- Revolution Idle for Windows x64 (Steam)
- BepInEx 6 IL2CPP build 785 or newer with generated `BepInEx/interop` assemblies
- .NET SDK 8 or newer and Rust/Cargo for building

The plugin itself targets .NET 6 because that is the runtime embedded by BepInEx IL2CPP.

## Automated install

From the repository root, run:

```console
.\plugin\install.cmd
```

From the `plugin` folder, run `.\install.cmd`. The launcher applies an execution-policy bypass only to the installer process; it does not change the system or user execution policy.

The installer asks for the Revolution Idle installation folder. If BepInEx is missing, it downloads and verifies the pinned Windows x64 IL2CPP build, launches the game once to generate interop assemblies, runs the plugin tests and Release builds, and installs the verified DLL and `client.exe` in the game root. On game startup, the plugin starts that client with the configured port and `--non-interactable`; client stdout and stderr are forwarded to BepInEx logging.

If only part of a BepInEx installation is present, the installer stops without overwriting it. Repair or remove that partial installation before retrying.

For non-interactive use:

```console
.\plugin\install.cmd -InstallationFolder "D:\SteamLibrary\steamapps\common\Revolution Idle"
```

## Build

From the repository root:

```powershell
cargo build --manifest-path client/Cargo.toml --release
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release
```

If the game is installed elsewhere, append `/p:GameDir='D:\path\to\Revolution Idle'` to both commands.

## Install

Create `BepInEx/plugins/RevIdle.ScoreTelemetry` under the Revolution Idle game folder and copy `plugin/src/bin/Release/RevIdle.ScoreTelemetry.dll` into it. Copy `client/target/release/client.exe` to the game root. Start the game once to create the config file:

`BepInEx/config/dev.tnthung.revolutionidle.scoretelemetry.cfg`

Configure the destination:

```ini
[Network]
Port = 19841
```

`0`, non-numeric values, and values outside `1..65535` disable the bridge. The bridge listens only on `127.0.0.1`.

The plugin starts the game-root client automatically after binding the configured port. `--non-interactable` disables only console input; client hotkeys and mouse capture remain enabled.

At startup, the plugin wraps the existing BepInEx log listeners to suppress the inactive Dilation coroutine message. Add exact, case-sensitive `(source, message)` pairs to `SuppressedMessages` in `src/Plugin.cs` to suppress additional messages, then rebuild, install, and restart the game. This filters BepInEx output; Unity's own log and listeners registered later are unaffected.

Click, scroll, and drag commands use the same raw packet connection. They dispatch to Unity uGUI handlers without focusing the game window or moving the cursor.

## Injected script controls

The bottom-right overlay has three 30 by 30 pixel buttons: Reload/Stop, Resume/Pause, and Capture. The first two switch their icon and action with the script phase. The disabled-state projection is:

| Script phase | Reload/Stop | Resume/Pause | Capture |
| --- | --- | --- | --- |
| Unloaded | disabled | disabled | enabled |
| Stopped | Reload | disabled | enabled |
| Running | Stop | Pause | disabled |
| Paused | Stop | Resume | enabled |
| Any phase while capturing | disabled | disabled | disabled |

The buttons use the current client-published state. Before a state for the current WebSocket connection generation is available, all three are disabled. Unity sends six independent best-effort action notifications (`ReloadScript`, `StopScript`, `PauseScript`, `ResumeScript`, `StartCapture`, and `StopCapture`); a failed notification is non-fatal and is not replayed after reconnect. The client sends one complete `StateUpdate { phase, capture }` after a lifecycle or capture transition and when a new connection generation becomes available. Unity does not request or poll for control state, and it does not optimistically change the projection.

Capture is one-shot: the first captured left-button down consumes its matching up, disarms capture, and causes the client to publish `capture: false`. The captured coordinate is resolved through the correlated `UiPathReq -> UiPathRes` exchange only for path lookup; action notifications and `StateUpdate` are not correlated request/response operations.

## Capture, invoke, and transfer UI elements

Run `capture` in the client and click an element. Capture consumes one click, then turns itself off. It prints client coordinates followed by `button: "<path>"`, `checkbox: "<path>"`, or `slot: "<path>"`, copies a valid path to the clipboard, and prints `Copied to clipboard`. It checks buttons first, then checkboxes, then drop slots, including their parent objects. If none exists, it prints only coordinates and leaves the clipboard unchanged. Use the copied paths in scripts:

```javascript
await rev.invoke(buttonPath);
await rev.invoke(checkboxPath);
await rev.scrollIntoView(buttonPath);
await rev.transfer(sourceSlotPath, destinationSlotPath);
const item = await rev.slot(sourceSlotPath); // unknown; null when empty
```

Only exact, case-sensitive paths are accepted; name lookup is not supported. Paths include the scene handle and escaped object names with sibling indexes. Capture again after scene or hierarchy changes. Hidden and inactive Unity UI Buttons can be invoked, but must still pass `IsInteractable()`. Invocation calls the registered `onClick` event on Unity's main thread without moving the mouse or raycasting. Errors reject the promise; paused scripts skip the action.

`scrollIntoView(path)` reveals a UI element by instantly adjusting its active containing scroll views, from the innermost outward, on their enabled axes. It moves only as far as needed within the content bounds and leaves already-visible elements in place. Targets larger than a viewport reveal the nearest edge. Inactive buffered children are supported when their containing scroll view is active; the game remains responsible for activating them. Open the correct page first: missing targets and inactive targets without an active containing scroll view reject. The promise resolves after positioning and layout updates, without clicking the target; use `invoke(path)` separately. Paused scripts skip the action.

Checkbox invocation calls the game's pointer-click handler and requires an active, interactable Toggle (including BundleToggle). Open the Automation tab before invoking its checkboxes. Slot reads return the generic slot's `Value` through the state serializer, or `null` when `Slotted` is false. Missing or unsupported slots reject. Reads run on Unity's main thread, support initialized inactive slots, and remain available while actions are paused.

Transfer requires two distinct slot objects with one drop handler each and exactly one draggable item in the source. Lookup includes hidden and inactive objects; no screen coordinates or raycasts are used. The plugin calls the item's drag lifecycle and the destination's drop handler directly on Unity's main thread, without activating the panels. The game controls compatibility, validation, and occupied-slot behavior. A resolved promise means handlers were called, not that the game accepted or completed the transfer; check game state before depending on the result. Slots and items must already be instantiated and initialized by the game; hidden-panel transfers remain subject to the handlers' own requirements.

These features require the updated client and plugin and use the loopback WebSocket bridge. Capture returns `{ "type": "button" | "checkbox" | "slot" | null, "path": string | null }` without interacting with the target.

## Read state

In JavaScript, `rev.state()` returns a promise. With no arguments it returns all supported keys; arguments return only those requested, case-sensitively. Requesting exactly one key unwraps the result to that key's value directly, instead of an object with one property:

```javascript
const state = await rev.state("score", "IP");
const ep = await rev.state("EP"); // "0e0", not { EP: "0e0" }
```

Each call fetches fresh values on demand over the WebSocket bridge. The returned object is frozen. Bridge or response errors reject the promise.

Expected returned fields are documented in the generated [state field reference](STATE_KEYS.md), a best-effort static model that omits likely ancestor/back-references and retains nested children, collection types, and compatibility aliases. Open [STATE_GRAPH.html](STATE_GRAPH.html) directly in a browser to search fields and paths in that model. Regenerate both after an interop assembly change:

```powershell
.\plugin\generate-state-reference.ps1
```

With no arguments, the generator reads `E:\SteamLibrary\steamapps\common\Revolution Idle`. It also writes `STATE_GRAPH.json` (the graph data) and re-embeds it into `STATE_GRAPH.html` from `STATE_GRAPH.template.html`.

Paths are case-sensitive property names separated by `.`, starting at `GameData`. Numeric segments index arrays/lists; dictionary segments use string, integer, or enum keys. For example:

```javascript
const full = await rev.state();
const selected = await rev.state("score", "eternity.dtpSpent", "eternity.dilationTree.top.0.level");
const aliases = await rev.state("DT", "DTP");
```

The JSON response preserves mixed types: BigDouble and integers outside JavaScript's safe range are strings, while safe numbers, booleans, strings, enums, dates, arrays, dictionaries, and nested gameplay objects use their native JSON representation.

## Uninstall

Stop the game, then remove `BepInEx/plugins/RevIdle.ScoreTelemetry` and the game-root `client.exe`. If no other plugins use BepInEx, its loader can also be removed by deleting the added `BepInEx`, `dotnet`, `winhttp.dll`, `doorstop_config.ini`, `.doorstop_version`, and `changelog.txt` entries beside `Revolution Idle.exe`. This plugin does not edit save files.
