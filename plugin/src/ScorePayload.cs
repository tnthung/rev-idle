using System.Globalization;
using System.Text;

namespace RevIdle.ScoreTelemetry;

internal static class ScorePayload
{
    public static byte[] Encode(double mantissa, double exponent)
    {
        string score = string.Concat(
            mantissa.ToString("R", CultureInfo.InvariantCulture),
            "e",
            exponent.ToString("R", CultureInfo.InvariantCulture));

        return Encoding.UTF8.GetBytes(string.Concat("{\"score\":\"", score, "\"}"));
    }
}
