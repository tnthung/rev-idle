using System.Runtime.InteropServices;
using System.Text;

namespace RevIdle.ScoreTelemetry;

internal sealed class Win32InputBridge : IDisposable
{
    private const uint WmNcDestroy = 0x0082;
    internal const nuint SubclassId = 1;
    private const string ConsoleWindowClass = "ConsoleWindowClass";

    private static readonly SubclassProc SubclassCallback = WindowProcedure;
    private static Win32InputBridge? _active;

    private readonly ClickCommandQueue _queue;
    private nint _window;
    private bool _disposed;

    internal Win32InputBridge(ClickCommandQueue queue) => _queue = queue;

    internal nint Window => _window;
    internal bool IsAttached => _window != 0;

    internal static bool IsCallbackIdentityValid(
        bool disposed,
        nint callbackWindow,
        nint activeWindow,
        nuint callbackSubclassId) =>
        !disposed && callbackWindow == activeWindow && callbackSubclassId == SubclassId;

    internal static bool CanReleaseManagedOwnership(
        bool windowDestroyed,
        bool currentThreadOwnsWindow,
        bool removeSucceeded) =>
        windowDestroyed || currentThreadOwnsWindow && removeSucceeded;

    internal bool TryAttach()
    {
        if (_disposed || _window != 0 || _active is not null)
            return false;

        try
        {
            var candidates = new List<nint>();
            if (!EnumWindows(
                    (window, _) =>
                    {
                        if (!IsWindowVisible(window))
                            return true;

                        uint processId;
                        if (GetWindowThreadProcessId(window, out processId) == 0 ||
                            processId != (uint)Environment.ProcessId)
                        {
                            return true;
                        }

                        var className = new StringBuilder(256);
                        if (GetClassName(window, className, className.Capacity) == 0 ||
                            string.Equals(className.ToString(), ConsoleWindowClass, StringComparison.Ordinal))
                        {
                            return true;
                        }

                        if (!GetClientRect(window, out Rect client) ||
                            client.Right <= client.Left ||
                            client.Bottom <= client.Top)
                        {
                            return true;
                        }

                        candidates.Add(window);
                        return true;
                    },
                    0))
            {
                return false;
            }

            if (candidates.Count != 1)
                return false;

            nint window = candidates[0];
            uint ownerThreadId = GetWindowThreadProcessId(window, out _);
            if (ownerThreadId == 0 || ownerThreadId != GetCurrentThreadId())
                return false;

            if (!SetWindowSubclass(window, SubclassCallback, SubclassId, 0))
                return false;

            _window = window;
            _active = this;
            return true;
        }
        catch
        {
            return false;
        }
    }

    public void Dispose()
    {
        if (_disposed)
            return;

        _disposed = true;
        _queue.Clear();

        nint window = _window;
        if (window == 0)
        {
            if (ReferenceEquals(_active, this))
                _active = null;
            return;
        }

        bool windowDestroyed = false;
        bool currentThreadOwnsWindow = false;
        bool removeSucceeded = false;
        try
        {
            windowDestroyed = !IsWindow(window);
            if (!windowDestroyed)
            {
                uint ownerThreadId = GetWindowThreadProcessId(window, out _);
                currentThreadOwnsWindow = ownerThreadId != 0 && ownerThreadId == GetCurrentThreadId();
                if (currentThreadOwnsWindow)
                    removeSucceeded = RemoveWindowSubclass(window, SubclassCallback, SubclassId);
            }
        }
        catch
        {
            return;
        }

        if (!CanReleaseManagedOwnership(windowDestroyed, currentThreadOwnsWindow, removeSucceeded))
            return;

        _window = 0;
        if (ReferenceEquals(_active, this))
            _active = null;
    }

    private static nint WindowProcedure(
        nint window,
        uint message,
        nuint wParam,
        nint lParam,
        nuint subclassId,
        nuint referenceData)
    {
        try
        {
            if (message == InputBridgeProtocol.MessageId &&
                _active is Win32InputBridge bridge &&
                IsCallbackIdentityValid(bridge._disposed, window, bridge._window, subclassId) &&
                InputBridgeProtocol.TryDecode(wParam, lParam, out ClickCommand command))
            {
                bridge._queue.TryEnqueue(command);
                return 0;
            }
        }
        catch
        {
        }

        nint result;
        try
        {
            result = DefSubclassProc(window, message, wParam, lParam);
        }
        catch
        {
            result = 0;
        }

        if (message == WmNcDestroy &&
            _active is Win32InputBridge destroyedBridge &&
            destroyedBridge._window == window &&
            subclassId == SubclassId)
        {
            destroyedBridge._window = 0;
            _active = null;
        }

        return result;
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool EnumWindows(EnumWindowsProc callback, nint lParam);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool IsWindowVisible(nint window);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool IsWindow(nint window);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint GetWindowThreadProcessId(nint window, out uint processId);

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern int GetClassName(nint window, StringBuilder className, int maxCount);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool GetClientRect(nint window, out Rect client);

    [DllImport("kernel32.dll")]
    private static extern uint GetCurrentThreadId();

    [DllImport("comctl32.dll", SetLastError = true)]
    private static extern bool SetWindowSubclass(
        nint window,
        SubclassProc callback,
        nuint subclassId,
        nuint referenceData);

    [DllImport("comctl32.dll", SetLastError = true)]
    private static extern bool RemoveWindowSubclass(nint window, SubclassProc callback, nuint subclassId);

    [DllImport("comctl32.dll")]
    private static extern nint DefSubclassProc(nint window, uint message, nuint wParam, nint lParam);

    [StructLayout(LayoutKind.Sequential)]
    private struct Rect
    {
        internal int Left;
        internal int Top;
        internal int Right;
        internal int Bottom;
    }

    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    private delegate bool EnumWindowsProc(nint window, nint lParam);

    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    private delegate nint SubclassProc(
        nint window,
        uint message,
        nuint wParam,
        nint lParam,
        nuint subclassId,
        nuint referenceData);
}
