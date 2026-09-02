using BepInEx;
using BepInEx.Logging;
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
    private static ManualLogSource? _logger;

    public override void Load()
    {
        _logger = Log;

        string configuredPort = Config.Bind(
            "Network",
            "Port",
            "19841",
            "UDP destination port on 127.0.0.1. Set to 0 to disable.").Value;

        _publisher = UdpScorePublisher.Create(configuredPort);
        AddComponent<ScoreTicker>();
    }

    internal static void LogBridgeInfo(string message) => _logger?.LogInfo($"[InputBridge] {message}");

    internal static void LogBridgeError(string message) => _logger?.LogError($"[InputBridge] {message}");

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
    private const float BridgeRetrySeconds = 1f;
    private const int MaxClicksPerFrame = 32;

    private float _elapsed;
    private float _bridgeRetryRemaining;
    private readonly ClickCommandQueue _clicks = new();
    private Win32InputBridge? _bridge;

    public ScoreTicker(IntPtr pointer) : base(pointer)
    {
    }

    public void Update()
    {
        float delta = Time.unscaledDeltaTime;
        TryAttachBridge(delta);
        DispatchQueuedClicks();

        if (AdvanceTimer(ref _elapsed, delta))
            Plugin.PublishScore();
    }

    public void OnDestroy() => StopBridge();

    public void OnApplicationQuit() => StopBridge();

    private void TryAttachBridge(float delta)
    {
        if (_bridge is not null)
        {
            if (_bridge.IsAttached)
                return;

            _bridge.Dispose();
            _bridge = null;
            _bridgeRetryRemaining = BridgeRetrySeconds;
            return;
        }

        if (_bridgeRetryRemaining > 0f)
        {
            _bridgeRetryRemaining = MathF.Max(0f, _bridgeRetryRemaining - delta);
            return;
        }

        var bridge = new Win32InputBridge(_clicks);
        if (bridge.TryAttach())
        {
            _bridge = bridge;
            Plugin.LogBridgeInfo($"attached to HWND {bridge.Window}");
        }
        else
        {
            bridge.Dispose();
            _bridgeRetryRemaining = BridgeRetrySeconds;
        }
    }

    private void DispatchQueuedClicks()
    {
        Win32InputBridge? bridge = _bridge;
        if (bridge is null || !bridge.IsAttached)
            return;

        int processed = 0;
        while (processed < MaxClicksPerFrame && _clicks.TryDequeue(out ClickCommand command))
        {
            try
            {
                if (UnityUiClickDispatcher.TryDispatch(bridge.Window, command, out string result))
                    Plugin.LogBridgeInfo(result);
                else
                    Plugin.LogBridgeError(result);
            }
            catch (Exception exception)
            {
                Plugin.LogBridgeError($"request {command.RequestId} failed: {exception.Message}");
            }

            processed++;
        }
    }

    private void StopBridge()
    {
        _bridge?.Dispose();
        _bridge = null;
        _clicks.Clear();
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
