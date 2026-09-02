namespace RevIdle.ScoreTelemetry;

internal readonly record struct ClickCommand(ulong RequestId, uint X, uint Y);

internal static class InputBridgeProtocol
{
    internal const uint MessageId = 0x8000 + 0x417;

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
