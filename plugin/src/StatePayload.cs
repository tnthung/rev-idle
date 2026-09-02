using System.Globalization;
using System.Text;

namespace RevIdle.ScoreTelemetry;

internal static class StatePayload
{
    public static bool TryEncode(GameData data, IReadOnlyList<string> keys, out byte[] payload)
    {
        var values = new List<(string Key, double Mantissa, double Exponent)>(keys.Count);
        foreach (string key in keys)
        {
            BigDouble value = default;
            bool found = true;
            switch (key)
            {
                case "score": value = data.score; break;
                case "income": value = data.income; break;
                case "IP": value = data.infinity.IP; break;
                case "infinities": value = data.infinity.infs; break;
                case "stars": value = data.infinity.stars; break;
                case "stardust": value = data.infinity.stardust; break;
                case "EP": value = data.eternity.EP; break;
                case "eternities": value = data.eternity.eters; break;
                case "DP": value = data.eternity.DP; break;
                case "AP": value = data.eternity.AP; break;
                case "RP": value = data.eternity.curRP; break;
                case "RPMax": value = data.eternity.maximumRP; break;
                case "RPSpent": value = data.eternity.spendRP; break;
                case "unities": value = data.unity.unities; break;
                case "passiveUnities": value = data.unity.passiveUnities; break;
                case "astrodust": value = data.unity.astrodust; break;
                case "singularities": value = data.singularity.singularity; break;
                case "atoms": value = data.singularity.atoms; break;
                case "PlP": value = data.plague.PlP; break;
                case "PlPperPlG": value = data.plague.PlPperPlG; break;
                case "PlG": value = data.plague.PlG; break;
                case "VE": value = data.plague.VE; break;
                case "ViP": value = data.plague.ViP; break;
                case "tarotSwords": value = data.tarot.swords; break;
                case "tarotWands": value = data.tarot.wands; break;
                case "tarotPentacles": value = data.tarot.pentacles; break;
                case "tarotCups": value = data.tarot.cups; break;
                case "goldTarotSwords": value = data.tarot.goldSwords; break;
                case "goldTarotWands": value = data.tarot.goldWands; break;
                case "goldTarotPentacles": value = data.tarot.goldPentacles; break;
                case "goldTarotCups": value = data.tarot.goldCups; break;
                case "tarotDraws": value = data.tarot.draws; break;
                case "timeSinceStart": value = data.timeSinceStart; break;
                case "timeInfinity": value = data.timeInf; break;
                case "timeEternity": value = data.timeEtr; break;
                case "timeUnity": value = data.timeUnity; break;
                case "timeTotal": value = data.timeTotal; break;
                default: found = false; break;
            }
            if (!found)
            {
                payload = Array.Empty<byte>();
                return false;
            }

            values.Add((key, value.Mantissa, value.Exponent));
        }
        payload = Encode(values);
        return true;
    }

    internal static byte[] Encode(IReadOnlyList<(string Key, double Mantissa, double Exponent)> values) => Encoding.UTF8.GetBytes(string.Concat("{", string.Join(",", values.Select(value => string.Concat("\"", value.Key, "\":\"", Format(value.Mantissa, value.Exponent), "\""))), "}"));

    internal static string Format(double mantissa, double exponent) => string.Concat(mantissa.ToString("R", CultureInfo.InvariantCulture), "e", exponent.ToString("R", CultureInfo.InvariantCulture));
}
