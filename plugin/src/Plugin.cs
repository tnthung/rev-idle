using BepInEx;
using BepInEx.Logging;
using BepInEx.Unity.IL2CPP;
using System.Diagnostics;
using System.Runtime.InteropServices;
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

    [DllImport("kernel32.dll")]
    internal static extern nint GetConsoleWindow();

    [DllImport("user32.dll")]
    internal static extern bool ShowWindow(nint window, int command);

    private static WsConnection? _connection;
    private static ControlBridge? _controlBridge;
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
        {
            RegisterHandlers(_connection, () => GameController.data, () => 0);
            _controlBridge = new ControlBridge(_connection, LogBridgeError);
            ClientProcess.Start(Paths.GameRootPath, _connection.Port, message => _logger?.LogInfo($"[Client] {message}"), message => _logger?.LogError($"[Client] {message}"));
        }
        AddComponent<ScoreTicker>();
        nint consoleWindow = GetConsoleWindow();
        if (consoleWindow != 0)
            ShowWindow(consoleWindow, 0);
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
                bool success = UnityUiClickDispatcher.TryFindUiPath(x, y, width, height, out string? type, out string? path, out string error);
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
            },
            (window, command) =>
            {
                bool success = KeyboardInput.TryDispatch(command, out string error);
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
        Func<nint, DragCommand, (bool Success, string Error)> drag,
        Func<nint, PressCommand, (bool Success, string Error)>? press = null)
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
        connection.Handler<UiPathReq>(async (context, packet) =>
        {
            nint window = getWindow();
            (bool success, string? type, string? path, string error) = capture(window, packet.X, packet.Y, packet.Width, packet.Height);
            if (!success)
                throw new InvalidOperationException(error);
            await context.Send(new UiPathRes(type, path));
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
        connection.Handler<PressCommand>((context, packet) =>
        {
            (bool success, string error) = (press ?? ((_, _) => (false, "keyboard input is not supported")))(getWindow(), packet);
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

    internal static void StopClient() => ClientProcess.Stop();

    internal static ControlBridge? ControlBridge
        => _controlBridge;
}

internal static class ClientProcess
{
    private static readonly object Sync = new();
    private static Process? _process;

    internal static ProcessStartInfo BuildStartInfo(string gameRoot, int port)
        => new()
        {
            FileName = Path.Combine(gameRoot, "client.exe"),
            WorkingDirectory = gameRoot,
            Arguments = $"--port {port} --non-interactable",
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };

    internal static void Start(string gameRoot, int port, Action<string> logInfo, Action<string> logError)
    {
        Process? process = null;
        try
        {
            process = new() { StartInfo = BuildStartInfo(gameRoot, port) };
            process.OutputDataReceived += (_, eventArgs) =>
            {
                if (eventArgs.Data is not null)
                    logInfo(eventArgs.Data);
            };
            process.ErrorDataReceived += (_, eventArgs) =>
            {
                if (eventArgs.Data is not null)
                    logError(eventArgs.Data);
            };
            lock (Sync)
            {
                if (!process.Start())
                {
                    process.Dispose();
                    logError("Failed to start client.exe.");
                    return;
                }
                _process = process;
            }
            process.BeginOutputReadLine();
            process.BeginErrorReadLine();
        }
        catch (Exception exception)
        {
            lock (Sync)
            {
                if (ReferenceEquals(_process, process))
                    _process = null;
            }
            try
            {
                if (process is not null && !process.HasExited)
                    process.Kill();
            }
            catch (InvalidOperationException)
            {
            }
            process?.Dispose();
            logError($"Failed to start client.exe: {exception.Message}");
        }
    }

    internal static void Stop()
    {
        Process? process;
        lock (Sync)
        {
            process = _process;
            _process = null;
        }
        if (process is null)
            return;
        try
        {
            if (!process.HasExited)
                process.Kill();
        }
        catch (InvalidOperationException)
        {
        }
        finally
        {
            process.Dispose();
        }
    }
}

public sealed class ScoreTicker : MonoBehaviour
{
    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(nint window);

    private ControlOverlay? _controlOverlay;

    public ScoreTicker(IntPtr pointer) : base(pointer)
    {
    }

    public void Update()
    {
        if (Application.isFocused && Input.GetKeyDown(KeyCode.F7))
        {
            nint consoleWindow = Plugin.GetConsoleWindow();
            if (consoleWindow != 0)
                Plugin.ShowWindow(consoleWindow, IsWindowVisible(consoleWindow) ? 0 : 4);
        }
        Plugin.PumpPackets(0);
        if (Plugin.ControlBridge is not ControlBridge bridge)
            return;
        _controlOverlay ??= ControlOverlay.Create(bridge.Send, bridge.Load);
        _controlOverlay?.Apply(bridge.State);
    }

    public void OnDestroy()
    {
        _controlOverlay?.Dispose();
        _controlOverlay = null;
        Plugin.StopClient();
        Plugin.StopServer();
    }

    public void OnApplicationQuit()
    {
        Plugin.StopClient();
        Plugin.StopServer();
    }
}
