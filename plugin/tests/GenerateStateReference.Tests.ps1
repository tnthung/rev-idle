param(
    [string]$GameDir = 'E:\SteamLibrary\steamapps\common\Revolution Idle'
)

$ErrorActionPreference = 'Stop'

$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('rev-idle-state-reference-tests-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
try {
    & (Join-Path (Split-Path -Parent $PSScriptRoot) 'generate-state-reference.ps1') -GameDir $GameDir -OutputPath (Join-Path $testRoot 'STATE_KEYS.md') -GraphOutputPath (Join-Path $testRoot 'STATE_GRAPH.json') -GraphViewerOutputPath (Join-Path $testRoot 'STATE_GRAPH.html')
    $graph = Get-Content -LiteralPath (Join-Path $testRoot 'STATE_GRAPH.json') -Raw | ConvertFrom-Json
    $manual = Get-Content -LiteralPath (Join-Path $testRoot 'STATE_KEYS.md') -Raw
    $checks = 0
    foreach ($case in @(
        @{ Type = 'GameData'; Field = 'attacks'; Returned = $true },
        @{ Type = 'Relic'; Field = 'Attacks'; Returned = $false },
        @{ Type = 'Relic'; Field = 'Elements'; Returned = $false },
        @{ Type = 'Relic'; Field = 'Minerals'; Returned = $false },
        @{ Type = 'Relic'; Field = 'Singularity'; Returned = $false },
        @{ Type = 'Relic'; Field = 'Tarot'; Returned = $false },
        @{ Type = 'Relic'; Field = 'Unity'; Returned = $false },
        @{ Type = 'Relic'; Field = 'amount'; Returned = $true },
        @{ Type = 'Relic'; Field = 'totalCost'; Returned = $true },
        @{ Type = 'DilationTree'; Field = 'center'; Returned = $true },
        @{ Type = 'DilationTree'; Field = 'bot'; Returned = $true },
        @{ Type = 'DilationTreeUpgrade'; Field = 'prev'; Returned = $false },
        @{ Type = 'DilationTreeUpgrade'; Field = 'axis'; Returned = $true },
        @{ Type = 'UnityZodiac'; Field = 'stats'; Returned = $true },
        @{ Type = 'UnityZodiac'; Field = 'planet'; Returned = $true },
        @{ Type = 'UnityZodiac'; Field = 'Element'; Returned = $true },
        @{ Type = 'UnityZodiac'; Field = 'Season'; Returned = $true },
        @{ Type = 'ZodiacStat'; Field = 'type'; Returned = $true },
        @{ Type = 'ElementsData'; Field = 'earth'; Returned = $true },
        @{ Type = 'ElementsData'; Field = 'fire'; Returned = $true },
        @{ Type = 'AttacksController'; Field = 'Attacks'; Returned = $true }
    )) {
        $type = $case.Type
        if ((@($graph.edges | Where-Object { $_.from -ceq $type -and $_.property -ceq $case.Field }).Count -gt 0) -ne $case.Returned) {
            throw "Graph field $type.$($case.Field): expected returned=$($case.Returned)."
        }
        $section = [regex]::Match($manual, '(?ms)^### `' + [regex]::Escape($type) + '`[^\r\n]*\r?\n(.*?)(?=^### |\z)').Groups[1].Value
        if ($section.Contains('| `' + $case.Field + '` |') -ne $case.Returned) {
            throw "Manual field $type.$($case.Field): expected returned=$($case.Returned)."
        }
        $checks += 2
    }
    if (@($graph.edges | Where-Object from -ceq 'Relic').Count -ne 13) { throw 'Relic should document its 13 returned fields.' }
    $checks++
    foreach ($case in @(
        @{ Type = 'AstroElementType'; Variants = 'Undefined=-1,Fire=0,Earth=1,Wind=2,Water=3,Light=4' },
        @{ Type = 'EtrDilTreeAxis'; Variants = 'UNKNOWN=-1,CENTER=0,MIDDLE=1,TOP=2,BOTTOM=3' }
    )) {
        if ((@(($graph.nodes | Where-Object id -ceq $case.Type).variants | ForEach-Object { "$($_.name)=$($_.value)" }) -join ',') -cne $case.Variants) {
            throw "Enum $($case.Type) variants are missing or incorrect."
        }
        $section = [regex]::Match($manual, '(?ms)^### `' + [regex]::Escape($case.Type) + '`[^\r\n]*\r?\n(.*?)(?=^### |\z)').Groups[1].Value
        if ((@([regex]::Matches($section, '\| `([^`]+)` \| `(-?\d+)` \|') | ForEach-Object { "$($_.Groups[1].Value)=$($_.Groups[2].Value)" }) -join ',') -cne $case.Variants) {
            throw "Manual enum $($case.Type) variants are missing or incorrect."
        }
        $checks += 2
    }
    foreach ($field in @('Element', 'Season', 'rarity', 'sign')) {
        if ($null -ne ($graph.edges | Where-Object { $_.from -ceq 'UnityZodiac' -and $_.property -ceq $field }).to) { throw "UnityZodiac.$field should be a scalar enum, not an object link." }
        $checks++
    }
    if (@($graph.edges | Where-Object { $_.from -ceq 'UnityData' -and $_.property -ceq 'planets' }).Count -ne 1 -or ($graph.edges | Where-Object { $_.from -ceq 'UnityData' -and $_.property -ceq 'planets' }).to -cne 'UnityPlanet') {
        throw 'Dictionary paths should follow their values, not their enum keys.'
    }
    $checks++
    $template = [IO.File]::ReadAllText((Join-Path (Split-Path -Parent $PSScriptRoot) 'STATE_GRAPH.template.html'))
    if ([IO.File]::ReadAllText((Join-Path $testRoot 'STATE_GRAPH.html')) -cne $template.Replace('__GRAPH_JSON__', ([IO.File]::ReadAllLines((Join-Path $testRoot 'STATE_GRAPH.json')) -join "`n"))) {
        throw 'Generated viewer does not embed the generated graph.'
    }
    $checks++
    Write-Output "$checks state reference checks passed."
} finally {
    if ([IO.Path]::GetFullPath($testRoot).StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()), [StringComparison]::OrdinalIgnoreCase) -and (Split-Path -Leaf $testRoot).StartsWith('rev-idle-state-reference-tests-')) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}
