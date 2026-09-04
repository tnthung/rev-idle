namespace RevIdle.ScoreTelemetry;

internal readonly record struct ClickCommand(ulong RequestId, uint X, uint Y);
internal readonly record struct ScrollCommand(ulong RequestId, uint X, uint Y, int Length, uint Axis);
internal readonly record struct DragCommand(ulong RequestId, uint StartX, uint StartY, uint EndX, uint EndY);

internal static class InputBridgeProtocol
{
    internal const uint MessageId = 0x8417;
    internal const uint ScrollMessageId = 0x8418;
    internal const uint DragStartMessageId = 0x8419;
    internal const uint DragEndMessageId = 0x841A;

    internal static bool TryDecode(nuint requestId, nint packedCoordinates, out ClickCommand command)
    {
        if (requestId == 0)
        {
            command = default;
            return false;
        }

        ulong packed = unchecked((ulong)(long)packedCoordinates);
        command = new ClickCommand((ulong)requestId, (uint)packed, (uint)(packed >> 32));
        return true;
    }

    internal static bool TryDecodeScroll(nuint packedRequest, nint packedCoordinates, out ScrollCommand command)
    {
        ulong request = (ulong)packedRequest;
        ulong requestId = request >> 33;
        if (requestId == 0)
        {
            command = default;
            return false;
        }

        ulong coordinates = unchecked((ulong)(long)packedCoordinates);
        command = new ScrollCommand(
            requestId,
            (uint)coordinates,
            (uint)(coordinates >> 32),
            unchecked((int)(uint)request),
            (uint)((request >> 32) & 1));
        return true;
    }

    internal static bool TryMapToUnity(
        ClickCommand command,
        int clientWidth,
        int clientHeight,
        int screenWidth,
        int screenHeight,
        out float unityX,
        out float unityY)
    {
        if (clientWidth <= 0 || clientHeight <= 0 || screenWidth <= 0 || screenHeight <= 0 ||
            command.X >= (uint)clientWidth || command.Y >= (uint)clientHeight)
        {
            unityX = 0;
            unityY = 0;
            return false;
        }

        unityX = command.X * (float)screenWidth / clientWidth;
        unityY = screenHeight - 1f - command.Y * (float)screenHeight / clientHeight;
        return true;
    }
}

internal static class DragCommandFactory
{
    internal static DragCommand FromEndpoints(ClickCommand start, ClickCommand end) =>
        new(end.RequestId, start.X, start.Y, end.X, end.Y);
}
