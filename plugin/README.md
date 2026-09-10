# Revolution Idle Score Telemetry

This BepInEx plugin exposes selected live Revolution Idle values through a loopback WebSocket bridge. Values are fetched on demand; the plugin does not push periodic telemetry.

## Requirements

- Revolution Idle for Windows x64 (Steam)
- BepInEx 6 IL2CPP build 785 or newer with generated `BepInEx/interop` assemblies
- .NET SDK 8 or newer for building

The plugin itself targets .NET 6 because that is the runtime embedded by BepInEx IL2CPP.

## Automated install

From the repository root, run:

```console
.\plugin\install.cmd
```

From the `plugin` folder, run `.\install.cmd`. The launcher applies an execution-policy bypass only to the installer process; it does not change the system or user execution policy.

The installer asks for the Revolution Idle installation folder. If BepInEx is missing, it downloads and verifies the pinned Windows x64 IL2CPP build, launches the game once to generate interop assemblies, runs the plugin tests and Release build, and installs the verified DLL.

If only part of a BepInEx installation is present, the installer stops without overwriting it. Repair or remove that partial installation before retrying.

For non-interactive use:

```console
.\plugin\install.cmd -InstallationFolder "D:\SteamLibrary\steamapps\common\Revolution Idle"
```

## Build

From the repository root:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --framework net8.0 -p:TargetFrameworks=net8.0
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release
```

If the game is installed elsewhere, append `/p:GameDir='D:\path\to\Revolution Idle'` to both commands.

## Install

Create `BepInEx/plugins/RevIdle.ScoreTelemetry` under the Revolution Idle game folder and copy `plugin/src/bin/Release/RevIdle.ScoreTelemetry.dll` into it. Start the game once to create the config file:

`BepInEx/config/dev.tnthung.revolutionidle.scoretelemetry.cfg`

Configure the destination:

```ini
[Network]
Port = 19841
```

`0`, non-numeric values, and values outside `1..65535` disable the bridge. The bridge listens only on `127.0.0.1`.

Click, scroll, and drag commands use the same raw packet connection. They dispatch to Unity uGUI handlers without focusing the game window or moving the cursor.

## Injected script controls

The bottom-right overlay has three 30 by 30 pixel buttons: Reload/Stop, Resume/Pause, and Capture. The first two switch their icon and action with the script phase. The disabled-state projection is:

| Script phase | Reload/Stop | Resume/Pause | Capture |
| --- | --- | --- | --- |
| Unloaded | disabled | disabled | disabled |
| Stopped | Reload | disabled | enabled |
| Running | Stop | Pause | disabled |
| Paused | Stop | Resume | enabled |
| Any phase while capturing | disabled | disabled | disabled |

The buttons use the current client-published state. Before a state for the current WebSocket connection generation is available, all three are disabled. Unity sends six independent best-effort action notifications (`ReloadScript`, `StopScript`, `PauseScript`, `ResumeScript`, `StartCapture`, and `StopCapture`); a failed notification is non-fatal and is not replayed after reconnect. The client sends one complete `StateUpdate { phase, capture }` after a lifecycle or capture transition and when a new connection generation becomes available. Unity does not request or poll for control state, and it does not optimistically change the projection.

Capture is one-shot: the first captured left-button down consumes its matching up, disarms capture, and causes the client to publish `capture: false`. The captured coordinate is resolved through the correlated `UiPathReq -> UiPathRes` exchange only for path lookup; action notifications and `StateUpdate` are not correlated request/response operations.

## Capture, invoke, and transfer UI elements

Run `capture` in the client and click an element. Capture consumes one click, then turns itself off. It prints client coordinates followed by `button: "<path>"` or `slot: "<path>"`, copies a valid path to the clipboard, and prints `Copied to clipboard`. It checks buttons first, then drop slots, including their parent objects. If neither exists, it prints only coordinates and leaves the clipboard unchanged. Use the copied paths in scripts:

```javascript
await rev.invoke(buttonPath);
await rev.transfer(sourceSlotPath, destinationSlotPath);
```

Only exact, case-sensitive paths are accepted; name lookup is not supported. Paths include the scene handle and escaped object names with sibling indexes. Capture again after scene or hierarchy changes. Hidden and inactive Unity UI Buttons can be invoked, but must still pass `IsInteractable()`. Invocation calls the registered `onClick` event on Unity's main thread without moving the mouse or raycasting. Errors reject the promise; paused scripts skip the action.

Transfer requires two distinct slot objects with one drop handler each and exactly one draggable item in the source. Lookup includes hidden and inactive objects; no screen coordinates or raycasts are used. The plugin calls the item's drag lifecycle and the destination's drop handler directly on Unity's main thread, without activating the panels. The game controls compatibility, validation, and occupied-slot behavior. A resolved promise means handlers were called, not that the game accepted or completed the transfer; check game state before depending on the result. Slots and items must already be instantiated and initialized by the game; hidden-panel transfers remain subject to the handlers' own requirements.

These features require the updated client and plugin and use the loopback WebSocket bridge. Capture returns `{ "type": "button" | "slot" | null, "path": string | null }` without interacting with the target.

## Read state

In JavaScript, `rev.state()` returns a promise. With no arguments it returns all supported keys; arguments return only those requested, case-sensitively. Requesting exactly one key unwraps the result to that key's value directly, instead of an object with one property:

```javascript
const state = await rev.state("score", "IP");
const ep = await rev.state("EP"); // "0e0", not { EP: "0e0" }
```

Each call fetches fresh values on demand over the WebSocket bridge. The returned object is frozen. Bridge or response errors reject the promise.

The complete nested state is documented in the generated [state path reference](STATE_KEYS.md). It includes every reachable gameplay property, collection element/value types, and all compatibility aliases. For finding which path(s) reach a given type, open [STATE_GRAPH.html](STATE_GRAPH.html) directly in a browser: search a type, field name, or alias to see every route from `GameData` that reaches it. Regenerate both after an interop assembly change:

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

Remove `BepInEx/plugins/RevIdle.ScoreTelemetry`. If no other plugins use BepInEx, its loader can also be removed by deleting the added `BepInEx`, `dotnet`, `winhttp.dll`, `doorstop_config.ini`, `.doorstop_version`, and `changelog.txt` entries beside `Revolution Idle.exe`. This plugin does not edit save files.
