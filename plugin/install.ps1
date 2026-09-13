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

function Assert-NoReparsePointPath {
    param([string]$Path)

    $currentPath = [System.IO.Path]::GetFullPath($Path)
    while (-not (Test-Path -LiteralPath $currentPath)) {
        $parentPath = Split-Path -Parent $currentPath
        if ([string]::IsNullOrEmpty($parentPath) -or $parentPath -eq $currentPath) {
            break
        }
        $currentPath = $parentPath
    }

    while (-not [string]::IsNullOrEmpty($currentPath)) {
        $item = Get-Item -LiteralPath $currentPath -Force
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Refusing to write through a reparse point: $($item.FullName)"
        }
        $parentPath = Split-Path -Parent $currentPath
        if ([string]::IsNullOrEmpty($parentPath) -or $parentPath -eq $currentPath) {
            break
        }
        $currentPath = $parentPath
    }
}

function Copy-NewFileAtomically {
    param([string]$SourcePath, [string]$DestinationPath)

    if (Test-Path -LiteralPath $DestinationPath) {
        throw "Refusing to overwrite an existing file: $DestinationPath"
    }

    $destinationDirectory = Split-Path -Parent $DestinationPath
    Assert-NoReparsePointPath $destinationDirectory
    New-Item -ItemType Directory -Path $destinationDirectory -Force | Out-Null
    Assert-NoReparsePointPath $DestinationPath

    $destinationName = [System.IO.Path]::GetFileName($DestinationPath)
    $stagedPath = Join-Path $destinationDirectory ("$destinationName." + [guid]::NewGuid().ToString("N") + ".tmp")
    $committed = $false
    try {
        [System.IO.File]::Copy($SourcePath, $stagedPath, $false)
        Assert-NoReparsePointPath $DestinationPath
        [System.IO.File]::Move($stagedPath, $DestinationPath)
        $committed = $true
    } catch {
        if ($committed -and (Test-Path -LiteralPath $DestinationPath -PathType Leaf)) {
            Remove-Item -LiteralPath $DestinationPath -Force
        }
        throw
    } finally {
        if (Test-Path -LiteralPath $stagedPath) {
            Remove-Item -LiteralPath $stagedPath -Force
        }
    }
}

function Expand-SafeZipArchive {
    param([string]$ArchivePath, [string]$DestinationPath)

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $destinationRoot = [System.IO.Path]::GetFullPath($DestinationPath).TrimEnd('\')
    $destinationPrefix = $destinationRoot + '\'
    Assert-NoReparsePointPath $destinationRoot
    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("rev-idle-zip-stage-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $stagingRoot | Out-Null

    try {
        $stagingPrefix = [System.IO.Path]::GetFullPath($stagingRoot).TrimEnd('\') + '\'
        $archive = [System.IO.Compression.ZipFile]::OpenRead($ArchivePath)
        try {
            foreach ($entry in $archive.Entries) {
                $relativePath = $entry.FullName.Replace('/', '\')
                $stagedPath = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($stagingPrefix, $relativePath))
                if (-not $stagedPath.StartsWith($stagingPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
                    throw "Unsafe ZIP entry: $($entry.FullName)"
                }
                if ($entry.FullName.EndsWith('/')) {
                    New-Item -ItemType Directory -Path $stagedPath -Force | Out-Null
                    continue
                }
                $stagedParent = Split-Path -Parent $stagedPath
                New-Item -ItemType Directory -Path $stagedParent -Force | Out-Null
                [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $stagedPath, $false)
            }
        } finally {
            $archive.Dispose()
        }

        $targets = @()
        foreach ($stagedFile in Get-ChildItem -LiteralPath $stagingRoot -File -Recurse) {
            $relativePath = $stagedFile.FullName.Substring($stagingPrefix.Length)
            $targetPath = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($destinationPrefix, $relativePath))
            if (-not $targetPath.StartsWith($destinationPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "Unsafe staged path: $relativePath"
            }
            Assert-NoReparsePointPath $targetPath
            if (Test-Path -LiteralPath $targetPath) {
                throw "Refusing to overwrite an existing file while installing BepInEx: $targetPath"
            }
            $targets += [pscustomobject]@{ Source = $stagedFile.FullName; Path = $targetPath }
        }

        $createdFiles = [System.Collections.Generic.List[string]]::new()
        $createdDirectories = [System.Collections.Generic.List[string]]::new()
        try {
            foreach ($target in $targets) {
                Assert-NoReparsePointPath $target.Path
                $parent = Split-Path -Parent $target.Path
                $missingDirectories = @()
                $currentDirectory = $parent
                while (-not (Test-Path -LiteralPath $currentDirectory)) {
                    $missingDirectories += $currentDirectory
                    $currentDirectory = Split-Path -Parent $currentDirectory
                }
                [array]::Reverse($missingDirectories)
                foreach ($directory in $missingDirectories) {
                    New-Item -ItemType Directory -Path $directory | Out-Null
                    $createdDirectories.Add($directory)
                }

                Copy-NewFileAtomically $target.Source $target.Path
                $createdFiles.Add($target.Path)
            }
        } catch {
            for ($index = $createdFiles.Count - 1; $index -ge 0; $index--) {
                Remove-Item -LiteralPath $createdFiles[$index] -Force -ErrorAction SilentlyContinue
            }
            for ($index = $createdDirectories.Count - 1; $index -ge 0; $index--) {
                Remove-Item -LiteralPath $createdDirectories[$index] -Force -ErrorAction SilentlyContinue
            }
            throw
        }
    } finally {
        if (Test-Path -LiteralPath $stagingRoot) {
            Remove-Item -LiteralPath $stagingRoot -Recurse -Force
        }
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

function Install-VerifiedFile {
    param([string]$SourcePath, [string]$DestinationPath, [string]$ExpectedSha256)

    if (-not (Test-Path -LiteralPath $SourcePath -PathType Leaf)) {
        throw "Source file is missing: $SourcePath"
    }

    $destinationDirectory = Split-Path -Parent $DestinationPath
    Assert-NoReparsePointPath $destinationDirectory
    New-Item -ItemType Directory -Path $destinationDirectory -Force | Out-Null
    Assert-NoReparsePointPath $DestinationPath

    $destinationName = [System.IO.Path]::GetFileName($DestinationPath)
    $operationId = [guid]::NewGuid().ToString("N")
    $stagedPath = Join-Path $destinationDirectory "$destinationName.$operationId.tmp"
    $backupPath = Join-Path $destinationDirectory "$destinationName.$operationId.backup"
    $hadExistingFile = Test-Path -LiteralPath $DestinationPath -PathType Leaf
    $committed = $false

    try {
        Copy-Item -LiteralPath $SourcePath -Destination $stagedPath
        $stagedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $stagedPath).Hash
        if (-not $stagedHash.Equals($ExpectedSha256, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Staged file SHA-256 mismatch. Expected $ExpectedSha256, got $stagedHash."
        }

        Assert-NoReparsePointPath $DestinationPath
        if ($hadExistingFile) {
            [System.IO.File]::Replace($stagedPath, $DestinationPath, $backupPath, $true)
        } else {
            [System.IO.File]::Move($stagedPath, $DestinationPath)
        }
        $committed = $true

        $installedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $DestinationPath).Hash
        if (-not $installedHash.Equals($ExpectedSha256, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Installed file SHA-256 mismatch. Expected $ExpectedSha256, got $installedHash."
        }

        if (Test-Path -LiteralPath $backupPath) {
            Remove-Item -LiteralPath $backupPath -Force
        }
    } catch {
        if ($committed) {
            if ($hadExistingFile -and (Test-Path -LiteralPath $backupPath -PathType Leaf)) {
                [System.IO.File]::Replace($backupPath, $DestinationPath, $null, $true)
            } elseif (-not $hadExistingFile -and (Test-Path -LiteralPath $DestinationPath -PathType Leaf)) {
                Remove-Item -LiteralPath $DestinationPath -Force
            }
        }
        throw
    } finally {
        if (Test-Path -LiteralPath $stagedPath) {
            Remove-Item -LiteralPath $stagedPath -Force
        }
        if (Test-Path -LiteralPath $backupPath) {
            Remove-Item -LiteralPath $backupPath -Force
        }
    }
}

function Install-PluginDll {
    param([string]$SourceDll, [string]$GameDirectory)

    if (-not (Test-Path -LiteralPath $SourceDll -PathType Leaf)) {
        throw "Built plugin DLL is missing: $SourceDll"
    }

    $pluginDirectory = Join-Path $GameDirectory "BepInEx\plugins\RevIdle.ScoreTelemetry"
    $installedDll = Join-Path $pluginDirectory "RevIdle.ScoreTelemetry.dll"
    $sourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $SourceDll).Hash
    Install-VerifiedFile $SourceDll $installedDll $sourceHash

    return $installedDll
}

function Install-ClientExe {
    param([string]$SourceExe, [string]$GameDirectory)

    if (-not (Test-Path -LiteralPath $SourceExe -PathType Leaf)) {
        throw "Built client executable is missing: $SourceExe"
    }

    $installedExe = Join-Path $GameDirectory "client.exe"
    $sourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $SourceExe).Hash
    Install-VerifiedFile $SourceExe $installedExe $sourceHash

    return $installedExe
}

function Select-TestTargetFramework {
    $supportedFrameworks = @("net8.0", "net9.0")

    $installedMajors = @(
        & dotnet --list-runtimes 2>$null |
            ForEach-Object { if ($_ -match '^Microsoft\.NETCore\.App (\d+)\.') { $Matches[1] } } |
            Sort-Object -Unique
    )

    foreach ($framework in $supportedFrameworks) {
        $frameworkMajor = $framework.Substring(3, $framework.Length - 5)
        if ($installedMajors -contains $frameworkMajor) {
            return $framework
        }
    }

    throw "No installed .NET runtime satisfies the test project (needs one of: $($supportedFrameworks -join ', '))."
}

function Invoke-ScoreTelemetryInstall {
    param([string]$RequestedInstallationFolder)

    if ([string]::IsNullOrWhiteSpace($RequestedInstallationFolder)) {
        $RequestedInstallationFolder = Read-Host "Enter the Revolution Idle installation folder"
    }
    $gameDirectory = Resolve-GameDirectory $RequestedInstallationFolder

    if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
        throw "The .NET SDK is required. Install .NET SDK 8 or newer and retry."
    }

    $testTargetFramework = Select-TestTargetFramework

    switch (Get-BepInExState $gameDirectory) {
        "Missing" { Install-BepInEx $gameDirectory }
        "Partial" { throw "A partial BepInEx installation was found. Repair or remove it before retrying." }
        "Complete" { Write-Host "Using the existing BepInEx installation." }
    }

    if (-not (Test-InteropReady $gameDirectory)) {
        Initialize-Interop $gameDirectory
    }

    $repositoryRoot = Split-Path -Parent $PSScriptRoot
    $testProject = Join-Path $repositoryRoot "plugin\tests\RevIdle.ScoreTelemetry.Tests.csproj"
    $pluginProject = Join-Path $repositoryRoot "plugin\src\RevolutionIdle.ScoreTelemetry.csproj"
    $builtDll = Join-Path $repositoryRoot "plugin\src\bin\Release\RevIdle.ScoreTelemetry.dll"
    $clientProject = Join-Path $repositoryRoot "client\Cargo.toml"
    $builtClient = Join-Path $repositoryRoot "client\target\release\client.exe"
    Invoke-CheckedProcess "dotnet" @("run", "--project", $testProject, "-f", $testTargetFramework, "-p:GameDir=$gameDirectory") "Plugin tests failed."
    Invoke-CheckedProcess "dotnet" @("build", $pluginProject, "-c", "Release", "-warnaserror", "-p:GameDir=$gameDirectory") "Plugin build failed."
    Invoke-CheckedProcess "cargo" @("build", "--release", "--manifest-path", $clientProject) "Client build failed."

    $installedDll = Install-PluginDll $builtDll $gameDirectory
    $installedClient = Install-ClientExe $builtClient $gameDirectory
    Write-Host "Installed Revolution Idle Score Telemetry to: $installedDll"
    Write-Host "Installed client to: $installedClient"
    Write-Host "Start Revolution Idle."
}

if ($MyInvocation.InvocationName -ne '.') {
    try {
        Invoke-ScoreTelemetryInstall $InstallationFolder
    } catch {
        Write-Error $_.Exception.Message
        exit 1
    }
}
