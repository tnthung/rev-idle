using BepInEx;
using BepInEx.Logging;
using BepInEx.Unity.IL2CPP;
using System.Text.Json;
using UnityEngine;

namespace RevIdle.ScoreTelemetry;

[BepInPlugin(PluginGuid, PluginName, PluginVersion)]
public sealed class Plugin : BasePlugin
{
    public const string PluginGuid = "dev.tnthung.revolutionidle.scoretelemetry";
    public const string PluginName = "Revolution Idle Score Telemetry";
    public const string PluginVersion = "0.1.0";

    private static WsConnection? _connection;
    private static ManualLogSource? _logger;

    public override void Load()
    {
        _logger = Log;

        string configuredPort = Config.Bind(
            "Network",
            "Port",
            "19841",
            "Raw packet server port on 127.0.0.1. Set to 0 to disable.").Value;

        _connection = WsConnection.Create(configuredPort, message => _logger?.LogError($"[WebSocket] {message}"));
        AddComponent<ScoreTicker>();
    }

    internal static void LogBridgeInfo(string message) => _logger?.LogInfo($"[InputBridge] {message}");

    internal static void LogBridgeError(string message) => _logger?.LogError($"[InputBridge] {message}");

    internal static void PumpPackets()
    {
        try
        {
            _connection?.Pump();
        }
        catch (Exception exception)
        {
            _logger?.LogError($"Packet dispatch failed: {exception}");
        }
    }

    internal static bool CompletePending(HttpScoreServer server, Func<object?> getData, nint window = 0) => server.CompletePendingRequest(request =>
    {
        if (request.Kind == HttpScoreServer.RequestKind.State)
        {
            object? data;
            try { data = getData(); }
            catch (Exception exception)
            {
                _logger?.LogError($"State data access failed: {exception}");
                return (500, Array.Empty<byte>());
            }
            if (data is null)
                return (503, Array.Empty<byte>());
            StatePayloadStatus status = StatePayload.Encode(data, request.Keys, out byte[] payload);
            if (status == StatePayloadStatus.SerializationFailure)
                _logger?.LogError("State serialization failed.");
            return status switch
            {
                StatePayloadStatus.Success => (200, payload),
                StatePayloadStatus.InvalidPath => (400, Array.Empty<byte>()),
                _ => (500, Array.Empty<byte>())
            };
        }

        if (request.Kind == HttpScoreServer.RequestKind.Invoke)
        {
            if (!UnityUiClickDispatcher.TryInvoke(request.Path!, out string error))
                return (400, JsonSerializer.SerializeToUtf8Bytes(new { error }));
            return (200, System.Text.Encoding.UTF8.GetBytes("{}"));
        }

        if (request.Kind == HttpScoreServer.RequestKind.Transfer)
        {
            if (!UnityUiClickDispatcher.TryTransfer(request.Path!, request.Destination!, out string error))
                return (400, JsonSerializer.SerializeToUtf8Bytes(new { error }));
            return (200, System.Text.Encoding.UTF8.GetBytes("{}"));
        }

        if (!UnityUiClickDispatcher.TryCapture(window, request.X, request.Y, out string? type, out string? path, out string captureResult))
            return (400, JsonSerializer.SerializeToUtf8Bytes(new { error = captureResult }));
        return (200, JsonSerializer.SerializeToUtf8Bytes(new { type, path }));
    });

    internal static void StopServer() => _connection?.Dispose();
}

public sealed class ScoreTicker : MonoBehaviour
{
    private const float BridgeRetrySeconds = 1f;
    private const int MaxClicksPerFrame = 32;
    private const int MaxScrollsPerFrame = 32;
    private const int MaxDragsPerFrame = 32;

    private float _bridgeRetryRemaining;
    private readonly ClickCommandQueue _clicks = new();
    private readonly ScrollCommandQueue _scrolls = new();
    private readonly DragCommandQueue _drags = new();
    private Win32InputBridge? _bridge;

    public ScoreTicker(IntPtr pointer) : base(pointer)
    {
    }

    public void Update()
    {
        float delta = Time.unscaledDeltaTime;
        TryAttachBridge(delta);
        DispatchQueuedClicks();
        DispatchQueuedScrolls();
        DispatchQueuedDrags();

        Plugin.PumpPackets();
    }

    public void OnDestroy()
    {
        StopBridge();
        Plugin.StopServer();
    }

    public void OnApplicationQuit()
    {
        StopBridge();
        Plugin.StopServer();
    }

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

        var bridge = new Win32InputBridge(_clicks, _scrolls, _drags);
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

    private void DispatchQueuedScrolls()
    {
        Win32InputBridge? bridge = _bridge;
        if (bridge is null || !bridge.IsAttached)
            return;

        int processed = 0;
        while (processed < MaxScrollsPerFrame && _scrolls.TryDequeue(out ScrollCommand command))
        {
            try
            {
                if (UnityUiClickDispatcher.TryDispatchScroll(bridge.Window, command, out string result))
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

    private void DispatchQueuedDrags()
    {
        Win32InputBridge? bridge = _bridge;
        if (bridge is null || !bridge.IsAttached)
            return;

        int processed = 0;
        while (processed < MaxDragsPerFrame && _drags.TryDequeue(out DragCommand command))
        {
            try
            {
                if (UnityUiClickDispatcher.TryDispatchDrag(bridge.Window, command, out string result))
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
        _scrolls.Clear();
        _drags.Clear();
    }

}
