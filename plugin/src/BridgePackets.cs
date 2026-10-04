using System.Text.Json;
using System.Text.Json.Serialization;

namespace RevIdle.ScoreTelemetry;

internal sealed record StateReq(IReadOnlyList<string> Keys) : IRequest<StateRes>;
internal sealed record StateRes(JsonElement Value, [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] IReadOnlyList<string[]>? BigNums = null);
internal sealed record UiPathReq(int X, int Y, int Width, int Height, bool IncludeRectTransform = false) : IRequest<UiPathRes>;
internal sealed record UiPathRes(string? Type, string? Path, string? RectTransformPath = null);
internal sealed record InvokeReq(string Path) : IRequest<InvokeRes>;
internal sealed record InvokeRes;
internal sealed record ScrollIntoViewReq(string Path) : IRequest<ScrollIntoViewRes>;
internal sealed record ScrollIntoViewRes;
internal sealed record InputReq(string Path, string Text) : IRequest<InputRes>;
internal sealed record InputRes;
internal sealed record TransferReq(string Source, string Destination) : IRequest<TransferRes>;
internal sealed record TransferRes;
internal sealed record SlotReq(string Path) : IRequest<SlotRes>;
internal sealed record SlotRes(JsonElement Value, [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] IReadOnlyList<string[]>? BigNums = null);
internal sealed record ClickCommand(int X, int Y, int Width, int Height);
internal sealed record ScrollCommand(int X, int Y, int Length, uint Axis, int Width, int Height);
internal sealed record DragCommand(int StartX, int StartY, int EndX, int EndY, int Width, int Height);
internal sealed record PressCommand(string Key);
internal sealed record ReloadScript;
internal sealed record ReloadLockedScript;
internal sealed record StopScript;
internal sealed record PauseScript;
internal sealed record ResumeScript;
internal sealed record ResumeLockedScript;
internal sealed record StartCapture;
internal sealed record StopCapture;
internal sealed record LockScript;
internal sealed record LoadScript(string Path, bool Locked);
internal sealed record RemoveScriptHistory(string Path);
internal sealed record StateUpdate(string Phase, bool Capture, bool Locked = false, IReadOnlyList<string>? Scripts = null, string? LockLabel = null);
internal sealed record ScriptUiLengthState(
    double? Fixed,
    double Min,
    double? Max);
internal sealed record ScriptUiBorderState(
    double Thickness,
    [property: JsonConverter(typeof(ScriptUiColorConverter))] byte[] Color);
internal sealed record ScriptUiCornerState(
    double TopLeft,
    double TopRight,
    double BottomLeft,
    double BottomRight);
internal sealed record ScriptUiPaddingState(
    double Top,
    double Right,
    double Bottom,
    double Left);
internal sealed record ScriptUiElementState(
    string Id,
    Guid InstanceId,
    ulong EventsVersion,
    string Text,
    string Font,
    string AlignX,
    string AlignY,
    double PosX,
    double PosY,
    ScriptUiLengthState LenX,
    ScriptUiLengthState LenY,
    [property: JsonConverter(typeof(ScriptUiColorConverter))] byte[] Color,
    [property: JsonConverter(typeof(ScriptUiColorConverter))] byte[] TextColor,
    ScriptUiBorderState Border,
    ScriptUiCornerState Corner,
    ScriptUiPaddingState Padding,
    string[] Events,
    bool Hidden = false,
    string BasedOn = "")
{
    public int Size { get; init; } = 14;
}
internal sealed record ScriptUiSnapshot(Guid? SessionId, ulong Revision, ScriptUiElementState[] Elements);
internal sealed record ScriptUiEvent(Guid SessionId, string ElementId, Guid InstanceId, ulong EventsVersion, string Event);
internal sealed record ScriptUiPointer(Guid SessionId, ulong PressId, string Phase, int X, int Y, int Width, int Height);
internal sealed record ScriptUiMeasureReq(Guid SessionId, ulong Revision, string ElementId, Guid InstanceId, string RelativeTo = "") : IRequest<ScriptUiMeasureRes>;
internal sealed record ScriptUiMeasureRes(double Width, double Height, double[]? GlobalX, double[]? GlobalY);

internal sealed class ScriptUiColorConverter : JsonConverter<byte[]>
{
    public override byte[] Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.Null)
            return Array.Empty<byte>();
        if (reader.TokenType != JsonTokenType.StartArray)
            throw new JsonException("Script UI colors must be arrays.");
        List<byte> values = new();
        while (reader.Read() && reader.TokenType != JsonTokenType.EndArray)
        {
            if (reader.TokenType != JsonTokenType.Number || !reader.TryGetByte(out byte value))
                throw new JsonException("Script UI color channels must be bytes.");
            values.Add(value);
        }
        if (reader.TokenType != JsonTokenType.EndArray)
            throw new JsonException("Script UI color array is incomplete.");
        return values.ToArray();
    }

    public override void Write(Utf8JsonWriter writer, byte[] value, JsonSerializerOptions options)
    {
        if (value is null)
        {
            writer.WriteNullValue();
            return;
        }
        writer.WriteStartArray();
        foreach (byte channel in value)
            writer.WriteNumberValue(channel);
        writer.WriteEndArray();
    }
}
