# Client UDP Listener and Automated Plugin Installer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the placeholder Rust client with a localhost score datagram listener and add one interactive PowerShell installer that bootstraps BepInEx, generates interop assemblies, builds, tests, and installs the plugin.

**Architecture:** The Tokio client uses one real IPv4 loopback `UdpSocket` and a testable single-datagram receive helper. The PowerShell installer keeps orchestration and small testable filesystem/archive helpers in one dot-sourceable script, pins the verified BepInEx archive and hash, launches only the requested game executable when interop generation is required, and installs one verified plugin DLL.

**Tech Stack:** Rust 2024, Tokio 1.53, PowerShell 5.1+, .NET SDK 9, BepInEx 6 IL2CPP build `6.0.0-be.785+6abdba4`.

**Spec:** `docs/superpowers/specs/2026-09-01-client-listener-installer-design.md`

## Global Constraints

- Bind only IPv4 loopback `127.0.0.1:19841`; do not accept remote-network traffic.
- Print every datagram exactly once using lossy UTF-8 conversion; do not parse or transform the JSON.
- Support Windows x64 Revolution Idle only and require `Revolution Idle.exe` in the supplied installation folder.
- Prompt with `Read-Host` when `-InstallationFolder` is omitted.
- Pin BepInEx version `6.0.0-be.785+6abdba4`, URL `https://builds.bepinex.dev/projects/bepinex_be/785/BepInEx-Unity.IL2CPP-win-x64-6.0.0-be.785%2B6abdba4.zip`, and SHA-256 `2A7CBF74D26ABE4765C3E662DB1721B923BAC39849EBFEF2CA5DC7DE7E2D9B7F`.
- Preserve a complete existing BepInEx installation, install only when all loader markers are absent, and reject partial installations.
- Reject archive entries that resolve outside the game folder and reject archive hash mismatches before extraction.
- Stop only the exact game process launched by the installer; never stop a pre-existing process.
- Run plugin tests and a warning-free Release build using the supplied game directory before copying the plugin DLL.
- Copy only `RevIdle.ScoreTelemetry.dll` into `BepInEx/plugins/RevIdle.ScoreTelemetry` and verify source/destination SHA-256 equality.

---

### Task 1: Receive and print UDP datagrams in the Rust client

**Files:**
- Modify: `client/src/main.rs`

**Interfaces:**
- Consumes: Tokio `UdpSocket::bind`, `UdpSocket::recv_from`, and the plugin's fixed default endpoint `127.0.0.1:19841`.
- Produces: `async fn receive_datagram(socket: &UdpSocket, buffer: &mut [u8]) -> io::Result<String>` and a binary that prints one received datagram per loop iteration.

- [ ] **Step 1: Add failing real-socket tests**

Replace `client/src/main.rs` with the existing placeholder `main` plus this test module. Do not add `receive_datagram` yet.

```rust
#[tokio::main]
async fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn receives_exact_utf8_datagram() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let payload = br#"{"score":"2.5e42"}"#;

        sender.send_to(payload, receiver.local_addr().unwrap()).await.unwrap();

        let mut buffer = [0_u8; 65_535];
        let received = receive_datagram(&receiver, &mut buffer).await.unwrap();
        assert_eq!(received, r#"{"score":"2.5e42"}"#);
    }

    #[tokio::test]
    async fn replaces_invalid_utf8_without_dropping_datagram() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();

        sender
            .send_to(&[b'f', 0xff, b'o'], receiver.local_addr().unwrap())
            .await
            .unwrap();

        let mut buffer = [0_u8; 65_535];
        let received = receive_datagram(&receiver, &mut buffer).await.unwrap();
        assert_eq!(received, "f\u{fffd}o");
    }
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml
```

Expected: exit code `101` with `cannot find function receive_datagram in this scope`. If failure is caused by syntax, dependency resolution, or a different error, correct the test and rerun until the missing production function is the reason.

- [ ] **Step 3: Implement the minimal listener**

Replace the placeholder production code above the tests with:

```rust
use std::io;
use tokio::net::UdpSocket;

const LISTEN_ADDRESS: &str = "127.0.0.1:19841";

async fn receive_datagram(socket: &UdpSocket, buffer: &mut [u8]) -> io::Result<String> {
    let (length, _) = socket.recv_from(buffer).await?;
    Ok(String::from_utf8_lossy(&buffer[..length]).into_owned())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = UdpSocket::bind(LISTEN_ADDRESS).await?;
    let mut buffer = [0_u8; 65_535];

    loop {
        println!("{}", receive_datagram(&socket, &mut buffer).await?);
    }
}
```

Keep the test module unchanged below it.

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml
```

Expected: both tests pass with no warnings.

- [ ] **Step 5: Verify the binary receives a real packet**

Start `cargo run --manifest-path client/Cargo.toml` as the exact process under test, send `{"score":"2.5e42"}` to `127.0.0.1:19841` with a one-shot `UdpClient`, assert the captured stdout contains that exact line, and stop only the process started for this check.

Expected captured line:

```text
{"score":"2.5e42"}
```

- [ ] **Step 6: Commit the client listener**

```powershell
git add -- client/src/main.rs
git commit -m "feat: receive score telemetry in client"
```

---

### Task 2: Automate BepInEx bootstrap, plugin build, and installation

**Files:**
- Create: `plugin/install.ps1`
- Create: `plugin/tests/InstallPlugin.Tests.ps1`
- Modify: `plugin/README.md`

**Interfaces:**
- Consumes: optional `-InstallationFolder`, `Revolution Idle.exe`, official pinned BepInEx ZIP, .NET SDK, existing plugin/test projects, and required BepInEx core/interop assemblies.
- Produces: `Resolve-GameDirectory`, `Assert-ArchiveHash`, `Expand-SafeZipArchive`, `Get-BepInExState`, `Test-InteropReady`, `Install-PluginDll`, and executable `Invoke-ScoreTelemetryInstall`; installed `BepInEx/plugins/RevIdle.ScoreTelemetry/RevIdle.ScoreTelemetry.dll`.

- [ ] **Step 1: Write failing PowerShell helper tests**

Create `plugin/tests/InstallPlugin.Tests.ps1`:

```powershell
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$installerPath = Join-Path (Split-Path -Parent $PSScriptRoot) "install.ps1"
. $installerPath

function Assert-Equal {
    param($Expected, $Actual, [string]$Name)
    if ($Expected -ne $Actual) {
        throw "$Name failed: expected '$Expected', got '$Actual'."
    }
}

function Assert-Throws {
    param([scriptblock]$Action, [string]$Name)
    $threw = $false
    try {
        & $Action
    } catch {
        $threw = $true
    }
    if (-not $threw) {
        throw "$Name failed: expected an exception."
    }
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("rev-idle-installer-tests-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $testRoot | Out-Null

try {
    $gameDir = Join-Path $testRoot "Revolution Idle"
    New-Item -ItemType Directory -Path $gameDir | Out-Null
    New-Item -ItemType File -Path (Join-Path $gameDir "Revolution Idle.exe") | Out-Null
    Assert-Equal ([System.IO.Path]::GetFullPath($gameDir)) (Resolve-GameDirectory $gameDir) "Resolve-GameDirectory accepts the game root"
    Assert-Throws { Resolve-GameDirectory (Join-Path $testRoot "missing") } "Resolve-GameDirectory rejects a missing folder"

    $fixture = Join-Path $testRoot "fixture"
    $fixtureCore = Join-Path $fixture "BepInEx\core"
    New-Item -ItemType Directory -Path $fixtureCore -Force | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $fixtureCore "sample.txt"), "fixture")
    $safeZip = Join-Path $testRoot "safe.zip"
    [System.IO.Compression.ZipFile]::CreateFromDirectory($fixture, $safeZip)
    $safeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $safeZip).Hash
    Assert-ArchiveHash $safeZip $safeHash
    Assert-Throws { Assert-ArchiveHash $safeZip ("0" * 64) } "Assert-ArchiveHash rejects a mismatch"

    $safeDestination = Join-Path $testRoot "safe-destination"
    Expand-SafeZipArchive $safeZip $safeDestination
    Assert-Equal "fixture" ([System.IO.File]::ReadAllText((Join-Path $safeDestination "BepInEx\core\sample.txt"))) "Expand-SafeZipArchive extracts a safe entry"

    $unsafeZip = Join-Path $testRoot "unsafe.zip"
    $unsafeStream = [System.IO.File]::Open($unsafeZip, [System.IO.FileMode]::CreateNew)
    $unsafeArchive = [System.IO.Compression.ZipArchive]::new($unsafeStream, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        $entry = $unsafeArchive.CreateEntry("../escape.txt")
        $writer = [System.IO.StreamWriter]::new($entry.Open())
        try {
            $writer.Write("escape")
        } finally {
            $writer.Dispose()
        }
    } finally {
        $unsafeArchive.Dispose()
        $unsafeStream.Dispose()
    }
    Assert-Throws { Expand-SafeZipArchive $unsafeZip (Join-Path $testRoot "unsafe-destination") } "Expand-SafeZipArchive rejects traversal"
    Assert-Equal $false (Test-Path -LiteralPath (Join-Path $testRoot "escape.txt")) "Unsafe entry stays unextracted"

    $sourceDll = Join-Path $testRoot "RevIdle.ScoreTelemetry.dll"
    [System.IO.File]::WriteAllBytes($sourceDll, [byte[]](1, 2, 3, 4))
    $installedDll = Install-PluginDll $sourceDll $gameDir
    Assert-Equal (Get-FileHash -Algorithm SHA256 -LiteralPath $sourceDll).Hash (Get-FileHash -Algorithm SHA256 -LiteralPath $installedDll).Hash "Install-PluginDll preserves bytes"

    Write-Output "4 installer tests passed."
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}
```

- [ ] **Step 2: Run the installer tests and verify RED**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File plugin/tests/InstallPlugin.Tests.ps1
```

Expected: non-zero exit because `plugin/install.ps1` does not exist. This is the missing production entry point, not a test syntax failure.

- [ ] **Step 3: Implement the tested installer helpers and orchestration**

Create `plugin/install.ps1`:

```powershell
[CmdletBinding()]
param(
    [string]$InstallationFolder
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$BepInExUrl = "https://builds.bepinex.dev/projects/bepinex_be/785/BepInEx-Unity.IL2CPP-win-x64-6.0.0-be.785%2B6abdba4.zip"
$BepInExSha256 = "2A7CBF74D26ABE4765C3E662DB1721B923BAC39849EBFEF2CA5DC7DE7E2D9B7F"
$BepInExMarkers = @(
    "winhttp.dll",
    "doorstop_config.ini",
    "BepInEx\core\BepInEx.Core.dll",
    "BepInEx\core\BepInEx.Unity.IL2CPP.dll"
)
$InteropAssemblies = @(
    "BepInEx\interop\Assembly-CSharp.dll",
    "BepInEx\interop\Il2Cppmscorlib.dll",
    "BepInEx\interop\UnityEngine.CoreModule.dll",
    "BepInEx\interop\UnityEngine.dll"
)

function Resolve-GameDirectory {
    param([string]$Candidate)

    if ([string]::IsNullOrWhiteSpace($Candidate)) {
        throw "The Revolution Idle installation folder is required."
    }

    $trimmed = $Candidate.Trim().Trim('"')
    $resolved = (Resolve-Path -LiteralPath $trimmed -ErrorAction Stop).Path
    if (-not (Test-Path -LiteralPath (Join-Path $resolved "Revolution Idle.exe") -PathType Leaf)) {
        throw "The selected folder does not contain Revolution Idle.exe: $resolved"
    }

    return [System.IO.Path]::GetFullPath($resolved)
}

function Assert-ArchiveHash {
    param([string]$ArchivePath, [string]$ExpectedSha256)

    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $ArchivePath).Hash
    if (-not $actual.Equals($ExpectedSha256, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "BepInEx archive SHA-256 mismatch. Expected $ExpectedSha256, got $actual."
    }
}

function Expand-SafeZipArchive {
    param([string]$ArchivePath, [string]$DestinationPath)

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    New-Item -ItemType Directory -Path $DestinationPath -Force | Out-Null

    $destinationRoot = [System.IO.Path]::GetFullPath($DestinationPath).TrimEnd('\') + '\'
    $archive = [System.IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        $targets = @()
        foreach ($entry in $archive.Entries) {
            $relativePath = $entry.FullName.Replace('/', '\')
            $targetPath = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($destinationRoot, $relativePath))
            if (-not $targetPath.StartsWith($destinationRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "Unsafe ZIP entry: $($entry.FullName)"
            }
            if (-not $entry.FullName.EndsWith('/') -and (Test-Path -LiteralPath $targetPath)) {
                throw "Refusing to overwrite an existing file while installing BepInEx: $targetPath"
            }
            $targets += [pscustomobject]@{ Entry = $entry; Path = $targetPath }
        }

        foreach ($target in $targets) {
            if ($target.Entry.FullName.EndsWith('/')) {
                New-Item -ItemType Directory -Path $target.Path -Force | Out-Null
                continue
            }
            $parent = Split-Path -Parent $target.Path
            New-Item -ItemType Directory -Path $parent -Force | Out-Null
            [System.IO.Compression.ZipFileExtensions]::ExtractToFile($target.Entry, $target.Path, $false)
        }
    } finally {
        $archive.Dispose()
    }
}

function Get-BepInExState {
    param([string]$GameDirectory)

    $presentCount = @($BepInExMarkers | Where-Object {
        Test-Path -LiteralPath (Join-Path $GameDirectory $_) -PathType Leaf
    }).Count

    if ($presentCount -eq 0) { return "Missing" }
    if ($presentCount -eq $BepInExMarkers.Count) { return "Complete" }
    return "Partial"
}

function Install-BepInEx {
    param([string]$GameDirectory)

    $temporaryDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("rev-idle-bepinex-" + [guid]::NewGuid().ToString("N"))
    $archivePath = Join-Path $temporaryDirectory "BepInEx.zip"
    New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
    try {
        Write-Host "Downloading BepInEx 6.0.0-be.785+6abdba4..."
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -UseBasicParsing -Uri $BepInExUrl -OutFile $archivePath
        Assert-ArchiveHash $archivePath $BepInExSha256
        Expand-SafeZipArchive $archivePath $GameDirectory
    } finally {
        if (Test-Path -LiteralPath $temporaryDirectory) {
            Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force
        }
    }

    if ((Get-BepInExState $GameDirectory) -ne "Complete") {
        throw "BepInEx extraction completed without the required loader files."
    }
}

function Test-InteropReady {
    param([string]$GameDirectory)

    return @($InteropAssemblies | Where-Object {
        -not (Test-Path -LiteralPath (Join-Path $GameDirectory $_) -PathType Leaf)
    }).Count -eq 0
}

function Initialize-Interop {
    param([string]$GameDirectory)

    if (Get-Process -Name "Revolution Idle" -ErrorAction SilentlyContinue) {
        throw "Revolution Idle is already running. Close it and run the installer again."
    }

    Write-Host "Launching Revolution Idle once to generate BepInEx interop assemblies. This can take several minutes..."
    $gameExecutable = Join-Path $GameDirectory "Revolution Idle.exe"
    $gameProcess = Start-Process -FilePath $gameExecutable -WorkingDirectory $GameDirectory -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddMinutes(5)
        while ([DateTime]::UtcNow -lt $deadline) {
            if (Test-InteropReady $GameDirectory) {
                return
            }
            if ($gameProcess.HasExited) {
                break
            }
            Start-Sleep -Seconds 1
            $gameProcess.Refresh()
        }
        throw "BepInEx interop generation did not finish within five minutes. Check BepInEx\LogOutput.log and retry."
    } finally {
        try {
            $gameProcess.Refresh()
            if (-not $gameProcess.HasExited) {
                Stop-Process -InputObject $gameProcess -Force
            }
        } catch [System.InvalidOperationException] {
            # The exact process started above has already exited.
        }
    }
}

function Invoke-CheckedProcess {
    param([string]$FilePath, [string[]]$Arguments, [string]$FailureMessage)

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$FailureMessage Exit code: $LASTEXITCODE."
    }
}

function Install-PluginDll {
    param([string]$SourceDll, [string]$GameDirectory)

    if (-not (Test-Path -LiteralPath $SourceDll -PathType Leaf)) {
        throw "Built plugin DLL is missing: $SourceDll"
    }

    $pluginDirectory = Join-Path $GameDirectory "BepInEx\plugins\RevIdle.ScoreTelemetry"
    $installedDll = Join-Path $pluginDirectory "RevIdle.ScoreTelemetry.dll"
    New-Item -ItemType Directory -Path $pluginDirectory -Force | Out-Null
    Copy-Item -LiteralPath $SourceDll -Destination $installedDll -Force

    $sourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $SourceDll).Hash
    $installedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedDll).Hash
    if ($sourceHash -ne $installedHash) {
        throw "Installed plugin DLL hash does not match the built artifact."
    }

    return $installedDll
}

function Invoke-ScoreTelemetryInstall {
    param([string]$RequestedInstallationFolder)

    if ([string]::IsNullOrWhiteSpace($RequestedInstallationFolder)) {
        $RequestedInstallationFolder = Read-Host "Enter the Revolution Idle installation folder"
    }
    $gameDirectory = Resolve-GameDirectory $RequestedInstallationFolder

    switch (Get-BepInExState $gameDirectory) {
        "Missing" { Install-BepInEx $gameDirectory }
        "Partial" { throw "A partial BepInEx installation was found. Repair or remove it before retrying." }
        "Complete" { Write-Host "Using the existing BepInEx installation." }
    }

    if (-not (Test-InteropReady $gameDirectory)) {
        Initialize-Interop $gameDirectory
    }

    if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
        throw "The .NET SDK is required. Install .NET SDK 9 and retry."
    }

    $repositoryRoot = Split-Path -Parent $PSScriptRoot
    $testProject = Join-Path $repositoryRoot "plugin\tests\RevIdle.ScoreTelemetry.Tests.csproj"
    $pluginProject = Join-Path $repositoryRoot "plugin\src\RevolutionIdle.ScoreTelemetry.csproj"
    $builtDll = Join-Path $repositoryRoot "plugin\src\bin\Release\RevIdle.ScoreTelemetry.dll"
    Invoke-CheckedProcess "dotnet" @("run", "--project", $testProject, "-p:GameDir=$gameDirectory") "Plugin tests failed."
    Invoke-CheckedProcess "dotnet" @("build", $pluginProject, "-c", "Release", "-p:GameDir=$gameDirectory") "Plugin build failed."

    $installedDll = Install-PluginDll $builtDll $gameDirectory
    Write-Host "Installed Revolution Idle Score Telemetry to: $installedDll"
    Write-Host "Start the client, then start Revolution Idle."
}

if ($MyInvocation.InvocationName -ne '.') {
    try {
        Invoke-ScoreTelemetryInstall $InstallationFolder
    } catch {
        Write-Error $_.Exception.Message
        exit 1
    }
}
```

- [ ] **Step 4: Run installer helper tests and verify GREEN**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File plugin/tests/InstallPlugin.Tests.ps1
```

Expected exact final line and exit code `0`:

```text
4 installer tests passed.
```

- [ ] **Step 5: Add automated installation documentation**

Insert this section in `plugin/README.md` before the manual Build section:

````markdown
## Automated install

From the repository root, run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\plugin\install.ps1
```

The installer asks for the Revolution Idle installation folder. If BepInEx is missing, it downloads and verifies the pinned Windows x64 IL2CPP build, launches the game once to generate interop assemblies, runs the plugin tests and Release build, and installs the verified DLL.

For non-interactive use:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\plugin\install.ps1 -InstallationFolder "D:\SteamLibrary\steamapps\common\Revolution Idle"
```
````

Keep the existing manual Build, Install, configuration, receiver, and uninstall sections intact.

- [ ] **Step 6: Run the complete automated checks**

Run:

```powershell
cargo test --manifest-path client/Cargo.toml
powershell -NoProfile -ExecutionPolicy Bypass -File plugin/tests/InstallPlugin.Tests.ps1
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release
```

Expected: two Rust tests pass, exact `4 installer tests passed.`, exact `4 tests passed.`, and the Release build exits `0` with `0 Warning(s)` and `0 Error(s)`.

- [ ] **Step 7: Run bounded real-folder verification**

Run the installer non-interactively against:

```text
C:\Program Files (x86)\Steam\steamapps\common\Revolution Idle
```

Expected: it reuses the complete existing BepInEx installation, skips interop generation because the required assemblies exist, runs tests and the Release build, installs the DLL, and reports the installed path. Verify the built and installed DLL SHA-256 values are identical. Do not launch or stop the game in this existing-runtime path.

- [ ] **Step 8: Review scope and commit the installer**

Run `git diff --check` and confirm only these files changed in Task 2:

```text
plugin/install.ps1
plugin/tests/InstallPlugin.Tests.ps1
plugin/README.md
```

Then commit:

```powershell
git add -- plugin/install.ps1 plugin/tests/InstallPlugin.Tests.ps1 plugin/README.md
git commit -m "feat: automate score plugin installation"
```

---

## Final Verification

- Run the full automated command set from Task 2 Step 6 on the final branch state.
- Run `git diff --check "$(git merge-base main HEAD)"..HEAD` and require no output.
- Confirm the worktree has no tracked or untracked task files outside the plan's declared paths.
- Review the complete branch diff against `docs/superpowers/specs/2026-09-01-client-listener-installer-design.md`.
- Confirm no process other than a process explicitly started by verification was stopped.
