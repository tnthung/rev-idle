using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Reflection;
using System.Reflection.Emit;
using System.Text;
using RevIdle.ScoreTelemetry;

InvalidPortsDisableServer();
await ServerReturnsSelectedState();
await ServerReturnsAllKeysInDocumentedOrder();
await ServerDeduplicatesSelectedKeys();
await ServerRejectsUnknownOrEmptyKeys();
await ServerMatchesExactRoutes();
await ServerReturnsMethodNotAllowed();
await ServerReturnsUnavailableState();
await KeepAliveRequestsUseContentLength();
StatePayloadFormatsBigDoubleValues();
StatePayloadSerializesCompleteGraph();
StatePayloadResolvesSelectedPaths();
StatePayloadDistinguishesInvalidPathsAndGetterFailures();
StatePayloadPreservesCollectionIndexesAcrossCycles();
StatePayloadSerializesGameSpecificScalars();
StatePayloadKeepsSelectedKeysForSharedValues();
StatePayloadSerializesIl2CppDatesAndNullables();
StatePayloadUsesIl2CppCollectionAccessors();
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
System.Console.WriteLine("30 tests passed.");

static void InvalidPortsDisableServer()
{
    foreach (string? value in new string?[] { null, "", "not-a-port", "0", "-1", "65536" })
    {
        using HttpScoreServer? server = HttpScoreServer.Create(value);
        if (server is not null)
            throw new InvalidOperationException($"{nameof(InvalidPortsDisableServer)}: '{value}' should be disabled.");
    }
}

static async Task ServerReturnsSelectedState()
{
    using HttpScoreServer server = CreateServer();
    using TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    await using NetworkStream stream = client.GetStream();
    ResponseReader reader = new(stream);
    await stream.WriteAsync(Encoding.ASCII.GetBytes("GET /state?key=score&key=IP HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"));
    IReadOnlyList<string> keys = await CompletePendingUntil(server, _ => (200, Encoding.UTF8.GetBytes("{\"score\":\"2.5e42\",\"IP\":\"7e8\"}")));
    Equal(true, keys.SequenceEqual(new[] { "score", "IP" }), nameof(ServerReturnsSelectedState));
    HttpResponse response = await reader.ReadResponse();
    Equal(200, response.StatusCode, nameof(ServerReturnsSelectedState));
    Equal("application/json", response.ContentType, nameof(ServerReturnsSelectedState));
    Equal(response.Body.Length, response.ContentLength, nameof(ServerReturnsSelectedState));
    Equal("close", response.Connection, nameof(ServerReturnsSelectedState));
    Equal("{\"score\":\"2.5e42\",\"IP\":\"7e8\"}", Encoding.UTF8.GetString(response.Body), nameof(ServerReturnsSelectedState));
}

static async Task ServerReturnsAllKeysInDocumentedOrder()
{
    using HttpScoreServer server = CreateServer();
    using TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    await using NetworkStream stream = client.GetStream();
    await stream.WriteAsync(Encoding.ASCII.GetBytes("GET /state HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"));
    IReadOnlyList<string> keys = await CompletePendingUntil(server, _ => (200, Array.Empty<byte>()));
    Equal(true, keys.SequenceEqual(new[] { "score", "income", "IP", "infinities", "stars", "stardust", "EP", "eternities", "DP", "AP", "RP", "RPMax", "RPSpent", "unities", "passiveUnities", "astrodust", "singularities", "atoms", "PlP", "PlPperPlG", "PlG", "VE", "ViP", "tarotSwords", "tarotWands", "tarotPentacles", "tarotCups", "goldTarotSwords", "goldTarotWands", "goldTarotPentacles", "goldTarotCups", "tarotDraws", "timeSinceStart", "timeInfinity", "timeEternity", "timeUnity", "timeTotal" }), nameof(ServerReturnsAllKeysInDocumentedOrder));
    HttpResponse response = await new ResponseReader(stream).ReadResponse();
    Equal(200, response.StatusCode, nameof(ServerReturnsAllKeysInDocumentedOrder));
}

static async Task ServerDeduplicatesSelectedKeys()
{
    using HttpScoreServer server = CreateServer();
    using TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    await using NetworkStream stream = client.GetStream();
    await stream.WriteAsync(Encoding.ASCII.GetBytes("GET /state?key=score&key=IP&key=score HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"));
    IReadOnlyList<string> keys = await CompletePendingUntil(server, _ => (200, Array.Empty<byte>()));
    Equal(true, keys.SequenceEqual(new[] { "score", "IP" }), nameof(ServerDeduplicatesSelectedKeys));
    Equal(200, (await new ResponseReader(stream).ReadResponse()).StatusCode, nameof(ServerDeduplicatesSelectedKeys));
}

static async Task ServerRejectsUnknownOrEmptyKeys()
{
    using HttpScoreServer server = CreateServer();
    HttpResponse unknown = await RequestWithServer(server, "GET /state?key=unknown HTTP/1.1");
    HttpResponse empty = await RequestWithServer(server, "GET /state?key= HTTP/1.1");
    Equal(400, unknown.StatusCode, nameof(ServerRejectsUnknownOrEmptyKeys));
    Equal(400, empty.StatusCode, nameof(ServerRejectsUnknownOrEmptyKeys));
}

static async Task ServerMatchesExactRoutes()
{
    using HttpScoreServer server = CreateServer();
    Equal(404, (await RequestWithServer(server, "GET /other HTTP/1.1")).StatusCode, nameof(ServerMatchesExactRoutes));
    Equal(404, (await RequestWithServer(server, "GET /stateful HTTP/1.1")).StatusCode, nameof(ServerMatchesExactRoutes));
    Equal(404, (await RequestWithServer(server, "GET /state/extra HTTP/1.1")).StatusCode, nameof(ServerMatchesExactRoutes));
}

static async Task ServerReturnsMethodNotAllowed()
{
    using HttpScoreServer server = CreateServer();
    Equal(405, (await RequestWithServer(server, "POST /state HTTP/1.1")).StatusCode, nameof(ServerReturnsMethodNotAllowed));
}

static async Task ServerReturnsUnavailableState()
{
    using HttpScoreServer server = CreateServer();
    using TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    await using NetworkStream stream = client.GetStream();
    await stream.WriteAsync(Encoding.ASCII.GetBytes("GET /state HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"));
    await CompletePendingUntil(server, _ => (503, Array.Empty<byte>()));
    HttpResponse response = await new ResponseReader(stream).ReadResponse();
    Equal(503, response.StatusCode, nameof(ServerReturnsUnavailableState));
    Equal(0, response.Body.Length, nameof(ServerReturnsUnavailableState));
}

static async Task KeepAliveRequestsUseContentLength()
{
    using HttpScoreServer server = CreateServer();
    using TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    await using NetworkStream stream = client.GetStream();
    ResponseReader reader = new(stream);
    await stream.WriteAsync(Encoding.ASCII.GetBytes("GET /state?key=score HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"));
    IReadOnlyList<string> firstKeys = await CompletePendingUntil(server, _ => (200, Encoding.UTF8.GetBytes("{\"score\":\"1e2\"}")));
    Equal(true, firstKeys.SequenceEqual(new[] { "score" }), nameof(KeepAliveRequestsUseContentLength));
    HttpResponse first = await reader.ReadResponse();
    Equal(200, first.StatusCode, nameof(KeepAliveRequestsUseContentLength));
    Equal("keep-alive", first.Connection, nameof(KeepAliveRequestsUseContentLength));
    Equal(first.Body.Length, first.ContentLength, nameof(KeepAliveRequestsUseContentLength));
    Equal("{\"score\":\"1e2\"}", Encoding.UTF8.GetString(first.Body), nameof(KeepAliveRequestsUseContentLength));

    await stream.WriteAsync(Encoding.ASCII.GetBytes("GET /state?key=IP HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"));
    IReadOnlyList<string> secondKeys = await CompletePendingUntil(server, _ => (200, Encoding.UTF8.GetBytes("{\"IP\":\"2e3\"}")));
    Equal(true, secondKeys.SequenceEqual(new[] { "IP" }), nameof(KeepAliveRequestsUseContentLength));
    HttpResponse second = await reader.ReadResponse();
    Equal(200, second.StatusCode, nameof(KeepAliveRequestsUseContentLength));
    Equal("close", second.Connection, nameof(KeepAliveRequestsUseContentLength));
    Equal(second.Body.Length, second.ContentLength, nameof(KeepAliveRequestsUseContentLength));
    Equal("{\"IP\":\"2e3\"}", Encoding.UTF8.GetString(second.Body), nameof(KeepAliveRequestsUseContentLength));
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

    StatePayloadStatus status = StatePayload.Encode(data, Array.Empty<string>(), out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadSerializesCompleteGraph));
    Equal("{\"Boolean\":true,\"Decimal\":12.5,\"Dictionary\":{\"2\":\"two\"},\"Double\":\"-Infinity\",\"Enum\":\"Second\",\"Float\":\"NaN\",\"LargeInteger\":\"9007199254740992\",\"List\":[3,4],\"Nested\":{\"Value\":\"nested\"},\"Null\":null,\"SafeInteger\":9007199254740991,\"Text\":\"value\",\"Values\":[1,2]}", Encoding.UTF8.GetString(payload), nameof(StatePayloadSerializesCompleteGraph));
}

static void StatePayloadResolvesSelectedPaths()
{
    var data = new PathFixture
    {
        eternity = new PathEternityFixture { dilationTree = new SerializerNestedFixture { Value = "tree" }, dtpMax = 12 },
        infinity = new PathInfinityFixture { IP = 11 },
        kinds = new Dictionary<SerializerFixtureKind, string> { [SerializerFixtureKind.Second] = "enum" },
        names = new Dictionary<string, string> { ["primary"] = "name" },
        numbers = new Dictionary<int, string> { [2] = "number" },
        rows = new List<SerializerNestedFixture> { new() { Value = "zero" }, new() { Value = "one" } }
    };

    StatePayloadStatus status = StatePayload.Encode(data, new[] { "IP", "DT", "DTP", "rows.1.Value", "numbers.2", "names.primary", "kinds.Second", "IP" }, out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadResolvesSelectedPaths));
    Equal("{\"IP\":11,\"DT\":{\"Value\":\"tree\"},\"DTP\":12,\"rows.1.Value\":\"one\",\"numbers.2\":\"number\",\"names.primary\":\"name\",\"kinds.Second\":\"enum\"}", Encoding.UTF8.GetString(payload), nameof(StatePayloadResolvesSelectedPaths));
}

static void StatePayloadDistinguishesInvalidPathsAndGetterFailures()
{
    var data = new PathFixture
    {
        rows = new List<SerializerNestedFixture> { new() { Value = "zero" } },
        numbers = new Dictionary<int, string> { [2] = "number" }
    };

    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "unknown" }, out byte[] unknownPayload), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(0, unknownPayload.Length, nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "rows.1.Value" }, out _), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "numbers.3" }, out _), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(StatePayloadStatus.InvalidPath, StatePayload.Encode(data, new[] { "" }, out _), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(StatePayloadStatus.SerializationFailure, StatePayload.Encode(new GetterFailureFixture(), Array.Empty<string>(), out byte[] failurePayload), nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
    Equal(0, failurePayload.Length, nameof(StatePayloadDistinguishesInvalidPathsAndGetterFailures));
}

static void StatePayloadPreservesCollectionIndexesAcrossCycles()
{
    var data = new CycleFixture();
    var shared = new CycleNodeFixture { Parent = data, Value = "shared" };
    var item = new CycleNodeFixture { Parent = data, Value = "item" };
    data.First = shared;
    data.Items = new List<CycleNodeFixture> { shared, item, item };
    data.Second = shared;

    StatePayloadStatus status = StatePayload.Encode(data, Array.Empty<string>(), out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadPreservesCollectionIndexesAcrossCycles));
    Equal("{\"First\":{\"Value\":\"shared\"},\"Items\":[null,{\"Value\":\"item\"},null]}", Encoding.UTF8.GetString(payload), nameof(StatePayloadPreservesCollectionIndexesAcrossCycles));
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

    StatePayloadStatus status = StatePayload.Encode(data, Array.Empty<string>(), out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadSerializesGameSpecificScalars));
    Equal("{\"Big\":\"2.5e42\",\"Date\":\"2026-09-03T01:02:03.0000000Z\",\"Obscured\":17}", Encoding.UTF8.GetString(payload), nameof(StatePayloadSerializesGameSpecificScalars));
}

static void StatePayloadKeepsSelectedKeysForSharedValues()
{
    var shared = new SerializerNestedFixture { Value = "shared" };
    var data = new SharedPathFixture { First = shared, Second = shared };

    StatePayloadStatus status = StatePayload.Encode(data, new[] { "First", "Second" }, out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadKeepsSelectedKeysForSharedValues));
    Equal("{\"First\":{\"Value\":\"shared\"},\"Second\":{\"Value\":\"shared\"}}", Encoding.UTF8.GetString(payload), nameof(StatePayloadKeepsSelectedKeysForSharedValues));
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

    StatePayloadStatus status = StatePayload.Encode(data, Array.Empty<string>(), out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadSerializesIl2CppDatesAndNullables));
    Equal("{\"Date\":\"2026-09-03T01:02:03.0000000Z\",\"HasValue\":17,\"NoValue\":null}", Encoding.UTF8.GetString(payload), nameof(StatePayloadSerializesIl2CppDatesAndNullables));
}

static void StatePayloadUsesIl2CppCollectionAccessors()
{
    static object Create(string assemblyName, string typeName, Type baseType) => Activator.CreateInstance(AssemblyBuilder.DefineDynamicAssembly(new AssemblyName(assemblyName), AssemblyBuilderAccess.Run).DefineDynamicModule("main").DefineType(typeName, TypeAttributes.Public | TypeAttributes.Class, baseType).CreateType())!;

    var data = new Il2CppCollectionFixture
    {
        Dictionary = Create("Il2Cppmscorlib", "Il2CppSystem.Collections.Generic.Dictionary`2", typeof(ReflectedDictionaryFixture)),
        List = Create("Il2Cppmscorlib", "Il2CppSystem.Collections.Generic.List`1", typeof(ReflectedListFixture))
    };

    StatePayloadStatus status = StatePayload.Encode(data, Array.Empty<string>(), out byte[] payload);

    Equal(StatePayloadStatus.Success, status, nameof(StatePayloadUsesIl2CppCollectionAccessors));
    Equal("{\"Dictionary\":{\"2\":\"two\"},\"List\":[3,4]}", Encoding.UTF8.GetString(payload), nameof(StatePayloadUsesIl2CppCollectionAccessors));
}

static HttpScoreServer CreateServer()
{
    using var probe = new TcpListener(IPAddress.Loopback, 0);
    probe.Start();
    int port = ((IPEndPoint)probe.LocalEndpoint).Port;
    probe.Stop();
    return HttpScoreServer.Create(port.ToString(CultureInfo.InvariantCulture)) ?? throw new InvalidOperationException("server failed to bind");
}

static async Task<HttpResponse> RequestWithServer(HttpScoreServer server, string request)
{
    using TcpClient client = new();
    await client.ConnectAsync(IPAddress.Loopback, server.Port);
    await using NetworkStream stream = client.GetStream();
    await stream.WriteAsync(Encoding.ASCII.GetBytes($"{request}\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"));
    return await new ResponseReader(stream).ReadResponse();
}

static async Task<IReadOnlyList<string>> CompletePendingUntil(HttpScoreServer server, Func<IReadOnlyList<string>, (int StatusCode, byte[] Body)> complete)
{
    IReadOnlyList<string>? observed = null;
    DateTime deadline = DateTime.UtcNow.AddSeconds(4);
    while (DateTime.UtcNow < deadline)
    {
        if (server.CompletePending(keys =>
        {
            observed = keys.ToArray();
            return complete(keys);
        }))
            return observed!;
        await Task.Delay(10);
    }
    throw new InvalidOperationException("timed out waiting for a pending request");
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

sealed record HttpResponse(int StatusCode, string ContentType, int ContentLength, string Connection, byte[] Body);

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

sealed class ResponseReader
{
    private readonly NetworkStream _stream;
    private byte[] _buffer = Array.Empty<byte>();

    public ResponseReader(NetworkStream stream)
    {
        _stream = stream;
    }

    public async Task<HttpResponse> ReadResponse()
    {
        byte[] readBuffer = new byte[1024];
        while (true)
        {
            int delimiter = FindHeaderDelimiter();
            if (delimiter >= 0)
            {
                string[] lines = Encoding.ASCII.GetString(_buffer, 0, delimiter).Split("\r\n");
                string[] status = lines[0].Split(' ', 3, StringSplitOptions.RemoveEmptyEntries);
                int contentLength = 0;
                string contentType = "";
                string connection = "";
                foreach (string line in lines.Skip(1))
                {
                    string[] pair = line.Split(':', 2);
                    if (pair.Length != 2)
                        continue;
                    if (pair[0].Equals("Content-Length", StringComparison.OrdinalIgnoreCase))
                        contentLength = int.Parse(pair[1].Trim(), CultureInfo.InvariantCulture);
                    if (pair[0].Equals("Content-Type", StringComparison.OrdinalIgnoreCase))
                        contentType = pair[1].Trim();
                    if (pair[0].Equals("Connection", StringComparison.OrdinalIgnoreCase))
                        connection = pair[1].Trim();
                }
                int bodyStart = delimiter + 4;
                if (_buffer.Length - bodyStart >= contentLength)
                {
                    byte[] body = _buffer[bodyStart..(bodyStart + contentLength)];
                    _buffer = _buffer[(bodyStart + contentLength)..];
                    return new HttpResponse(int.Parse(status[1], CultureInfo.InvariantCulture), contentType, contentLength, connection, body);
                }
            }

            int read = await _stream.ReadAsync(readBuffer).AsTask().WaitAsync(TimeSpan.FromSeconds(4));
            if (read == 0)
                throw new InvalidOperationException("connection closed before response");
            int oldLength = _buffer.Length;
            Array.Resize(ref _buffer, oldLength + read);
            readBuffer.AsSpan(0, read).CopyTo(_buffer.AsSpan(oldLength));
        }
    }

    private int FindHeaderDelimiter()
    {
        for (int i = 0; i <= _buffer.Length - 4; i++)
            if (_buffer[i] == '\r' && _buffer[i + 1] == '\n' && _buffer[i + 2] == '\r' && _buffer[i + 3] == '\n')
                return i;
        return -1;
    }
}
