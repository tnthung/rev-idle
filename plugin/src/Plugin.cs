using BepInEx;
using BepInEx.Unity.IL2CPP;
using UnityEngine;

namespace RevIdle.ScoreTelemetry;

[BepInPlugin(PluginGuid, PluginName, PluginVersion)]
public sealed class Plugin : BasePlugin
{
    public const string PluginGuid = "dev.tnthung.revolutionidle.scoretelemetry";
    public const string PluginName = "Revolution Idle Score Telemetry";
    public const string PluginVersion = "0.1.0";

    private static UdpScorePublisher? _publisher;

    public override void Load()
    {
        string configuredPort = Config.Bind(
            "Network",
            "Port",
            "19841",
            "UDP destination port on 127.0.0.1. Set to 0 to disable.").Value;

        _publisher = UdpScorePublisher.Create(configuredPort);
        if (_publisher is not null)
            AddComponent<ScoreTicker>();
    }

    internal static void PublishScore()
    {
        try
        {
            GameData? data = GameController.data;
            if (data is null || _publisher is null)
                return;

            BigDouble score = data.score;
            _publisher.Publish(ScorePayload.Encode(score.Mantissa, score.Exponent));
        }
        catch
        {
        }
    }
}

public sealed class ScoreTicker : MonoBehaviour
{
    private const float IntervalSeconds = 0.05f;
    private float _elapsed;

    public ScoreTicker(IntPtr pointer) : base(pointer)
    {
    }

    public void Update()
    {
        if (AdvanceTimer(ref _elapsed, Time.unscaledDeltaTime))
            Plugin.PublishScore();
    }

    internal static bool AdvanceTimer(ref float elapsed, float delta)
    {
        elapsed += delta;
        if (elapsed + 0.000001f < IntervalSeconds)
            return false;

        elapsed %= IntervalSeconds;
        if (elapsed > IntervalSeconds - 0.000001f)
            elapsed = 0f;
        return true;
    }
}
