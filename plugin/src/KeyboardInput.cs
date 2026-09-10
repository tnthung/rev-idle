using System.Runtime.InteropServices;

namespace RevIdle.ScoreTelemetry;

internal static class KeyboardInput
{
    private const uint KeyDown = 0x0100;
    private const uint KeyUp = 0x0101;

    internal static bool TryDispatch(PressCommand command, out string result)
    {
        nint window = GetActiveWindow();
        if (window == 0)
        {
            result = "game window is unavailable";
            return false;
        }

        return TryDispatch(
            command,
            window,
            (target, message, key, details) => PostMessageW(target, message, key, details),
            out result);
    }

    internal static bool TryDispatch(
        PressCommand command,
        nint window,
        Func<nint, uint, nuint, nint, bool> post,
        out string result)
    {
        string key = command.Key.ToLowerInvariant();
        uint virtualKey;
        if (key.Length == 1 && key[0] >= 'a' && key[0] <= 'z')
        {
            virtualKey = (uint)(key[0] - 'a' + 'A');
        }
        else if (key.Length == 1 && key[0] >= '0' && key[0] <= '9')
        {
            virtualKey = key[0];
        }
        else if ((key.Length == 2 || key.Length == 3) && key[0] == 'f' &&
            int.TryParse(key[1..], out int functionKey) && functionKey >= 1 && functionKey <= 12)
        {
            virtualKey = (uint)(0x70 + functionKey - 1);
        }
        else
        {
            virtualKey = key switch
            {
                "backspace" => 0x08,
                "tab" => 0x09,
                "enter" => 0x0D,
                "escape" => 0x1B,
                "space" => 0x20,
                "left" => 0x25,
                "up" => 0x26,
                "right" => 0x27,
                "down" => 0x28,
                _ => 0,
            };
        }

        if (virtualKey == 0)
        {
            result = $"unsupported key: {key}";
            return false;
        }
        uint details = 1 | MapVirtualKeyW(virtualKey, 0) << 16;
        if (key is "left" or "up" or "right" or "down")
        {
            details |= 1 << 24;
        }
        if (!post(window, KeyDown, virtualKey, unchecked((nint)details)))
        {
            result = $"failed to press '{key}'";
            return false;
        }
        if (!post(window, KeyUp, virtualKey, unchecked((nint)(details | 0xC0000000))))
        {
            result = $"failed to release '{key}'";
            return false;
        }

        result = $"pressed '{key}'";
        return true;
    }

    [DllImport("user32.dll")]
    private static extern nint GetActiveWindow();

    [DllImport("user32.dll")]
    private static extern uint MapVirtualKeyW(uint code, uint mapType);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool PostMessageW(nint window, uint message, nuint wParam, nint lParam);
}
