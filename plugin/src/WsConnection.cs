using System.Collections.Concurrent;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Channels;

namespace RevIdle.ScoreTelemetry;

internal interface IRequest<TResponse>
{
}

internal sealed class WsNotConnectedException : Exception
{
    public WsNotConnectedException() : base("WebSocket connection is not connected.")
    {
    }
}

internal sealed class PacketContext
{
    private readonly Func<object?, Task> _send;

    internal PacketContext(CancellationToken cancellationToken, Func<object?, Task> send)
    {
        CancellationToken = cancellationToken;
        _send = send;
    }

    public CancellationToken CancellationToken { get; }

    public Task Send<TPacket>(TPacket packet)
        => _send(packet);
}

internal sealed class WsConnection : IDisposable
{
    private const int OutboundCapacity = 256;
    private static readonly JsonSerializerOptions SerializerOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase
    };
    private readonly TcpListener? _listener;
    private Task? _acceptTask;
    private readonly CancellationTokenSource _stopping = new();
    private readonly object _gate = new();
    private readonly ConcurrentDictionary<Guid, Pending> _pending = new();
    private readonly object _tombstoneGate = new();
    private readonly Queue<(long Generation, Guid Uuid)> _timedOut = new();
    private readonly HashSet<(long Generation, Guid Uuid)> _timedOutSet = new();
    private readonly ConcurrentDictionary<string, HandlerRegistration> _handlers = new();
    private readonly ConcurrentQueue<InboundWork> _inbound = new();
    private readonly ConcurrentDictionary<Task, byte> _handlerTasks = new();
    private readonly TimeSpan _responseTimeout;
    private readonly Action<string>? _log;
    private Session? _active;
    private long _nextGeneration;
    private int _disposed;

    private WsConnection(TcpListener listener, TimeSpan responseTimeout, Action<string>? log)
    {
        _listener = listener;
        _responseTimeout = responseTimeout;
        _log = log;
        listener.Start();
        Port = ((IPEndPoint)listener.LocalEndpoint).Port;
        Task acceptTask = Task.Run(AcceptLoop);
        _acceptTask = acceptTask;
        ObserveLifecycle(acceptTask, "WebSocket accept loop");
    }

    private WsConnection()
    {
        _responseTimeout = TimeSpan.FromSeconds(2);
    }

    public int Port { get; }

    internal bool ConnectedForTest
    {
        get
        {
            lock (_gate)
                return _active is not null;
        }
    }

    internal int HandlerTaskCountForTest
        => _handlerTasks.Count;

    internal Action? PendingRemovedForTest { get; set; }

    internal Action<string>? LifecycleSynchronizationForTest { get; set; }

    internal int QueuedOutboundForTest
    {
        get
        {
            lock (_gate)
                return _active?.OutboundCount ?? 0;
        }
    }

    public static WsConnection? Create(string? configuredPort, Action<string>? log = null)
    {
        if (!int.TryParse(configuredPort, NumberStyles.None, CultureInfo.InvariantCulture, out int port) || port is < 1 or > 65535)
            return null;
        try
        {
            return new WsConnection(new TcpListener(IPAddress.Loopback, port), TimeSpan.FromSeconds(2), log);
        }
        catch
        {
            return null;
        }
    }

    internal static WsConnection CreateForTest(int port, TimeSpan responseTimeout, Action<string>? log = null)
    {
        if (port is < 0 or > 65535)
            throw new ArgumentOutOfRangeException(nameof(port));
        if (responseTimeout <= TimeSpan.Zero)
            throw new ArgumentOutOfRangeException(nameof(responseTimeout));
        return new WsConnection(new TcpListener(IPAddress.Loopback, port), responseTimeout, log);
    }

    public static WsConnection DisconnectedForTest()
        => new();

    public static string SerializeForTest(Guid uuid, object packet)
        => JsonSerializer.Serialize(new Envelope(uuid, packet.GetType().Name, packet), SerializerOptions);

    public void Handler<TPacket>(Func<PacketContext, TPacket, Task> handler)
    {
        if (handler is null)
            throw new ArgumentNullException(nameof(handler));
        HandlerRegistration registration = new(async (context, payload) =>
        {
            TPacket packet = payload.Deserialize<TPacket>(SerializerOptions)
                ?? throw new InvalidOperationException("Packet payload was null.");
            await handler(context, packet).ConfigureAwait(false);
        });
        if (!_handlers.TryAdd(typeof(TPacket).Name, registration))
            throw new InvalidOperationException($"Handler already registered for {typeof(TPacket).Name}.");
    }

    public async Task<TResponse> Request<TResponse>(IRequest<TResponse> packet)
    {
        Session session = GetActive();
        Guid uuid = Guid.NewGuid();
        Pending pending = new(session.Generation, typeof(TResponse).Name);
        if (!_pending.TryAdd(uuid, pending))
            throw new InvalidOperationException("Generated a duplicate request UUID.");

        try
        {
            await session.EnqueueAsync(new Outbound(
                session.Generation,
                Serialize(uuid, packet.GetType().Name, packet),
                new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously))).ConfigureAwait(false);
            using CancellationTokenSource timeout = new(_responseTimeout);
            JsonElement payload = await pending.Completion.Task.WaitAsync(timeout.Token).ConfigureAwait(false);
            return payload.Deserialize<TResponse>(SerializerOptions)
                ?? throw new InvalidOperationException("Response payload was null.");
        }
        catch (OperationCanceledException)
        {
            if (_pending.TryRemove(new KeyValuePair<Guid, Pending>(uuid, pending)))
            {
                RecordTimedOut(session.Generation, uuid);
                session.TryEnqueue(new Outbound(
                    session.Generation,
                    Serialize(uuid, "Cancel", new Cancel()),
                    new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously)));
                throw new TimeoutException("WebSocket response timed out.");
            }
            JsonElement responsePayload = await pending.Completion.Task.ConfigureAwait(false);
            return responsePayload.Deserialize<TResponse>(SerializerOptions)
                ?? throw new InvalidOperationException("Response payload was null.");
        }
        finally
        {
            _pending.TryRemove(new KeyValuePair<Guid, Pending>(uuid, pending));
        }
    }

    public Task Send<TPacket>(TPacket packet)
    {
        Session session = GetActive();
        return session.EnqueueAsync(new Outbound(
            session.Generation,
            Serialize(Guid.NewGuid(), packet?.GetType().Name ?? typeof(TPacket).Name, packet),
            new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously)));
    }

    public void Pump()
    {
        while (_inbound.TryDequeue(out InboundWork? work))
        {
            bool started;
            lock (_gate)
                started = ReferenceEquals(_active, work.Session) && !work.Session.IsClosed && work.Entry.TryStart();
            if (!started)
            {
                work.Session.RemoveInbound(work.Uuid, work.Entry);
                work.Entry.Dispose();
                continue;
            }
            Task task = RunHandler(work);
            TrackTask(task);
        }
    }

    private void TrackTask(Task task)
    {
        _handlerTasks.TryAdd(task, 0);
        _ = task.ContinueWith(completed =>
        {
            _handlerTasks.TryRemove(task, out _);
            if (completed.IsFaulted)
                Report($"WebSocket handler failed: {completed.Exception?.GetBaseException().Message}");
        }, TaskScheduler.Default);
    }

    private async Task RunHandler(InboundWork work)
    {
        PacketContext context = new(work.Entry.Cancellation, packet => SendResponse(work, packet));
        try
        {
            await work.Handler.Handler(context, work.Payload).ConfigureAwait(false);
        }
        catch (Exception exception)
        {
            if (!work.Entry.HasResponded && !work.Entry.IsCancelled)
                await SendRemoteError(work, exception.Message).ConfigureAwait(false);
            else if (!work.Entry.IsCancelled)
                Report($"WebSocket handler failed after response: {exception.Message}");
        }
        finally
        {
            work.Session.RemoveInbound(work.Uuid, work.Entry);
            work.Entry.Dispose();
        }
    }

    private Task SendResponse(InboundWork work, object? packet)
    {
        try
        {
            byte[] bytes = Serialize(work.Uuid, packet?.GetType().Name ?? "Null", packet);
            if (!work.Entry.TryReserveResponse())
                return Task.FromException(new InvalidOperationException("Packet already has a response or was cancelled."));
            return work.Session.EnqueueAsync(new Outbound(work.Session.Generation, bytes,
                new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously)));
        }
        catch (Exception exception)
        {
            return Task.FromException(exception);
        }
    }

    private Task SendRemoteError(InboundWork work, string message)
        => SendRemoteError(work.Session, work.Uuid, message);

    private Task SendRemoteError(Session session, Guid uuid, string message)
        => session.EnqueueAsync(new Outbound(
            session.Generation,
            Serialize(uuid, "RemoteError", new RemoteError(message)),
            new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously)));

    public void Dispose()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0)
            return;
        _stopping.Cancel();
        _listener?.Stop();
        Session? session;
        lock (_gate)
            session = _active;
        session?.Stop();
        foreach ((Guid uuid, Pending pending) in _pending)
            if (_pending.TryRemove(new KeyValuePair<Guid, Pending>(uuid, pending)))
                pending.Completion.TrySetException(new WsNotConnectedException());
        Task? sessionTask = session?.RunTask;
        try
        {
            Task[] lifecycleTasks = new[] { _acceptTask, sessionTask }.Where(task => task is not null).Cast<Task>().ToArray();
            if (lifecycleTasks.Length > 0)
                Task.WaitAll(lifecycleTasks, TimeSpan.FromSeconds(1));
        }
        catch
        {
        }
        _stopping.Dispose();
    }

    private Session GetActive()
    {
        lock (_gate)
        {
            if (_active is not null)
                return _active;
        }
        throw new WsNotConnectedException();
    }

    private async Task AcceptLoop()
    {
        try
        {
            while (!_stopping.IsCancellationRequested)
            {
                TcpClient client = await _listener!.AcceptTcpClientAsync(_stopping.Token).ConfigureAwait(false);
                Session? session = null;
                lock (_gate)
                {
                    if (_active is null && Volatile.Read(ref _disposed) == 0)
                    {
                        session = new Session(this, client, ++_nextGeneration);
                        _active = session;
                    }
                }
                if (session is null)
                {
                    client.Dispose();
                    continue;
                }
                session.Start();
            }
        }
        catch (OperationCanceledException) when (_stopping.IsCancellationRequested)
        {
        }
        catch (ObjectDisposedException) when (_stopping.IsCancellationRequested)
        {
        }
        catch (SocketException) when (_stopping.IsCancellationRequested)
        {
        }
    }

    private void ObserveLifecycle(Task task, string name)
    {
        _ = task.ContinueWith(completed =>
        {
            if (completed.IsFaulted)
                Report($"{name} failed: {completed.Exception?.GetBaseException().Message}");
        }, TaskScheduler.Default);
    }

    internal void ObserveLifecycleForTest(Task task)
        => ObserveLifecycle(task, "WebSocket test lifecycle");

    private void CloseSession(Session session)
    {
        Queue<InboundWork> closedInbound = new();
        lock (_gate)
        {
            if (!ReferenceEquals(_active, session))
                return;
            _active = null;

            Queue<InboundWork> retainedInbound = new();
            while (_inbound.TryDequeue(out InboundWork? work))
            {
                if (ReferenceEquals(work.Session, session))
                    closedInbound.Enqueue(work);
                else
                    retainedInbound.Enqueue(work);
            }
            while (retainedInbound.Count > 0)
                _inbound.Enqueue(retainedInbound.Dequeue());
        }
        session.CancelInbound();
        while (closedInbound.Count > 0)
            closedInbound.Dequeue().Entry.Dispose();
        foreach ((Guid uuid, Pending pending) in _pending)
        {
            if (pending.Generation == session.Generation && _pending.TryRemove(new KeyValuePair<Guid, Pending>(uuid, pending)))
                pending.Completion.TrySetException(new WsNotConnectedException());
        }
        lock (_tombstoneGate)
        {
            _timedOutSet.RemoveWhere(item => item.Generation == session.Generation);
            Queue<(long Generation, Guid Uuid)> retained = new(_timedOut.Count);
            while (_timedOut.Count > 0)
            {
                (long generation, Guid uuid) = _timedOut.Dequeue();
                if (generation != session.Generation)
                    retained.Enqueue((generation, uuid));
            }
            while (retained.Count > 0)
                _timedOut.Enqueue(retained.Dequeue());
        }
    }

    private void HandleEnvelope(Session session, Envelope envelope)
    {
        lock (_gate)
            if (!ReferenceEquals(_active, session) || session.IsClosed)
                return;
        if (envelope.Type == "Cancel")
        {
            session.CancelInbound(envelope.Uuid);
            return;
        }
        Pending? pending = null;
        lock (_gate)
        {
            if (!ReferenceEquals(_active, session) || session.IsClosed)
                return;
            if (_pending.TryGetValue(envelope.Uuid, out Pending? candidate) && candidate.Generation == session.Generation &&
                _pending.TryRemove(new KeyValuePair<Guid, Pending>(envelope.Uuid, candidate)))
                pending = candidate;
        }
        if (pending is not null)
        {
            PendingRemovedForTest?.Invoke();
            if (envelope.Type == "RemoteError")
            {
                if (envelope.Payload is JsonElement remotePayload)
                    try
                    {
                        RemoteError? error = remotePayload.Deserialize<RemoteError>(SerializerOptions);
                        pending.Completion.TrySetException(new InvalidOperationException(error?.Message ?? "Remote error payload was invalid."));
                    }
                    catch (Exception exception)
                    {
                        pending.Completion.TrySetException(new InvalidOperationException("Malformed RemoteError payload.", exception));
                    }
                else
                    pending.Completion.TrySetException(new InvalidOperationException("Remote error payload was invalid."));
            }
            else if (envelope.Type != pending.ExpectedType)
                pending.Completion.TrySetException(new InvalidOperationException($"Expected {pending.ExpectedType}, got {envelope.Type}."));
            else if (envelope.Payload is not JsonElement responsePayload)
                pending.Completion.TrySetException(new InvalidOperationException("Response payload was invalid."));
            else
                pending.Completion.TrySetResult(responsePayload.Clone());
            return;
        }
        lock (_gate)
            if (!ReferenceEquals(_active, session) || session.IsClosed)
                return;
        if (envelope.Type == "RemoteError")
        {
            try
            {
                if (envelope.Payload is JsonElement reportPayload)
                {
                    RemoteError? error = reportPayload.Deserialize<RemoteError>(SerializerOptions);
                    Report(error?.Message ?? "Remote error payload was invalid.");
                }
                else
                    Report("Remote error payload was invalid.");
            }
            catch (Exception exception)
            {
                Report($"Malformed RemoteError packet: {exception.Message}");
            }
            return;
        }
        lock (_tombstoneGate)
            if (_timedOutSet.Contains((session.Generation, envelope.Uuid)))
                return;
        if (!_handlers.TryGetValue(envelope.Type, out HandlerRegistration? handler))
        {
            TrackTask(SendRemoteError(session, envelope.Uuid, $"Unknown packet type: {envelope.Type}"));
            return;
        }
        Inbound inbound = new();
        InboundWork work = new(session, envelope.Uuid, envelope.Payload is JsonElement payload ? payload.Clone() : JsonSerializer.SerializeToElement(envelope.Payload, SerializerOptions), handler, inbound);
        lock (_gate)
        {
            if (!ReferenceEquals(_active, session) || session.IsClosed || !session.TryAddInbound(envelope.Uuid, inbound))
                return;
            _inbound.Enqueue(work);
        }
        LifecycleSynchronizationForTest?.Invoke("inbound-registered");
    }

    private void RecordTimedOut(long generation, Guid uuid)
    {
        lock (_tombstoneGate)
        {
            _timedOut.Enqueue((generation, uuid));
            _timedOutSet.Add((generation, uuid));
            while (_timedOut.Count > 1024)
                _timedOutSet.Remove(_timedOut.Dequeue());
        }
    }

    private void Report(string message)
    {
        try { _log?.Invoke(message); } catch { }
    }

    private static byte[] Serialize(Guid uuid, string type, object? payload)
        => Encoding.UTF8.GetBytes(JsonSerializer.Serialize(new Envelope(uuid, type, payload), SerializerOptions));

    private sealed class Session
    {
        private readonly WsConnection _owner;
        private readonly TcpClient _client;
        private readonly WebSocket _socket;
        private readonly Channel<Outbound> _outbound = Channel.CreateBounded<Outbound>(OutboundCapacity);
        private readonly CancellationTokenSource _stopping = new();
        private readonly ConcurrentDictionary<Guid, Inbound> _inbound = new();
        private Task? _runTask;
        private int _closed;

        public Session(WsConnection owner, TcpClient client, long generation)
        {
            _owner = owner;
            _client = client;
            Generation = generation;
            _socket = WebSocket.CreateFromStream(
                client.GetStream(),
                isServer: true,
                subProtocol: null,
                keepAliveInterval: Timeout.InfiniteTimeSpan);
        }

        public long Generation { get; }

        public Task? RunTask
            => _runTask;

        public bool IsClosed
            => Volatile.Read(ref _closed) != 0;

        public int OutboundCount
            => _outbound.Reader.Count;

        public bool TryAddInbound(Guid uuid, Inbound inbound)
            => _inbound.TryAdd(uuid, inbound);

        public void Start()
        {
            Task runTask = Task.Run(RunAsync);
            _runTask = runTask;
            _owner.ObserveLifecycle(runTask, $"WebSocket session {Generation}");
        }

        public void RemoveInbound(Guid uuid, Inbound inbound)
            => ((ICollection<KeyValuePair<Guid, Inbound>>)_inbound).Remove(new KeyValuePair<Guid, Inbound>(uuid, inbound));

        public void CancelInbound()
        {
            foreach (Inbound inbound in _inbound.Values)
                inbound.Cancel();
        }

        public void CancelInbound(Guid uuid)
        {
            if (_inbound.TryGetValue(uuid, out Inbound? inbound))
                inbound.Cancel();
        }

        public async Task RunAsync()
        {
            Task writer = WriterLoop();
            Task reader = ReaderLoop();
            await Task.WhenAny(writer, reader).ConfigureAwait(false);
            Stop();
            try { await Task.WhenAll(writer, reader).ConfigureAwait(false); } catch { }
            _stopping.Dispose();
        }

        public async Task EnqueueAsync(Outbound outbound)
        {
            if (Volatile.Read(ref _closed) != 0)
                throw new WsNotConnectedException();
            try
            {
                await _outbound.Writer.WriteAsync(outbound, _stopping.Token).ConfigureAwait(false);
                await outbound.Completion.Task.ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                throw new WsNotConnectedException();
            }
            catch (ChannelClosedException)
            {
                throw new WsNotConnectedException();
            }
        }

        public bool TryEnqueue(Outbound outbound)
            => Volatile.Read(ref _closed) == 0 && _outbound.Writer.TryWrite(outbound);

        public void Stop()
        {
            lock (_owner._gate)
            {
                if (_closed != 0)
                    return;
                _closed = 1;
            }
            _owner.LifecycleSynchronizationForTest?.Invoke("stop-before-cancel");
            _stopping.Cancel();
            _outbound.Writer.TryComplete();
            while (_outbound.Reader.TryRead(out Outbound? outbound))
                outbound?.Completion.TrySetException(new WsNotConnectedException());
            CancelInbound();
            try { _socket.Dispose(); } catch { }
            _client.Dispose();
            _owner.CloseSession(this);
        }

        private async Task WriterLoop()
        {
            try
            {
                await foreach (Outbound outbound in _outbound.Reader.ReadAllAsync(_stopping.Token).ConfigureAwait(false))
                {
                    if (Volatile.Read(ref _closed) != 0)
                    {
                        outbound.Completion.TrySetException(new WsNotConnectedException());
                        continue;
                    }
                    try
                    {
                        await _socket.SendAsync(outbound.Bytes, WebSocketMessageType.Text, true, _stopping.Token).ConfigureAwait(false);
                        outbound.Completion.TrySetResult(null);
                    }
                    catch (Exception exception)
                    {
                        outbound.Completion.TrySetException(new WsNotConnectedException());
                        if (!_stopping.IsCancellationRequested)
                            _owner.Report($"WebSocket writer failed: {exception.Message}");
                        Stop();
                        return;
                    }
                }
            }
            catch (OperationCanceledException) when (_stopping.IsCancellationRequested)
            {
            }
            finally
            {
                while (_outbound.Reader.TryRead(out Outbound? outbound))
                    outbound?.Completion.TrySetException(new WsNotConnectedException());
            }
        }

        private async Task ReaderLoop()
        {
            try
            {
                byte[] buffer = new byte[4096];
                using MemoryStream message = new();
                while (!_stopping.IsCancellationRequested)
                {
                    WebSocketReceiveResult result = await _socket.ReceiveAsync(buffer, _stopping.Token).ConfigureAwait(false);
                    if (result.MessageType == WebSocketMessageType.Close)
                        return;
                    if (result.MessageType != WebSocketMessageType.Text)
                        continue;
                    message.Write(buffer, 0, result.Count);
                    if (!result.EndOfMessage)
                        continue;
                    string text = Encoding.UTF8.GetString(message.ToArray());
                    message.SetLength(0);
                    try
                    {
                        Envelope? envelope = JsonSerializer.Deserialize<Envelope>(text, SerializerOptions);
                        if (envelope is null)
                            throw new JsonException("Envelope was null.");
                        _owner.HandleEnvelope(this, envelope);
                    }
                    catch (Exception exception)
                    {
                        _owner.Report($"Malformed WebSocket packet: {exception.Message}");
                    }
                }
            }
            catch (OperationCanceledException) when (_stopping.IsCancellationRequested)
            {
            }
            catch (Exception exception) when (!_stopping.IsCancellationRequested)
            {
                _owner.Report($"WebSocket reader failed: {exception.Message}");
            }
            finally
            {
                Stop();
            }
        }
    }

    private sealed class HandlerRegistration
    {
        public HandlerRegistration(Func<PacketContext, JsonElement, Task> handler)
        {
            Handler = handler;
        }

        public Func<PacketContext, JsonElement, Task> Handler { get; }
    }

    private sealed record InboundWork(Session Session, Guid Uuid, JsonElement Payload, HandlerRegistration Handler, Inbound Entry);

    private sealed class Inbound : IDisposable
    {
        private const int Cancelled = 1;
        private const int Started = 2;
        private const int Responded = 4;
        private readonly CancellationTokenSource _cancellation = new();
        private int _state;

        public CancellationToken Cancellation => _cancellation.Token;
        public bool IsCancelled => (Volatile.Read(ref _state) & Cancelled) != 0;
        public bool HasResponded => (Volatile.Read(ref _state) & Responded) != 0;

        public void Cancel()
        {
            while (true)
            {
                int state = Volatile.Read(ref _state);
                if ((state & Cancelled) != 0)
                    return;
                if (Interlocked.CompareExchange(ref _state, state | Cancelled, state) == state)
                {
                    _cancellation.Cancel();
                    return;
                }
            }
        }

        public bool TryStart()
        {
            while (true)
            {
                int state = Volatile.Read(ref _state);
                if ((state & (Cancelled | Started)) != 0)
                    return false;
                if (Interlocked.CompareExchange(ref _state, state | Started, state) == state)
                    return true;
            }
        }

        public bool TryReserveResponse()
        {
            while (true)
            {
                int state = Volatile.Read(ref _state);
                if ((state & (Cancelled | Responded)) != 0)
                    return false;
                if (Interlocked.CompareExchange(ref _state, state | Responded, state) == state)
                    return true;
            }
        }

        public void Dispose()
        {
            Cancel();
            _cancellation.Dispose();
        }
    }

    private sealed record Cancel;

    private sealed record RemoteError(string Message);

    private sealed class Pending
    {
        public Pending(long generation, string expectedType)
        {
            Generation = generation;
            ExpectedType = expectedType;
            Completion = new(TaskCreationOptions.RunContinuationsAsynchronously);
        }

        public long Generation { get; }
        public string ExpectedType { get; }
        public TaskCompletionSource<JsonElement> Completion { get; }
    }

    private sealed record Outbound(long Generation, byte[] Bytes, TaskCompletionSource<object?> Completion);

    private sealed class Envelope
    {
        public Envelope(Guid uuid, string type, object? payload)
        {
            Uuid = uuid;
            Type = type;
            Payload = payload;
        }

        [JsonPropertyName("uuid")]
        public Guid Uuid { get; init; }

        [JsonPropertyName("type")]
        public string Type { get; init; } = "";

        [JsonPropertyName("payload")]
        public object? Payload { get; init; }

        public Envelope()
        {
        }
    }
}
