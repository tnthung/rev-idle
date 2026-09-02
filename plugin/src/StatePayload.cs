using System.Globalization;
using System.Text;

namespace RevIdle.ScoreTelemetry;

internal static class StatePayload
{
    public static bool TryEncode(GameData data, IReadOnlyList<string> keys, out byte[] payload)
    {
        var values = new List<string>(keys.Count);
        dynamic game = data;
        foreach (string key in keys)
        {
            object? value = key switch
            {
                "score" => game.score,
                "income" => game.income,
                "IP" => game.infinity.IP,
                "infinities" => game.infinity.infs,
                "stars" => game.infinity.stars,
                "stardust" => game.infinity.stardust,
                "EP" => game.eternity.EP,
                "eternities" => game.eternity.eters,
                "DP" => game.eternity.DP,
                "AP" => game.eternity.AP,
                "RP" => game.eternity.curRP,
                "RPMax" => game.eternity.maximumRP,
                "RPSpent" => game.eternity.spendRP,
                "unities" => game.unity.unities,
                "passiveUnities" => game.unity.passiveUnities,
                "astrodust" => game.unity.astrodust,
                "singularities" => game.singularity.singularity,
                "atoms" => game.singularity.atoms,
                "PlP" => game.plague.PlP,
                "PlPperPlG" => game.plague.PlPperPlG,
                "PlG" => game.plague.PlG,
                "VE" => game.plague.VE,
                "ViP" => game.plague.ViP,
                "tarotSwords" => game.tarot.swords,
                "tarotWands" => game.tarot.wands,
                "tarotPentacles" => game.tarot.pentacles,
                "tarotCups" => game.tarot.cups,
                "goldTarotSwords" => game.tarot.goldSwords,
                "goldTarotWands" => game.tarot.goldWands,
                "goldTarotPentacles" => game.tarot.goldPentacles,
                "goldTarotCups" => game.tarot.goldCups,
                "tarotDraws" => game.tarot.draws,
                "timeSinceStart" => game.timeSinceStart,
                "timeInfinity" => game.timeInfinity,
                "timeEternity" => game.timeEternity,
                "timeUnity" => game.timeUnity,
                "timeTotal" => game.timeTotal,
                _ => null
            };
            if (value is null) { payload = Array.Empty<byte>(); return false; }
            dynamic dynamicValue = value;
            values.Add(value is double number ? number.ToString("R", CultureInfo.InvariantCulture) : string.Concat(dynamicValue.Mantissa.ToString("R", CultureInfo.InvariantCulture), "e", dynamicValue.Exponent.ToString("R", CultureInfo.InvariantCulture)));
        }
        payload = Encoding.UTF8.GetBytes(string.Concat("{", string.Join(",", keys.Select((key, index) => string.Concat("\"", key, "\":\"", values[index], "\""))), "}"));
        return true;
    }
}
