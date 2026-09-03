param(
    [string]$GameDir = 'E:\SteamLibrary\steamapps\common\Revolution Idle',
    [string]$OutputPath = (Join-Path $PSScriptRoot 'STATE_KEYS.md'),
    [string]$GraphOutputPath = (Join-Path $PSScriptRoot 'STATE_GRAPH.json'),
    [string]$GraphTemplatePath = (Join-Path $PSScriptRoot 'STATE_GRAPH.template.html'),
    [string]$GraphViewerOutputPath = (Join-Path $PSScriptRoot 'STATE_GRAPH.html')
)

$ErrorActionPreference = 'Stop'
$cecilPath = Join-Path $GameDir 'BepInEx\core\Mono.Cecil.dll'
$assemblyPath = Join-Path $GameDir 'BepInEx\interop\Assembly-CSharp.dll'
if (-not (Test-Path -LiteralPath $cecilPath) -or -not (Test-Path -LiteralPath $assemblyPath)) {
    throw "Missing Mono.Cecil.dll or Assembly-CSharp.dll under '$GameDir'."
}

Add-Type -Path $cecilPath
$resolver = [Mono.Cecil.DefaultAssemblyResolver]::new()
foreach ($searchDirectory in @((Join-Path $GameDir 'BepInEx\core'), (Join-Path $GameDir 'BepInEx\interop'), (Join-Path $GameDir 'Revolution Idle_Data\Managed'), $GameDir)) {
    if (Test-Path -LiteralPath $searchDirectory) { $resolver.AddSearchDirectory($searchDirectory) }
}
$readerParameters = [Mono.Cecil.ReaderParameters]::new()
$readerParameters.AssemblyResolver = $resolver
$assembly = [Mono.Cecil.AssemblyDefinition]::ReadAssembly($assemblyPath, $readerParameters)
$module = $assembly.MainModule

function Get-TypeDefinition([Mono.Cecil.TypeReference]$reference) {
    try { return $reference.Resolve() } catch { return $null }
}

function Get-TypeLabel([Mono.Cecil.TypeReference]$reference) {
    if ($reference -is [Mono.Cecil.ArrayType]) { return "$(Get-TypeLabel $reference.ElementType)[]" }
    if ($reference.IsGenericInstance) {
        $baseName = $reference.Name -replace ('`' + '[0-9]+$'), ''
        $arguments = @($reference.GenericArguments | ForEach-Object { Get-TypeLabel $_ }) -join ', '
        return "$baseName<$arguments>"
    }
    return $reference.FullName.Replace('/', '.')
}

function Test-ExcludedType([Mono.Cecil.TypeReference]$reference) {
    if ($reference.FullName -in @('UnityEngine.Object', 'UnityEngine.Events.UnityEventBase', 'System.Delegate', 'System.MulticastDelegate', 'Il2CppSystem.Delegate', 'Il2CppSystem.MulticastDelegate')) { return $true }
    $current = Get-TypeDefinition $reference
    while ($null -ne $current) {
        if ($current.FullName -in @('System.Delegate', 'System.MulticastDelegate', 'Il2CppSystem.Delegate', 'Il2CppSystem.MulticastDelegate', 'UnityEngine.Object', 'UnityEngine.Events.UnityEventBase')) { return $true }
        $current = Get-TypeDefinition $current.BaseType
    }
    return $false
}

function Test-ExcludedProperty([Mono.Cecil.PropertyDefinition]$property) {
    if ($property.Name -in @('Pointer', 'ObjectClass', 'WasCollected', 'Data', 'Controller', 'Parent') -or $property.Name.StartsWith('prop_') -or $property.Name.Contains('BackingField')) { return $true }
    return Test-ExcludedType $property.PropertyType
}

function Get-EligibleProperties([Mono.Cecil.TypeDefinition]$type) {
    $result = [System.Collections.Generic.List[object]]::new()
    $names = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $current = $type
    while ($null -ne $current -and $current.FullName -notin @('System.Object', 'System.ValueType', 'Il2CppSystem.Object', 'Il2CppInterop.Runtime.InteropTypes.Il2CppObjectBase', 'UnityEngine.Object', 'UnityEngine.Events.UnityEventBase', 'Il2CppSystem.Delegate', 'Il2CppSystem.MulticastDelegate', 'System.Delegate', 'System.MulticastDelegate')) {
        foreach ($property in $current.Properties) {
            if ($names.Add($property.Name) -and $null -ne $property.GetMethod -and $property.GetMethod.IsPublic -and -not $property.GetMethod.IsStatic -and $property.Parameters.Count -eq 0 -and -not (Test-ExcludedProperty $property)) { [void]$result.Add($property) }
        }
        $current = Get-TypeDefinition $current.BaseType
    }
    $result.Sort([Comparison[object]]{ param($left, $right) [StringComparer]::Ordinal.Compare($left.Name, $right.Name) })
    return @($result)
}

function Get-EligibleStaticProperties([Mono.Cecil.TypeDefinition]$type) {
    $result = [System.Collections.Generic.List[object]]::new()
    $names = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $current = $type
    while ($null -ne $current -and $current.FullName -notin @('System.Object', 'System.ValueType', 'Il2CppSystem.Object', 'Il2CppInterop.Runtime.InteropTypes.Il2CppObjectBase', 'UnityEngine.Object', 'UnityEngine.Events.UnityEventBase', 'Il2CppSystem.Delegate', 'Il2CppSystem.MulticastDelegate', 'System.Delegate', 'System.MulticastDelegate')) {
        foreach ($property in $current.Properties) {
            if ($names.Add($property.Name) -and $null -ne $property.GetMethod -and $property.GetMethod.IsPublic -and $property.GetMethod.IsStatic -and $property.Parameters.Count -eq 0 -and -not (Test-ExcludedProperty $property)) { [void]$result.Add($property) }
        }
        $current = Get-TypeDefinition $current.BaseType
    }
    $result.Sort([Comparison[object]]{ param($left, $right) [StringComparer]::Ordinal.Compare($left.Name, $right.Name) })
    return @($result)
}

$primitiveNodeTypes = @('BigDouble')

function Get-GameplayTypes([Mono.Cecil.TypeReference]$reference) {
    if ($reference -is [Mono.Cecil.ArrayType]) { return @(Get-GameplayTypes $reference.ElementType) }
    if ($reference.IsGenericInstance) {
        $found = [System.Collections.Generic.List[object]]::new()
        foreach ($argument in $reference.GenericArguments) { foreach ($type in @(Get-GameplayTypes $argument)) { if ($null -ne $type -and -not ($found | Where-Object FullName -eq $type.FullName)) { [void]$found.Add($type) } } }
        return @($found)
    }
    $definition = Get-TypeDefinition $reference
    if ($null -ne $definition -and $definition.Module.Assembly.Name.Name -eq 'Assembly-CSharp') { return @($definition) }
    return @()
}

function Get-GraphNodeTypes([Mono.Cecil.TypeReference]$reference) {
    if ($reference -is [Mono.Cecil.ArrayType]) { return @(Get-GraphNodeTypes $reference.ElementType) }
    if ($reference.IsGenericInstance) {
        $found = [System.Collections.Generic.List[object]]::new()
        foreach ($argument in $reference.GenericArguments) { foreach ($type in @(Get-GraphNodeTypes $argument)) { if ($null -ne $type -and -not ($found | Where-Object FullName -eq $type.FullName)) { [void]$found.Add($type) } } }
        return @($found)
    }
    $definition = Get-TypeDefinition $reference
    if ($null -ne $definition -and $definition.Module.Assembly.Name.Name -eq 'Assembly-CSharp' -and $definition.Name -notin $primitiveNodeTypes) { return @($definition) }
    return @()
}

function Get-CollectionDescription([Mono.Cecil.TypeReference]$reference) {
    if ($reference -is [Mono.Cecil.ArrayType]) { return "array of $(Get-TypeLabel $reference.ElementType)" }
    if (-not $reference.IsGenericInstance) { return $null }
    if ($reference.Name.StartsWith('List') -or $reference.FullName.StartsWith('Il2CppInterop.Runtime.InteropTypes.Arrays.')) { return "list/array of $(Get-TypeLabel $reference.GenericArguments[0])" }
    if ($reference.Name.StartsWith('Dictionary')) { return "dictionary keyed by $(Get-TypeLabel $reference.GenericArguments[0]), values $(Get-TypeLabel $reference.GenericArguments[1])" }
    return $null
}

function Test-SupportedDictionaryKey([Mono.Cecil.TypeReference]$reference) {
    if (-not $reference.IsGenericInstance -or -not $reference.Name.StartsWith('Dictionary')) { return $false }
    $key = $reference.GenericArguments[0]
    $keyDefinition = Get-TypeDefinition $key
    if ($key.FullName -eq 'System.String' -or $key.IsEnum -or ($null -ne $keyDefinition -and $keyDefinition.IsEnum)) { return $true }
    return $key.FullName -in @('System.SByte', 'System.Byte', 'System.Int16', 'System.UInt16', 'System.Int32', 'System.UInt32', 'System.Int64', 'System.UInt64')
}

$gameData = @($module.Types | Where-Object FullName -eq 'GameData')[0]
if ($null -eq $gameData) { throw 'GameData was not found in Assembly-CSharp.dll.' }
$reachable = [System.Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)
$pending = [System.Collections.Generic.Queue[object]]::new()
$pending.Enqueue($gameData)
while ($pending.Count -gt 0) {
    $type = $pending.Dequeue()
    if ($reachable.ContainsKey($type.FullName)) { continue }
    $properties = @(Get-EligibleProperties $type)
    $reachable[$type.FullName] = [pscustomobject]@{ Type = $type; Properties = $properties }
    foreach ($property in $properties) {
        foreach ($child in @(Get-GameplayTypes $property.PropertyType)) { if (-not $reachable.ContainsKey($child.FullName)) { $pending.Enqueue($child) } }
    }
}

$aliases = [ordered]@{
    score='gameData.score'; income='gameData.income'; IP='gameData.infinity.IP'; infinities='gameData.infinity.infs'; stars='gameData.infinity.stars'; stardust='gameData.infinity.stardust'; EP='gameData.eternity.EP'; eternities='gameData.eternity.eters'; DP='gameData.eternity.DP'; AP='gameData.eternity.AP'; RP='gameData.eternity.curRP'; RPMax='gameData.eternity.maximumRP'; RPSpent='gameData.eternity.spendRP'; unities='gameData.unity.unities'; passiveUnities='gameData.unity.passiveUnities'; astrodust='gameData.unity.astrodust'; singularities='gameData.singularity.singularity'; atoms='gameData.singularity.atoms'; PlP='gameData.plague.PlP'; PlPperPlG='gameData.plague.PlPperPlG'; PlG='gameData.plague.PlG'; VE='gameData.plague.VE'; ViP='gameData.plague.ViP'; tarotSwords='gameData.tarot.swords'; tarotWands='gameData.tarot.wands'; tarotPentacles='gameData.tarot.pentacles'; tarotCups='gameData.tarot.cups'; goldTarotSwords='gameData.tarot.goldSwords'; goldTarotWands='gameData.tarot.goldWands'; goldTarotPentacles='gameData.tarot.goldPentacles'; goldTarotCups='gameData.tarot.goldCups'; tarotDraws='gameData.tarot.draws'; timeSinceStart='gameData.timeSinceStart'; timeInfinity='gameData.timeInf'; timeEternity='gameData.timeEtr'; timeUnity='gameData.timeUnity'; timeTotal='gameData.timeTotal'; DT='gameData.eternity.dilationTree'; DTP='gameData.eternity.dtpMax'; nextEP='eternityController.EPGain'; nextBrokenEP='eternityController.brokenEPGain'
}

$extraRootNames = @('Controller', 'AttacksController', 'AutomationController', 'ElementsController', 'EternityController', 'GameController', 'InfinityController', 'MacroController', 'MineralsController', 'PlagueController', 'SaveController', 'SingularityController', 'TarotController', 'UnityController')
$extraRoots = [System.Collections.Generic.List[object]]::new()
foreach ($rootName in $extraRootNames) {
    $rootType = @($module.Types | Where-Object FullName -eq $rootName)[0]
    if ($null -eq $rootType) { Write-Warning "Extra root type '$rootName' was not found in Assembly-CSharp.dll."; continue }
    $rootKey = [char]::ToLowerInvariant($rootName[0]) + $rootName.Substring(1)
    [void]$extraRoots.Add([pscustomobject]@{ Key = $rootKey; Type = $rootType; Properties = @(Get-EligibleStaticProperties $rootType) })
}

# GameData is not an implicit or default root: like every *Controller extra
# root, it is only reachable by naming its key ("gameData") as a request
# path's first segment (see plugin/src/ExtraRoots.cs).
$allRoots = [System.Collections.Generic.List[object]]::new()
[void]$allRoots.Add([pscustomobject]@{ Key = 'gameData'; Type = $gameData; Properties = @(Get-EligibleProperties $gameData) })
foreach ($root in $extraRoots) { [void]$allRoots.Add($root) }
$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add('# Complete state path reference')
$lines.Add('')
$propertyCount = 0
foreach ($entry in $reachable.Values) { $propertyCount += @($entry.Properties).Count }
$lines.Add('Generated deterministically from `BepInEx/interop/Assembly-CSharp.dll` by `generate-state-reference.ps1`.')
$lines.Add("Reachable gameplay types: **$($reachable.Count)**. Properties: **$propertyCount**.")
$lines.Add('')
$lines.Add('## Path grammar and JSON behavior')
$lines.Add('')
$lines.Add('There is no implicit or default root. Every path is case-sensitive public property names separated by `.`, and the first segment must always name one of the root keys below -- `gameData` included, the same as any `*Controller` root. Numeric segments index arrays/lists; dictionary segments resolve string, integer, or enum keys. Collections are documented below each property. A keyless request is rejected; selected requests return a flat object keyed by the requested path.')
$lines.Add('')
$lines.Add('JSON follows the serializer policy: BigDouble and large integers are strings; safe integers, finite floating-point values, booleans, strings, enums, dates, arrays/lists, dictionaries, and gameplay objects use their native JSON forms. Non-finite floating-point values are `"NaN"`, `"Infinity"`, or `"-Infinity"`; repeated collection objects serialize as `null` to preserve indexes.')
$lines.Add('')
$lines.Add('## Compatibility aliases')
$lines.Add('')
$lines.Add('| Alias | Canonical target |')
$lines.Add('| --- | --- |')
foreach ($alias in $aliases.GetEnumerator()) { $lines.Add([string]::Format('| `{0}` | `{1}` |', $alias.Key, $alias.Value)) }
$lines.Add('')
$lines.Add('## Roots')
$lines.Add('')
$lines.Add('`gameData` is the game''s main save-data graph (detailed in Reachable gameplay types below); the `*Controller` keys are static-only types with no property anywhere in that graph pointing at them, so they are otherwise unreachable no matter how deep a path goes. Every root is addressed the same way: name its key as the request path''s first segment.')
$lines.Add('')
$lines.Add('| Root key | CLR type |')
$lines.Add('| --- | --- |')
foreach ($root in $allRoots) { $lines.Add([string]::Format('| `{0}` | `{1}` |', $root.Key, $root.Type.FullName)) }
$lines.Add('')
foreach ($root in $extraRoots) {
    $lines.Add([string]::Format('### `{0}` (root key `{1}`)', $root.Type.FullName, $root.Key))
    $lines.Add('')
    $lines.Add('| Property | CLR type | Collection path extension |')
    $lines.Add('| --- | --- | --- |')
    foreach ($property in $root.Properties) {
        $typeLabel = Get-TypeLabel $property.PropertyType
        $collection = Get-CollectionDescription $property.PropertyType
        $dictionary = $property.PropertyType.IsGenericInstance -and $property.PropertyType.Name.StartsWith('Dictionary')
        $extension = if ($null -eq $collection) { '' } elseif ($dictionary -and -not (Test-SupportedDictionaryKey $property.PropertyType)) { 'whole-property only; not key-addressable' } elseif ($dictionary) { 'append `.<string|integer|enum-key>`' } else { 'append `.<numeric-index>`' }
        $lines.Add([string]::Format('| `{0}` | `{1}` | {2} {3} |', $property.Name, $typeLabel, $collection, $extension))
    }
    $lines.Add('')
}
$lines.Add('## Reachable gameplay types')
$lines.Add('')
$sortedEntries = [System.Collections.Generic.List[object]]::new()
foreach ($entry in $reachable.Values) { [void]$sortedEntries.Add($entry) }
$sortedEntries.Sort([Comparison[object]]{ param($left, $right) [StringComparer]::Ordinal.Compare($left.Type.FullName, $right.Type.FullName) })
foreach ($entry in $sortedEntries) {
    $lines.Add([string]::Format('### `{0}`', $entry.Type.FullName.Replace('/', '.')))
    $lines.Add('')
    $lines.Add('| Property | CLR type | Collection path extension |')
    $lines.Add('| --- | --- | --- |')
    foreach ($property in $entry.Properties) {
        $typeLabel = Get-TypeLabel $property.PropertyType
        $collection = Get-CollectionDescription $property.PropertyType
        $dictionary = $property.PropertyType.IsGenericInstance -and $property.PropertyType.Name.StartsWith('Dictionary')
        $extension = if ($null -eq $collection) { '' } elseif ($dictionary -and -not (Test-SupportedDictionaryKey $property.PropertyType)) { 'whole-property only; not key-addressable' } elseif ($dictionary) { 'append `.<string|integer|enum-key>`' } else { 'append `.<numeric-index>`' }
        $lines.Add([string]::Format('| `{0}` | `{1}` | {2} {3} |', $property.Name, $typeLabel, $collection, $extension))
    }
    $lines.Add('')
}
[void]$lines.RemoveAt($lines.Count - 1)
[IO.File]::WriteAllLines($OutputPath, $lines, [Text.UTF8Encoding]::new($false))
Write-Output "Generated $OutputPath ($($reachable.Count) types)."

$graphNodeNames = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($entry in $reachable.Values) {
    if ($entry.Type.Name -notin $primitiveNodeTypes) { [void]$graphNodeNames.Add($entry.Type.FullName) }
}

$edges = [System.Collections.Generic.List[object]]::new()
foreach ($entry in $reachable.Values) {
    if ($entry.Type.Name -in $primitiveNodeTypes) { continue }
    foreach ($property in $entry.Properties) {
        $propertyType = $property.PropertyType
        $collection = Get-CollectionDescription $propertyType
        $dictionary = $propertyType.IsGenericInstance -and $propertyType.Name.StartsWith('Dictionary')
        $kind = if ($null -eq $collection) { 'plain' } elseif ($dictionary) { 'dict' } else { 'list' }
        if ($dictionary -and -not (Test-SupportedDictionaryKey $propertyType)) { $kind = 'dict-unkeyed' }
        $targets = @(Get-GraphNodeTypes $propertyType)
        $valueTypeLabel = Get-TypeLabel $propertyType
        if ($targets.Count -eq 0) {
            [void]$edges.Add([pscustomobject]@{ From = $entry.Type.FullName; To = $null; Property = $property.Name; Kind = $kind; ValueType = $valueTypeLabel })
        } else {
            foreach ($target in $targets) {
                [void]$edges.Add([pscustomobject]@{ From = $entry.Type.FullName; To = $target.FullName; Property = $property.Name; Kind = $kind; ValueType = $valueTypeLabel })
            }
        }
    }
}

# Extra roots are static-only types with no path in from GameData (see
# plugin/src/ExtraRoots.cs). They're still graph nodes -- their static
# properties become edges the same way -- but flagged so the viewer knows not
# to look for (or complain about the absence of) a route from GameData; they
# are addressed by their root key as the request path's first segment instead.
foreach ($root in $extraRoots) {
    [void]$graphNodeNames.Add($root.Type.FullName)
    foreach ($property in $root.Properties) {
        $propertyType = $property.PropertyType
        $collection = Get-CollectionDescription $propertyType
        $dictionary = $propertyType.IsGenericInstance -and $propertyType.Name.StartsWith('Dictionary')
        $kind = if ($null -eq $collection) { 'plain' } elseif ($dictionary) { 'dict' } else { 'list' }
        if ($dictionary -and -not (Test-SupportedDictionaryKey $propertyType)) { $kind = 'dict-unkeyed' }
        $targets = @(Get-GraphNodeTypes $propertyType)
        $valueTypeLabel = Get-TypeLabel $propertyType
        if ($targets.Count -eq 0) {
            [void]$edges.Add([pscustomobject]@{ From = $root.Type.FullName; To = $null; Property = $property.Name; Kind = $kind; ValueType = $valueTypeLabel })
        } else {
            foreach ($target in $targets) {
                [void]$edges.Add([pscustomobject]@{ From = $root.Type.FullName; To = $target.FullName; Property = $property.Name; Kind = $kind; ValueType = $valueTypeLabel })
            }
        }
    }
}

function ConvertTo-JsonString([string]$value) {
    if ($null -eq $value) { return 'null' }
    $escaped = $value.Replace('\', '\\').Replace('"', '\"').Replace("`n", '\n').Replace("`r", '')
    return "`"$escaped`""
}

$jsonLines = [System.Collections.Generic.List[string]]::new()
$jsonLines.Add('{')
$jsonLines.Add('  "nodes": [')
$sortedNodeNames = @($graphNodeNames) | Sort-Object
for ($i = 0; $i -lt $sortedNodeNames.Count; $i++) {
    $comma = if ($i -eq $sortedNodeNames.Count - 1) { '' } else { ',' }
    $jsonLines.Add("    {`"id`": $(ConvertTo-JsonString $sortedNodeNames[$i])}$comma")
}
$jsonLines.Add('  ],')
$jsonLines.Add('  "edges": [')
for ($i = 0; $i -lt $edges.Count; $i++) {
    $edge = $edges[$i]
    $comma = if ($i -eq $edges.Count - 1) { '' } else { ',' }
    $toJson = if ($null -eq $edge.To) { 'null' } else { ConvertTo-JsonString $edge.To }
    $jsonLines.Add("    {`"from`": $(ConvertTo-JsonString $edge.From), `"to`": $toJson, `"property`": $(ConvertTo-JsonString $edge.Property), `"kind`": $(ConvertTo-JsonString $edge.Kind), `"valueType`": $(ConvertTo-JsonString $edge.ValueType)}$comma")
}
$jsonLines.Add('  ],')
$jsonLines.Add('  "aliases": [')
$aliasEntries = @($aliases.GetEnumerator())
for ($i = 0; $i -lt $aliasEntries.Count; $i++) {
    $comma = if ($i -eq $aliasEntries.Count - 1) { '' } else { ',' }
    $jsonLines.Add("    {`"alias`": $(ConvertTo-JsonString $aliasEntries[$i].Key), `"path`": $(ConvertTo-JsonString $aliasEntries[$i].Value)}$comma")
}
$jsonLines.Add('  ],')
$jsonLines.Add('  "roots": [')
for ($i = 0; $i -lt $allRoots.Count; $i++) {
    $comma = if ($i -eq $allRoots.Count - 1) { '' } else { ',' }
    $jsonLines.Add("    {`"key`": $(ConvertTo-JsonString $allRoots[$i].Key), `"node`": $(ConvertTo-JsonString $allRoots[$i].Type.FullName)}$comma")
}
$jsonLines.Add('  ]')
$jsonLines.Add('}')
[IO.File]::WriteAllLines($GraphOutputPath, $jsonLines, [Text.UTF8Encoding]::new($false))
Write-Output "Generated $GraphOutputPath ($($sortedNodeNames.Count) nodes, $($edges.Count) edges, $($aliasEntries.Count) aliases)."

if (Test-Path -LiteralPath $GraphTemplatePath) {
    $graphJson = ($jsonLines -join "`n")
    $template = [IO.File]::ReadAllText($GraphTemplatePath)
    $viewerHtml = $template.Replace('__GRAPH_JSON__', $graphJson)
    [IO.File]::WriteAllText($GraphViewerOutputPath, $viewerHtml, [Text.UTF8Encoding]::new($false))
    Write-Output "Generated $GraphViewerOutputPath (embedded graph data)."
} else {
    Write-Warning "Skipped viewer generation: template not found at '$GraphTemplatePath'."
}
