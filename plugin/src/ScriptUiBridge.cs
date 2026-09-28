namespace RevIdle.ScoreTelemetry;

internal sealed class ScriptUiBridge
{
    private readonly WsConnection _connection;
    private readonly Action<string>? _log;
    private readonly object _gate = new();
    private ScriptUiSnapshot? _snapshot;
    private long _snapshotGeneration;
    private long _highestGeneration;
    private ulong _revision;
    private long _observedConnectionGeneration;
    private ScriptUiOverlay? _overlay;
    private readonly Queue<(ScriptUiPointer Pointer, long Generation)> _pointers = new();

    internal ScriptUiBridge(WsConnection connection, Action<string>? log = null)
    {
        _connection = connection ?? throw new ArgumentNullException(nameof(connection));
        _log = log;
        connection.Handler<ScriptUiSnapshot>((context, packet) =>
        {
            if (context.CancellationToken.IsCancellationRequested ||
                context.Generation == 0 ||
                _connection.ConnectionGeneration != context.Generation)
                return Task.CompletedTask;
            AcceptSnapshot(packet, context.Generation);
            return Task.CompletedTask;
        });
        connection.Handler<ScriptUiPointer>((context, packet) =>
        {
            if (context.CancellationToken.IsCancellationRequested ||
                context.Generation == 0 ||
                _connection.ConnectionGeneration != context.Generation)
                return Task.CompletedTask;
            if (AcceptsPointer(packet, context.Generation))
            {
                lock (_gate)
                {
                    while (_pointers.Count >= 256)
                        _pointers.Dequeue();
                    _pointers.Enqueue((packet, context.Generation));
                }
            }
            return Task.CompletedTask;
        });
    }

    internal ScriptUiSnapshot? Snapshot
    {
        get
        {
            lock (_gate)
                return _snapshot is null ? null : Clone(_snapshot);
        }
    }

    internal ScriptUiSnapshot? CurrentSnapshot => Snapshot;

    internal long SnapshotGeneration
    {
        get
        {
            lock (_gate)
                return _snapshotGeneration;
        }
    }

    internal bool Connected => _connection.ConnectionGeneration != 0;

    internal bool EventsEnabled
        => EventsEnabledFor(_connection.ConnectionGeneration);

    internal void AttachOverlay(ScriptUiOverlay overlay)
    {
        lock (_gate)
            _overlay = overlay ?? throw new ArgumentNullException(nameof(overlay));
        ObserveConnectionGeneration();
    }

    internal void DetachOverlay(ScriptUiOverlay overlay)
    {
        lock (_gate)
        {
            if (ReferenceEquals(_overlay, overlay))
                _overlay = null;
        }
    }

    internal bool TryAcceptSnapshot(ScriptUiSnapshot snapshot, long generation)
        => AcceptSnapshot(snapshot, generation);

    internal bool SendEvent(ScriptUiEvent scriptEvent)
    {
        long generation = _connection.ConnectionGeneration;
        lock (_gate)
        {
            if (_snapshot is null ||
                generation == 0 ||
                _snapshotGeneration != generation ||
                _snapshot.SessionId is not Guid sessionId ||
                scriptEvent.SessionId != sessionId)
                return false;
        }
        Task send;
        try
        {
            send = _connection.Send(scriptEvent, generation);
        }
        catch (Exception exception)
        {
            Report($"Script UI event send failed: {exception.Message}");
            return false;
        }
        _ = send.ContinueWith(completed =>
        {
            if (completed.IsFaulted)
                Report($"Script UI event send failed: {completed.Exception?.GetBaseException().Message}");
            else if (completed.IsCanceled)
                Report("Script UI event send was canceled.");
        }, TaskScheduler.Default);
        return true;
    }

    internal bool AcceptsPointer(ScriptUiPointer pointer, long generation)
    {
        if (!EventsEnabledFor(generation) ||
            pointer.SessionId == Guid.Empty ||
            pointer.Phase is not ("down" or "up") ||
            pointer.Width < 0 ||
            pointer.Height < 0 ||
            (pointer.Phase == "down" && (pointer.Width == 0 || pointer.Height == 0)))
            return false;
        lock (_gate)
            return _snapshot?.SessionId == pointer.SessionId && _snapshotGeneration == generation;
    }

    internal void FlushPointers()
    {
        ScriptUiOverlay? overlay;
        (ScriptUiPointer Pointer, long Generation)[] pointers;
        lock (_gate)
        {
            overlay = _overlay;
            pointers = _pointers.ToArray();
            _pointers.Clear();
        }
        if (overlay is null)
            return;
        foreach ((ScriptUiPointer pointer, long generation) in pointers)
            if (AcceptsPointer(pointer, generation))
                overlay.HandlePointer(pointer, generation);
    }

    internal void ClearQueuedPointers()
    {
        lock (_gate)
            _pointers.Clear();
    }

    private bool EventsEnabledFor(long generation)
    {
        ObserveConnectionGeneration();
        if (generation == 0 || _connection.ConnectionGeneration != generation)
            return false;
        lock (_gate)
            return _snapshot is not null && _snapshotGeneration == generation;
    }

    private void ObserveConnectionGeneration()
    {
        long generation = _connection.ConnectionGeneration;
        ScriptUiOverlay? overlay = null;
        lock (_gate)
        {
            if (generation != _observedConnectionGeneration)
            {
                _observedConnectionGeneration = generation;
                if (generation == 0 || generation != _snapshotGeneration)
                {
                    overlay = _overlay;
                    _pointers.Clear();
                }
            }
        }
        overlay?.ClearPendingPointers();
    }

    private bool AcceptSnapshot(ScriptUiSnapshot? snapshot, long generation)
    {
        ObserveConnectionGeneration();
        string? error = Validate(snapshot);
        if (error is not null)
        {
            Report($"Malformed Script UI snapshot: {error}");
            return false;
        }
        if (generation <= 0)
            return false;
        lock (_gate)
        {
            if (generation < _highestGeneration)
                return false;
            if (_snapshotGeneration == generation && snapshot!.Revision <= _revision)
                return false;
            if (_snapshotGeneration != generation || _snapshot?.SessionId != snapshot!.SessionId)
                _pointers.Clear();
            _highestGeneration = generation;
            _snapshotGeneration = generation;
            _revision = snapshot!.Revision;
            _snapshot = Clone(snapshot);
        }
        return true;
    }

    private static ScriptUiSnapshot Clone(ScriptUiSnapshot snapshot)
        => new(
            snapshot.SessionId,
            snapshot.Revision,
            snapshot.Elements.Select(element => new ScriptUiElementState(
                element.Id,
                element.InstanceId,
                element.EventsVersion,
                element.Text,
                element.AlignX,
                element.AlignY,
                element.PosX,
                element.PosY,
                new ScriptUiLengthState(element.LenX.Fixed, element.LenX.Min, element.LenX.Max),
                new ScriptUiLengthState(element.LenY.Fixed, element.LenY.Min, element.LenY.Max),
                element.Color.ToArray(),
                element.TextColor.ToArray(),
                new ScriptUiBorderState(element.Border.Thickness, element.Border.Color.ToArray()),
                new ScriptUiCornerState(element.Corner.TopLeft, element.Corner.TopRight, element.Corner.BottomLeft, element.Corner.BottomRight),
                new ScriptUiPaddingState(element.Padding.Top, element.Padding.Right, element.Padding.Bottom, element.Padding.Left),
                element.Events.ToArray())).ToArray());

    private static string? Validate(ScriptUiSnapshot? snapshot)
    {
        if (snapshot is null)
            return "payload is null";
        if (snapshot.SessionId is null && snapshot.Elements is { Length: > 0 })
            return "a null session requires an empty element list";
        if (snapshot.SessionId is Guid sessionId && sessionId == Guid.Empty)
            return "sessionId is empty";
        if (snapshot.Elements is null)
            return "elements is null";
        HashSet<string> ids = new(StringComparer.Ordinal);
        HashSet<Guid> instances = new();
        foreach (ScriptUiElementState? element in snapshot.Elements)
        {
            if (element is null)
                return "element is null";
            if (string.IsNullOrEmpty(element.Id) || !ids.Add(element.Id))
                return "element ids must be nonempty and unique";
            if (element.InstanceId == Guid.Empty || !instances.Add(element.InstanceId))
                return $"element '{element.Id}' has an empty or duplicate instanceId";
            if (element.Text is null || !double.IsFinite(element.PosX) || !double.IsFinite(element.PosY))
                return $"element '{element.Id}' has invalid text or position";
            if (element.AlignX is not ("left" or "center" or "right") || element.AlignY is not ("top" or "center" or "bottom"))
                return $"element '{element.Id}' has invalid alignment";
            string? lengthError = ValidateLength(element.LenX, $"{element.Id}.lenX") ?? ValidateLength(element.LenY, $"{element.Id}.lenY");
            if (lengthError is not null)
                return lengthError;
            if (!ValidateColor(element.Color) || !ValidateColor(element.TextColor))
                return $"element '{element.Id}' has invalid color";
            if (element.Border is null || !double.IsFinite(element.Border.Thickness) || element.Border.Thickness < 0 || !ValidateColor(element.Border.Color))
                return $"element '{element.Id}' has invalid border";
            if (element.Corner is null ||
                !ValidNonNegative(element.Corner.TopLeft) ||
                !ValidNonNegative(element.Corner.TopRight) ||
                !ValidNonNegative(element.Corner.BottomLeft) ||
                !ValidNonNegative(element.Corner.BottomRight))
                return $"element '{element.Id}' has invalid corner";
            if (element.Padding is null ||
                !ValidNonNegative(element.Padding.Top) ||
                !ValidNonNegative(element.Padding.Right) ||
                !ValidNonNegative(element.Padding.Bottom) ||
                !ValidNonNegative(element.Padding.Left))
                return $"element '{element.Id}' has invalid padding";
            if (element.Events is null)
                return $"element '{element.Id}' has null events";
            HashSet<string> events = new(StringComparer.Ordinal);
            foreach (string? eventName in element.Events)
            {
                if (eventName is null || eventName is not ("hover" or "leave" or "click") || !events.Add(eventName))
                    return $"element '{element.Id}' has invalid events";
            }
        }
        return null;
    }

    private static string? ValidateLength(ScriptUiLengthState? length, string name)
    {
        if (length is null ||
            !ValidNonNegative(length.Min) ||
            (length.Max is double max && (!double.IsFinite(max) || max < length.Min)) ||
            (length.Fixed is double fixedLength && (!double.IsFinite(fixedLength) || fixedLength < 0)))
            return $"{name} is invalid";
        return null;
    }

    private static bool ValidateColor(byte[]? color)
        => color is { Length: 4 };

    private static bool ValidNonNegative(double value)
        => double.IsFinite(value) && value >= 0;

    private void Report(string message)
    {
        try { _log?.Invoke(message); } catch { }
    }
}
