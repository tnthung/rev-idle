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
