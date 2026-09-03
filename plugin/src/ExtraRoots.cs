using System.Reflection;

namespace RevIdle.ScoreTelemetry;

/// <summary>
/// Captures every public static property on a type as a plain string-keyed
/// dictionary, the moment the request is served. StatePayload's path
/// resolver and serializer already understand IDictionary values (that's
/// how dictionary-typed gameplay fields are addressed), so this reuses that
/// support to expose static-only state -- state with no path in from
/// GameData because nothing GameData references points at it -- the same
/// way any other object's fields are addressed and dumped.
/// </summary>
internal static class StaticPropertySnapshot
{
    public static Dictionary<string, object?> Capture(Type type)
    {
        Dictionary<string, object?> values = new(StringComparer.Ordinal);
        foreach (PropertyInfo property in type.GetProperties(BindingFlags.Public | BindingFlags.Static | BindingFlags.FlattenHierarchy))
        {
            if (!property.CanRead || property.GetMethod is not { IsPublic: true } || property.GetIndexParameters().Length != 0)
                continue;
            // A throwing getter is treated as absent (null), not fatal, the
            // same way StatePayload treats every other property -- see its
            // TryGetPropertyValue.
            object? value;
            try { value = property.GetValue(null); }
            catch { value = null; }
            values[property.Name] = value;
        }
        return values;
    }
}

/// <summary>
/// Top-level roots reachable alongside GameData, for state that lives on
/// static-only *Controller types. Each game system has one (EternityController,
/// InfinityController, UnityController, ...) holding computed values -- like
/// the EP a break would currently grant -- that nothing on GameData ever
/// references, so no dotted path from GameData can reach them no matter how
/// deep. A request path whose first segment matches a key here resolves the
/// remaining segments against that controller's static properties instead.
/// </summary>
internal static class ExtraRoots
{
    // First letter of each type name lowercased: EternityController -> "eternityController".
    private static readonly IReadOnlyDictionary<string, Type> Roots = new[]
    {
        typeof(Controller),
        typeof(AttacksController),
        typeof(AutomationController),
        typeof(ElementsController),
        typeof(EternityController),
        typeof(GameController),
        typeof(InfinityController),
        typeof(MacroController),
        typeof(MineralsController),
        typeof(PlagueController),
        typeof(SaveController),
        typeof(SingularityController),
        typeof(TarotController),
        typeof(UnityController),
    }.ToDictionary(type => char.ToLowerInvariant(type.Name[0]) + type.Name[1..], StringComparer.Ordinal);

    public static bool TryGet(string firstSegment, out object? root)
    {
        if (Roots.TryGetValue(firstSegment, out Type? type))
        {
            root = StaticPropertySnapshot.Capture(type);
            return true;
        }
        root = null;
        return false;
    }
}
