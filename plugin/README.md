# Revolution Idle Score Telemetry

This proof-of-concept BepInEx plugin sends the live Revolution Idle state to a local UDP listener every 50 ms.

Payload:

```json
{"score":"1.2345678901234567e123","income":"5e90","timeInf":123.4}
```

All readable top-level `GameData` fields are emitted alongside `score`. Primitive numeric and Boolean values keep their JSON types; complex values use their invariant string representation. Large-number values use the same mantissa/exponent string representation as `score`.

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

`0`, non-numeric values, and values outside `1..65535` disable output. The destination is always `127.0.0.1`; send failures are intentionally ignored.

## Background UI click probe

The plugin accepts only the fixed background-input bridge message and dispatches matching clicks to Unity uGUI handlers. It does not use `Input.GetMouseButtonDown()`, focus the game window, or move the cursor by default.

Run the background click probe from the repository root:

```powershell
cargo +1.97.1 run --manifest-path client/Cargo.toml --bin window_message_probe
```

Add `--focus` only when explicit window focus is desired.

## Receive a packet

Run this in PowerShell before starting the game:

```powershell
$udp = [Net.Sockets.UdpClient]::new(19841)
try {
    while ($true) {
        $packet = $udp.ReceiveAsync().GetAwaiter().GetResult()
        [Text.Encoding]::UTF8.GetString($packet.Buffer)
    }
} finally {
    $udp.Dispose()
}
```

## Uninstall

Remove `BepInEx/plugins/RevIdle.ScoreTelemetry`. If no other plugins use BepInEx, its loader can also be removed by deleting the added `BepInEx`, `dotnet`, `winhttp.dll`, `doorstop_config.ini`, `.doorstop_version`, and `changelog.txt` entries beside `Revolution Idle.exe`. This plugin does not edit save files.
