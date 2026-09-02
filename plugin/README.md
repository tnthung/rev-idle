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
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
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

## Read state

The HTTP endpoint is `GET http://127.0.0.1:19841/state`. In PowerShell, request all values or only the values you need:

```powershell
Invoke-RestMethod http://127.0.0.1:19841/state
Invoke-RestMethod 'http://127.0.0.1:19841/state?key=score&key=IP'
```

In JavaScript, `rev.state()` returns a promise. With no arguments it returns all supported keys; arguments return only those requested, case-sensitively:

```javascript
const state = await rev.state("score", "IP");
```

Each call fetches fresh values on demand. The returned object is frozen. Requests reuse one HTTP client and its keep-alive connection when possible. HTTP or response errors reject the promise.

All values are BigDouble strings. Supported keys:

| Key | GameData mapping | Key | GameData mapping |
| --- | --- | --- | --- |
| `score` | `score` | `income` | `income` |
| `IP` | `infinity.IP` | `infinities` | `infinity.infs` |
| `stars` | `infinity.stars` | `stardust` | `infinity.stardust` |
| `EP` | `eternity.EP` | `eternities` | `eternity.eters` |
| `DP` | `eternity.DP` | `AP` | `eternity.AP` |
| `RP` | `eternity.curRP` | `RPMax` | `eternity.maximumRP` |
| `RPSpent` | `eternity.spendRP` | `unities` | `unity.unities` |
| `passiveUnities` | `unity.passiveUnities` | `astrodust` | `unity.astrodust` |
| `singularities` | `singularity.singularity` | `atoms` | `singularity.atoms` |
| `PlP` | `plague.PlP` | `PlPperPlG` | `plague.PlPperPlG` |
| `PlG` | `plague.PlG` | `VE` | `plague.VE` |
| `ViP` | `plague.ViP` | `tarotSwords` | `tarot.swords` |
| `tarotWands` | `tarot.wands` | `tarotPentacles` | `tarot.pentacles` |
| `tarotCups` | `tarot.cups` | `goldTarotSwords` | `tarot.goldSwords` |
| `goldTarotWands` | `tarot.goldWands` | `goldTarotPentacles` | `tarot.goldPentacles` |
| `goldTarotCups` | `tarot.goldCups` | `tarotDraws` | `tarot.draws` |
| `timeSinceStart` | `timeSinceStart` | `timeInfinity` | `timeInf` |
| `timeEternity` | `timeEtr` | `timeUnity` | `timeUnity` |
| `timeTotal` | `timeTotal` |  |  |

## Uninstall

Remove `BepInEx/plugins/RevIdle.ScoreTelemetry`. If no other plugins use BepInEx, its loader can also be removed by deleting the added `BepInEx`, `dotnet`, `winhttp.dll`, `doorstop_config.ini`, `.doorstop_version`, and `changelog.txt` entries beside `Revolution Idle.exe`. This plugin does not edit save files.
