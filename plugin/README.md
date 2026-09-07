# Revolution Idle Score Telemetry

This BepInEx plugin exposes selected live Revolution Idle values through a loopback HTTP server. Values are fetched on demand; the plugin does not push periodic telemetry.

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

`0`, non-numeric values, and values outside `1..65535` disable the server. The server listens only on `127.0.0.1`.

## Background UI click probe

The plugin accepts only the fixed background-input bridge message and dispatches matching clicks to Unity uGUI handlers. It does not use `Input.GetMouseButtonDown()`, focus the game window, or move the cursor by default.

Run the background click probe from the repository root:

```powershell
cargo +1.97.1 run --manifest-path client/Cargo.toml --bin window_message_probe
```

Add `--focus` only when explicit window focus is desired.

## Invoke UI buttons

Run `capture` in the client and click a button to print its client coordinates, GameObject name, and hierarchy path. Use the printed name in a script:

```javascript
await rev.invoke("button_name");
```

Names are case-sensitive. Active buttons take precedence over inactive duplicates. If a name still matches multiple buttons, pass the printed path instead. Paths identify the current scene and hierarchy; capture again after those change. Hidden and inactive Unity UI Buttons can be invoked, but must still pass `IsInteractable()`. Invocation calls the registered `onClick` event on Unity's main thread without moving the mouse or raycasting. Errors reject the promise; paused scripts skip the action.

Capture performs a read-only raycast asynchronously; the normal mouse click still reaches the game. If that click changes the UI before the lookup runs, capture can report the new UI at those coordinates. Coordinates remain in the output if the lookup fails.

Both features require the updated plugin: `POST /invoke?name=...` invokes a button, and `GET /capture?x=...&y=...` returns its name and path without invoking it.

## Read state

The HTTP endpoint is `GET http://127.0.0.1:19841/state`. In PowerShell, request all values or only the values you need:

```powershell
Invoke-RestMethod http://127.0.0.1:19841/state
Invoke-RestMethod 'http://127.0.0.1:19841/state?key=score&key=IP'
```

In JavaScript, `rev.state()` returns a promise. With no arguments it returns all supported keys; arguments return only those requested, case-sensitively. Requesting exactly one key unwraps the result to that key's value directly, instead of an object with one property:

```javascript
const state = await rev.state("score", "IP");
const ep = await rev.state("EP"); // "0e0", not { EP: "0e0" }
```

Each call fetches fresh values on demand. The returned object is frozen. Requests reuse one HTTP client and its keep-alive connection when possible. HTTP or response errors reject the promise.

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
