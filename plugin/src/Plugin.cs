using BepInEx;
using BepInEx.Logging;
using BepInEx.Unity.IL2CPP;
using System.Text.Json;
using UnityEngine;

namespace RevIdle.ScoreTelemetry;

[BepInPlugin(PluginGuid, PluginName, PluginVersion)]
public sealed class Plugin : BasePlugin
{
    private const int MaxPacketsPerFrame = 32;
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
        if (_connection is not null)
            RegisterHandlers(_connection, () => GameController.data, () => 0);
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
            (window, x, y, width, height) =>
            {
                bool success = UnityUiClickDispatcher.TryCapture(x, y, width, height, out string? type, out string? path, out string error);
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
            },
            (window, command) =>
            {
                bool success = UnityUiClickDispatcher.TryDispatch(command, out string error);
                return (success, error);
            },
            (window, command) =>
            {
                bool success = UnityUiClickDispatcher.TryDispatchScroll(command, out string error);
                return (success, error);
            },
            (window, command) =>
            {
                bool success = UnityUiClickDispatcher.TryDispatchDrag(command, out string error);
                return (success, error);
            });

    internal static void RegisterHandlers(
        WsConnection connection,
        Func<object?> getData,
        Func<nint> getWindow,
        Func<nint, int, int, int, int, (bool Success, string? Type, string? Path, string Error)> capture,
        Func<string, (bool Success, string Error)> invoke,
        Func<string, string, (bool Success, string Error)> transfer,
        Func<nint, ClickCommand, (bool Success, string Error)> click,
        Func<nint, ScrollCommand, (bool Success, string Error)> scroll,
        Func<nint, DragCommand, (bool Success, string Error)> drag)
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
            (bool success, string? type, string? path, string error) = capture(window, packet.X, packet.Y, packet.Width, packet.Height);
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
        connection.Handler<ClickCommand>((context, packet) =>
        {
            (bool success, string error) = click(getWindow(), packet);
            if (!success)
                throw new InvalidOperationException(error);
            LogBridgeInfo(error);
            return Task.CompletedTask;
        });
        connection.Handler<ScrollCommand>((context, packet) =>
        {
            (bool success, string error) = scroll(getWindow(), packet);
            if (!success)
                throw new InvalidOperationException(error);
            LogBridgeInfo(error);
            return Task.CompletedTask;
        });
        connection.Handler<DragCommand>((context, packet) =>
        {
            (bool success, string error) = drag(getWindow(), packet);
            if (!success)
                throw new InvalidOperationException(error);
            LogBridgeInfo(error);
            return Task.CompletedTask;
        });
    }

    internal static void PumpPackets(nint _)
    {
        try
        {
            for (int index = 0; index < MaxPacketsPerFrame; index++)
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
    public ScoreTicker(IntPtr pointer) : base(pointer)
    {
    }

    public void Update()
    {
        Plugin.PumpPackets(0);
    }

    public void OnDestroy()
    {
        Plugin.StopServer();
    }

    public void OnApplicationQuit()
    {
        Plugin.StopServer();
    }
}
