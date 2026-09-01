# Revolution Idle Score Telemetry

This proof-of-concept BepInEx plugin sends the live Revolution Idle score to a local UDP listener every 50 ms.

Payload:

```json
{"score":"1.2345678901234567e123"}
```

## Requirements

- Revolution Idle for Windows x64 (Steam)
- BepInEx 6 IL2CPP build 785 or newer with generated `BepInEx/interop` assemblies
- .NET SDK 9 for building

The plugin itself targets .NET 6 because that is the runtime embedded by BepInEx IL2CPP.

## Automated install

From the repository root, run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\plugin\install.ps1
```

The installer asks for the Revolution Idle installation folder. If BepInEx is missing, it downloads and verifies the pinned Windows x64 IL2CPP build, launches the game once to generate interop assemblies, runs the plugin tests and Release build, and installs the verified DLL.

If only part of a BepInEx installation is present, the installer stops without overwriting it. Repair or remove that partial installation before retrying.

For non-interactive use:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\plugin\install.ps1 -InstallationFolder "D:\SteamLibrary\steamapps\common\Revolution Idle"
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
