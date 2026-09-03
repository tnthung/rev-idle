param(
    [string]$GameDir = 'E:\SteamLibrary\steamapps\common\Revolution Idle',
    [string]$OutputPath = (Join-Path $PSScriptRoot 'STATE_KEYS.md')
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
    if ($reference.FullName -like 'UnityEngine.*' -or $reference.FullName -like 'Il2CppSystem.Func*' -or $reference.FullName -like 'Il2CppSystem.Action*' -or $reference.FullName -in @('UnityEngine.Events.UnityEvent', 'UnityEngine.Events.UnityEventBase', 'System.Delegate', 'System.MulticastDelegate', 'Il2CppSystem.Delegate', 'Il2CppSystem.MulticastDelegate')) { return $true }
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
    score='score'; income='income'; IP='infinity.IP'; infinities='infinity.infs'; stars='infinity.stars'; stardust='infinity.stardust'; EP='eternity.EP'; eternities='eternity.eters'; DP='eternity.DP'; AP='eternity.AP'; RP='eternity.curRP'; RPMax='eternity.maximumRP'; RPSpent='eternity.spendRP'; unities='unity.unities'; passiveUnities='unity.passiveUnities'; astrodust='unity.astrodust'; singularities='singularity.singularity'; atoms='singularity.atoms'; PlP='plague.PlP'; PlPperPlG='plague.PlPperPlG'; PlG='plague.PlG'; VE='plague.VE'; ViP='plague.ViP'; tarotSwords='tarot.swords'; tarotWands='tarot.wands'; tarotPentacles='tarot.pentacles'; tarotCups='tarot.cups'; goldTarotSwords='tarot.goldSwords'; goldTarotWands='tarot.goldWands'; goldTarotPentacles='tarot.goldPentacles'; goldTarotCups='tarot.goldCups'; tarotDraws='tarot.draws'; timeSinceStart='timeSinceStart'; timeInfinity='timeInf'; timeEternity='timeEtr'; timeUnity='timeUnity'; timeTotal='timeTotal'; DT='eternity.dilationTree'; DTP='eternity.dtpMax'
}
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
$lines.Add('Paths are case-sensitive public property names separated by `.`, starting at `GameData`. Numeric segments index arrays/lists; dictionary segments resolve string, integer, or enum keys. Collections are documented below each property. A no-argument request returns the complete nested graph; selected requests return a flat object keyed by the requested path.')
$lines.Add('')
$lines.Add('JSON follows the serializer policy: BigDouble and large integers are strings; safe integers, finite floating-point values, booleans, strings, enums, dates, arrays/lists, dictionaries, and gameplay objects use their native JSON forms. Non-finite floating-point values are `"NaN"`, `"Infinity"`, or `"-Infinity"`; repeated collection objects serialize as `null` to preserve indexes.')
$lines.Add('')
$lines.Add('## Compatibility aliases')
$lines.Add('')
$lines.Add('| Alias | Canonical target |')
$lines.Add('| --- | --- |')
foreach ($alias in $aliases.GetEnumerator()) { $lines.Add([string]::Format('| `{0}` | `{1}` |', $alias.Key, $alias.Value)) }
$lines.Add('')
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
