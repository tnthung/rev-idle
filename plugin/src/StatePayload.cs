using System.Collections;
using System.Collections.Concurrent;
using System.Globalization;
using System.Reflection;
using System.Text;
using System.Text.Json;

namespace RevIdle.ScoreTelemetry;

internal enum StatePayloadStatus
{
    Success,
    InvalidPath,
    SerializationFailure
}

internal static class StatePayload
{
    private const long MaxSafeInteger = 9007199254740991;
    private const int MaxTraversalDepth = 64;
    private static readonly ConcurrentDictionary<Type, PropertyInfo[]> Properties = new();

    public static bool TryEncode(GameData data, IReadOnlyList<string> keys, out byte[] payload) => Encode(data, keys, out payload, out _) == StatePayloadStatus.Success;

    internal static StatePayloadStatus Encode(object data, IReadOnlyList<string> paths, out byte[] payload, out string? failedPath)
    {
        failedPath = null;
        // There is no implicit or default root: every path must name one of
        // ExtraRoots' keys (GameData included) as its first segment, so a
        // keyless request has no root to resolve against and is rejected
        // rather than falling back to an implicit whole-GameData dump.
        if (paths.Count == 0)
        {
            payload = Array.Empty<byte>();
            return StatePayloadStatus.InvalidPath;
        }
        try
        {
            List<(string Path, object? Value)> values = new(paths.Count);
            HashSet<string> seenPaths = new(StringComparer.Ordinal);
            foreach (string path in paths)
            {
                failedPath = path;
                if (!seenPaths.Add(path))
                    continue;
                if (!TryResolve(data, path, out object? value))
                {
                    payload = Array.Empty<byte>();
                    return StatePayloadStatus.InvalidPath;
                }
                values.Add((path, value));
            }

            using MemoryStream stream = new();
            using (var writer = new Utf8JsonWriter(stream))
            {
                writer.WriteStartObject();
                foreach ((string path, object? value) in values)
                {
                    failedPath = path;
                    writer.WritePropertyName(path);
                    WriteValue(writer, value, new HashSet<object>(ReferenceEqualityComparer.Instance), new HashSet<nint>(), false, 0);
                }
                writer.WriteEndObject();
            }
            payload = stream.ToArray();
            failedPath = null;
            return StatePayloadStatus.Success;
        }
        catch
        {
            payload = Array.Empty<byte>();
            return StatePayloadStatus.SerializationFailure;
        }
    }

    internal static JsonElement EncodeValue(object? value)
    {
        using MemoryStream stream = new();
        using (var writer = new Utf8JsonWriter(stream))
            WriteValue(writer, value, new HashSet<object>(ReferenceEqualityComparer.Instance), new HashSet<nint>(), false, 0);
        using JsonDocument document = JsonDocument.Parse(stream.ToArray());
        return document.RootElement.Clone();
    }

    private static bool TryResolve(object data, string path, out object? value)
    {
        if (path.Length == 0)
        {
            value = null;
            return false;
        }

        string[] segments = ResolveAlias(path).Split('.');
        if (!ExtraRoots.TryGet(segments[0], data, out object? root))
        {
            value = null;
            return false;
        }
        value = root;

        foreach (string segment in segments.Skip(1))
        {
            if (value is null || segment.Length == 0 || IsExcludedType(value.GetType()))
                return false;

            if (TryGetDictionaryValue(value, segment, out object? dictionaryValue, out bool dictionary))
            {
                value = dictionaryValue;
                continue;
            }
            if (dictionary)
                return false;

            if (int.TryParse(segment, NumberStyles.None, CultureInfo.InvariantCulture, out int index))
            {
                if (!TryGetCollectionValue(value, index, out object? collectionValue))
                    return false;
                value = collectionValue;
                continue;
            }

            Type type = value.GetType();
            if (value is IList
                || IsReflectedCollection(type)
                || type.IsPrimitive
                || type.IsEnum
                || type == typeof(decimal)
                || value is string or DateTime or DateTimeOffset
                || type.FullName == "BigDouble" && type.Assembly.GetName().Name == "Assembly-CSharp"
                || type.Assembly.GetName().Name == "ACTk.Runtime" && type.Namespace == "CodeStage.AntiCheat.ObscuredTypes"
                || type.Assembly.GetName().Name == "Il2Cppmscorlib" && type.FullName is "Il2CppSystem.DateTime" or "Il2CppSystem.DateTimeOffset" or "Il2CppSystem.Object"
                || type.Assembly.GetName().Name == "Il2Cppmscorlib" && type.FullName?.StartsWith("Il2CppSystem.Nullable`1", StringComparison.Ordinal) == true
                || type.Assembly.GetName().Name == "UnityEngine.CoreModule" && type.FullName == "UnityEngine.Color")
                return false;

            PropertyInfo? property = GetProperties(type).FirstOrDefault(property => property.Name == segment);
            if (property is null)
                return false;
            value = TryGetPropertyValue(property, value);
        }
        return value is null || !IsExcludedType(value.GetType());
    }

    // Some IL2CPP-backed getters throw instead of returning null for an
    // unset value (GameData.DateOFFull does this for its Nullable<DateTime>
    // whenever it has never been set), which would otherwise fail an entire
    // request over one unrelated, optional field. Treat that one property as
    // absent rather than letting its exception abort serialization.
    private static object? TryGetPropertyValue(PropertyInfo property, object instance)
    {
        try { return property.GetValue(instance); }
        catch { return null; }
    }

    // Every alias expands to a path that already names its root explicitly
    // (see ExtraRoots) -- there is no implicit or default root, GameData
    // included, so a raw path with no root segment (the "_ => path"
    // fallback) is left as-is and simply fails to resolve.
    private static string ResolveAlias(string path) => path switch
    {
        "score" => "gameData.score",
        "scoreEternity" => "gameData.scoreEternity",
        "scoreInfinity" => "gameData.scoreInfinity",
        "scoreUnity" => "gameData.scoreUnity",
        "scorePromotion" => "gameData.scorePromotion",
        "scoreEquality" => "gameData.scoreEquality",
        "income" => "gameData.income",
        "IP" => "gameData.infinity.IP",
        "infinities" => "gameData.infinity.infs",
        "stars" => "gameData.infinity.stars",
        "stardust" => "gameData.infinity.stardust",
        "EP" => "gameData.eternity.EP",
        "eternities" => "gameData.eternity.eters",
        "DP" => "gameData.eternity.DP",
        "AP" => "gameData.eternity.AP",
        "RP" => "gameData.eternity.curRP",
        "RPMax" => "gameData.eternity.maximumRP",
        "RPSpent" => "gameData.eternity.spendRP",
        "unities" => "gameData.unity.unities",
        "passiveUnities" => "gameData.unity.passiveUnities",
        "astrodust" => "gameData.unity.astrodust",
        "singularities" => "gameData.singularity.singularity",
        "atoms" => "gameData.singularity.atoms",
        "PlP" => "gameData.plague.PlP",
        "PlPperPlG" => "gameData.plague.PlPperPlG",
        "PlG" => "gameData.plague.PlG",
        "VE" => "gameData.plague.VE",
        "ViP" => "gameData.plague.ViP",
        "tarotSwords" => "gameData.tarot.swords",
        "tarotWands" => "gameData.tarot.wands",
        "tarotPentacles" => "gameData.tarot.pentacles",
        "tarotCups" => "gameData.tarot.cups",
        "goldTarotSwords" => "gameData.tarot.goldSwords",
        "goldTarotWands" => "gameData.tarot.goldWands",
        "goldTarotPentacles" => "gameData.tarot.goldPentacles",
        "goldTarotCups" => "gameData.tarot.goldCups",
        "tarotDraws" => "gameData.tarot.draws",
        "timeSinceStart" => "gameData.timeSinceStart",
        "timeInfinity" => "gameData.timeInf",
        "timeEternity" => "gameData.timeEtr",
        "timeUnity" => "gameData.timeUnity",
        "timeEquality" => "gameData.timeEquality",
        "timeTotal" => "gameData.timeTotal",
        "DT" => "gameData.eternity.dilationTree",
        // "DTP" follows how the rest of the codebase already uses the term
        // (dilation tree points owned, e.g. the unity_loop.js DTP loadouts) --
        // not the cap. Use "DTPMax" for eternity.dtpMax.
        "DTP" => "gameData.eternity.dtpBought",
        "DTPMax" => "gameData.eternity.dtpMax",
        // EternityController/InfinityController are static-only types with no
        // path in from GameData (see ExtraRoots); these are the EP/IP amounts
        // a break would currently grant, not the already-banked totals.
        "nextEP" => "eternityController.EPGain",
        "nextBrokenEP" => "eternityController.brokenEPGain",
        "nextIP" => "infinityController.IPGain",
        "nextBrokenIP" => "infinityController.brokenIPGain",

        // GameData top-level: master unlock flags and cross-layer records.
        "infinityUnlocked" => "gameData.InfinityUnlocked",
        "eternityUnlocked" => "gameData.EternityUnlocked",
        "unityUnlocked" => "gameData.UnityUnlocked",
        "automationUnlocked" => "gameData.AutomationUnlocked",
        "attacksUnlocked" => "gameData.AttacksUnlocked",
        "mineralsUnlocked" => "gameData.MineralsUnlocked",
        "promotionUnlocked" => "gameData.PromotionUnlocked",
        "bestEP" => "gameData.bestEPs",
        "bestIP" => "gameData.bestIPs",
        "achievementsUnlockedCount" => "gameData.CountUnlockedAch",

        // Eternity/dilation siblings of EP/DP/AP/RP/DT/DTP already aliased above.
        "dtpFree" => "gameData.eternity.dtpFree",
        "dtpSpent" => "gameData.eternity.dtpSpent",
        "maxDP" => "gameData.eternity.maxDP",
        "dilationMaxScore" => "gameData.eternity.dilationMaxScore",
        "dilationMaxScoreCurrent" => "gameData.eternity.dilationMaxScoreCurrent",
        "dilationUnlocked" => "gameData.eternity.DilationUnlocked",
        "dilationTreeUnlocked" => "gameData.eternity.DilationTreeUnlocked",
        "challengesCompletedCount" => "gameData.eternity.ChallengesCompletedCount",
        "maxChallengeDiff" => "gameData.eternity.maxChalDiff",
        "supernovaLv" => "gameData.eternity.supernovaLv",
        "supernovaReq" => "gameData.eternity.supernovaReq",
        "supernovaUnlocked" => "gameData.eternity.SupernovaUnlocked",
        "ownedAnimalsCount" => "gameData.eternity.OwnedAnimalsCount",
        "labPtsNow" => "gameData.eternity.labPtsNow",
        "labPtsIncome" => "gameData.eternity.labPtsIncome",
        "labUnlocked" => "gameData.eternity.LabUnlocked",

        // Infinity: generator/star economy and challenge-unlock flags.
        "dustPerSec" => "gameData.infinity.dustPerSec",
        "stardustEffect" => "gameData.infinity.stardustEffect",
        "starCost" => "gameData.infinity.starCost",
        "totalStarBase" => "gameData.infinity.totalStarBase",
        "infinityChallengesCompleted" => "gameData.infinity.TotalChallengesCompleted",
        "infinityChallengeUnlocked" => "gameData.infinity.ChallengeUnlocked",
        "starUnlocked" => "gameData.infinity.StarUnlocked",
        "genMult" => "gameData.infinity.genMult",

        // Unity: trial/leveling progress and core unlock flags.
        "unityLevel" => "gameData.unity.currentLevel",
        "unityLuck" => "gameData.unity.luck",
        "trialsCompleted" => "gameData.unity.TrialCountCompleted",
        "trialsPending" => "gameData.unity.TrialCountPending",
        "trialsUnlocked" => "gameData.unity.TrialsUnlocked",
        "planetShopUnlocked" => "gameData.unity.PlanetShopUnlocked",
        "sacrificeUnlocked" => "gameData.unity.SacrificeUnlocked",
        "infiniteTimes" => "gameData.unity.infiniteTimes",

        // Singularity: atom economy and readiness/unlock flags.
        "atomsGain" => "gameData.singularity.atomsGain",
        "maxAtoms" => "gameData.singularity.maxAtoms",
        "atomsThreshold" => "gameData.singularity.atomsThreshold",
        "singMult" => "gameData.singularity.singMult",
        "singularityUnlocked" => "gameData.singularity.Unlocked",
        "singReady" => "gameData.singularity.SingReady",
        "singularityLuck" => "gameData.singularity.luck",

        // Plague: cure progress and unlock/stage tracking.
        "plagueUnlocked" => "gameData.plague.Unlocked",
        "cureProgress" => "gameData.plague.CureProgress",
        "cureReady" => "gameData.plague.CureReady",
        "plagueMaxStage" => "gameData.plague.maxStage",
        "plagueSpreadSpeed" => "gameData.plague.spreadSpeed",
        "plagueLocalSpeed" => "gameData.plague.localSpeed",

        // Tarot: deck-wide totals and unlock flag.
        "tarotUnlocked" => "gameData.tarot.Unlocked",
        "tarotTotalCards" => "gameData.tarot.totalCards",
        "tarotTotalUnlocked" => "gameData.tarot.totalUnlocked",
        "tarotLocalSpeed" => "gameData.tarot.localSpeed",

        _ => path
    };

    private static void WriteValue(Utf8JsonWriter writer, object? value, HashSet<object> references, HashSet<nint> pointers, bool collectionElement, int depth)
    {
        if (depth > MaxTraversalDepth)
            throw new InvalidOperationException("State traversal exceeded the maximum depth.");
        if (value is null)
        {
            writer.WriteNullValue();
            return;
        }

        Type type = value.GetType();
        if (IsExcludedType(type))
        {
            if (collectionElement)
            {
                writer.WriteNullValue();
                return;
            }
            throw new InvalidOperationException("Excluded runtime values cannot be serialized directly.");
        }
        if (type.FullName == "BigDouble" && type.Assembly.GetName().Name == "Assembly-CSharp")
        {
            writer.WriteStringValue(Format((double)type.GetProperty("Mantissa")!.GetValue(value)!, (double)type.GetProperty("Exponent")!.GetValue(value)!));
            return;
        }
        if (type.Assembly.GetName().Name == "ACTk.Runtime" && type.Namespace == "CodeStage.AntiCheat.ObscuredTypes")
        {
            WriteValue(writer, type.GetMethod("GetDecrypted", BindingFlags.Public | BindingFlags.Instance, null, Type.EmptyTypes, null)!.Invoke(value, null), references, pointers, collectionElement, depth + 1);
            return;
        }
        if (type.Assembly.GetName().Name == "Il2Cppmscorlib" && type.FullName == "Il2CppSystem.Object")
        {
            if (value is not Il2CppSystem.Object wrapper || wrapper.Pointer == IntPtr.Zero)
                throw new InvalidOperationException("The IL2CPP object wrapper has no runtime value.");
            nint objectClass = wrapper.ObjectClass;
            if (objectClass == 0)
                throw new InvalidOperationException("The IL2CPP object wrapper has no runtime type.");
            string objectNamespace = Il2CppInterop.Runtime.IL2CPP.il2cpp_class_get_namespace_(objectClass)!;
            string objectName = Il2CppInterop.Runtime.IL2CPP.il2cpp_class_get_name_(objectClass)!;
            object unboxed = (objectNamespace, objectName) switch
            {
                ("System", "Boolean") => wrapper.Unbox<bool>(),
                ("System", "Byte") => wrapper.Unbox<byte>(),
                ("System", "SByte") => wrapper.Unbox<sbyte>(),
                ("System", "Int16") => wrapper.Unbox<short>(),
                ("System", "UInt16") => wrapper.Unbox<ushort>(),
                ("System", "Int32") => wrapper.Unbox<int>(),
                ("System", "UInt32") => wrapper.Unbox<uint>(),
                ("System", "Int64") => wrapper.Unbox<long>(),
                ("System", "UInt64") => wrapper.Unbox<ulong>(),
                ("System", "Char") => wrapper.Unbox<char>(),
                ("System", "Single") => wrapper.Unbox<float>(),
                ("System", "Double") => wrapper.Unbox<double>(),
                ("System", "Decimal") => wrapper.Unbox<decimal>(),
                ("System", "String") => Il2CppInterop.Runtime.IL2CPP.Il2CppStringToManaged(wrapper.Pointer)!,
                ("System", "DateTime") => wrapper.Unbox<Il2CppSystem.DateTime>(),
                ("System", "DateTimeOffset") => wrapper.Unbox<Il2CppSystem.DateTimeOffset>(),
                ("", "BigDouble") => wrapper.Unbox<BigDouble>(),
                ("UnityEngine", "Color") => wrapper.Unbox<UnityEngine.Color>(),
                _ => throw new InvalidOperationException($"Unsupported IL2CPP object value {objectNamespace}.{objectName}.")
            };
            WriteValue(writer, unboxed, references, pointers, collectionElement, depth + 1);
            return;
        }
        if (type.Assembly.GetName().Name == "UnityEngine.CoreModule" && type.FullName == "UnityEngine.Color")
        {
            UnityEngine.Color color = (UnityEngine.Color)value;
            writer.WriteStartObject();
            writer.WriteNumber("r", color.r);
            writer.WriteNumber("g", color.g);
            writer.WriteNumber("b", color.b);
            writer.WriteNumber("a", color.a);
            writer.WriteEndObject();
            return;
        }
        if (type.Assembly.GetName().Name == "Il2Cppmscorlib" && type.FullName == "Il2CppSystem.DateTime")
        {
            writer.WriteStringValue(new DateTime(
                Convert.ToInt64(type.GetProperty("Ticks")!.GetValue(value), CultureInfo.InvariantCulture),
                (DateTimeKind)Convert.ToInt32(type.GetProperty("Kind")!.GetValue(value), CultureInfo.InvariantCulture)).ToString("O", CultureInfo.InvariantCulture));
            return;
        }
        if (type.Assembly.GetName().Name == "Il2Cppmscorlib" && type.FullName == "Il2CppSystem.DateTimeOffset")
        {
            object offset = type.GetProperty("Offset")!.GetValue(value)!;
            writer.WriteStringValue(new DateTimeOffset(
                Convert.ToInt64(type.GetProperty("Ticks")!.GetValue(value), CultureInfo.InvariantCulture),
                TimeSpan.FromTicks(Convert.ToInt64(offset.GetType().GetProperty("Ticks")!.GetValue(offset), CultureInfo.InvariantCulture))).ToString("O", CultureInfo.InvariantCulture));
            return;
        }
        if (type.Assembly.GetName().Name == "Il2Cppmscorlib" && type.FullName?.StartsWith("Il2CppSystem.Nullable`1", StringComparison.Ordinal) == true)
        {
            if ((bool)type.GetProperty("HasValue")!.GetValue(value)!)
                WriteValue(writer, type.GetProperty("Value")!.GetValue(value), references, pointers, collectionElement, depth + 1);
            else
                writer.WriteNullValue();
            return;
        }
        if (value is string text)
        {
            writer.WriteStringValue(text);
            return;
        }
        if (value is bool boolean)
        {
            writer.WriteBooleanValue(boolean);
            return;
        }
        if (type.IsEnum)
        {
            writer.WriteStringValue(value.ToString());
            return;
        }
        if (value is char character)
        {
            writer.WriteStringValue(character.ToString());
            return;
        }
        if (value is DateTime dateTime)
        {
            writer.WriteStringValue(dateTime.ToString("O", CultureInfo.InvariantCulture));
            return;
        }
        if (value is DateTimeOffset dateTimeOffset)
        {
            writer.WriteStringValue(dateTimeOffset.ToString("O", CultureInfo.InvariantCulture));
            return;
        }
        if (TryWriteNumber(writer, value))
            return;

        if (!TryVisit(value, references, pointers))
        {
            if (collectionElement)
                writer.WriteNullValue();
            return;
        }

        if (value is IDictionary dictionary)
        {
            writer.WriteStartObject();
            foreach (DictionaryEntry entry in dictionary)
            {
                writer.WritePropertyName(FormatDictionaryKey(entry.Key));
                WriteValue(writer, entry.Value, references, pointers, true, depth + 1);
            }
            writer.WriteEndObject();
            return;
        }
        if (IsReflectedDictionary(type))
        {
            writer.WriteStartObject();
            object enumerator = type.GetMethod("GetEnumerator", BindingFlags.Public | BindingFlags.Instance)!.Invoke(value, null)!;
            MethodInfo moveNext = enumerator.GetType().GetMethod("MoveNext", BindingFlags.Public | BindingFlags.Instance)!;
            PropertyInfo current = enumerator.GetType().GetProperty("Current", BindingFlags.Public | BindingFlags.Instance)!;
            while ((bool)moveNext.Invoke(enumerator, null)!)
            {
                object pair = current.GetValue(enumerator)!;
                writer.WritePropertyName(FormatDictionaryKey(pair.GetType().GetProperty("Key")!.GetValue(pair)!));
                WriteValue(writer, pair.GetType().GetProperty("Value")!.GetValue(pair), references, pointers, true, depth + 1);
            }
            writer.WriteEndObject();
            return;
        }
        if (IsReflectedCollection(type))
        {
            writer.WriteStartArray();
            int count = GetCollectionCount(value);
            PropertyInfo item = GetCollectionItem(type);
            for (int index = 0; index < count; index++)
                WriteValue(writer, item.GetValue(value, new object[] { index }), references, pointers, true, depth + 1);
            writer.WriteEndArray();
            return;
        }
        if (value is IEnumerable enumerable)
        {
            writer.WriteStartArray();
            foreach (object? item in enumerable)
                WriteValue(writer, item, references, pointers, true, depth + 1);
            writer.WriteEndArray();
            return;
        }

        writer.WriteStartObject();
        foreach (PropertyInfo property in GetProperties(type))
        {
            object? propertyValue = TryGetPropertyValue(property, value);
            if (IsExcludedType(propertyValue?.GetType()))
                continue;
            if (propertyValue is not null && IsTrackable(propertyValue.GetType()) && IsVisited(propertyValue, references, pointers))
                continue;
            writer.WritePropertyName(property.Name);
            WriteValue(writer, propertyValue, references, pointers, false, depth + 1);
        }
        writer.WriteEndObject();
    }

    private static bool TryWriteNumber(Utf8JsonWriter writer, object value)
    {
        switch (value)
        {
            case sbyte number: writer.WriteNumberValue(number); return true;
            case byte number: writer.WriteNumberValue(number); return true;
            case short number: writer.WriteNumberValue(number); return true;
            case ushort number: writer.WriteNumberValue(number); return true;
            case int number: writer.WriteNumberValue(number); return true;
            case uint number: writer.WriteNumberValue(number); return true;
            case long number:
                if (number is >= -MaxSafeInteger and <= MaxSafeInteger) writer.WriteNumberValue(number); else writer.WriteStringValue(number.ToString(CultureInfo.InvariantCulture));
                return true;
            case ulong number:
                if (number <= MaxSafeInteger) writer.WriteNumberValue(number); else writer.WriteStringValue(number.ToString(CultureInfo.InvariantCulture));
                return true;
            case float number:
                if (float.IsNaN(number)) writer.WriteStringValue("NaN"); else if (float.IsPositiveInfinity(number)) writer.WriteStringValue("Infinity"); else if (float.IsNegativeInfinity(number)) writer.WriteStringValue("-Infinity"); else writer.WriteNumberValue(number);
                return true;
            case double number:
                if (double.IsNaN(number)) writer.WriteStringValue("NaN"); else if (double.IsPositiveInfinity(number)) writer.WriteStringValue("Infinity"); else if (double.IsNegativeInfinity(number)) writer.WriteStringValue("-Infinity"); else writer.WriteNumberValue(number);
                return true;
            case decimal number: writer.WriteNumberValue(number); return true;
            default: return false;
        }
    }

    private static PropertyInfo[] GetProperties(Type type) => Properties.GetOrAdd(type, type =>
    {
        List<PropertyInfo> properties = new();
        HashSet<string> names = new(StringComparer.Ordinal);
        for (Type? current = type; current is not null
            && current != typeof(object)
            && current != typeof(ValueType)
            && current.FullName is not "Il2CppSystem.Object"
            && current.FullName is not "Il2CppInterop.Runtime.InteropTypes.Il2CppObjectBase"
            && current.FullName is not "UnityEngine.Object"
            && current.FullName is not "UnityEngine.Events.UnityEventBase"
            && current.FullName is not "Il2CppSystem.Delegate"
            && current.FullName is not "Il2CppSystem.MulticastDelegate"
            && current.FullName is not "System.Delegate"
            && current.FullName is not "System.MulticastDelegate";
            current = current.BaseType)
        {
            foreach (PropertyInfo property in current.GetProperties(BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly))
                if (names.Add(property.Name) && property.CanRead && property.GetMethod is { IsPublic: true } && property.GetIndexParameters().Length == 0 && !IsExcludedProperty(property))
                    properties.Add(property);
        }
        return properties.OrderBy(property => property.Name, StringComparer.Ordinal).ToArray();
    });

    private static bool IsExcludedProperty(PropertyInfo property) => property.Name is "Pointer" or "ObjectClass" or "WasCollected" or "Data" or "Controller" or "Parent"
        || property.Name.StartsWith("prop_", StringComparison.Ordinal)
        || property.Name.Contains("BackingField", StringComparison.Ordinal)
        || IsExcludedType(property.PropertyType);

    private static bool IsExcludedType(Type? type)
    {
        for (; type is not null; type = type.BaseType)
            if (type.FullName is "System.Delegate"
                or "System.MulticastDelegate"
                or "Il2CppSystem.Delegate"
                or "Il2CppSystem.MulticastDelegate"
                or "UnityEngine.Object"
                or "UnityEngine.Events.UnityEventBase")
                return true;
        return false;
    }

    private static bool IsTrackable(Type type) => !type.IsValueType && type != typeof(string);

    private static bool TryVisit(object value, HashSet<object> references, HashSet<nint> pointers)
    {
        if (!IsTrackable(value.GetType()))
            return true;
        if (TryGetPointer(value, out nint pointer))
            return pointers.Add(pointer);
        return references.Add(value);
    }

    private static bool IsVisited(object value, HashSet<object> references, HashSet<nint> pointers)
    {
        if (TryGetPointer(value, out nint pointer))
            return pointers.Contains(pointer);
        return references.Contains(value);
    }

    private static bool TryGetPointer(object value, out nint pointer)
    {
        Type? type = value.GetType();
        while (type is not null && type.FullName != "Il2CppInterop.Runtime.InteropTypes.Il2CppObjectBase")
            type = type.BaseType;
        if (type is null)
        {
            pointer = 0;
            return false;
        }
        pointer = (nint)type.GetProperty("Pointer", BindingFlags.Public | BindingFlags.Instance)!.GetValue(value)!;
        return pointer != 0;
    }

    private static bool TryGetDictionaryValue(object value, string segment, out object? result, out bool dictionary)
    {
        if (value is IDictionary clrDictionary)
        {
            dictionary = true;
            Type keyType = value.GetType().IsGenericType ? value.GetType().GetGenericArguments()[0] : typeof(object);
            if (!TryParseDictionaryKey(segment, keyType, out object? key) || key is null || !clrDictionary.Contains(key))
            {
                result = null;
                return false;
            }
            result = clrDictionary[key];
            return true;
        }
        if (!IsReflectedDictionary(value.GetType()))
        {
            result = null;
            dictionary = false;
            return false;
        }

        dictionary = true;
        Type reflectedKeyType = value.GetType().GetGenericArguments()[0];
        if (!TryParseDictionaryKey(segment, reflectedKeyType, out object? reflectedKey))
        {
            result = null;
            return false;
        }
        MethodInfo? tryGetValue = value.GetType().GetMethods(BindingFlags.Public | BindingFlags.Instance).FirstOrDefault(method => method.Name == "TryGetValue" && method.GetParameters().Length == 2);
        if (tryGetValue is null)
        {
            result = null;
            return false;
        }
        object?[] arguments = { reflectedKey, null };
        if (!(bool)tryGetValue.Invoke(value, arguments)!)
        {
            result = null;
            return false;
        }
        result = arguments[1];
        return true;
    }

    private static bool TryParseDictionaryKey(string segment, Type type, out object? key)
    {
        if (type == typeof(string))
        {
            key = segment;
            return true;
        }
        if (type.IsEnum)
        {
            try { key = Enum.Parse(type, segment, false); return true; } catch { key = null; return false; }
        }
        try
        {
            key = Convert.ChangeType(segment, type, CultureInfo.InvariantCulture);
            return type == typeof(sbyte) || type == typeof(byte) || type == typeof(short) || type == typeof(ushort) || type == typeof(int) || type == typeof(uint) || type == typeof(long) || type == typeof(ulong);
        }
        catch
        {
            key = null;
            return false;
        }
    }

    private static bool TryGetCollectionValue(object value, int index, out object? result)
    {
        if (value is IList list)
        {
            if (index >= list.Count)
            {
                result = null;
                return false;
            }
            result = list[index];
            return true;
        }
        if (!IsReflectedCollection(value.GetType()) || index >= GetCollectionCount(value))
        {
            result = null;
            return false;
        }
        result = GetCollectionItem(value.GetType()).GetValue(value, new object[] { index });
        return true;
    }

    private static bool IsReflectedCollection(Type type) => type.FullName?.StartsWith("Il2CppSystem.Collections.Generic.List`1", StringComparison.Ordinal) == true
        || type.FullName?.StartsWith("Il2CppInterop.Runtime.InteropTypes.Arrays.", StringComparison.Ordinal) == true;

    private static bool IsReflectedDictionary(Type type) => type.FullName?.StartsWith("Il2CppSystem.Collections.Generic.Dictionary`2", StringComparison.Ordinal) == true;

    private static int GetCollectionCount(object value)
    {
        PropertyInfo? count = value.GetType().GetProperty("Count", BindingFlags.Public | BindingFlags.Instance);
        return (int)(count ?? value.GetType().GetProperty("Length", BindingFlags.Public | BindingFlags.Instance)!).GetValue(value)!;
    }

    private static PropertyInfo GetCollectionItem(Type type) => type.GetProperties(BindingFlags.Public | BindingFlags.Instance).First(property => property.Name == "Item" && property.GetIndexParameters().Length == 1 && property.GetIndexParameters()[0].ParameterType == typeof(int));

    private static string FormatDictionaryKey(object key) => key is string text ? text : key.GetType().IsEnum ? key.ToString()! : Convert.ToString(key, CultureInfo.InvariantCulture)!;

    internal static byte[] Encode(IReadOnlyList<(string Key, double Mantissa, double Exponent)> values) => Encoding.UTF8.GetBytes(string.Concat("{", string.Join(",", values.Select(value => string.Concat("\"", value.Key, "\":\"", Format(value.Mantissa, value.Exponent), "\""))), "}"));

    internal static string Format(double mantissa, double exponent) => string.Concat(mantissa.ToString("R", CultureInfo.InvariantCulture), "e", exponent.ToString("R", CultureInfo.InvariantCulture));
}
