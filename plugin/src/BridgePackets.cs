using System.Text.Json;

namespace RevIdle.ScoreTelemetry;

internal sealed record StateReq(IReadOnlyList<string> Keys) : IRequest<StateRes>;
internal sealed record StateRes(JsonElement Value);
internal sealed record CaptureReq(int X, int Y, int Width, int Height) : IRequest<CaptureRes>;
internal sealed record CaptureRes(string? Type, string? Path);
internal sealed record InvokeReq(string Path) : IRequest<InvokeRes>;
internal sealed record InvokeRes;
internal sealed record TransferReq(string Source, string Destination) : IRequest<TransferRes>;
internal sealed record TransferRes;
internal sealed record ClickCommand(int X, int Y, int Width, int Height);
internal sealed record ScrollCommand(int X, int Y, int Length, uint Axis, int Width, int Height);
internal sealed record DragCommand(int StartX, int StartY, int EndX, int EndY, int Width, int Height);
