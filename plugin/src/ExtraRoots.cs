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
/// Every top-level root a state path can be addressed against: GameData
/// itself, plus the static-only *Controller types (EternityController,
/// InfinityController, UnityController, ...) holding computed values -- like
/// the EP a break would currently grant -- that nothing on GameData ever
/// references, so no dotted path from GameData can reach them no matter how
/// deep. There is no implicit or default root: a request path's first
/// segment must always name one of these keys, GameData included.
/// </summary>
internal static class ExtraRoots
{
    // First letter of each type name lowercased: EternityController -> "eternityController".
    private static readonly IReadOnlyDictionary<string, Type> ControllerRoots = new[]
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

    public const string GameDataKey = "gameData";

    // The root object passed into StatePayload.Encode is addressed by
    // GameDataKey regardless of its static type, so unit tests can stand in
    // arbitrary fixtures for it without depending on the real GameData type.
    public static bool TryGet(string firstSegment, object data, out object? root)
    {
        if (firstSegment == GameDataKey)
        {
            root = data;
            return true;
        }
        if (ControllerRoots.TryGetValue(firstSegment, out Type? type))
        {
            root = StaticPropertySnapshot.Capture(type);
            return true;
        }
        root = null;
        return false;
    }
}
