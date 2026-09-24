# Client UDP Listener and Automated Plugin Installer Design

## Goal

Turn the existing Rust client into a localhost UDP receiver for Revolution Idle score telemetry, and provide one PowerShell script that prompts for the game installation folder and completes the BepInEx bootstrap, interop generation, plugin build, and plugin installation.

## Scope

The change adds two related entry points:

- `client/src/main.rs` listens on `127.0.0.1:19841` and prints every received datagram.
- `plugin/install.ps1` performs the complete Windows installation workflow.

The existing plugin payload, destination, and update interval remain unchanged. The installer supports the Windows x64 Steam build of Revolution Idle only.

## Client Listener

The Tokio client binds an IPv4 `UdpSocket` to `127.0.0.1:19841`. It allocates one maximum-size UDP buffer and continuously waits for datagrams. Each datagram is decoded with UTF-8 lossy conversion and printed exactly once with a trailing console newline. Lossy conversion ensures that arbitrary received bytes are observable instead of terminating the listener.

Failure to bind or receive is fatal and returns an error from `main`; the operating-system error remains visible to the user. The listener does not parse JSON, transform score values, send acknowledgements, or accept remote-network traffic.

The receive operation is separated into a small async function so tests can use real loopback sockets. Tests cover an exact UTF-8 payload and invalid UTF-8 bytes without mocking Tokio networking.

## Installer Interface

`plugin/install.ps1` accepts an optional `-InstallationFolder` parameter. If it is absent, the script calls `Read-Host` and asks the user for the Revolution Idle installation folder. This preserves interactive use while allowing automated smoke tests.

The folder must contain `Revolution Idle.exe`. The script resolves the supplied path before making changes and rejects missing or incorrect folders.

## BepInEx Bootstrap

The installer checks for `winhttp.dll`, `doorstop_config.ini`, `BepInEx/core/BepInEx.Core.dll`, and `BepInEx/core/BepInEx.Unity.IL2CPP.dll`. A complete existing installation is preserved and reused. If none of those files exists, the script downloads this exact official artifact over HTTPS:

- Version: `6.0.0-be.785+6abdba4`
- Artifact: `BepInEx-Unity.IL2CPP-win-x64-6.0.0-be.785+6abdba4.zip`
- URL: `https://builds.bepinex.dev/projects/bepinex_be/785/BepInEx-Unity.IL2CPP-win-x64-6.0.0-be.785%2B6abdba4.zip`
- SHA-256: `2A7CBF74D26ABE4765C3E662DB1721B923BAC39849EBFEF2CA5DC7DE7E2D9B7F`

The archive is downloaded to a unique temporary directory. The installer verifies its SHA-256 before extraction. Every ZIP entry is resolved against the game folder and rejected if it would escape that folder. After extraction, the installer verifies the expected BepInEx files and removes the temporary directory in a `finally` block.

If at least one but not all required BepInEx files exists, the script stops with a clear error instead of silently overwriting an unknown loader configuration.

## Interop Generation

The plugin build requires game-specific assemblies under `BepInEx/interop`. If the required assemblies and BepInEx's `assembly-hash.txt` completion marker already exist, the installer proceeds immediately. Otherwise it launches `Revolution Idle.exe`, waits up to five minutes for the completion marker and all required interop assemblies to appear, and reports that first-time generation may take several minutes.

The installer records the exact process it launches. Once interop generation succeeds or times out, it stops only that process if it is still running. It never stops a pre-existing Revolution Idle process. If the game is already running while interop files are missing, installation stops and asks the user to close the game before retrying.

## Build and Installation

After the runtime and interop prerequisites are ready, the installer runs:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj --property:GameDir=<installation folder>
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release --property:GameDir=<installation folder>
```

Both commands must exit successfully. The script then creates `BepInEx/plugins/RevIdle.ScoreTelemetry`, copies only `RevIdle.ScoreTelemetry.dll`, and verifies that the source and installed DLL SHA-256 hashes match. It prints the installed path and next step on success.

Missing `dotnet`, download failures, unsafe or corrupted archives, game launch failure, interop timeout, build failure, and copy/hash failure all terminate with actionable error messages and a non-zero exit code.

## Testing

Development follows red-green TDD:

- Rust unit tests use real loopback UDP sockets and initially fail before the receive helper exists.
- PowerShell tests dot-source the not-yet-existing installer to establish RED, then exercise its path-validation, archive-verification, safe-extraction, and DLL-copy helpers against temporary directories and synthetic ZIP fixtures. The installer does not start when it is dot-sourced.
- Archive helpers accept an expected hash supplied by the orchestration layer, allowing tests to use small local fixtures while production always supplies the pinned official SHA-256.
- The final validation runs `cargo test` in `client`, the installer smoke test, the plugin test executable, and the plugin Release build with clean output.

No live game is launched by automated tests. A bounded manual verification runs the complete installer against the real local game folder only after automated checks pass.
