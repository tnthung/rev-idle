using System.Diagnostics;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;

namespace RevIdle.ScoreTelemetry;

internal sealed class HttpScoreServer : IDisposable
{
    private enum PendingState
    {
        Created,
        Queued,
        Claimed,
        Completed,
        Faulted,
        TimedOut,
        Stopped
    }

    private sealed class Pending
    {
        public Pending(IReadOnlyList<string> keys, long deadline)
        {
            Keys = keys;
            Deadline = deadline;
            Completion = new TaskCompletionSource<(int StatusCode, byte[] Body)>(TaskCreationOptions.RunContinuationsAsynchronously);
            State = PendingState.Created;
        }

        public IReadOnlyList<string> Keys { get; }
        public long Deadline { get; }
        public TaskCompletionSource<(int StatusCode, byte[] Body)> Completion { get; }
        public LinkedListNode<Pending>? Node { get; set; }
        public PendingState State { get; set; }
    }

    private const int DeadlineMilliseconds = 2000;
    private const int MaxHeaderBytes = 8192;
    private readonly object _gate = new();
    private readonly TcpListener _listener;
    private readonly LinkedList<Pending> _pending = new();
    private readonly Dictionary<TcpClient, Task> _clients = new();
    private readonly CancellationTokenSource _stopping = new();
    private readonly Task _acceptLoop;
    private int _disposed;

    private HttpScoreServer(int port)
    {
        _listener = new TcpListener(IPAddress.Loopback, port);
        _listener.Start();
        Port = ((IPEndPoint)_listener.LocalEndpoint).Port;
        _acceptLoop = Task.Run(AcceptLoop);
        _ = _acceptLoop.ContinueWith(task => _ = task.Exception, CancellationToken.None, TaskContinuationOptions.OnlyOnFaulted | TaskContinuationOptions.ExecuteSynchronously, TaskScheduler.Default);
    }

    public int Port { get; }

    public static HttpScoreServer? Create(string? configuredPort)
    {
        if (!int.TryParse(configuredPort, NumberStyles.None, CultureInfo.InvariantCulture, out int port) || port is < 1 or > 65535)
            return null;
        try { return new HttpScoreServer(port); } catch { return null; }
    }

    public bool CompletePending(Func<IReadOnlyList<string>, (int StatusCode, byte[] Body)> complete)
    {
        Pending? pending = null;
        lock (_gate)
        {
            if (_disposed != 0)
                return false;

            while (_pending.First is { } node)
            {
                Pending candidate = node.Value;
                _pending.Remove(node);
                candidate.Node = null;
                if (candidate.State != PendingState.Queued)
                    continue;
                if (Stopwatch.GetTimestamp() >= candidate.Deadline)
                {
                    candidate.State = PendingState.TimedOut;
                    candidate.Completion.TrySetCanceled();
                    continue;
                }

                candidate.State = PendingState.Claimed;
                pending = candidate;
                break;
            }
        }

        if (pending is null)
            return false;

        try
        {
            (int statusCode, byte[] body) = complete(pending.Keys);
            pending.Completion.TrySetResult((statusCode, body));
            lock (_gate)
            {
                if (pending.State == PendingState.Claimed)
                    pending.State = PendingState.Completed;
            }
        }
        catch (Exception exception)
        {
            pending.Completion.TrySetException(exception);
            lock (_gate)
            {
                if (pending.State == PendingState.Claimed)
                    pending.State = PendingState.Faulted;
            }
        }

        return true;
    }

    private async Task AcceptLoop()
    {
        try
        {
            while (true)
            {
                TcpClient client = await _listener.AcceptTcpClientAsync(_stopping.Token).ConfigureAwait(false);
                RegisterClient(client);
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

    private void RegisterClient(TcpClient client)
    {
        lock (_gate)
        {
            if (_disposed != 0)
            {
                client.Dispose();
                return;
            }

            TaskCompletionSource start = new(TaskCreationOptions.RunContinuationsAsynchronously);
            _clients.Add(client, start.Task);
            Task task = Task.Run(async () =>
            {
                await start.Task.ConfigureAwait(false);
                await ProcessClient(client).ConfigureAwait(false);
            });
            _clients[client] = task;
            start.TrySetResult();
            _ = task.ContinueWith(completed =>
            {
                if (completed.IsFaulted)
                    _ = completed.Exception;
                lock (_gate)
                    _clients.Remove(client);
            }, CancellationToken.None, TaskContinuationOptions.ExecuteSynchronously, TaskScheduler.Default);
        }
    }

    private async Task ProcessClient(TcpClient client)
    {
        try
        {
            using (client)
            {
                await using NetworkStream stream = client.GetStream();
                while (!_stopping.IsCancellationRequested)
                {
                    string? header;
                    using (CancellationTokenSource deadline = CancellationTokenSource.CreateLinkedTokenSource(_stopping.Token))
                    {
                        deadline.CancelAfter(DeadlineMilliseconds);
                        try { header = await ReadHeader(stream, deadline.Token).ConfigureAwait(false); }
                        catch (OperationCanceledException) { return; }
                        catch (IOException) { return; }
                        catch (ObjectDisposedException) { return; }
                    }

                    if (header is null)
                        return;

                    string[] lines = header.Split("\r\n", StringSplitOptions.None);
                    string[] request = lines[0].Split(' ', StringSplitOptions.RemoveEmptyEntries);
                    if (request.Length < 2)
                    {
                        await WriteResponse(stream, 400, "Bad Request", Array.Empty<byte>(), true).ConfigureAwait(false);
                        return;
                    }
                    if (request[0] != "GET")
                    {
                        await WriteResponse(stream, 405, "Method Not Allowed", Array.Empty<byte>(), true).ConfigureAwait(false);
                        return;
                    }

                    string[] target = request[1].Split('?', 2, StringSplitOptions.None);
                    if (target[0] != "/state")
                    {
                        await WriteResponse(stream, 404, "Not Found", Array.Empty<byte>(), true).ConfigureAwait(false);
                        return;
                    }

                    List<string> keys = new();
                    HashSet<string> seen = new(StringComparer.Ordinal);
                    if (target.Length == 2)
                    {
                        foreach (string parameter in target[1].Split('&', StringSplitOptions.RemoveEmptyEntries))
                        {
                            string[] pair = parameter.Split('=', 2, StringSplitOptions.None);
                            if (pair.Length != 2 || pair[0] != "key" || pair[1].Length == 0 || !SupportedKeys.Contains(pair[1]))
                            {
                                await WriteResponse(stream, 400, "Bad Request", Array.Empty<byte>(), true).ConfigureAwait(false);
                                return;
                            }
                            if (seen.Add(pair[1]))
                                keys.Add(pair[1]);
                        }
                    }
                    if (keys.Count == 0)
                        keys.AddRange(AllKeys);

                    bool close = lines.Skip(1).Any(line =>
                    {
                        string[] pair = line.Split(':', 2, StringSplitOptions.None);
                        return pair.Length == 2 && pair[0].Equals("Connection", StringComparison.OrdinalIgnoreCase) && pair[1].Split(',').Any(value => value.Trim().Equals("close", StringComparison.OrdinalIgnoreCase));
                    });
                    Pending pending = new(keys.ToArray(), Stopwatch.GetTimestamp() + Stopwatch.Frequency * DeadlineMilliseconds / 1000);
                    lock (_gate)
                    {
                        if (_disposed != 0)
                            return;
                        pending.State = PendingState.Queued;
                        pending.Node = _pending.AddLast(pending);
                    }

                    (int statusCode, byte[] body) result;
                    using (CancellationTokenSource deadline = CancellationTokenSource.CreateLinkedTokenSource(_stopping.Token))
                    {
                        deadline.CancelAfter(DeadlineMilliseconds);
                        try { result = await pending.Completion.Task.WaitAsync(deadline.Token).ConfigureAwait(false); }
                        catch (OperationCanceledException)
                        {
                            StopPending(pending, _stopping.IsCancellationRequested ? PendingState.Stopped : PendingState.TimedOut);
                            return;
                        }
                    }

                    await WriteResponse(stream, result.statusCode, result.statusCode == 200 ? "OK" : result.statusCode == 503 ? "Service Unavailable" : "Error", result.body, close).ConfigureAwait(false);
                    if (close)
                        return;
                }
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (IOException)
        {
        }
        catch (ObjectDisposedException)
        {
        }
    }

    private void StopPending(Pending pending, PendingState state)
    {
        lock (_gate)
        {
            if (pending.State == PendingState.Queued)
            {
                if (pending.Node is not null)
                    _pending.Remove(pending.Node);
                pending.Node = null;
                pending.State = state;
                pending.Completion.TrySetCanceled();
            }
            else if (pending.State == PendingState.Created)
            {
                pending.State = state;
                pending.Completion.TrySetCanceled();
            }
            else if (pending.State == PendingState.Claimed)
            {
                pending.State = state;
                pending.Completion.TrySetCanceled();
            }
        }
    }

    private static async Task<string?> ReadHeader(NetworkStream stream, CancellationToken cancellationToken)
    {
        byte[] bytes = new byte[MaxHeaderBytes];
        int count = 0;
        while (count < bytes.Length)
        {
            int read = await stream.ReadAsync(bytes.AsMemory(count, 1), cancellationToken).ConfigureAwait(false);
            if (read == 0)
                return null;
            count += read;
            if (count >= 4 && bytes[count - 4] == '\r' && bytes[count - 3] == '\n' && bytes[count - 2] == '\r' && bytes[count - 1] == '\n')
                return Encoding.ASCII.GetString(bytes, 0, count - 4);
        }
        return null;
    }

    private async Task WriteResponse(NetworkStream stream, int statusCode, string reason, byte[] body, bool close)
    {
        byte[] response = Encoding.ASCII.GetBytes($"HTTP/1.1 {statusCode} {reason}\r\nContent-Type: application/json\r\nContent-Length: {body.Length}\r\nConnection: {(close ? "close" : "keep-alive")}\r\n\r\n").Concat(body).ToArray();
        using CancellationTokenSource deadline = CancellationTokenSource.CreateLinkedTokenSource(_stopping.Token);
        deadline.CancelAfter(DeadlineMilliseconds);
        await stream.WriteAsync(response.AsMemory(), deadline.Token).ConfigureAwait(false);
    }

    private void WaitForOwnedWork()
    {
        Task[] tasks;
        lock (_gate)
            tasks = _clients.Values.Append(_acceptLoop).Distinct().ToArray();
        try { Task.WhenAll(tasks).WaitAsync(TimeSpan.FromMilliseconds(DeadlineMilliseconds)).GetAwaiter().GetResult(); }
        catch (Exception)
        {
            foreach (Task task in tasks)
                if (task.IsFaulted)
                    _ = task.Exception;
        }
    }

    private static readonly string[] AllKeys = { "score", "income", "IP", "infinities", "stars", "stardust", "EP", "eternities", "DP", "AP", "RP", "RPMax", "RPSpent", "unities", "passiveUnities", "astrodust", "singularities", "atoms", "PlP", "PlPperPlG", "PlG", "VE", "ViP", "tarotSwords", "tarotWands", "tarotPentacles", "tarotCups", "goldTarotSwords", "goldTarotWands", "goldTarotPentacles", "goldTarotCups", "tarotDraws", "timeSinceStart", "timeInfinity", "timeEternity", "timeUnity", "timeTotal" };
    private static readonly HashSet<string> SupportedKeys = new(AllKeys, StringComparer.Ordinal);

    public void Dispose()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0)
            return;

        _stopping.Cancel();
        TcpClient[] clients;
        lock (_gate)
        {
            clients = _clients.Keys.ToArray();
            while (_pending.First is { } node)
            {
                Pending pending = node.Value;
                _pending.Remove(node);
                pending.Node = null;
                if (pending.State == PendingState.Queued)
                {
                    pending.State = PendingState.Stopped;
                    pending.Completion.TrySetCanceled();
                }
            }
        }

        _listener.Stop();
        foreach (TcpClient client in clients)
        {
            try { client.Dispose(); } catch { }
        }
        WaitForOwnedWork();
    }
}
