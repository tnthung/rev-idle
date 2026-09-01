using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;
using RevIdle.ScoreTelemetry;

await ScorePayloadUsesInvariantRoundTripFormatting();
InvalidPortsDisablePublishing();
await PublisherSendsExactPayloadToLoopback();
Console.WriteLine("3 tests passed.");

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

static void Equal<T>(T expected, T actual, string testName)
{
    if (!EqualityComparer<T>.Default.Equals(expected, actual))
        throw new InvalidOperationException($"{testName}: expected '{expected}', got '{actual}'.");
}
