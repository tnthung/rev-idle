using System.Collections.Concurrent;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Net.WebSockets;
using System.Reflection;
using System.Reflection.Emit;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using RevIdle.ScoreTelemetry;

StatePayloadFormatsBigDoubleValues();
StatePayloadSerializesCompleteGraph();
StatePayloadResolvesSelectedPaths();
StatePayloadDistinguishesInvalidPathsAndGetterFailures();
StatePayloadPreservesCollectionIndexesAcrossCycles();
StatePayloadSerializesGameSpecificScalars();
StatePayloadKeepsSelectedKeysForSharedValues();
StatePayloadSerializesIl2CppDatesAndNullables();
StatePayloadRejectsUnsupportedIl2CppObjectWrappers();
StatePayloadUsesIl2CppCollectionAccessors();
StatePayloadIncludesInheritedGameplayProperties();
StatePayloadSerializesUnityColorAsChannels();
StatePayloadRejectsRunawayValueTraversal();
StatePayloadRejectsImplementationPropertyPaths();
StatePayloadExcludesIl2CppDelegatesAndUnityEvents();
StatePayloadExcludesRuntimeTypesAtEveryBoundary();
BridgePacketPayloadsMatchSharedFixture();
await BridgeStateHandlerPreservesMixedJsonAndSelectedKeys();
await BridgeStateHandlerReportsMissingData();
await BridgeStateHandlerReportsInvalidPath();
await BridgeUiHandlersReportCorrelatedErrorsOnPumpThread();
await BridgeHandlersStartQueuedRequestWhileEarlierResponseIsPending();
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
DragCommandFactoryCombinesStartAndEndEndpoints();
DragQueuePairsStartAndEndByRequestIdAndRejectsDuplicates();
DragQueueIgnoresEndWithoutMatchingStart();
WsEnvelopeMatchesSharedFixture();
await WsDisconnectedCallsFailImmediately();
await WsRequestCorrelatesResponseByUuidAndType();
await WsConcurrentSendsShareOneWriter();
await WsHandlersStartWithoutWaitingForEarlierHandlers();
await WsResponseTypeMismatchFailsRequest();
await WsRemoteErrorFailsRequest();
await WsTimeoutSendsCancelAndDropsLateResponse();
await WsDisconnectFailsPendingAndAllowsNewClient();
await WsRejectsSecondActiveClient();
await WsMalformedPacketsDoNotStopReader();
await WsMissingUuidPacketIsReportedAndReaderContinues();
await WsCancelPreventsQueuedHandlerStart();
await WsCancelBlocksRunningHandlerResponse();
await WsDuplicateActiveUuidKeepsOriginalOwner();
await WsHandlerFailureSendsRemoteErrorWhenQueueBusy();
await WsDisconnectPressureFailsQueuedSend();
await WsDisposeDoesNotWaitForArbitraryHandler();
await WsLifecycleFaultsAreObserved();
await WsResponseSerializationFailureDoesNotConsumeResponse();
await WsHandlerErrorsAreObservedAndTasksCleaned();
await WsUnknownPacketGetsRemoteError();
await WsPumpRunsSynchronousHandlerPrefixOnCallerThread();
await WsOldGenerationBufferedFrameIsIgnoredAfterReconnect();
await WsCloseVsPumpRegistrationInterleaving();
await WsBlockingSynchronousPrefixDoesNotBlockStop();
await WsHandlerCanInitiateNestedRequestWithoutAwait();
await WsMalformedCorrelatedRemoteErrorFailsPromptly();
await WsTimeoutRemovalRaceAwaitsWinningCompletion();
await WsLateRemoteErrorIsReportedBeforeTombstoneDiscard();
System.Console.WriteLine("80 tests passed.");

static void WsEnvelopeMatchesSharedFixture()
{
    Guid uuid = Guid.Parse("7747b71a-66bc-4fc6-bf85-e9828277addf");
    string actual = WsConnection.SerializeForTest(uuid, new TestReq("hello"));
    string expected = File.ReadAllText(Path.Combine(
        AppContext.BaseDirectory,
        "protocol",
        "test-request.json")).TrimEnd();
    Equal(expected, actual, nameof(WsEnvelopeMatchesSharedFixture));
}

static void BridgePacketPayloadsMatchSharedFixture()
{
    using JsonDocument fixture = JsonDocument.Parse(File.ReadAllText(Path.Combine(
        AppContext.BaseDirectory,
        "protocol",
        "bridge-packets.json")));
    (string Name, object Packet)[] packets = new[]
    {
        ("StateReq", (object)new StateReq(new[] { "score", "eternity.dtpSpent" })),
        ("StateRes", new StateRes(JsonSerializer.Deserialize<JsonElement>("{\"score\":\"1e3\",\"enabled\":true,\"nested\":{\"value\":null},\"items\":[1,\"two\",false]}"))),
        ("CaptureReq", new CaptureReq(123, -45)),
        ("CaptureRes", new CaptureRes("slot", "scene:1/Canvas[0]/Inventory/3")),
        ("InvokeReq", new InvokeReq("scene:1/Canvas[0]/Buy DTP & More[0]")),
        ("InvokeRes", new InvokeRes()),
        ("TransferReq", new TransferReq("scene:1/Canvas[0]/Inventory/3", "scene:1/Canvas[0]/Combine/0")),
        ("TransferRes", new TransferRes())
    };

    foreach ((string name, object packet) in packets)
    {
        using JsonDocument envelope = JsonDocument.Parse(WsConnection.SerializeForTest(Guid.Empty, packet));
        Equal(name, envelope.RootElement.GetProperty("type").GetString(), nameof(BridgePacketPayloadsMatchSharedFixture));
        Equal(
            JsonNode.Parse(fixture.RootElement.GetProperty(name).GetRawText())!.ToJsonString(),
            JsonNode.Parse(envelope.RootElement.GetProperty("payload").GetRawText())!.ToJsonString(),
            nameof(BridgePacketPayloadsMatchSharedFixture));
    }

    Equal(
        "{\"type\":null,\"path\":null}",
        JsonSerializer.Serialize(new CaptureRes(null, null), new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase }),
        nameof(BridgePacketPayloadsMatchSharedFixture));
}

static async Task BridgeStateHandlerPreservesMixedJsonAndSelectedKeys()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    int dataThread = 0;
    int pumpThread = 0;
    Plugin.RegisterHandlers(server, () =>
    {
        Volatile.Write(ref dataThread, Environment.CurrentManagedThreadId);
        return new BridgeStateFixture();
    }, () => 0);
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid uuid = Guid.NewGuid();
        string envelope = $"{{\"uuid\":\"{uuid}\",\"type\":\"StateReq\",\"payload\":{{\"keys\":[\"gameData.Score\",\"gameData.Enabled\",\"gameData.Nested\",\"gameData.Items\"]}}}}";
        using JsonDocument response = await SendLiteralBridgeRequestAndPump(server, peer, 0, envelope, thread =>
        {
            if (Volatile.Read(ref dataThread) == 0)
                Volatile.Write(ref pumpThread, thread);
        });
        Equal(uuid, response.RootElement.GetProperty("uuid").GetGuid(), nameof(BridgeStateHandlerPreservesMixedJsonAndSelectedKeys));
        Equal("StateRes", response.RootElement.GetProperty("type").GetString(), nameof(BridgeStateHandlerPreservesMixedJsonAndSelectedKeys));
        Equal(pumpThread, dataThread, nameof(BridgeStateHandlerPreservesMixedJsonAndSelectedKeys));
        Equal(
            "{\"gameData.Score\":\"1e3\",\"gameData.Enabled\":true,\"gameData.Nested\":{\"Value\":null},\"gameData.Items\":[1,\"two\",false]}",
            JsonNode.Parse(response.RootElement.GetProperty("payload").GetProperty("value").GetRawText())!.ToJsonString(),
            nameof(BridgeStateHandlerPreservesMixedJsonAndSelectedKeys));
    }
}

static async Task BridgeStateHandlerReportsMissingData()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    Plugin.RegisterHandlers(server, () => null, () => 0);
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid uuid = Guid.NewGuid();
        using JsonDocument response = await SendLiteralBridgeRequestAndPump(
            server,
            peer,
            0,
            $"{{\"uuid\":\"{uuid}\",\"type\":\"StateReq\",\"payload\":{{\"keys\":[\"gameData.Score\"]}}}}",
            null);
        AssertRemoteError(response, uuid, nameof(BridgeStateHandlerReportsMissingData), "State data is unavailable.");
    }
}

static async Task BridgeStateHandlerReportsInvalidPath()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    Plugin.RegisterHandlers(server, () => new BridgeStateFixture(), () => 0);
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid uuid = Guid.NewGuid();
        using JsonDocument response = await SendLiteralBridgeRequestAndPump(
            server,
            peer,
            0,
            $"{{\"uuid\":\"{uuid}\",\"type\":\"StateReq\",\"payload\":{{\"keys\":[\"gameData.Missing\"]}}}}",
            null);
        AssertRemoteError(response, uuid, nameof(BridgeStateHandlerReportsInvalidPath), "State path is invalid.");
    }
}

static async Task BridgeUiHandlersReportCorrelatedErrorsOnPumpThread()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    int windowThread = 0;
    int captureThread = 0;
    int capturePumpThread = 0;
    int captureX = 0;
    int captureY = 0;
    nint captureWindow = 0;
    int invokeThread = 0;
    int invokePumpThread = 0;
    string? invokedPath = null;
    int transferThread = 0;
    int transferPumpThread = 0;
    string? transferredSource = null;
    string? transferredDestination = null;
    Plugin.RegisterHandlers(server, () => new BridgeStateFixture(), () =>
    {
        Volatile.Write(ref windowThread, Environment.CurrentManagedThreadId);
        return (nint)0x1234;
    },
    (window, x, y) =>
    {
        Volatile.Write(ref captureThread, Environment.CurrentManagedThreadId);
        captureWindow = window;
        captureX = x;
        captureY = y;
        return (true, "slot", "captured", "");
    },
    path =>
    {
        Volatile.Write(ref invokeThread, Environment.CurrentManagedThreadId);
        invokedPath = path;
        return (false, "invoke failed");
    },
    (source, destination) =>
    {
        Volatile.Write(ref transferThread, Environment.CurrentManagedThreadId);
        transferredSource = source;
        transferredDestination = destination;
        return (true, "");
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid captureUuid = Guid.NewGuid();
        using (JsonDocument capture = await SendLiteralBridgeRequestAndPump(
            server,
            peer,
            (nint)0x5678,
            $"{{\"uuid\":\"{captureUuid}\",\"type\":\"CaptureReq\",\"payload\":{{\"x\":123,\"y\":-45}}}}",
            thread =>
            {
                if (Volatile.Read(ref captureThread) == 0)
                    Volatile.Write(ref capturePumpThread, thread);
            }))
        {
            Equal(captureUuid, capture.RootElement.GetProperty("uuid").GetGuid(), nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
            Equal("CaptureRes", capture.RootElement.GetProperty("type").GetString(), nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
            Equal("slot", capture.RootElement.GetProperty("payload").GetProperty("type").GetString(), nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
            Equal("captured", capture.RootElement.GetProperty("payload").GetProperty("path").GetString(), nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
        }
        Equal(capturePumpThread, windowThread, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
        Equal(capturePumpThread, captureThread, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
        Equal((nint)0x1234, captureWindow, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
        Equal(123, captureX, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
        Equal(-45, captureY, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));

        Guid invokeUuid = Guid.NewGuid();
        using (JsonDocument invoke = await SendLiteralBridgeRequestAndPump(
            server,
            peer,
            0,
            $"{{\"uuid\":\"{invokeUuid}\",\"type\":\"InvokeReq\",\"payload\":{{\"path\":\"scene:1/Buy[0]\"}}}}",
            thread =>
            {
                if (Volatile.Read(ref invokeThread) == 0)
                    Volatile.Write(ref invokePumpThread, thread);
            }))
            AssertRemoteError(invoke, invokeUuid, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread), "invoke failed");
        Equal(invokePumpThread, invokeThread, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
        Equal("scene:1/Buy[0]", invokedPath, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));

        Guid transferUuid = Guid.NewGuid();
        using (JsonDocument transfer = await SendLiteralBridgeRequestAndPump(
            server,
            peer,
            0,
            $"{{\"uuid\":\"{transferUuid}\",\"type\":\"TransferReq\",\"payload\":{{\"source\":\"source\",\"destination\":\"destination\"}}}}",
            thread =>
            {
                if (Volatile.Read(ref transferThread) == 0)
                    Volatile.Write(ref transferPumpThread, thread);
            }))
        {
            Equal(transferUuid, transfer.RootElement.GetProperty("uuid").GetGuid(), nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
            Equal("TransferRes", transfer.RootElement.GetProperty("type").GetString(), nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
        }
        Equal(transferPumpThread, transferThread, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
        Equal("source", transferredSource, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
        Equal("destination", transferredDestination, nameof(BridgeUiHandlersReportCorrelatedErrorsOnPumpThread));
    }
}

static async Task BridgeHandlersStartQueuedRequestWhileEarlierResponseIsPending()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    int dataCalls = 0;
    int outboundSends = 0;
    TaskCompletionSource<bool> firstSendStarted = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> releaseFirstSend = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.OutboundSendGateForTest = () =>
    {
        if (Interlocked.Increment(ref outboundSends) == 1)
        {
            firstSendStarted.TrySetResult(true);
            return releaseFirstSend.Task;
        }
        return Task.CompletedTask;
    };
    Plugin.RegisterHandlers(server, () =>
    {
        Interlocked.Increment(ref dataCalls);
        return new BridgeStateFixture();
    }, () => 0);
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    try
    {
        using (client)
        using (peer)
        {
            Guid firstUuid = Guid.NewGuid();
            Guid secondUuid = Guid.NewGuid();
            await SendText(peer, $"{{\"uuid\":\"{firstUuid}\",\"type\":\"StateReq\",\"payload\":{{\"keys\":[\"gameData.Text\"]}}}}");
            DateTime firstDeadline = DateTime.UtcNow.AddSeconds(2);
            while (!firstSendStarted.Task.IsCompleted && DateTime.UtcNow < firstDeadline)
            {
                Plugin.PumpPackets(0);
                await Task.Delay(1);
            }
            await firstSendStarted.Task.WaitAsync(TimeSpan.FromSeconds(2));
            Equal(1, Volatile.Read(ref dataCalls), nameof(BridgeHandlersStartQueuedRequestWhileEarlierResponseIsPending));
            Equal(true, server.HandlerTaskCountForTest > 0, nameof(BridgeHandlersStartQueuedRequestWhileEarlierResponseIsPending));

            await SendText(peer, $"{{\"uuid\":\"{secondUuid}\",\"type\":\"StateReq\",\"payload\":{{\"keys\":[\"gameData.Score\"]}}}}");
            DateTime secondDeadline = DateTime.UtcNow.AddSeconds(2);
            while (Volatile.Read(ref dataCalls) < 2 && DateTime.UtcNow < secondDeadline)
            {
                Plugin.PumpPackets(0);
                await Task.Delay(1);
            }
            Equal(2, Volatile.Read(ref dataCalls), nameof(BridgeHandlersStartQueuedRequestWhileEarlierResponseIsPending));
            Equal(true, server.HandlerTaskCountForTest >= 2, nameof(BridgeHandlersStartQueuedRequestWhileEarlierResponseIsPending));

            releaseFirstSend.TrySetResult(true);
            using JsonDocument firstResponse = JsonDocument.Parse(await ReceiveText(peer));
            using JsonDocument secondResponse = JsonDocument.Parse(await ReceiveText(peer));
            Equal(true, new[] { firstResponse.RootElement.GetProperty("uuid").GetGuid(), secondResponse.RootElement.GetProperty("uuid").GetGuid() }.Contains(firstUuid), nameof(BridgeHandlersStartQueuedRequestWhileEarlierResponseIsPending));
            Equal(true, new[] { firstResponse.RootElement.GetProperty("uuid").GetGuid(), secondResponse.RootElement.GetProperty("uuid").GetGuid() }.Contains(secondUuid), nameof(BridgeHandlersStartQueuedRequestWhileEarlierResponseIsPending));
        }
    }
    finally
    {
        releaseFirstSend.TrySetResult(true);
    }
}

static async Task<JsonDocument> SendLiteralBridgeRequestAndPump(
    WsConnection server,
    WebSocket peer,
    nint window,
    string envelope,
    Action<int>? beforePump)
{
    await SendText(peer, envelope);
    Task<string> response = ReceiveText(peer);
    DateTime deadline = DateTime.UtcNow.AddSeconds(2);
    while (!response.IsCompleted && DateTime.UtcNow < deadline)
    {
        beforePump?.Invoke(Environment.CurrentManagedThreadId);
        Plugin.PumpPackets(window);
        await Task.Delay(1);
    }
    return JsonDocument.Parse(await response.WaitAsync(TimeSpan.FromSeconds(2)));
}

static void AssertRemoteError(JsonDocument response, Guid uuid, string testName, string? expectedMessage = null)
{
    Equal(uuid, response.RootElement.GetProperty("uuid").GetGuid(), testName);
    Equal("RemoteError", response.RootElement.GetProperty("type").GetString(), testName);
    string? message = response.RootElement.GetProperty("payload").GetProperty("message").GetString();
    Equal(true, message is not null, testName);
    if (expectedMessage is not null)
        Equal(expectedMessage, message, testName);
}

static async Task WsDisconnectedCallsFailImmediately()
{
    using WsConnection connection = WsConnection.DisconnectedForTest();
    await ThrowsAsync<WsNotConnectedException>(() => connection.Request(new TestReq("request")), nameof(WsDisconnectedCallsFailImmediately));
    await ThrowsAsync<WsNotConnectedException>(() => connection.Send(new TestReq("send")), nameof(WsDisconnectedCallsFailImmediately));
}

static async Task WsRequestCorrelatesResponseByUuidAndType()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task<TestRes> request = server.Request(new TestReq("request"));
        using JsonDocument requestDocument = JsonDocument.Parse(await ReceiveText(peer));
        JsonElement requestEnvelope = requestDocument.RootElement;
        Equal("TestReq", requestEnvelope.GetProperty("type").GetString(), nameof(WsRequestCorrelatesResponseByUuidAndType));
        Guid uuid = requestEnvelope.GetProperty("uuid").GetGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestRes("response")));
        TestRes response = await request.WaitAsync(TimeSpan.FromSeconds(2));
        Equal("response", response.Value, nameof(WsRequestCorrelatesResponseByUuidAndType));
    }
}

static async Task WsConcurrentSendsShareOneWriter()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task[] sends = Enumerable.Range(0, 32)
            .Select(value => server.Send(new TestReq(value.ToString(CultureInfo.InvariantCulture))))
            .ToArray();
        HashSet<string> values = new();
        for (int index = 0; index < sends.Length; index++)
        {
            using JsonDocument document = JsonDocument.Parse(await ReceiveText(peer));
            values.Add(document.RootElement.GetProperty("payload").GetProperty("value").GetString()!);
        }
        await Task.WhenAll(sends);
        Equal(32, values.Count, nameof(WsConcurrentSendsShareOneWriter));
    }
}

static async Task WsHandlersStartWithoutWaitingForEarlierHandlers()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    TaskCompletionSource<bool> firstStarted = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> releaseFirst = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.Handler<TestReq>(async (context, packet) =>
    {
        if (packet.Value == "first")
        {
            firstStarted.TrySetResult(true);
            await releaseFirst.Task;
        }
        await context.Send(new TestRes(packet.Value));
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid firstUuid = Guid.NewGuid();
        Guid secondUuid = Guid.NewGuid();
        await SendText(peer, WsConnection.SerializeForTest(firstUuid, new TestReq("first")));
        await SendText(peer, WsConnection.SerializeForTest(secondUuid, new TestReq("second")));
        DateTime deadline = DateTime.UtcNow.AddSeconds(2);
        while (!firstStarted.Task.IsCompleted && DateTime.UtcNow < deadline)
        {
            server.Pump();
            await Task.Delay(5);
        }
        await firstStarted.Task.WaitAsync(TimeSpan.FromSeconds(2));
        server.Pump();
        using JsonDocument responseDocument = JsonDocument.Parse(await ReceiveText(peer));
        Equal(secondUuid, responseDocument.RootElement.GetProperty("uuid").GetGuid(), nameof(WsHandlersStartWithoutWaitingForEarlierHandlers));
        releaseFirst.TrySetResult(true);
    }
}

static async Task WsResponseTypeMismatchFailsRequest()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task<TestRes> request = server.Request(new TestReq("request"));
        using JsonDocument requestDocument = JsonDocument.Parse(await ReceiveText(peer));
        Guid uuid = requestDocument.RootElement.GetProperty("uuid").GetGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new OtherRes("wrong")));
        await ThrowsAsync<InvalidOperationException>(() => request, nameof(WsResponseTypeMismatchFailsRequest));
    }
}

static async Task WsRemoteErrorFailsRequest()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task<TestRes> request = server.Request(new TestReq("request"));
        using JsonDocument requestDocument = JsonDocument.Parse(await ReceiveText(peer));
        Guid uuid = requestDocument.RootElement.GetProperty("uuid").GetGuid();
        await SendText(peer, JsonSerializer.Serialize(new
        {
            uuid,
            type = "RemoteError",
            payload = new { message = "remote failed" }
        }));
        try
        {
            await request;
            throw new InvalidOperationException($"{nameof(WsRemoteErrorFailsRequest)}: request unexpectedly succeeded.");
        }
        catch (InvalidOperationException exception)
        {
            Equal("remote failed", exception.Message, nameof(WsRemoteErrorFailsRequest));
        }
    }
}

static async Task WsTimeoutSendsCancelAndDropsLateResponse()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromMilliseconds(20));
    TaskCompletionSource<bool> lateReceived = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.Handler<TestRes>((context, packet) =>
    {
        lateReceived.TrySetResult(true);
        return Task.CompletedTask;
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task<TestRes> request = server.Request(new TestReq("request"));
        using JsonDocument requestDocument = JsonDocument.Parse(await ReceiveText(peer));
        Guid uuid = requestDocument.RootElement.GetProperty("uuid").GetGuid();
        await ThrowsAsync<TimeoutException>(() => request, nameof(WsTimeoutSendsCancelAndDropsLateResponse));
        using JsonDocument cancelDocument = JsonDocument.Parse(await ReceiveText(peer));
        Equal(uuid, cancelDocument.RootElement.GetProperty("uuid").GetGuid(), nameof(WsTimeoutSendsCancelAndDropsLateResponse));
        Equal("Cancel", cancelDocument.RootElement.GetProperty("type").GetString(), nameof(WsTimeoutSendsCancelAndDropsLateResponse));
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestRes("late")));
        for (int attempt = 0; attempt < 10; attempt++)
        {
            server.Pump();
            await Task.Delay(5);
        }
        Equal(false, lateReceived.Task.IsCompleted, nameof(WsTimeoutSendsCancelAndDropsLateResponse));
    }
}

static async Task WsDisconnectFailsPendingAndAllowsNewClient()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    (TcpClient firstClient, WebSocket firstPeer) = await ConnectRawClient(server);
    Task<TestRes> request = server.Request(new TestReq("old"));
    await ReceiveText(firstPeer);
    firstPeer.Dispose();
    firstClient.Dispose();
    await ThrowsAsync<WsNotConnectedException>(() => request, nameof(WsDisconnectFailsPendingAndAllowsNewClient));

    (TcpClient secondClient, WebSocket secondPeer) = await ConnectRawClient(server);
    using (firstClient)
    using (firstPeer)
    using (secondClient)
    using (secondPeer)
    {
        Task send = server.Send(new TestReq("new"));
        using JsonDocument document = JsonDocument.Parse(await ReceiveText(secondPeer));
        Equal("new", document.RootElement.GetProperty("payload").GetProperty("value").GetString(), nameof(WsDisconnectFailsPendingAndAllowsNewClient));
        await send;
    }
}

static async Task WsRejectsSecondActiveClient()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    (TcpClient firstClient, WebSocket firstPeer) = await ConnectRawClient(server);
    using (firstClient)
    using (firstPeer)
    using (TcpClient secondClient = new())
    {
        await secondClient.ConnectAsync(IPAddress.Loopback, server.Port);
        using WebSocket secondPeer = WebSocket.CreateFromStream(
            secondClient.GetStream(),
            isServer: false,
            subProtocol: null,
            keepAliveInterval: Timeout.InfiniteTimeSpan);
        bool secondClosed = false;
        try
        {
            await ReceiveTextWithTimeout(secondPeer, TimeSpan.FromMilliseconds(250));
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception)
        {
            secondClosed = true;
        }
        Equal(true, secondClosed || secondPeer.State != WebSocketState.Open, nameof(WsRejectsSecondActiveClient));
        Task send = server.Send(new TestReq("first remains active"));
        using JsonDocument document = JsonDocument.Parse(await ReceiveText(firstPeer));
        Equal("first remains active", document.RootElement.GetProperty("payload").GetProperty("value").GetString(), nameof(WsRejectsSecondActiveClient));
        await send;
    }
}

static async Task WsMalformedPacketsDoNotStopReader()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    server.Handler<TestReq>((context, packet) => context.Send(new TestRes(packet.Value)));
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        await SendText(peer, "{not valid json");
        Guid uuid = Guid.NewGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestReq("valid")));
        using CancellationTokenSource pumping = new();
        Task pump = Task.Run(async () =>
        {
            try
            {
                while (true)
                {
                    server.Pump();
                    await Task.Delay(5, pumping.Token);
                }
            }
            catch (OperationCanceledException) when (pumping.IsCancellationRequested)
            {
            }
        });
        using JsonDocument document = JsonDocument.Parse(await ReceiveText(peer));
        pumping.Cancel();
        await pump;
        Equal(uuid, document.RootElement.GetProperty("uuid").GetGuid(), nameof(WsMalformedPacketsDoNotStopReader));
    }
}

static async Task WsMissingUuidPacketIsReportedAndReaderContinues()
{
    ConcurrentQueue<string> reports = new();
    int handled = 0;
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2), reports.Enqueue);
    server.Handler<TestReq>((context, packet) =>
    {
        Interlocked.Increment(ref handled);
        return context.Send(new TestRes(packet.Value));
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    using (CancellationTokenSource pumping = new())
    {
        Task pump = Task.Run(async () =>
        {
            try
            {
                while (true)
                {
                    server.Pump();
                    await Task.Delay(5, pumping.Token);
                }
            }
            catch (OperationCanceledException) when (pumping.IsCancellationRequested)
            {
            }
        });
        try
        {
            await SendText(peer, "{\"type\":\"TestReq\",\"payload\":{\"value\":\"missing-uuid\"}}");
            DateTime deadline = DateTime.UtcNow.AddSeconds(1);
            while (!reports.Any(report => report.Contains("uuid", StringComparison.OrdinalIgnoreCase)) && DateTime.UtcNow < deadline)
                await Task.Delay(5);
            Equal(true, reports.Any(report => report.Contains("uuid", StringComparison.OrdinalIgnoreCase)), nameof(WsMissingUuidPacketIsReportedAndReaderContinues));
            Guid uuid = Guid.NewGuid();
            await SendText(peer, WsConnection.SerializeForTest(uuid, new TestReq("valid-after-missing-uuid")));
            using JsonDocument response = JsonDocument.Parse(await ReceiveText(peer));
            Equal(uuid, response.RootElement.GetProperty("uuid").GetGuid(), nameof(WsMissingUuidPacketIsReportedAndReaderContinues));
            Equal("valid-after-missing-uuid", response.RootElement.GetProperty("payload").GetProperty("value").GetString(), nameof(WsMissingUuidPacketIsReportedAndReaderContinues));
            Equal(1, Volatile.Read(ref handled), nameof(WsMissingUuidPacketIsReportedAndReaderContinues));
        }
        finally
        {
            pumping.Cancel();
            await pump.WaitAsync(TimeSpan.FromSeconds(2));
        }
    }
}

static async Task WsCancelPreventsQueuedHandlerStart()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    TaskCompletionSource<bool> started = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.Handler<TestReq>((context, packet) =>
    {
        started.TrySetResult(true);
        return context.Send(new TestRes(packet.Value));
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid uuid = Guid.NewGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestReq("cancelled")));
        await SendText(peer, JsonSerializer.Serialize(new
        {
            uuid,
            type = "Cancel",
            payload = new { }
        }));
        await Task.Delay(25);
        server.Pump();
        await Task.Delay(25);
        Equal(false, started.Task.IsCompleted, nameof(WsCancelPreventsQueuedHandlerStart));
    }
}

static async Task WsCancelBlocksRunningHandlerResponse()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    TaskCompletionSource<bool> started = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> responseBlocked = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.Handler<TestReq>(async (context, packet) =>
    {
        started.TrySetResult(true);
        while (!context.CancellationToken.IsCancellationRequested)
            await Task.Delay(5);
        try
        {
            await context.Send(new TestRes(packet.Value));
        }
        catch (Exception)
        {
            responseBlocked.TrySetResult(true);
        }
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid uuid = Guid.NewGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestReq("running")));
        DateTime deadline = DateTime.UtcNow.AddSeconds(2);
        while (!started.Task.IsCompleted && DateTime.UtcNow < deadline)
        {
            server.Pump();
            await Task.Delay(5);
        }
        await started.Task.WaitAsync(TimeSpan.FromSeconds(2));
        await SendText(peer, JsonSerializer.Serialize(new { uuid, type = "Cancel", payload = new { } }));
        await responseBlocked.Task.WaitAsync(TimeSpan.FromSeconds(2));
        Equal(true, responseBlocked.Task.IsCompletedSuccessfully, nameof(WsCancelBlocksRunningHandlerResponse));
    }
}

static async Task WsDuplicateActiveUuidKeepsOriginalOwner()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    TaskCompletionSource<bool> firstStarted = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> releaseFirst = new(TaskCreationOptions.RunContinuationsAsynchronously);
    int handlerCount = 0;
    server.Handler<TestReq>(async (context, packet) =>
    {
        Interlocked.Increment(ref handlerCount);
        if (packet.Value == "first")
        {
            firstStarted.TrySetResult(true);
            await releaseFirst.Task;
        }
        await context.Send(new TestRes(packet.Value));
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid uuid = Guid.NewGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestReq("first")));
        DateTime deadline = DateTime.UtcNow.AddSeconds(2);
        while (!firstStarted.Task.IsCompleted && DateTime.UtcNow < deadline)
        {
            server.Pump();
            await Task.Delay(5);
        }
        await firstStarted.Task.WaitAsync(TimeSpan.FromSeconds(2));
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestReq("duplicate")));
        for (int attempt = 0; attempt < 10; attempt++)
        {
            server.Pump();
            await Task.Delay(5);
        }
        releaseFirst.TrySetResult(true);
        using JsonDocument response = JsonDocument.Parse(await ReceiveText(peer));
        Equal("first", response.RootElement.GetProperty("payload").GetProperty("value").GetString(), nameof(WsDuplicateActiveUuidKeepsOriginalOwner));
        Equal(1, Volatile.Read(ref handlerCount), nameof(WsDuplicateActiveUuidKeepsOriginalOwner));
    }
}

static async Task WsHandlerFailureSendsRemoteErrorWhenQueueBusy()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    server.Handler<TestReq>((context, packet) => throw new InvalidOperationException("handler failed"));
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task[] sends = Enumerable.Range(0, 512)
            .Select(value => server.Send(new TestReq($"out-{value}")))
            .ToArray();
        Guid uuid = Guid.NewGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestReq("fail")));
        using CancellationTokenSource pumping = new();
        Task pump = Task.Run(async () =>
        {
            try
            {
                while (true)
                {
                    server.Pump();
                    await Task.Delay(5, pumping.Token);
                }
            }
            catch (OperationCanceledException) when (pumping.IsCancellationRequested)
            {
            }
        });
        bool foundRemoteError = false;
        for (int index = 0; index < sends.Length + 1; index++)
        {
            using JsonDocument document = JsonDocument.Parse(await ReceiveText(peer));
            if (document.RootElement.GetProperty("uuid").GetGuid() == uuid)
            {
                Equal("RemoteError", document.RootElement.GetProperty("type").GetString(), nameof(WsHandlerFailureSendsRemoteErrorWhenQueueBusy));
                foundRemoteError = true;
                break;
            }
        }
        pumping.Cancel();
        await pump;
        await Task.WhenAll(sends).WaitAsync(TimeSpan.FromSeconds(2));
        Equal(true, foundRemoteError, nameof(WsHandlerFailureSendsRemoteErrorWhenQueueBusy));
    }
}

static async Task WsDisconnectPressureFailsQueuedSend()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        string value = new('x', 512);
        Task[] sends = Enumerable.Range(0, 512)
            .Select(index => server.Send(new TestReq($"{index}:{value}")))
            .ToArray();
        DateTime deadline = DateTime.UtcNow.AddSeconds(2);
        while (server.QueuedOutboundForTest == 0 && DateTime.UtcNow < deadline)
            await Task.Delay(1);
        Equal(true, server.QueuedOutboundForTest > 0, nameof(WsDisconnectPressureFailsQueuedSend));
        peer.Dispose();
        client.Dispose();
        int failures = 0;
        foreach (Task send in sends)
        {
            try
            {
                await send.WaitAsync(TimeSpan.FromSeconds(2));
            }
            catch (WsNotConnectedException)
            {
                failures++;
            }
        }
        Equal(true, failures > 0, nameof(WsDisconnectPressureFailsQueuedSend));
    }
}

static async Task WsDisposeDoesNotWaitForArbitraryHandler()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    TaskCompletionSource<bool> started = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> release = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.Handler<TestReq>(async (context, packet) =>
    {
        started.TrySetResult(true);
        await release.Task;
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        await SendText(peer, WsConnection.SerializeForTest(Guid.NewGuid(), new TestReq("non-cooperative")));
        DateTime startDeadline = DateTime.UtcNow.AddSeconds(2);
        while (!started.Task.IsCompleted && DateTime.UtcNow < startDeadline)
        {
            server.Pump();
            await Task.Delay(5);
        }
        await started.Task.WaitAsync(TimeSpan.FromSeconds(2));
        DateTime disposedAt = DateTime.UtcNow;
        server.Dispose();
        Equal(true, DateTime.UtcNow - disposedAt < TimeSpan.FromMilliseconds(500), nameof(WsDisposeDoesNotWaitForArbitraryHandler));
        release.TrySetResult(true);
        DateTime cleanupDeadline = DateTime.UtcNow.AddSeconds(2);
        while (server.HandlerTaskCountForTest != 0 && DateTime.UtcNow < cleanupDeadline)
            await Task.Delay(5);
        Equal(0, server.HandlerTaskCountForTest, nameof(WsDisposeDoesNotWaitForArbitraryHandler));
    }
}

static async Task WsLifecycleFaultsAreObserved()
{
    ConcurrentQueue<string> reports = new();
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2), reports.Enqueue);
    server.ObserveLifecycleForTest(Task.FromException(new InvalidOperationException("lifecycle failed")));
    DateTime deadline = DateTime.UtcNow.AddSeconds(1);
    while (!reports.Any(report => report.Contains("lifecycle failed", StringComparison.Ordinal)) && DateTime.UtcNow < deadline)
        await Task.Delay(5);
    Equal(true, reports.Any(report => report.Contains("lifecycle failed", StringComparison.Ordinal)), nameof(WsLifecycleFaultsAreObserved));
}

static async Task WsResponseSerializationFailureDoesNotConsumeResponse()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    server.Handler<TestReq>(async (context, packet) =>
    {
        try
        {
            await context.Send(new ThrowingResponse());
        }
        catch (InvalidOperationException)
        {
        }
        await context.Send(new TestRes("valid after serialization failure"));
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid uuid = Guid.NewGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestReq("request")));
        using CancellationTokenSource pumping = new();
        Task pump = Task.Run(async () =>
        {
            try
            {
                while (true)
                {
                    server.Pump();
                    await Task.Delay(5, pumping.Token);
                }
            }
            catch (OperationCanceledException) when (pumping.IsCancellationRequested)
            {
            }
        });
        using JsonDocument response = JsonDocument.Parse(await ReceiveText(peer));
        pumping.Cancel();
        await pump;
        Equal(uuid, response.RootElement.GetProperty("uuid").GetGuid(), nameof(WsResponseSerializationFailureDoesNotConsumeResponse));
        Equal("valid after serialization failure", response.RootElement.GetProperty("payload").GetProperty("value").GetString(), nameof(WsResponseSerializationFailureDoesNotConsumeResponse));
    }
}

static async Task WsHandlerErrorsAreObservedAndTasksCleaned()
{
    ConcurrentQueue<string> reports = new();
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2), reports.Enqueue);
    server.Handler<TestReq>(async (context, packet) =>
    {
        await context.Send(new TestRes(packet.Value));
        throw new InvalidOperationException("after response");
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid uuid = Guid.NewGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestReq("observed")));
        using CancellationTokenSource pumping = new();
        Task pump = Task.Run(async () =>
        {
            try
            {
                while (true)
                {
                    server.Pump();
                    await Task.Delay(5, pumping.Token);
                }
            }
            catch (OperationCanceledException) when (pumping.IsCancellationRequested)
            {
            }
        });
        using JsonDocument response = JsonDocument.Parse(await ReceiveText(peer));
        pumping.Cancel();
        await pump;
        DateTime deadline = DateTime.UtcNow.AddSeconds(2);
        while (reports.IsEmpty && DateTime.UtcNow < deadline)
            await Task.Delay(5);
        Equal(true, reports.Any(report => report.Contains("after response", StringComparison.Ordinal)), nameof(WsHandlerErrorsAreObservedAndTasksCleaned));
        Equal(0, server.HandlerTaskCountForTest, nameof(WsHandlerErrorsAreObservedAndTasksCleaned));
        Equal(uuid, response.RootElement.GetProperty("uuid").GetGuid(), nameof(WsHandlerErrorsAreObservedAndTasksCleaned));
    }
}

static async Task WsUnknownPacketGetsRemoteError()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Guid uuid = Guid.NewGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new UnknownReq("unknown")));
        using JsonDocument response = JsonDocument.Parse(await ReceiveText(peer));
        Equal(uuid, response.RootElement.GetProperty("uuid").GetGuid(), nameof(WsUnknownPacketGetsRemoteError));
        Equal("RemoteError", response.RootElement.GetProperty("type").GetString(), nameof(WsUnknownPacketGetsRemoteError));
        Equal(true, response.RootElement.GetProperty("payload").GetProperty("message").GetString()!.Contains("UnknownReq", StringComparison.Ordinal), nameof(WsUnknownPacketGetsRemoteError));
    }
}

static async Task WsPumpRunsSynchronousHandlerPrefixOnCallerThread()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    TaskCompletionSource<bool> release = new(TaskCreationOptions.RunContinuationsAsynchronously);
    int observedThread = 0;
    server.Handler<TestReq>(async (context, packet) =>
    {
        Volatile.Write(ref observedThread, Environment.CurrentManagedThreadId);
        await release.Task;
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        await SendText(peer, WsConnection.SerializeForTest(Guid.NewGuid(), new TestReq("thread")));
        TaskCompletionSource<int> pumpThread = new(TaskCreationOptions.RunContinuationsAsynchronously);
        Task pump = Task.Run(() =>
        {
            DateTime deadline = DateTime.UtcNow.AddSeconds(2);
            while (Volatile.Read(ref observedThread) == 0 && DateTime.UtcNow < deadline)
            {
                int callerThread = Environment.CurrentManagedThreadId;
                server.Pump();
                if (Volatile.Read(ref observedThread) != 0)
                {
                    pumpThread.TrySetResult(callerThread);
                    return;
                }
                Thread.Yield();
            }
            pumpThread.TrySetException(new TimeoutException("Pump did not start the handler."));
        });
        int callerThread = await pumpThread.Task.WaitAsync(TimeSpan.FromSeconds(2));
        await pump;
        Equal(callerThread, Volatile.Read(ref observedThread), nameof(WsPumpRunsSynchronousHandlerPrefixOnCallerThread));
        release.TrySetResult(true);
    }
}

static async Task WsOldGenerationBufferedFrameIsIgnoredAfterReconnect()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    ConcurrentQueue<string> handled = new();
    server.Handler<TestReq>(async (context, packet) =>
    {
        handled.Enqueue(packet.Value);
        await context.Send(new TestRes(packet.Value));
    });
    (TcpClient firstClient, WebSocket firstPeer) = await ConnectRawClient(server);
    await SendText(firstPeer, WsConnection.SerializeForTest(Guid.NewGuid(), new TestReq("old-generation")));
    await Task.Delay(25);
    firstPeer.Dispose();
    firstClient.Dispose();
    DateTime disconnectDeadline = DateTime.UtcNow.AddSeconds(2);
    while (server.ConnectedForTest && DateTime.UtcNow < disconnectDeadline)
        await Task.Delay(5);

    (TcpClient secondClient, WebSocket secondPeer) = await ConnectRawClient(server);
    using (firstClient)
    using (firstPeer)
    using (secondClient)
    using (secondPeer)
    {
        await SendText(secondPeer, WsConnection.SerializeForTest(Guid.NewGuid(), new TestReq("new-generation")));
        using CancellationTokenSource pumping = new();
        Task pump = Task.Run(async () =>
        {
            try
            {
                while (true)
                {
                    server.Pump();
                    await Task.Delay(5, pumping.Token);
                }
            }
            catch (OperationCanceledException) when (pumping.IsCancellationRequested)
            {
            }
        });
        using JsonDocument response = JsonDocument.Parse(await ReceiveText(secondPeer));
        pumping.Cancel();
        await pump;
        Equal("new-generation", response.RootElement.GetProperty("payload").GetProperty("value").GetString(), nameof(WsOldGenerationBufferedFrameIsIgnoredAfterReconnect));
        Equal(1, handled.Count, nameof(WsOldGenerationBufferedFrameIsIgnoredAfterReconnect));
        Equal("new-generation", handled.Single(), nameof(WsOldGenerationBufferedFrameIsIgnoredAfterReconnect));
    }
}

static async Task WsCloseVsPumpRegistrationInterleaving()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    TaskCompletionSource<bool> registered = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> stopPaused = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> releaseStop = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> started = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> releaseHandler = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.LifecycleSynchronizationForTest = stage =>
    {
        if (stage == "inbound-registered")
            registered.TrySetResult(true);
        if (stage == "stop-before-cancel")
        {
            stopPaused.TrySetResult(true);
            releaseStop.Task.GetAwaiter().GetResult();
        }
    };
    server.Handler<TestReq>(async (context, packet) =>
    {
        started.TrySetResult(true);
        await releaseHandler.Task;
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        try
        {
            await SendText(peer, WsConnection.SerializeForTest(Guid.NewGuid(), new TestReq("close-race")));
            await registered.Task.WaitAsync(TimeSpan.FromSeconds(2));
            peer.Dispose();
            client.Dispose();
            await stopPaused.Task.WaitAsync(TimeSpan.FromSeconds(2));
            server.Pump();
            Equal(false, started.Task.IsCompleted, nameof(WsCloseVsPumpRegistrationInterleaving));
        }
        finally
        {
            releaseStop.TrySetResult(true);
            releaseHandler.TrySetResult(true);
        }
    }
}

static async Task WsBlockingSynchronousPrefixDoesNotBlockStop()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    TaskCompletionSource<bool> registered = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> prefixStarted = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> releasePrefix = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> stopReached = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.LifecycleSynchronizationForTest = stage =>
    {
        if (stage == "inbound-registered")
            registered.TrySetResult(true);
        if (stage == "stop-before-cancel")
            stopReached.TrySetResult(true);
    };
    server.Handler<TestReq>((context, packet) =>
    {
        prefixStarted.TrySetResult(true);
        releasePrefix.Task.GetAwaiter().GetResult();
        return Task.CompletedTask;
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task pump = Task.CompletedTask;
        try
        {
            await SendText(peer, WsConnection.SerializeForTest(Guid.NewGuid(), new TestReq("blocking-prefix")));
            await registered.Task.WaitAsync(TimeSpan.FromSeconds(2));
            pump = Task.Run(server.Pump);
            await prefixStarted.Task.WaitAsync(TimeSpan.FromSeconds(2));
            peer.Dispose();
            client.Dispose();
            await stopReached.Task.WaitAsync(TimeSpan.FromMilliseconds(500));
        }
        finally
        {
            releasePrefix.TrySetResult(true);
            await pump.WaitAsync(TimeSpan.FromSeconds(2));
        }
    }
}

static async Task WsHandlerCanInitiateNestedRequestWithoutAwait()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    TaskCompletionSource<Task<TestRes>> nestedStarted = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.Handler<TestReq>((context, packet) =>
    {
        if (packet.Value == "outer")
            nestedStarted.TrySetResult(server.Request(new TestReq("nested")));
        return context.Send(new TestRes(packet.Value));
    });
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    using (CancellationTokenSource pumping = new())
    {
        Task pump = Task.Run(async () =>
        {
            try
            {
                while (true)
                {
                    server.Pump();
                    await Task.Delay(5, pumping.Token);
                }
            }
            catch (OperationCanceledException) when (pumping.IsCancellationRequested)
            {
            }
        });
        try
        {
            Guid outerUuid = Guid.NewGuid();
            await SendText(peer, WsConnection.SerializeForTest(outerUuid, new TestReq("outer")));
            using JsonDocument nested = JsonDocument.Parse(await ReceiveText(peer));
            Equal("TestReq", nested.RootElement.GetProperty("type").GetString(), nameof(WsHandlerCanInitiateNestedRequestWithoutAwait));
            Equal("nested", nested.RootElement.GetProperty("payload").GetProperty("value").GetString(), nameof(WsHandlerCanInitiateNestedRequestWithoutAwait));
            Guid nestedUuid = nested.RootElement.GetProperty("uuid").GetGuid();
            await SendText(peer, WsConnection.SerializeForTest(nestedUuid, new TestRes("nested-response")));
            using JsonDocument outer = JsonDocument.Parse(await ReceiveText(peer));
            Equal(outerUuid, outer.RootElement.GetProperty("uuid").GetGuid(), nameof(WsHandlerCanInitiateNestedRequestWithoutAwait));
            Equal("TestRes", outer.RootElement.GetProperty("type").GetString(), nameof(WsHandlerCanInitiateNestedRequestWithoutAwait));
            Equal("outer", outer.RootElement.GetProperty("payload").GetProperty("value").GetString(), nameof(WsHandlerCanInitiateNestedRequestWithoutAwait));
            Task<TestRes> nestedRequest = await nestedStarted.Task.WaitAsync(TimeSpan.FromSeconds(2));
            TestRes nestedResponse = await nestedRequest.WaitAsync(TimeSpan.FromSeconds(2));
            Equal("nested-response", nestedResponse.Value, nameof(WsHandlerCanInitiateNestedRequestWithoutAwait));
        }
        finally
        {
            pumping.Cancel();
            await pump.WaitAsync(TimeSpan.FromSeconds(2));
        }
    }
}

static async Task WsMalformedCorrelatedRemoteErrorFailsPromptly()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromSeconds(2));
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task<TestRes> request = server.Request(new TestReq("malformed-error"));
        using JsonDocument outbound = JsonDocument.Parse(await ReceiveText(peer));
        Guid uuid = outbound.RootElement.GetProperty("uuid").GetGuid();
        await SendText(peer, $"{{\"uuid\":\"{uuid}\",\"type\":\"RemoteError\",\"payload\":{{\"message\":7}}}}");
        DateTime deadline = DateTime.UtcNow.AddSeconds(1);
        while (!request.IsCompleted && DateTime.UtcNow < deadline)
            await Task.Delay(5);
        Equal(true, request.IsCompleted, nameof(WsMalformedCorrelatedRemoteErrorFailsPromptly));
        await ThrowsAsync<InvalidOperationException>(() => request, nameof(WsMalformedCorrelatedRemoteErrorFailsPromptly));
    }
}

static async Task WsTimeoutRemovalRaceAwaitsWinningCompletion()
{
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromMilliseconds(20));
    TaskCompletionSource<bool> removed = new(TaskCreationOptions.RunContinuationsAsynchronously);
    TaskCompletionSource<bool> release = new(TaskCreationOptions.RunContinuationsAsynchronously);
    server.PendingRemovedForTest = () =>
    {
        removed.TrySetResult(true);
        release.Task.GetAwaiter().GetResult();
    };
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task<TestRes> request = server.Request(new TestReq("timeout-race"));
        using JsonDocument outbound = JsonDocument.Parse(await ReceiveText(peer));
        Guid uuid = outbound.RootElement.GetProperty("uuid").GetGuid();
        await SendText(peer, WsConnection.SerializeForTest(uuid, new TestRes("winner")));
        await removed.Task.WaitAsync(TimeSpan.FromSeconds(2));
        await Task.Delay(50);
        Equal(false, request.IsCompleted, nameof(WsTimeoutRemovalRaceAwaitsWinningCompletion));
        release.TrySetResult(true);
        TestRes response = await request.WaitAsync(TimeSpan.FromSeconds(2));
        Equal("winner", response.Value, nameof(WsTimeoutRemovalRaceAwaitsWinningCompletion));
    }
}

static async Task WsLateRemoteErrorIsReportedBeforeTombstoneDiscard()
{
    ConcurrentQueue<string> reports = new();
    using WsConnection server = WsConnection.CreateForTest(0, TimeSpan.FromMilliseconds(20), reports.Enqueue);
    (TcpClient client, WebSocket peer) = await ConnectRawClient(server);
    using (client)
    using (peer)
    {
        Task<TestRes> request = server.Request(new TestReq("late-error"));
        using JsonDocument outbound = JsonDocument.Parse(await ReceiveText(peer));
        Guid uuid = outbound.RootElement.GetProperty("uuid").GetGuid();
        await ThrowsAsync<TimeoutException>(() => request, nameof(WsLateRemoteErrorIsReportedBeforeTombstoneDiscard));
        await SendText(peer, $"{{\"uuid\":\"{uuid}\",\"type\":\"RemoteError\",\"payload\":{{\"message\":\"late remote error\"}}}}");
        DateTime deadline = DateTime.UtcNow.AddSeconds(1);
        while (!reports.Any(report => report.Contains("late remote error", StringComparison.Ordinal)) && DateTime.UtcNow < deadline)
            await Task.Delay(5);
        Equal(true, reports.Any(report => report.Contains("late remote error", StringComparison.Ordinal)), nameof(WsLateRemoteErrorIsReportedBeforeTombstoneDiscard));
    }
}

static void StatePayloadFormatsBigDoubleValues()
{
    byte[] payload = StatePayload.Encode(new[] { ("score", 2.5, 42d), ("timeInfinity", 3.5, 7d), ("timeEternity", 4.5, 8d) });
    Equal("{\"score\":\"2.5e42\",\"timeInfinity\":\"3.5e7\",\"timeEternity\":\"4.5e8\"}", Encoding.UTF8.GetString(payload), nameof(StatePayloadFormatsBigDoubleValues));
}

static void StatePayloadSerializesCompleteGraph()
{
    var data = new SerializerFixture
    {
        Boolean = true,
        Decimal = 12.5m,
        Dictionary = new Dictionary<int, string> { [2] = "two" },
        Double = double.NegativeInfinity,
        Enum = SerializerFixtureKind.Second,
        Float = float.NaN,
        LargeInteger = 9007199254740992,
        List = new List<int> { 3, 4 },
        Nested = new SerializerNestedFixture { Value = "nested" },
        Null = null,
        SafeInteger = 9007199254740991,
        Text = "value",
        Values = new[] { 1, 2 }
    };

    StatePayloadStatus status = StatePayload.Encode(data, new[] { "gameData" }, out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadSerializesCompleteGraph));
    Equal("{\"gameData\":{\"Boolean\":true,\"Decimal\":12.5,\"Dictionary\":{\"2\":\"two\"},\"Double\":\"-Infinity\",\"Enum\":\"Second\",\"Float\":\"NaN\",\"LargeInteger\":\"9007199254740992\",\"List\":[3,4],\"Nested\":{\"Value\":\"nested\"},\"Null\":null,\"SafeInteger\":9007199254740991,\"Text\":\"value\",\"Values\":[1,2]}}", Encoding.UTF8.GetString(payload), nameof(StatePayloadSerializesCompleteGraph));
}

static void StatePayloadResolvesSelectedPaths()
{
    var data = new PathFixture
    {
        eternity = new PathEternityFixture { dilationTree = new SerializerNestedFixture { Value = "tree" }, dtpBought = 5, dtpMax = 12 },
        infinity = new PathInfinityFixture { IP = 11 },
        kinds = new Dictionary<SerializerFixtureKind, string> { [SerializerFixtureKind.Second] = "enum" },
        names = new Dictionary<string, string> { ["primary"] = "name" },
        numbers = new Dictionary<int, string> { [2] = "number" },
        rows = new List<SerializerNestedFixture> { new() { Value = "zero" }, new() { Value = "one" } }
    };

    // "IP", "DT", "DTP", "DTPMax" go through ResolveAlias (which expands to a
    // gameData.-prefixed path); the rest are raw paths that must name the
    // gameData root explicitly themselves since there is no implicit root.
    StatePayloadStatus status = StatePayload.Encode(data, new[] { "IP", "DT", "DTP", "DTPMax", "gameData.rows.1.Value", "gameData.numbers.2", "gameData.names.primary", "gameData.kinds.Second", "IP" }, out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadResolvesSelectedPaths));
    Equal("{\"IP\":11,\"DT\":{\"Value\":\"tree\"},\"DTP\":5,\"DTPMax\":12,\"gameData.rows.1.Value\":\"one\",\"gameData.numbers.2\":\"number\",\"gameData.names.primary\":\"name\",\"gameData.kinds.Second\":\"enum\"}", Encoding.UTF8.GetString(payload), nameof(StatePayloadResolvesSelectedPaths));
}

static void StatePayloadDistinguishesInvalidPathsAndGetterFailures()
{
    var data = new PathFixture
    {
        rows = new List<SerializerNestedFixture> { new() { Value = "zero" } },
        numbers = new Dictionary<int, string> { [2] = "number" }
    };

    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.unknown" }, out byte[] unknownPayload), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(0, unknownPayload.Length, nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.rows.1.Value" }, out _), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.numbers.3" }, out _), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "" }, out _), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));

    // A throwing getter serializes as null for that one property rather than
    // failing the whole request: some real IL2CPP getters (GameData.DateOFFull,
    // a Nullable<DateTime> that was never set) always throw, and one such
    // property must not take down every other value in the same graph.
    Equal(StatePayloadStatus.Success, StatePayload.Encode(new GetterFailureFixture(), new[] { "gameData" }, out byte[] canonicalPayload), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal("{\"gameData\":{\"Value\":null}}", Encoding.UTF8.GetString(canonicalPayload), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(StatePayloadStatus.Success, StatePayload.Encode(new GetterFailureFixture(), new[] { "gameData.Value" }, out byte[] selectedPayload), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal("{\"gameData.Value\":null}", Encoding.UTF8.GetString(selectedPayload), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
}

static void StatePayloadPreservesCollectionIndexesAcrossCycles()
{
    var data = new CycleFixture();
    var shared = new CycleNodeFixture { Parent = data, Value = "shared" };
    var item = new CycleNodeFixture { Parent = data, Value = "item" };
    data.First = shared;
    data.Items = new List<CycleNodeFixture> { shared, item, item };
    data.Second = shared;

    StatePayloadStatus status = StatePayload.Encode(data, new[] { "gameData" }, out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadPreservesCollectionIndexesAcrossCycles));
    Equal("{\"gameData\":{\"First\":{\"Value\":\"shared\"},\"Items\":[null,{\"Value\":\"item\"},null]}}", Encoding.UTF8.GetString(payload), nameof(StatePayloadPreservesCollectionIndexesAcrossCycles));
}

static void StatePayloadSerializesGameSpecificScalars()
{
    static object CreateBigDouble()
    {
        TypeBuilder type = AssemblyBuilder.DefineDynamicAssembly(new AssemblyName("Assembly-CSharp"), AssemblyBuilderAccess.Run).DefineDynamicModule("main").DefineType("BigDouble", TypeAttributes.Public | TypeAttributes.Class);
        MethodBuilder mantissa = type.DefineMethod("get_Mantissa", MethodAttributes.Public | MethodAttributes.SpecialName | MethodAttributes.HideBySig, typeof(double), Type.EmptyTypes);
        mantissa.GetILGenerator().Emit(OpCodes.Ldc_R8, 2.5);
        mantissa.GetILGenerator().Emit(OpCodes.Ret);
        type.DefineProperty("Mantissa", PropertyAttributes.None, typeof(double), null).SetGetMethod(mantissa);
        MethodBuilder exponent = type.DefineMethod("get_Exponent", MethodAttributes.Public | MethodAttributes.SpecialName | MethodAttributes.HideBySig, typeof(double), Type.EmptyTypes);
        exponent.GetILGenerator().Emit(OpCodes.Ldc_R8, 42d);
        exponent.GetILGenerator().Emit(OpCodes.Ret);
        type.DefineProperty("Exponent", PropertyAttributes.None, typeof(double), null).SetGetMethod(exponent);
        return Activator.CreateInstance(type.CreateType())!;
    }

    static object CreateObscuredInt()
    {
        TypeBuilder type = AssemblyBuilder.DefineDynamicAssembly(new AssemblyName("ACTk.Runtime"), AssemblyBuilderAccess.Run).DefineDynamicModule("main").DefineType("CodeStage.AntiCheat.ObscuredTypes.ObscuredInt", TypeAttributes.Public | TypeAttributes.Class);
        MethodBuilder getDecrypted = type.DefineMethod("GetDecrypted", MethodAttributes.Public, typeof(int), Type.EmptyTypes);
        getDecrypted.GetILGenerator().Emit(OpCodes.Ldc_I4_S, 17);
        getDecrypted.GetILGenerator().Emit(OpCodes.Ret);
        return Activator.CreateInstance(type.CreateType())!;
    }

    var data = new GameScalarFixture
    {
        Big = CreateBigDouble(),
        Date = new DateTime(2026, 9, 3, 1, 2, 3, DateTimeKind.Utc),
        Obscured = CreateObscuredInt()
    };

    StatePayloadStatus status = StatePayload.Encode(data, new[] { "gameData" }, out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadSerializesGameSpecificScalars));
    Equal("{\"gameData\":{\"Big\":\"2.5e42\",\"Date\":\"2026-09-03T01:02:03.0000000Z\",\"Obscured\":17}}", Encoding.UTF8.GetString(payload), nameof(StatePayloadSerializesGameSpecificScalars));
}

static void StatePayloadKeepsSelectedKeysForSharedValues()
{
    var shared = new SerializerNestedFixture { Value = "shared" };
    var data = new SharedPathFixture { First = shared, Second = shared };

    StatePayloadStatus status = StatePayload.Encode(data, new[] { "gameData.First", "gameData.Second" }, out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadKeepsSelectedKeysForSharedValues));
    Equal("{\"gameData.First\":{\"Value\":\"shared\"},\"gameData.Second\":{\"Value\":\"shared\"}}", Encoding.UTF8.GetString(payload), nameof(StatePayloadKeepsSelectedKeysForSharedValues));
}

static void StatePayloadSerializesIl2CppDatesAndNullables()
{
    static (object Date, object HasValue, object NoValue) CreateValues()
    {
        ModuleBuilder module = AssemblyBuilder.DefineDynamicAssembly(new AssemblyName("Il2Cppmscorlib"), AssemblyBuilderAccess.Run).DefineDynamicModule("main");
        TypeBuilder dateType = module.DefineType("Il2CppSystem.DateTime", TypeAttributes.Public | TypeAttributes.Class);
        MethodBuilder ticks = dateType.DefineMethod("get_Ticks", MethodAttributes.Public | MethodAttributes.SpecialName | MethodAttributes.HideBySig, typeof(long), Type.EmptyTypes);
        ticks.GetILGenerator().Emit(OpCodes.Ldc_I8, 639239941230000000L);
        ticks.GetILGenerator().Emit(OpCodes.Ret);
        dateType.DefineProperty("Ticks", PropertyAttributes.None, typeof(long), null).SetGetMethod(ticks);
        MethodBuilder kind = dateType.DefineMethod("get_Kind", MethodAttributes.Public | MethodAttributes.SpecialName | MethodAttributes.HideBySig, typeof(int), Type.EmptyTypes);
        kind.GetILGenerator().Emit(OpCodes.Ldc_I4_1);
        kind.GetILGenerator().Emit(OpCodes.Ret);
        dateType.DefineProperty("Kind", PropertyAttributes.None, typeof(int), null).SetGetMethod(kind);

        TypeBuilder nullableType = module.DefineType("Il2CppSystem.Nullable`1", TypeAttributes.Public | TypeAttributes.Class);
        FieldBuilder hasValueField = nullableType.DefineField("hasValue", typeof(bool), FieldAttributes.Private);
        FieldBuilder valueField = nullableType.DefineField("value", typeof(int), FieldAttributes.Private);
        ConstructorBuilder constructor = nullableType.DefineConstructor(MethodAttributes.Public, CallingConventions.Standard, new[] { typeof(bool), typeof(int) });
        ILGenerator constructorIl = constructor.GetILGenerator();
        constructorIl.Emit(OpCodes.Ldarg_0);
        constructorIl.Emit(OpCodes.Call, typeof(object).GetConstructor(Type.EmptyTypes)!);
        constructorIl.Emit(OpCodes.Ldarg_0);
        constructorIl.Emit(OpCodes.Ldarg_1);
        constructorIl.Emit(OpCodes.Stfld, hasValueField);
        constructorIl.Emit(OpCodes.Ldarg_0);
        constructorIl.Emit(OpCodes.Ldarg_2);
        constructorIl.Emit(OpCodes.Stfld, valueField);
        constructorIl.Emit(OpCodes.Ret);
        MethodBuilder hasValue = nullableType.DefineMethod("get_HasValue", MethodAttributes.Public | MethodAttributes.SpecialName | MethodAttributes.HideBySig, typeof(bool), Type.EmptyTypes);
        hasValue.GetILGenerator().Emit(OpCodes.Ldarg_0);
        hasValue.GetILGenerator().Emit(OpCodes.Ldfld, hasValueField);
        hasValue.GetILGenerator().Emit(OpCodes.Ret);
        nullableType.DefineProperty("HasValue", PropertyAttributes.None, typeof(bool), null).SetGetMethod(hasValue);
        MethodBuilder value = nullableType.DefineMethod("get_Value", MethodAttributes.Public | MethodAttributes.SpecialName | MethodAttributes.HideBySig, typeof(int), Type.EmptyTypes);
        value.GetILGenerator().Emit(OpCodes.Ldarg_0);
        value.GetILGenerator().Emit(OpCodes.Ldfld, valueField);
        value.GetILGenerator().Emit(OpCodes.Ret);
        nullableType.DefineProperty("Value", PropertyAttributes.None, typeof(int), null).SetGetMethod(value);

        Type date = dateType.CreateType();
        Type nullable = nullableType.CreateType();
        return (Activator.CreateInstance(date)!, Activator.CreateInstance(nullable, true, 17)!, Activator.CreateInstance(nullable, false, 0)!);
    }

    (object date, object hasValue, object noValue) = CreateValues();
    var data = new Il2CppScalarFixture { Date = date, HasValue = hasValue, NoValue = noValue };

    StatePayloadStatus status = StatePayload.Encode(data, new[] { "gameData" }, out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadSerializesIl2CppDatesAndNullables));
    Equal("{\"gameData\":{\"Date\":\"2026-09-03T01:02:03.0000000Z\",\"HasValue\":17,\"NoValue\":null}}", Encoding.UTF8.GetString(payload), nameof(StatePayloadSerializesIl2CppDatesAndNullables));
}

static void StatePayloadRejectsUnsupportedIl2CppObjectWrappers()
{
    object wrapper = Activator.CreateInstance(AssemblyBuilder.DefineDynamicAssembly(new AssemblyName("Il2Cppmscorlib"), AssemblyBuilderAccess.Run).DefineDynamicModule("main").DefineType("Il2CppSystem.Object", TypeAttributes.Public | TypeAttributes.Class).CreateType())!;
    var data = new Il2CppObjectFixture { Value = wrapper };

    Equal(StatePayloadStatus.SerializationFailure, StatePayload.Encode(data, new[] { "gameData" }, out byte[] canonicalPayload), nameof(StatePayloadRejectsUnsupportedIl2CppObjectWrappers));
    Equal(0, canonicalPayload.Length, nameof(StatePayloadRejectsUnsupportedIl2CppObjectWrappers));
    Equal(StatePayloadStatus.SerializationFailure, StatePayload.Encode(data, new[] { "gameData.Value" }, out byte[] selectedPayload), nameof(StatePayloadRejectsUnsupportedIl2CppObjectWrappers));
    Equal(0, selectedPayload.Length, nameof(StatePayloadRejectsUnsupportedIl2CppObjectWrappers));

    data = new Il2CppObjectFixture { Value = System.Runtime.CompilerServices.RuntimeHelpers.GetUninitializedObject(typeof(Il2CppSystem.Object)) };
    Equal(StatePayloadStatus.SerializationFailure, StatePayload.Encode(data, new[] { "gameData" }, out _), nameof(StatePayloadRejectsUnsupportedIl2CppObjectWrappers));
}

static void StatePayloadUsesIl2CppCollectionAccessors()
{
    static object Create(string assemblyName, string typeName, Type baseType) => Activator.CreateInstance(AssemblyBuilder.DefineDynamicAssembly(new AssemblyName(assemblyName), AssemblyBuilderAccess.Run).DefineDynamicModule("main").DefineType(typeName, TypeAttributes.Public | TypeAttributes.Class, baseType).CreateType())!;

    var data = new Il2CppCollectionFixture
    {
        Dictionary = Create("Il2Cppmscorlib", "Il2CppSystem.Collections.Generic.Dictionary`2", typeof(ReflectedDictionaryFixture)),
        List = Create("Il2Cppmscorlib", "Il2CppSystem.Collections.Generic.List`1", typeof(ReflectedListFixture))
    };

    StatePayloadStatus status = StatePayload.Encode(data, new[] { "gameData" }, out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadUsesIl2CppCollectionAccessors));
    Equal("{\"gameData\":{\"Dictionary\":{\"2\":\"two\"},\"List\":[3,4]}}", Encoding.UTF8.GetString(payload), nameof(StatePayloadUsesIl2CppCollectionAccessors));
}

static void StatePayloadIncludesInheritedGameplayProperties()
{
    var data = new InheritedFixtureRoot { Item = new InheritedFixtureDerived() };

    Equal(StatePayloadStatus.Success, StatePayload.Encode(data, new[] { "gameData" }, out byte[] canonicalPayload), nameof(StatePayloadIncludesInheritedGameplayProperties));
    Equal("{\"gameData\":{\"Item\":{\"Base\":\"base\",\"Derived\":\"derived\",\"Hidden\":\"derived hidden\"}}}", Encoding.UTF8.GetString(canonicalPayload), nameof(StatePayloadIncludesInheritedGameplayProperties));
    Equal(StatePayloadStatus.Success, StatePayload.Encode(data, new[] { "gameData.Item.Base", "gameData.Item.Hidden" }, out byte[] selectedPayload), nameof(StatePayloadIncludesInheritedGameplayProperties));
    Equal("{\"gameData.Item.Base\":\"base\",\"gameData.Item.Hidden\":\"derived hidden\"}", Encoding.UTF8.GetString(selectedPayload), nameof(StatePayloadIncludesInheritedGameplayProperties));
}

static void StatePayloadSerializesUnityColorAsChannels()
{
    var data = new ColorFixture { Color = default };

    Equal(StatePayloadStatus.Success, StatePayload.Encode(data, new[] { "gameData" }, out byte[] canonicalPayload), nameof(StatePayloadSerializesUnityColorAsChannels));
    Equal("{\"gameData\":{\"Color\":{\"r\":0,\"g\":0,\"b\":0,\"a\":0}}}", Encoding.UTF8.GetString(canonicalPayload), nameof(StatePayloadSerializesUnityColorAsChannels));
    Equal(StatePayloadStatus.Success, StatePayload.Encode(data, new[] { "gameData.Color" }, out byte[] selectedPayload), nameof(StatePayloadSerializesUnityColorAsChannels));
    Equal("{\"gameData.Color\":{\"r\":0,\"g\":0,\"b\":0,\"a\":0}}", Encoding.UTF8.GetString(selectedPayload), nameof(StatePayloadSerializesUnityColorAsChannels));
}

static void StatePayloadRejectsRunawayValueTraversal()
{
    Equal(StatePayloadStatus.SerializationFailure, StatePayload.Encode(new SelfReturningValueFixture(), new[] { "gameData" }, out byte[] payload), nameof(StatePayloadRejectsRunawayValueTraversal));
    Equal(0, payload.Length, nameof(StatePayloadRejectsRunawayValueTraversal));
}

static void StatePayloadRejectsImplementationPropertyPaths()
{
    var data = new TerminalPathFixture { Color = default, playerId = "player", rows = new List<int> { 1 } };

    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.playerId.Length" }, out _), nameof(StatePayloadRejectsImplementationPropertyPaths));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.rows.Count" }, out _), nameof(StatePayloadRejectsImplementationPropertyPaths));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.rows.Capacity" }, out _), nameof(StatePayloadRejectsImplementationPropertyPaths));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.Color.gamma" }, out _), nameof(StatePayloadRejectsImplementationPropertyPaths));
}

static void StatePayloadExcludesIl2CppDelegatesAndUnityEvents()
{
    static Type CreateDerivedType(ModuleBuilder module, string name, Type? baseType = null) => module.DefineType(name, TypeAttributes.Public | TypeAttributes.Class, baseType).CreateType();

    static void AddThrowingProperty(TypeBuilder type, string name, Type propertyType)
    {
        MethodBuilder getter = type.DefineMethod($"get_{name}", MethodAttributes.Public | MethodAttributes.SpecialName | MethodAttributes.HideBySig, propertyType, Type.EmptyTypes);
        getter.GetILGenerator().Emit(OpCodes.Ldstr, "excluded getter invoked");
        getter.GetILGenerator().Emit(OpCodes.Newobj, typeof(InvalidOperationException).GetConstructor(new[] { typeof(string) })!);
        getter.GetILGenerator().Emit(OpCodes.Throw);
        type.DefineProperty(name, PropertyAttributes.None, propertyType, null).SetGetMethod(getter);
    }

    ModuleBuilder module = AssemblyBuilder.DefineDynamicAssembly(new AssemblyName("ExcludedFixtures"), AssemblyBuilderAccess.Run).DefineDynamicModule("main");
    Type unityEvent = CreateDerivedType(module, "UnityEngine.Events.UnityEvent", CreateDerivedType(module, "UnityEngine.Events.UnityEventBase"));
    Type il2CppDelegate = CreateDerivedType(module, "Il2CppSystem.Delegate");
    Type il2CppMulticastDelegate = CreateDerivedType(module, "Il2CppSystem.MulticastDelegate", il2CppDelegate);
    Type il2CppFunc = CreateDerivedType(module, "Il2CppSystem.Func`2", il2CppMulticastDelegate);
    TypeBuilder itemType = module.DefineType("FilteredGameplayFixture", TypeAttributes.Public | TypeAttributes.Class);
    AddThrowingProperty(itemType, "Event", unityEvent);
    AddThrowingProperty(itemType, "Provider", il2CppFunc);
    MethodBuilder value = itemType.DefineMethod("get_Value", MethodAttributes.Public | MethodAttributes.SpecialName | MethodAttributes.HideBySig, typeof(int), Type.EmptyTypes);
    value.GetILGenerator().Emit(OpCodes.Ldc_I4_7);
    value.GetILGenerator().Emit(OpCodes.Ret);
    itemType.DefineProperty("Value", PropertyAttributes.None, typeof(int), null).SetGetMethod(value);
    var data = new FilteredMemberRoot { Item = Activator.CreateInstance(itemType.CreateType())! };

    Equal(StatePayloadStatus.Success, StatePayload.Encode(data, new[] { "gameData" }, out byte[] canonicalPayload), nameof(StatePayloadExcludesIl2CppDelegatesAndUnityEvents));
    Equal("{\"gameData\":{\"Item\":{\"Value\":7}}}", Encoding.UTF8.GetString(canonicalPayload), nameof(StatePayloadExcludesIl2CppDelegatesAndUnityEvents));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.Item.Event" }, out _), nameof(StatePayloadExcludesIl2CppDelegatesAndUnityEvents));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.Item.Provider" }, out _), nameof(StatePayloadExcludesIl2CppDelegatesAndUnityEvents));
}

static void StatePayloadExcludesRuntimeTypesAtEveryBoundary()
{
    Action callback = static () => { };
    object unityObject = System.Runtime.CompilerServices.RuntimeHelpers.GetUninitializedObject(typeof(UnityEngine.Object));
    var data = new ExcludedRuntimeFixture { Selected = callback, Unity = unityObject, Values = new[] { callback, unityObject } };

    Equal(StatePayloadStatus.Success, StatePayload.Encode(data, new[] { "gameData" }, out byte[] canonicalPayload), nameof(StatePayloadExcludesRuntimeTypesAtEveryBoundary));
    Equal("{\"gameData\":{\"Values\":[null,null]}}", Encoding.UTF8.GetString(canonicalPayload), nameof(StatePayloadExcludesRuntimeTypesAtEveryBoundary));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.Selected" }, out _), nameof(StatePayloadExcludesRuntimeTypesAtEveryBoundary));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.Unity" }, out _), nameof(StatePayloadExcludesRuntimeTypesAtEveryBoundary));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.Values.0" }, out _), nameof(StatePayloadExcludesRuntimeTypesAtEveryBoundary));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "gameData.Values.1" }, out _), nameof(StatePayloadExcludesRuntimeTypesAtEveryBoundary));
}

static async Task<(TcpClient Client, WebSocket Socket)> ConnectRawClient(WsConnection server)
{
    TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    WebSocket socket = WebSocket.CreateFromStream(
        client.GetStream(),
        isServer: false,
        subProtocol: null,
        keepAliveInterval: TimeSpan.FromSeconds(5));
    DateTime deadline = DateTime.UtcNow.AddSeconds(2);
    while (!server.ConnectedForTest && DateTime.UtcNow < deadline)
        await Task.Delay(1);
    if (!server.ConnectedForTest)
        throw new InvalidOperationException("server did not accept the raw client");
    return (client, socket);
}

static Task<string> ReceiveText(WebSocket socket)
    => ReceiveTextWithTimeout(socket, TimeSpan.FromSeconds(2));

static async Task<string> ReceiveTextWithTimeout(WebSocket socket, TimeSpan timeout)
{
    using CancellationTokenSource cancellation = new(timeout);
    using MemoryStream message = new();
    byte[] buffer = new byte[4096];
    while (true)
    {
        WebSocketReceiveResult result = await socket.ReceiveAsync(buffer, cancellation.Token);
        if (result.MessageType == WebSocketMessageType.Close)
            throw new InvalidOperationException("WebSocket closed before a text message.");
        if (result.MessageType != WebSocketMessageType.Text)
            continue;
        message.Write(buffer, 0, result.Count);
        if (result.EndOfMessage)
            return Encoding.UTF8.GetString(message.ToArray());
    }
}

static Task SendText(WebSocket socket, string text)
    => socket.SendAsync(Encoding.UTF8.GetBytes(text), WebSocketMessageType.Text, true, CancellationToken.None);

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

static void DragCommandFactoryCombinesStartAndEndEndpoints()
{
    const string testName = nameof(DragCommandFactoryCombinesStartAndEndEndpoints);
    var start = new ClickCommand(7, 100, 200);
    var end = new ClickCommand(7, 300, 400);
    DragCommand drag = DragCommandFactory.FromEndpoints(start, end);
    Equal(7UL, drag.RequestId, testName);
    Equal(100U, drag.StartX, testName);
    Equal(200U, drag.StartY, testName);
    Equal(300U, drag.EndX, testName);
    Equal(400U, drag.EndY, testName);
}

static void DragQueuePairsStartAndEndByRequestIdAndRejectsDuplicates()
{
    const string testName = nameof(DragQueuePairsStartAndEndByRequestIdAndRejectsDuplicates);
    var queue = new DragCommandQueue();

    Equal(true, queue.TryBeginDrag(new ClickCommand(5, 10, 20)), testName);
    Equal(false, queue.TryBeginDrag(new ClickCommand(5, 99, 99)), testName);
    Equal(false, queue.TryDequeue(out _), testName);

    Equal(true, queue.TryEndDrag(new ClickCommand(5, 30, 40)), testName);
    Equal(false, queue.TryEndDrag(new ClickCommand(5, 30, 40)), testName);

    Equal(true, queue.TryDequeue(out DragCommand command), testName);
    Equal(5UL, command.RequestId, testName);
    Equal(10U, command.StartX, testName);
    Equal(20U, command.StartY, testName);
    Equal(30U, command.EndX, testName);
    Equal(40U, command.EndY, testName);

    queue.Clear();
    Equal(false, queue.TryDequeue(out _), testName);
}

static void DragQueueIgnoresEndWithoutMatchingStart()
{
    const string testName = nameof(DragQueueIgnoresEndWithoutMatchingStart);
    var queue = new DragCommandQueue();
    Equal(false, queue.TryEndDrag(new ClickCommand(9, 1, 1)), testName);
    Equal(false, queue.TryDequeue(out _), testName);
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

static async Task ThrowsAsync<TException>(Func<Task> action, string testName)
    where TException : Exception
{
    try
    {
        await action();
    }
    catch (TException)
    {
        return;
    }
    catch (Exception exception)
    {
        throw new InvalidOperationException($"{testName}: expected {typeof(TException).Name}, got {exception.GetType().Name}.", exception);
    }
    throw new InvalidOperationException($"{testName}: expected {typeof(TException).Name}, but the action succeeded.");
}

sealed record TestReq(string Value) : IRequest<TestRes>;
sealed record TestRes(string Value);
sealed record OtherRes(string Value);
sealed record UnknownReq(string Value);

sealed class BridgeStateFixture
{
    public string Score { get; init; } = "1e3";
    public bool Enabled { get; init; } = true;
    public BridgeNestedStateFixture Nested { get; init; } = new();
    public object[] Items { get; init; } = new object[] { 1, "two", false };
    public string Text { get; init; } = "small";
}

sealed class BridgeNestedStateFixture
{
    public object? Value { get; init; }
}

sealed class ThrowingResponse
{
    public int Value => throw new InvalidOperationException("serialization failed");
}

enum SerializerFixtureKind
{
    First,
    Second
}

sealed class SerializerFixture
{
    public bool Boolean { get; init; }
    public decimal Decimal { get; init; }
    public Dictionary<int, string> Dictionary { get; init; } = new();
    public double Double { get; init; }
    public SerializerFixtureKind Enum { get; init; }
    public float Float { get; init; }
    public long LargeInteger { get; init; }
    public List<int> List { get; init; } = new();
    public SerializerNestedFixture Nested { get; init; } = new();
    public object? Null { get; init; }
    public long SafeInteger { get; init; }
    public string Text { get; init; } = "";
    public int[] Values { get; init; } = Array.Empty<int>();
}

sealed class SerializerNestedFixture
{
    public string Value { get; init; } = "";
}

sealed class PathFixture
{
    public PathEternityFixture eternity { get; init; } = new();
    public PathInfinityFixture infinity { get; init; } = new();
    public Dictionary<SerializerFixtureKind, string> kinds { get; init; } = new();
    public Dictionary<string, string> names { get; init; } = new();
    public Dictionary<int, string> numbers { get; init; } = new();
    public List<SerializerNestedFixture> rows { get; init; } = new();
}

sealed class PathEternityFixture
{
    public SerializerNestedFixture dilationTree { get; init; } = new();
    public int dtpBought { get; init; }
    public int dtpMax { get; init; }
}

sealed class PathInfinityFixture
{
    public int IP { get; init; }
}

sealed class GetterFailureFixture
{
    public int Value => throw new InvalidOperationException("getter failed");
}

sealed class CycleFixture
{
    public CycleNodeFixture? First { get; set; }
    public List<CycleNodeFixture> Items { get; set; } = new();
    public CycleNodeFixture? Second { get; set; }
}

sealed class CycleNodeFixture
{
    public CycleFixture? Parent { get; init; }
    public string Value { get; init; } = "";
}

sealed class GameScalarFixture
{
    public object Big { get; init; } = new();
    public DateTime Date { get; init; }
    public object Obscured { get; init; } = new();
}

sealed class SharedPathFixture
{
    public SerializerNestedFixture First { get; init; } = new();
    public SerializerNestedFixture Second { get; init; } = new();
}

sealed class Il2CppScalarFixture
{
    public object Date { get; init; } = new();
    public object HasValue { get; init; } = new();
    public object NoValue { get; init; } = new();
}

sealed class Il2CppCollectionFixture
{
    public object Dictionary { get; init; } = new();
    public object List { get; init; } = new();
}

sealed class Il2CppObjectFixture
{
    public object Value { get; init; } = new();
}

public class ReflectedListFixture : System.Collections.IEnumerable
{
    public int Count => 2;
    public int this[int index] => index + 3;
    System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator() => throw new InvalidOperationException("CLR enumeration must not be used");
}

public class ReflectedDictionaryFixture : System.Collections.IEnumerable
{
    public List<KeyValuePair<int, string>>.Enumerator GetEnumerator() => new List<KeyValuePair<int, string>> { new(2, "two") }.GetEnumerator();
    System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator() => throw new InvalidOperationException("CLR enumeration must not be used");
}

sealed class InheritedFixtureRoot
{
    public InheritedFixtureDerived Item { get; init; } = new();
}

class InheritedFixtureBase
{
    public string Base => "base";
    public string Hidden => "base hidden";
}

sealed class InheritedFixtureDerived : InheritedFixtureBase
{
    public string Derived => "derived";
    public new string Hidden => "derived hidden";
}

sealed class FilteredMemberRoot
{
    public object Item { get; init; } = new();
}

sealed class ColorFixture
{
    public UnityEngine.Color Color { get; init; }
}

readonly struct SelfReturningValueFixture
{
    public SelfReturningValueFixture Value => this;
}

sealed class TerminalPathFixture
{
    public UnityEngine.Color Color { get; init; }
    public string playerId { get; init; } = "";
    public List<int> rows { get; init; } = new();
}

sealed class ExcludedRuntimeFixture
{
    public object Selected { get; init; } = new();
    public object Unity { get; init; } = new();
    public object[] Values { get; init; } = Array.Empty<object>();
}
