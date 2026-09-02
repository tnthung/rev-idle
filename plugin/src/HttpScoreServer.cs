using System.Collections.Concurrent;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;

namespace RevIdle.ScoreTelemetry;

internal sealed class HttpScoreServer : IDisposable
{
    private sealed class Pending
    {
        public Pending(IReadOnlyList<string> keys)
        {
            Keys = keys;
            Completion = new TaskCompletionSource<(int StatusCode, byte[] Body)>(TaskCreationOptions.RunContinuationsAsynchronously);
        }

        public IReadOnlyList<string> Keys { get; }
        public TaskCompletionSource<(int StatusCode, byte[] Body)> Completion { get; }
    }

    private const int TimeoutMilliseconds = 2000;
    private const int MaxHeaderBytes = 8192;
    private readonly TcpListener _listener;
    private readonly ConcurrentQueue<Pending> _pending = new();
    private readonly ConcurrentDictionary<TcpClient, byte> _clients = new();
    private readonly CancellationTokenSource _stopping = new();
    private int _disposed;

    private HttpScoreServer(int port)
    {
        _listener = new TcpListener(IPAddress.Loopback, port);
        _listener.Start();
        Port = ((IPEndPoint)_listener.LocalEndpoint).Port;
        _ = Task.Run(AcceptLoop);
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
        while (_pending.TryDequeue(out Pending? pending))
        {
            if (pending.Completion.Task.IsCompleted)
                continue;
            try { pending.Completion.TrySetResult(complete(pending.Keys)); }
            catch (Exception exception) { pending.Completion.TrySetException(exception); }
            return true;
        }
        return false;
    }

    private async Task AcceptLoop()
    {
        try
        {
            while (!_stopping.IsCancellationRequested)
            {
                TcpClient client = await _listener.AcceptTcpClientAsync(_stopping.Token).ConfigureAwait(false);
                if (Volatile.Read(ref _disposed) != 0)
                {
                    client.Dispose();
                    continue;
                }
                _clients.TryAdd(client, 0);
                _ = ProcessClient(client);
            }
        }
        catch (OperationCanceledException) when (_stopping.IsCancellationRequested) { }
        catch (ObjectDisposedException) when (_stopping.IsCancellationRequested) { }
        catch (SocketException) when (_stopping.IsCancellationRequested) { }
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
                using CancellationTokenSource headerTimeout = CancellationTokenSource.CreateLinkedTokenSource(_stopping.Token);
                headerTimeout.CancelAfter(TimeoutMilliseconds);
                string? header = await ReadHeader(stream, headerTimeout.Token).ConfigureAwait(false);
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
                if (target.Length == 2)
                {
                    HashSet<string> seen = new(StringComparer.Ordinal);
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
                    if (keys.Count == 0)
                    {
                        await WriteResponse(stream, 400, "Bad Request", Array.Empty<byte>(), true).ConfigureAwait(false);
                        return;
                    }
                }
                if (keys.Count == 0)
                    keys.AddRange(AllKeys);

                bool close = lines.Skip(1).Any(line => line.StartsWith("Connection:", StringComparison.OrdinalIgnoreCase) && line.Contains("close", StringComparison.OrdinalIgnoreCase));
                Pending pending = new(keys);
                _pending.Enqueue(pending);
                using CancellationTokenSource responseTimeout = CancellationTokenSource.CreateLinkedTokenSource(_stopping.Token);
                responseTimeout.CancelAfter(TimeoutMilliseconds);
                try
                {
                    (int statusCode, byte[] body) = await pending.Completion.Task.WaitAsync(responseTimeout.Token).ConfigureAwait(false);
                    await WriteResponse(stream, statusCode, statusCode == 200 ? "OK" : statusCode == 503 ? "Service Unavailable" : "Error", body, close).ConfigureAwait(false);
                }
                catch (OperationCanceledException) { return; }
                if (close)
                    return;
            }
            }
        }
        catch (OperationCanceledException) { }
        catch (IOException) { }
        catch (ObjectDisposedException) { }
        finally { _clients.TryRemove(client, out _); }
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
        using CancellationTokenSource timeout = CancellationTokenSource.CreateLinkedTokenSource(_stopping.Token);
        timeout.CancelAfter(TimeoutMilliseconds);
        await stream.WriteAsync(response.AsMemory(), timeout.Token).ConfigureAwait(false);
    }

    public void Dispose()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0)
            return;
        _stopping.Cancel();
        _listener.Stop();
        while (_pending.TryDequeue(out Pending? pending))
            pending.Completion.TrySetCanceled();
        foreach (TcpClient client in _clients.Keys)
            client.Dispose();
        _stopping.Dispose();
    }

    private static readonly string[] AllKeys = { "score", "income", "IP", "infinities", "stars", "stardust", "EP", "eternities", "DP", "AP", "RP", "RPMax", "RPSpent", "unities", "passiveUnities", "astrodust", "singularities", "atoms", "PlP", "PlPperPlG", "PlG", "VE", "ViP", "tarotSwords", "tarotWands", "tarotPentacles", "tarotCups", "goldTarotSwords", "goldTarotWands", "goldTarotPentacles", "goldTarotCups", "tarotDraws", "timeSinceStart", "timeInfinity", "timeEternity", "timeUnity", "timeTotal" };
    private static readonly HashSet<string> SupportedKeys = new(AllKeys, StringComparer.Ordinal);
}
