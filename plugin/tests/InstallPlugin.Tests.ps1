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
    $sourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $sourceDll).Hash
    $installedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedDll).Hash
    Assert-Equal $sourceHash $installedHash "Install-PluginDll preserves bytes"

    Write-Output "4 installer tests passed."
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}
