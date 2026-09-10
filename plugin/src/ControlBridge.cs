namespace RevIdle.ScoreTelemetry;

internal enum ControlCommand
{
    Reload,
    Stop,
    Resume,
    Pause,
    Capture
}

internal enum ScriptPhase
{
    Unloaded,
    Stopped,
    Running,
    Paused
}

internal readonly record struct ControlState(ScriptPhase Phase, bool Capture);

internal sealed class ControlBridge
{
    private readonly WsConnection _connection;
    private readonly Action<string>? _log;
    private readonly object _gate = new();
    private ControlState? _state;
    private long _stateGeneration;

    internal ControlBridge(WsConnection connection, Action<string>? log = null)
    {
        _connection = connection ?? throw new ArgumentNullException(nameof(connection));
        _log = log;
        connection.Handler<StateUpdate>((context, packet) =>
        {
            ScriptPhase? phase = packet.Phase switch
            {
                "unloaded" => ScriptPhase.Unloaded,
                "stopped" => ScriptPhase.Stopped,
                "running" => ScriptPhase.Running,
                "paused" => ScriptPhase.Paused,
                _ => null
            };
            if (phase is null)
                return Task.CompletedTask;

            if (context.CancellationToken.IsCancellationRequested ||
                context.Generation == 0 ||
                _connection.ConnectionGeneration != context.Generation)
                return Task.CompletedTask;
            lock (_gate)
            {
                if (context.CancellationToken.IsCancellationRequested ||
                    _connection.ConnectionGeneration != context.Generation)
                    return Task.CompletedTask;
                _state = new ControlState(phase.Value, packet.Capture);
                _stateGeneration = context.Generation;
            }
            return Task.CompletedTask;
        });
    }

    internal bool Connected
        => _connection.ConnectionGeneration != 0;

    internal ControlState? State
    {
        get
        {
            long generation = _connection.ConnectionGeneration;
            lock (_gate)
            {
                if (generation == 0 || _stateGeneration != generation)
                {
                    _state = null;
                    _stateGeneration = 0;
                    return null;
                }
                return _state;
            }
        }
    }

    internal void Send(ControlCommand command)
    {
        long expectedGeneration;
        lock (_gate)
        {
            if (_state is null)
            {
                Report("Control notification ignored because state is unavailable.");
                return;
            }
            expectedGeneration = _stateGeneration;
        }
        object packet = command switch
        {
            ControlCommand.Reload => new ReloadScript(),
            ControlCommand.Stop => new StopScript(),
            ControlCommand.Resume => new ResumeScript(),
            ControlCommand.Pause => new PauseScript(),
            ControlCommand.Capture => new StartCapture(),
            _ => throw new ArgumentOutOfRangeException(nameof(command))
        };
        Task send;
        try
        {
            send = _connection.Send(packet, expectedGeneration);
        }
        catch (Exception exception)
        {
            Report($"Control notification send failed: {exception.Message}");
            return;
        }
        _ = send.ContinueWith(completed =>
        {
            if (completed.IsFaulted)
                Report($"Control notification send failed: {completed.Exception?.GetBaseException().Message}");
            else if (completed.IsCanceled)
                Report("Control notification send was canceled.");
        }, TaskScheduler.Default);
    }

    private void Report(string message)
    {
        try { _log?.Invoke(message); } catch { }
    }
}
