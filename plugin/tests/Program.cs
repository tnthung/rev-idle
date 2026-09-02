using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;
using RevIdle.ScoreTelemetry;

await ScorePayloadUsesInvariantRoundTripFormatting();
InvalidPortsDisablePublishing();
await PublisherSendsExactPayloadToLoopback();
TickerUsesFiftyMillisecondAccumulator();
BridgeDecodesFullWidthCoordinates();
BridgeRejectsZeroRequestId();
BridgeMapsTopLeftClientCoordinatesToUnityCoordinates();
BridgeQueueRejectsDuplicatesAndOverflow();
BridgeQueueDequeuesInOrderAndClears();
BridgeCallbackIdentityRequiresUndisposedActiveWindowAndSubclass();
BridgeOwnershipReleaseRequiresDestroyedOrSuccessfulOwnerRemoval();
System.Console.WriteLine("11 tests passed.");

static Task ScorePayloadUsesInvariantRoundTripFormatting()
{
    CultureInfo previous = CultureInfo.CurrentCulture;
    CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("fr-FR");
    try
    {
        string actual = Encoding.UTF8.GetString(ScorePayload.Encode(1.2345678901234567, 123));
        Equal("{\"score\":\"1.2345678901234567e123\"}", actual, nameof(ScorePayloadUsesInvariantRoundTripFormatting));
    }
    finally
    {
        CultureInfo.CurrentCulture = previous;
    }

    return Task.CompletedTask;
}

static void InvalidPortsDisablePublishing()
{
    foreach (string? value in new string?[] { null, "", "not-a-port", "0", "-1", "65536" })
    {
        using UdpScorePublisher? publisher = UdpScorePublisher.Create(value);
        if (publisher is not null)
            throw new InvalidOperationException($"{nameof(InvalidPortsDisablePublishing)}: '{value}' should be disabled.");
    }
}

static async Task PublisherSendsExactPayloadToLoopback()
{
    using var receiver = new UdpClient(new IPEndPoint(IPAddress.Loopback, 0));
    int port = ((IPEndPoint)receiver.Client.LocalEndPoint!).Port;
    using UdpScorePublisher publisher = UdpScorePublisher.Create(port.ToString(CultureInfo.InvariantCulture))
        ?? throw new InvalidOperationException($"{nameof(PublisherSendsExactPayloadToLoopback)}: valid port was rejected.");
    byte[] payload = Encoding.UTF8.GetBytes("{\"score\":\"2.5e42\"}");

    publisher.Publish(payload);

    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(2));
    UdpReceiveResult result = await receiver.ReceiveAsync(timeout.Token);
    Equal("127.0.0.1", result.RemoteEndPoint.Address.ToString(), nameof(PublisherSendsExactPayloadToLoopback));
    Equal("{\"score\":\"2.5e42\"}", Encoding.UTF8.GetString(result.Buffer), nameof(PublisherSendsExactPayloadToLoopback));
}

static void TickerUsesFiftyMillisecondAccumulator()
{
    float elapsed = 0f;
    Equal(false, ScoreTicker.AdvanceTimer(ref elapsed, 0.049f), nameof(TickerUsesFiftyMillisecondAccumulator));
    Equal(true, ScoreTicker.AdvanceTimer(ref elapsed, 0.001f), nameof(TickerUsesFiftyMillisecondAccumulator));
    Near(0f, elapsed, nameof(TickerUsesFiftyMillisecondAccumulator));
    Equal(true, ScoreTicker.AdvanceTimer(ref elapsed, 0.12f), nameof(TickerUsesFiftyMillisecondAccumulator));
    Near(0.02f, elapsed, nameof(TickerUsesFiftyMillisecondAccumulator));
}

static void BridgeDecodesFullWidthCoordinates()
{
    nint packed = unchecked((nint)(long)0x12345678ABCDEF01UL);
    Equal(true, InputBridgeProtocol.TryDecode(42, packed, out ClickCommand command), nameof(BridgeDecodesFullWidthCoordinates));
    Equal(42UL, command.RequestId, nameof(BridgeDecodesFullWidthCoordinates));
    Equal(0xABCDEF01U, command.X, nameof(BridgeDecodesFullWidthCoordinates));
    Equal(0x12345678U, command.Y, nameof(BridgeDecodesFullWidthCoordinates));
}

static void BridgeRejectsZeroRequestId()
{
    Equal(false, InputBridgeProtocol.TryDecode(0, 0, out _), nameof(BridgeRejectsZeroRequestId));
}

static void BridgeMapsTopLeftClientCoordinatesToUnityCoordinates()
{
    var command = new ClickCommand(1, 1200, 80);
    Equal(true, InputBridgeProtocol.TryMapToUnity(command, 1920, 1080, 1920, 1080, out float x, out float y), nameof(BridgeMapsTopLeftClientCoordinatesToUnityCoordinates));
    Near(1200f, x, nameof(BridgeMapsTopLeftClientCoordinatesToUnityCoordinates));
    Near(999f, y, nameof(BridgeMapsTopLeftClientCoordinatesToUnityCoordinates));
    Equal(false, InputBridgeProtocol.TryMapToUnity(command, 1200, 1080, 1920, 1080, out _, out _), nameof(BridgeMapsTopLeftClientCoordinatesToUnityCoordinates));
}

static void BridgeQueueRejectsDuplicatesAndOverflow()
{
    var queue = new ClickCommandQueue();
    Equal(true, queue.TryEnqueue(new ClickCommand(1, 1, 1)), nameof(BridgeQueueRejectsDuplicatesAndOverflow));
    Equal(false, queue.TryEnqueue(new ClickCommand(1, 2, 2)), nameof(BridgeQueueRejectsDuplicatesAndOverflow));
    for (ulong id = 2; id <= 32; id++)
        Equal(true, queue.TryEnqueue(new ClickCommand(id, 1, 1)), nameof(BridgeQueueRejectsDuplicatesAndOverflow));
    Equal(false, queue.TryEnqueue(new ClickCommand(33, 1, 1)), nameof(BridgeQueueRejectsDuplicatesAndOverflow));
}

static void BridgeQueueDequeuesInOrderAndClears()
{
    var queue = new ClickCommandQueue();
    queue.TryEnqueue(new ClickCommand(10, 1, 2));
    queue.TryEnqueue(new ClickCommand(11, 3, 4));
    Equal(true, queue.TryDequeue(out ClickCommand first), nameof(BridgeQueueDequeuesInOrderAndClears));
    Equal(10UL, first.RequestId, nameof(BridgeQueueDequeuesInOrderAndClears));
    queue.Clear();
    Equal(false, queue.TryDequeue(out _), nameof(BridgeQueueDequeuesInOrderAndClears));
}

static void BridgeCallbackIdentityRequiresUndisposedActiveWindowAndSubclass()
{
    const string testName = nameof(BridgeCallbackIdentityRequiresUndisposedActiveWindowAndSubclass);
    nint activeWindow = (nint)0x1234;

    Equal(true, Win32InputBridge.IsCallbackIdentityValid(false, activeWindow, activeWindow, Win32InputBridge.SubclassId), testName);
    Equal(false, Win32InputBridge.IsCallbackIdentityValid(true, activeWindow, activeWindow, Win32InputBridge.SubclassId), testName);
    Equal(false, Win32InputBridge.IsCallbackIdentityValid(false, (nint)0x5678, activeWindow, Win32InputBridge.SubclassId), testName);
    Equal(false, Win32InputBridge.IsCallbackIdentityValid(false, activeWindow, activeWindow, Win32InputBridge.SubclassId + 1), testName);
}

static void BridgeOwnershipReleaseRequiresDestroyedOrSuccessfulOwnerRemoval()
{
    const string testName = nameof(BridgeOwnershipReleaseRequiresDestroyedOrSuccessfulOwnerRemoval);

    Equal(true, Win32InputBridge.CanReleaseManagedOwnership(true, false, false), testName);
    Equal(true, Win32InputBridge.CanReleaseManagedOwnership(false, true, true), testName);
    Equal(false, Win32InputBridge.CanReleaseManagedOwnership(false, true, false), testName);
    Equal(false, Win32InputBridge.CanReleaseManagedOwnership(false, false, true), testName);
}

static void Near(float expected, float actual, string testName)
{
    if (MathF.Abs(expected - actual) > 0.0001f)
        throw new InvalidOperationException($"{testName}: expected approximately '{expected}', got '{actual}'.");
}

static void Equal<T>(T expected, T actual, string testName)
{
    if (!EqualityComparer<T>.Default.Equals(expected, actual))
        throw new InvalidOperationException($"{testName}: expected '{expected}', got '{actual}'.");
}
