using System.Globalization;
using System.Net;
using System.Net.Sockets;

namespace RevIdle.ScoreTelemetry;

internal sealed class UdpScorePublisher : IDisposable
{
    private readonly UdpClient _client;
    private readonly IPEndPoint _destination;

    private UdpScorePublisher(int port)
    {
        _client = new UdpClient(AddressFamily.InterNetwork);
        _destination = new IPEndPoint(IPAddress.Loopback, port);
    }

    public static UdpScorePublisher? Create(string? configuredPort)
    {
        if (!int.TryParse(configuredPort, NumberStyles.None, CultureInfo.InvariantCulture, out int port)
            || port is < 1 or > 65535)
            return null;

        try
        {
            return new UdpScorePublisher(port);
        }
        catch
        {
            return null;
        }
    }

    public void Publish(byte[] payload)
    {
        try
        {
            _client.Send(payload, payload.Length, _destination);
        }
        catch
        {
        }
    }

    public void Dispose()
    {
        try
        {
            _client.Dispose();
        }
        catch
        {
        }
    }
}
