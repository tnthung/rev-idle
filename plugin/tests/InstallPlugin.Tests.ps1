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

function Assert-ThrowsLike {
    param([scriptblock]$Action, [string]$Pattern, [string]$Name)
    try {
        & $Action
    } catch {
        if ($_.Exception.Message -like $Pattern) {
            return
        }
        throw "$Name failed with an unexpected exception: $($_.Exception.Message)"
    }
    throw "$Name failed: expected an exception matching '$Pattern'."
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

    $launcherPath = Join-Path (Split-Path -Parent $PSScriptRoot) "install.cmd"
    $savedErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $launcherOutput = & $env:ComSpec /d /c "`"$launcherPath`" -InstallationFolder `"$testRoot`"" 2>&1
        $launcherExitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $savedErrorActionPreference
    }
    Assert-Equal 1 $launcherExitCode "install.cmd forwards the installer failure exit code"
    Assert-Equal $true (($launcherOutput -join [Environment]::NewLine) -like "*does not contain Revolution Idle.exe*") "install.cmd reaches install.ps1 with execution-policy bypass"

    $stateGame = Join-Path $testRoot "state-game"
    New-Item -ItemType Directory -Path $stateGame | Out-Null
    Assert-Equal "Missing" (Get-BepInExState $stateGame) "Get-BepInExState detects a missing runtime"
    New-Item -ItemType File -Path (Join-Path $stateGame "winhttp.dll") | Out-Null
    Assert-Equal "Partial" (Get-BepInExState $stateGame) "Get-BepInExState detects a partial runtime"
    foreach ($marker in @("doorstop_config.ini", "BepInEx\core\BepInEx.Core.dll", "BepInEx\core\BepInEx.Unity.IL2CPP.dll")) {
        $markerPath = Join-Path $stateGame $marker
        New-Item -ItemType Directory -Path (Split-Path -Parent $markerPath) -Force | Out-Null
        New-Item -ItemType File -Path $markerPath -Force | Out-Null
    }
    Assert-Equal "Complete" (Get-BepInExState $stateGame) "Get-BepInExState detects a complete runtime"

    $interopGame = Join-Path $testRoot "interop-game"
    foreach ($assembly in $InteropAssemblies) {
        $assemblyPath = Join-Path $interopGame $assembly
        New-Item -ItemType Directory -Path (Split-Path -Parent $assemblyPath) -Force | Out-Null
        Copy-Item -LiteralPath ([System.Management.Automation.PSObject].Assembly.Location) -Destination $assemblyPath
    }
    Assert-Equal $false (Test-InteropReady $interopGame) "Test-InteropReady waits for BepInEx generation to finish"
    New-Item -ItemType File -Path (Join-Path $interopGame "BepInEx\interop\assembly-hash.txt") | Out-Null
    Assert-Equal $true (Test-InteropReady $interopGame) "Test-InteropReady accepts completed BepInEx generation"

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

    $duplicateZip = Join-Path $testRoot "duplicate.zip"
    $duplicateStream = [System.IO.File]::Open($duplicateZip, [System.IO.FileMode]::CreateNew)
    $duplicateArchive = [System.IO.Compression.ZipArchive]::new($duplicateStream, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($content in @("first", "second")) {
            $entry = $duplicateArchive.CreateEntry("BepInEx/core/BepInEx.Core.dll")
            $writer = [System.IO.StreamWriter]::new($entry.Open())
            try {
                $writer.Write($content)
            } finally {
                $writer.Dispose()
            }
        }
    } finally {
        $duplicateArchive.Dispose()
        $duplicateStream.Dispose()
    }
    $rollbackDestination = Join-Path $testRoot "rollback-destination"
    Assert-Throws { Expand-SafeZipArchive $duplicateZip $rollbackDestination } "Expand-SafeZipArchive rejects duplicate entries"
    Assert-Equal $false (Test-Path -LiteralPath (Join-Path $rollbackDestination "BepInEx\core\BepInEx.Core.dll")) "Failed extraction leaves no partial file"

    $junctionExtractionDestination = Join-Path $testRoot "junction-extraction"
    $outsideExtractionDirectory = Join-Path $testRoot "outside-extraction"
    New-Item -ItemType Directory -Path $junctionExtractionDestination | Out-Null
    New-Item -ItemType Directory -Path $outsideExtractionDirectory | Out-Null
    $extractionJunction = Join-Path $junctionExtractionDestination "BepInEx"
    New-Item -ItemType Junction -Path $extractionJunction -Target $outsideExtractionDirectory | Out-Null
    try {
        Assert-Throws { Expand-SafeZipArchive $safeZip $junctionExtractionDestination } "Expand-SafeZipArchive rejects a junction destination"
    Assert-Equal $false (Test-Path -LiteralPath (Join-Path $outsideExtractionDirectory "core\sample.txt")) "Extraction junction target stays unchanged"
    } finally {
        if (Test-Path -LiteralPath $extractionJunction) {
            [System.IO.Directory]::Delete($extractionJunction)
        }
    }

    $newFileSource = Join-Path $testRoot "new-file-source.bin"
    $newFileDestination = Join-Path $testRoot "new-file-destination\installed.bin"
    [System.IO.File]::WriteAllBytes($newFileSource, [byte[]](5, 6, 7))
    Copy-NewFileAtomically $newFileSource $newFileDestination
    Assert-Equal "5,6,7" (([System.IO.File]::ReadAllBytes($newFileDestination)) -join ',') "Copy-NewFileAtomically commits a complete file"

    $lockedSource = Join-Path $testRoot "locked-source.bin"
    $failedCopyDestination = Join-Path $testRoot "failed-copy\installed.bin"
    [System.IO.File]::WriteAllBytes($lockedSource, [byte[]](8, 8, 8))
    $lockedStream = [System.IO.File]::Open($lockedSource, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::None)
    try {
        Assert-Throws { Copy-NewFileAtomically $lockedSource $failedCopyDestination } "Copy-NewFileAtomically surfaces copy failure"
    } finally {
        $lockedStream.Dispose()
    }
    Assert-Equal $false (Test-Path -LiteralPath $failedCopyDestination) "Failed new-file copy leaves no destination"
    $failedCopyDirectory = Split-Path -Parent $failedCopyDestination
    $failedCopyStagingFiles = @(if (Test-Path -LiteralPath $failedCopyDirectory) {
        Get-ChildItem -LiteralPath $failedCopyDirectory | Where-Object { $_.Name.EndsWith('.tmp') }
    })
    Assert-Equal 0 $failedCopyStagingFiles.Count "Failed new-file copy removes staging files"

    $collisionDestination = Join-Path $testRoot "collision\installed.bin"
    New-Item -ItemType Directory -Path (Split-Path -Parent $collisionDestination) | Out-Null
    [System.IO.File]::WriteAllBytes($collisionDestination, [byte[]](4, 4, 4))
    Assert-Throws { Copy-NewFileAtomically $newFileSource $collisionDestination } "Copy-NewFileAtomically rejects a concurrent destination"
    Assert-Equal "4,4,4" (([System.IO.File]::ReadAllBytes($collisionDestination)) -join ',') "Rejected destination remains owned by its creator"

    $sourceDll = Join-Path $testRoot "RevIdle.ScoreTelemetry.dll"
    [System.IO.File]::WriteAllBytes($sourceDll, [byte[]](1, 2, 3, 4))

    $junctionGame = Join-Path $testRoot "junction-game"
    $junctionPlugins = Join-Path $junctionGame "BepInEx\plugins"
    $outsidePluginDirectory = Join-Path $testRoot "outside-plugin"
    New-Item -ItemType Directory -Path $junctionPlugins -Force | Out-Null
    New-Item -ItemType Directory -Path $outsidePluginDirectory | Out-Null
    $pluginJunction = Join-Path $junctionPlugins "RevIdle.ScoreTelemetry"
    New-Item -ItemType Junction -Path $pluginJunction -Target $outsidePluginDirectory | Out-Null
    try {
        Assert-Throws { Install-PluginDll $sourceDll $junctionGame } "Install-PluginDll rejects a junction destination"
        Assert-Equal $false (Test-Path -LiteralPath (Join-Path $outsidePluginDirectory "RevIdle.ScoreTelemetry.dll")) "Junction target stays unchanged"
    } finally {
        if (Test-Path -LiteralPath $pluginJunction) {
            [System.IO.Directory]::Delete($pluginJunction)
        }
    }

    $atomicDirectory = Join-Path $testRoot "atomic-plugin"
    $atomicDestination = Join-Path $atomicDirectory "RevIdle.ScoreTelemetry.dll"
    New-Item -ItemType Directory -Path $atomicDirectory | Out-Null
    [System.IO.File]::WriteAllBytes($atomicDestination, [byte[]](9, 9, 9))
    Assert-ThrowsLike { Install-VerifiedFile $sourceDll $atomicDestination ("0" * 64) } "Staged file SHA-256 mismatch*" "Install-VerifiedFile rejects a bad staged hash"
    Assert-Equal "9,9,9" (([System.IO.File]::ReadAllBytes($atomicDestination)) -join ',') "Failed staged verification preserves the installed DLL"
    $sourceDllHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $sourceDll).Hash
    Install-VerifiedFile $sourceDll $atomicDestination $sourceDllHash
    Assert-Equal "1,2,3,4" (([System.IO.File]::ReadAllBytes($atomicDestination)) -join ',') "Install-VerifiedFile atomically replaces the installed DLL"
    $stagingFiles = @(Get-ChildItem -LiteralPath $atomicDirectory | Where-Object {
        $_.Name.EndsWith('.tmp') -or $_.Name.EndsWith('.backup')
    })
    Assert-Equal 0 $stagingFiles.Count "Install-VerifiedFile removes staging files"

    $installedDll = Install-PluginDll $sourceDll $gameDir
    $sourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $sourceDll).Hash
    $installedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedDll).Hash
    Assert-Equal $sourceHash $installedHash "Install-PluginDll preserves bytes"

    $sourceClient = Join-Path $testRoot "client.exe"
    [System.IO.File]::WriteAllBytes($sourceClient, [byte[]](11, 12, 13, 14))
    $installedClient = Install-ClientExe $sourceClient $gameDir
    $clientHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $sourceClient).Hash
    Assert-Equal $clientHash (Get-FileHash -Algorithm SHA256 -LiteralPath $installedClient).Hash "Install-ClientExe preserves bytes"
    Assert-Equal (Join-Path $gameDir "client.exe") $installedClient "Install-ClientExe installs in the game root"

    $installFixtureRoot = Join-Path $testRoot "install-fixture"
    $installFixturePlugin = Join-Path $installFixtureRoot "plugin"
    New-Item -ItemType Directory -Path $installFixturePlugin -Force | Out-Null
    Copy-Item -LiteralPath $installerPath -Destination (Join-Path $installFixturePlugin "install.ps1")
    [System.IO.File]::WriteAllText((Join-Path $installFixturePlugin "generate-state-reference.ps1"), @'
param([string]$GameDir)
[System.IO.File]::WriteAllText((Join-Path $PSScriptRoot 'generated-state-reference.txt'), $GameDir)
'@)
    . (Join-Path $installFixturePlugin "install.ps1")
    function Get-BepInExState { return "Complete" }
    function Test-InteropReady { return $true }
    function Select-TestTargetFramework { return "net8.0" }
    function Invoke-CheckedProcess {}
    function Install-PluginDll { return (Join-Path $gameDir "RevIdle.ScoreTelemetry.dll") }
    function Install-ClientExe { return (Join-Path $gameDir "client.exe") }
    Invoke-ScoreTelemetryInstall $gameDir
    $generatedStateReference = Join-Path $installFixturePlugin "generated-state-reference.txt"
    Assert-Equal $true (Test-Path -LiteralPath $generatedStateReference -PathType Leaf) "installer generates the state reference"
    Assert-Equal ([System.IO.Path]::GetFullPath($gameDir)) ([System.IO.File]::ReadAllText($generatedStateReference)) "installer passes the resolved game directory to the state generator"

    Write-Output "17 installer tests passed."
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}
