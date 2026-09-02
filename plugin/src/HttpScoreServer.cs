using System.Collections.Concurrent;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;

namespace RevIdle.ScoreTelemetry;

internal sealed class HttpScoreServer : IDisposable
{
    private sealed record Pending(IReadOnlyList<string> Keys, TaskCompletionSource<(int StatusCode, byte[] Body)> Completion);
    private readonly TcpListener _listener;
    private readonly ConcurrentQueue<Pending> _pending = new();
    private readonly CancellationTokenSource _stopping = new();
    private int _disposed;

    private HttpScoreServer(int port)
    {
        _listener = new TcpListener(IPAddress.Loopback, port);
        _listener.Start();
        Port = ((IPEndPoint)_listener.LocalEndpoint).Port;
        _ = AcceptLoop();
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
        if (!_pending.TryDequeue(out Pending? pending))
            return false;
        pending.Completion.TrySetResult(complete(pending.Keys));
        return true;
    }

    private async Task AcceptLoop()
    {
        while (!_stopping.IsCancellationRequested)
        {
            try { await ProcessClient(await _listener.AcceptTcpClientAsync(_stopping.Token)); }
            catch (OperationCanceledException) { }
            catch (ObjectDisposedException) { }
            catch { }
        }
    }

    private async Task ProcessClient(TcpClient client)
    {
        using (client)
        {
        await using NetworkStream stream = client.GetStream();
        stream.ReadTimeout = 2000;
        stream.WriteTimeout = 2000;
        while (!_stopping.IsCancellationRequested)
        {
            string? header;
            try { header = await ReadHeader(stream); } catch { return; }
            if (header is null) return;
            string[] lines = header.Split("\r\n");
            string[] request = lines[0].Split(' ', StringSplitOptions.RemoveEmptyEntries);
            if (request.Length < 2) { await WriteResponse(stream, 400, "Bad Request", Array.Empty<byte>(), false); return; }
            if (request[0] != "GET") { await WriteResponse(stream, 405, "Method Not Allowed", Array.Empty<byte>(), false); return; }
            if (!request[1].StartsWith("/state", StringComparison.Ordinal)) { await WriteResponse(stream, 404, "Not Found", Array.Empty<byte>(), false); return; }
            string[] parts = request[1].Split('?', 2);
            List<string> keys = new();
            if (parts.Length == 2)
            {
                foreach (string parameter in parts[1].Split('&', StringSplitOptions.RemoveEmptyEntries))
                {
                    string[] pair = parameter.Split('=', 2);
                    if (pair.Length != 2 || pair[0] != "key" || !SupportedKeys.Contains(pair[1]) || keys.Contains(pair[1]))
                    { await WriteResponse(stream, 400, "Bad Request", Array.Empty<byte>(), false); return; }
                    keys.Add(pair[1]);
                }
            }
            TaskCompletionSource<(int StatusCode, byte[] Body)> completion = new(TaskCreationOptions.RunContinuationsAsynchronously);
            _pending.Enqueue(new Pending(keys, completion));
            (int status, byte[] body) result;
            try { result = await completion.Task.WaitAsync(TimeSpan.FromSeconds(2)); } catch { return; }
            bool close = lines.Any(line => line.Equals("Connection: close", StringComparison.OrdinalIgnoreCase));
            await WriteResponse(stream, result.status, result.status == 200 ? "OK" : result.status == 503 ? "Service Unavailable" : "Error", result.body, close);
            if (close) return;
        }
        }
    }

    private static async Task<string?> ReadHeader(NetworkStream stream)
    {
        byte[] bytes = new byte[8192];
        int count = 0;
        while (count < bytes.Length)
        {
            int read = await stream.ReadAsync(bytes.AsMemory(count, 1));
            if (read == 0) return null;
            count += read;
            if (count >= 4 && bytes[count - 4] == '\r' && bytes[count - 3] == '\n' && bytes[count - 2] == '\r' && bytes[count - 1] == '\n')
                return Encoding.ASCII.GetString(bytes, 0, count - 4);
        }
        return null;
    }

    private static async Task WriteResponse(NetworkStream stream, int status, string reason, byte[] body, bool close)
    {
        byte[] response = Encoding.ASCII.GetBytes($"HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {body.Length}\r\nConnection: {(close ? "close" : "keep-alive")}\r\n\r\n").Concat(body).ToArray();
        await stream.WriteAsync(response);
    }

    private static readonly HashSet<string> SupportedKeys = new(StringComparer.Ordinal) { "score", "income", "IP", "infinities", "stars", "stardust", "EP", "eternities", "DP", "AP", "RP", "RPMax", "RPSpent", "unities", "passiveUnities", "astrodust", "singularities", "atoms", "PlP", "PlPperPlG", "PlG", "VE", "ViP", "tarotSwords", "tarotWands", "tarotPentacles", "tarotCups", "goldTarotSwords", "goldTarotWands", "goldTarotPentacles", "goldTarotCups", "tarotDraws", "timeSinceStart", "timeInfinity", "timeEternity", "timeUnity", "timeTotal" };

    public void Dispose()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0) return;
        _stopping.Cancel();
        _listener.Stop();
        while (_pending.TryDequeue(out Pending? pending)) pending.Completion.TrySetCanceled();
    }
}
