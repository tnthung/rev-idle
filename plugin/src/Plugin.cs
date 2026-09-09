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
    private static nint _window;

    public override void Load()
    {
        _logger = Log;

        string configuredPort = Config.Bind(
            "Network",
            "Port",
            "19841",
            "Raw packet server port on 127.0.0.1. Set to 0 to disable.").Value;

        _connection = WsConnection.Create(configuredPort, message => _logger?.LogError($"[WebSocket] {message}"));
        if (_connection is not null)
            RegisterHandlers(_connection, () => GameController.data, () => _window);
        AddComponent<ScoreTicker>();
    }

    internal static void LogBridgeInfo(string message) => _logger?.LogInfo($"[InputBridge] {message}");

    internal static void LogBridgeError(string message) => _logger?.LogError($"[InputBridge] {message}");

    internal static void RegisterHandlers(
        WsConnection connection,
        Func<object?> getData,
        Func<nint> getWindow)
        => RegisterHandlers(
            connection,
            getData,
            getWindow,
            (window, x, y) =>
            {
                bool success = UnityUiClickDispatcher.TryCapture(window, x, y, out string? type, out string? path, out string error);
                return (success, type, path, error);
            },
            path =>
            {
                bool success = UnityUiClickDispatcher.TryInvoke(path, out string error);
                return (success, error);
            },
            (source, destination) =>
            {
                bool success = UnityUiClickDispatcher.TryTransfer(source, destination, out string error);
                return (success, error);
            });

    internal static void RegisterHandlers(
        WsConnection connection,
        Func<object?> getData,
        Func<nint> getWindow,
        Func<nint, int, int, (bool Success, string? Type, string? Path, string Error)> capture,
        Func<string, (bool Success, string Error)> invoke,
        Func<string, string, (bool Success, string Error)> transfer)
    {
        _connection = connection;
        connection.Handler<StateReq>(async (context, packet) =>
        {
            object? data = getData();
            if (data is null)
                throw new InvalidOperationException("State data is unavailable.");

            StatePayloadStatus status = StatePayload.Encode(data, packet.Keys, out byte[] payload);
            if (status == StatePayloadStatus.InvalidPath)
                throw new InvalidOperationException("State path is invalid.");
            if (status == StatePayloadStatus.SerializationFailure)
                throw new InvalidOperationException("State serialization failed.");

            using JsonDocument document = JsonDocument.Parse(payload);
            JsonElement value = document.RootElement.Clone();
            await context.Send(new StateRes(value));
        });
        connection.Handler<CaptureReq>(async (context, packet) =>
        {
            nint window = getWindow();
            (bool success, string? type, string? path, string error) = capture(window, packet.X, packet.Y);
            if (!success)
                throw new InvalidOperationException(error);
            await context.Send(new CaptureRes(type, path));
        });
        connection.Handler<InvokeReq>(async (context, packet) =>
        {
            (bool success, string error) = invoke(packet.Path);
            if (!success)
                throw new InvalidOperationException(error);
            await context.Send(new InvokeRes());
        });
        connection.Handler<TransferReq>(async (context, packet) =>
        {
            (bool success, string error) = transfer(packet.Source, packet.Destination);
            if (!success)
                throw new InvalidOperationException(error);
            await context.Send(new TransferRes());
        });
    }

    internal static void PumpPackets(nint window)
    {
        _window = window;
        try
        {
            _connection?.Pump();
        }
        catch (Exception exception)
        {
            _logger?.LogError($"Packet dispatch failed: {exception}");
        }
    }

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

        Plugin.PumpPackets(_bridge?.Window ?? 0);
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
