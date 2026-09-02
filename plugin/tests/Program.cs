using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;
using RevIdle.ScoreTelemetry;

InvalidPortsDisableServer();
await ServerReturnsQueuedState();
await ServerReturnsNotFound();
await ServerReturnsMethodNotAllowed();
await ServerReturnsUnavailableState();
await ServerDisposesPendingRequest();
BridgeDecodesFullWidthCoordinates();
BridgeRejectsZeroRequestId();
BridgeMapsTopLeftClientCoordinatesToUnityCoordinates();
BridgeQueueRejectsDuplicatesAndOverflow();
BridgeQueueDequeuesInOrderAndClears();
BridgeCallbackIdentityRequiresUndisposedActiveWindowAndSubclass();
BridgeOwnershipReleaseRequiresDestroyedOrSuccessfulOwnerRemoval();
DispatcherSkipsNonClickableRaycasts();
ScrollProtocolDecodesCoordinatesSignedLengthAxisAndRequestId();
ScrollProtocolRejectsZeroRequestId();
ScrollQueuePreservesOrderAndRejectsDuplicates();
DispatcherFindsFirstScrollableRaycast();
System.Console.WriteLine("20 tests passed.");

static void InvalidPortsDisableServer()
{
    foreach (string? value in new string?[] { null, "", "not-a-port", "0", "-1", "65536" })
    {
        using HttpScoreServer? server = HttpScoreServer.Create(value);
        if (server is not null)
            throw new InvalidOperationException($"{nameof(InvalidPortsDisableServer)}: '{value}' should be disabled.");
    }
}

static async Task ServerReturnsQueuedState()
{
    using HttpScoreServer server = CreateServer();
    using TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    await using NetworkStream stream = client.GetStream();
    await stream.WriteAsync(Encoding.ASCII.GetBytes("GET /state HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"));
    await WaitForPending(server);
    server.CompletePending(_ => (200, Encoding.UTF8.GetBytes("{\"score\":\"2.5e42\"}")));
    string response = await ReadResponse(stream);
    Equal("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 18\r\nConnection: close\r\n\r\n{\"score\":\"2.5e42\"}", response, nameof(ServerReturnsQueuedState));
}

static async Task ServerReturnsNotFound()
{
    Equal(true, (await Request("GET /other HTTP/1.1", null)).StartsWith("HTTP/1.1 404 Not Found\r\n", StringComparison.Ordinal), nameof(ServerReturnsNotFound));
}

static async Task ServerReturnsMethodNotAllowed()
{
    Equal(true, (await Request("POST /state HTTP/1.1", null)).StartsWith("HTTP/1.1 405 Method Not Allowed\r\n", StringComparison.Ordinal), nameof(ServerReturnsMethodNotAllowed));
}

static async Task ServerReturnsUnavailableState()
{
    using HttpScoreServer server = CreateServer();
    Equal(true, (await RequestWithServer(server, "GET /state HTTP/1.1", null)).StartsWith("HTTP/1.1 503 Service Unavailable\r\n", StringComparison.Ordinal), nameof(ServerReturnsUnavailableState));
}

static async Task ServerDisposesPendingRequest()
{
    HttpScoreServer server = CreateServer();
    using TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    await using NetworkStream stream = client.GetStream();
    await stream.WriteAsync(Encoding.ASCII.GetBytes("GET /state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"));
    await WaitForPending(server);
    server.Dispose();
    Equal(0, await stream.ReadAsync(new byte[1]), nameof(ServerDisposesPendingRequest));
}

static HttpScoreServer CreateServer()
{
    using var probe = new TcpListener(IPAddress.Loopback, 0);
    probe.Start();
    int port = ((IPEndPoint)probe.LocalEndpoint).Port;
    probe.Stop();
    return HttpScoreServer.Create(port.ToString(CultureInfo.InvariantCulture)) ?? throw new InvalidOperationException("server failed to bind");
}

static async Task WaitForPending(HttpScoreServer server)
{
    await Task.Delay(100);
}

static async Task<string> Request(string request, byte[]? payload)
{
    using HttpScoreServer server = CreateServer();
    return await RequestWithServer(server, request, payload);
}

static async Task<string> RequestWithServer(HttpScoreServer server, string request, byte[]? payload)
{
    using TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    await using NetworkStream stream = client.GetStream();
    await stream.WriteAsync(Encoding.ASCII.GetBytes($"{request}\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"));
    if (request.StartsWith("GET /state", StringComparison.Ordinal))
    {
        await WaitForPending(server);
        server.CompletePending(_ => payload is null ? (503, Array.Empty<byte>()) : (200, payload));
    }
    return await ReadResponse(stream);
}

static async Task<string> ReadResponse(NetworkStream stream)
{
    using var memory = new MemoryStream();
    byte[] buffer = new byte[1024];
    int read;
    while ((read = await stream.ReadAsync(buffer)) > 0)
        memory.Write(buffer, 0, read);
    return Encoding.UTF8.GetString(memory.ToArray());
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

static void DispatcherSkipsNonClickableRaycasts()
{
    const string testName = nameof(DispatcherSkipsNonClickableRaycasts);

    Equal(2, UnityUiClickDispatcher.FindFirstClickableIndex(new[] { false, false, true }), testName);
    Equal(-1, UnityUiClickDispatcher.FindFirstClickableIndex(new[] { false, false }), testName);
}

static void ScrollProtocolDecodesCoordinatesSignedLengthAxisAndRequestId()
{
    const string testName = nameof(ScrollProtocolDecodesCoordinatesSignedLengthAxisAndRequestId);
    Equal((uint)0x8418, InputBridgeProtocol.ScrollMessageId, testName);
    Equal(true, InputBridgeProtocol.TryDecodeScroll(
        unchecked((nuint)((42UL << 33) | (1UL << 32) | 0xFFFFFFFCUL)),
        unchecked((nint)(long)0x12345678_ABCDEF01UL),
        out ScrollCommand command), testName);
    Equal(42UL, command.RequestId, testName);
    Equal(-4, command.Length, testName);
    Equal(1U, command.Axis, testName);
    Equal(0xABCDEF01U, command.X, testName);
    Equal(0x12345678U, command.Y, testName);
}

static void ScrollProtocolRejectsZeroRequestId()
{
    Equal(false, InputBridgeProtocol.TryDecodeScroll(
        unchecked((nuint)((1UL << 32) | 5UL)),
        (nint)1,
        out _), nameof(ScrollProtocolRejectsZeroRequestId));
}

static void ScrollQueuePreservesOrderAndRejectsDuplicates()
{
    const string testName = nameof(ScrollQueuePreservesOrderAndRejectsDuplicates);
    var queue = new ScrollCommandQueue();
    Equal(true, InputBridgeProtocol.TryDecodeScroll(
        unchecked((nuint)((7UL << 33) | unchecked((uint)-2))),
        unchecked((nint)(long)0x00000002_00000001UL),
        out ScrollCommand first), testName);
    Equal(true, queue.TryEnqueue(first), testName);
    Equal(false, queue.TryEnqueue(first), testName);
    Equal(true, queue.TryDequeue(out ScrollCommand command), testName);
    Equal(7UL, command.RequestId, testName);
}

static void DispatcherFindsFirstScrollableRaycast()
{
    const string testName = nameof(DispatcherFindsFirstScrollableRaycast);
    Equal(1, UnityUiClickDispatcher.FindFirstScrollableIndex(new[] { false, true, false }), testName);
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
