namespace RevIdle.ScoreTelemetry;

internal sealed class ClickCommandQueue
{
    private const int MaxPending = 32;
    private const int MaxRecentAccepted = 256;

    private readonly Queue<ClickCommand> pending = new();
    private readonly HashSet<ulong> acceptedRequestIds = new();
    private readonly Queue<ulong> recentAcceptedRequestIds = new();

    internal bool TryEnqueue(ClickCommand command)
    {
        if (command.RequestId == 0 || pending.Count >= MaxPending || acceptedRequestIds.Contains(command.RequestId))
            return false;

        pending.Enqueue(command);
        acceptedRequestIds.Add(command.RequestId);
        recentAcceptedRequestIds.Enqueue(command.RequestId);
        if (recentAcceptedRequestIds.Count > MaxRecentAccepted)
            acceptedRequestIds.Remove(recentAcceptedRequestIds.Dequeue());
        return true;
    }

    internal bool TryDequeue(out ClickCommand command)
    {
        if (pending.Count == 0)
        {
            command = default;
            return false;
        }

        command = pending.Dequeue();
        return true;
    }

    internal void Clear()
    {
        pending.Clear();
        acceptedRequestIds.Clear();
        recentAcceptedRequestIds.Clear();
    }
}

internal sealed class ScrollCommandQueue
{
    private const int MaxPending = 32;
    private const int MaxRecentAccepted = 256;
    private readonly Queue<ScrollCommand> pending = new();
    private readonly HashSet<ulong> acceptedRequestIds = new();
    private readonly Queue<ulong> recentAcceptedRequestIds = new();

    internal bool TryEnqueue(ScrollCommand command)
    {
        if (command.RequestId == 0 || pending.Count >= MaxPending || !acceptedRequestIds.Add(command.RequestId))
            return false;

        pending.Enqueue(command);
        recentAcceptedRequestIds.Enqueue(command.RequestId);
        if (recentAcceptedRequestIds.Count > MaxRecentAccepted)
            acceptedRequestIds.Remove(recentAcceptedRequestIds.Dequeue());
        return true;
    }

    internal bool TryDequeue(out ScrollCommand command)
    {
        if (pending.Count == 0)
        {
            command = default;
            return false;
        }

        command = pending.Dequeue();
        return true;
    }

    internal void Clear()
    {
        pending.Clear();
        acceptedRequestIds.Clear();
        recentAcceptedRequestIds.Clear();
    }
}
