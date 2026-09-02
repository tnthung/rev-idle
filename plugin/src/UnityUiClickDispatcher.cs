using System.Runtime.InteropServices;
using UnityEngine;
using UnityEngine.EventSystems;

namespace RevIdle.ScoreTelemetry;

internal static class UnityUiClickDispatcher
{
    internal static bool TryDispatch(
        nint window,
        ClickCommand command,
        out string result)
    {
        if (window == 0 || !GetClientRect(window, out Rect client))
        {
            result = $"invalid bounds at ({command.X}, {command.Y})";
            return false;
        }

        int clientWidth = client.Right - client.Left;
        int clientHeight = client.Bottom - client.Top;
        if (!InputBridgeProtocol.TryMapToUnity(
                command,
                clientWidth,
                clientHeight,
                Screen.width,
                Screen.height,
                out float unityX,
                out float unityY))
        {
            result = $"invalid bounds at ({command.X}, {command.Y})";
            return false;
        }

        EventSystem? eventSystem = EventSystem.current;
        if (eventSystem is null)
        {
            result = "no EventSystem";
            return false;
        }

        var pointerData = new PointerEventData(eventSystem)
        {
            button = PointerEventData.InputButton.Left,
            clickCount = 1,
            clickTime = Time.unscaledTime,
            position = new Vector2(unityX, unityY)
        };

        var raycasts = new Il2CppSystem.Collections.Generic.List<RaycastResult>();
        eventSystem.RaycastAll(pointerData, raycasts);

        RaycastResult hit = default!;
        bool hasHit = false;
        for (int index = 0; index < raycasts.Count; index++)
        {
            RaycastResult candidate = raycasts[index];
            if (candidate.gameObject == null)
                continue;

            hit = candidate;
            hasHit = true;
            break;
        }

        if (!hasHit)
        {
            result = $"no raycast hit at ({command.X}, {command.Y})";
            return false;
        }

        GameObject hitObject = hit.gameObject!;
        pointerData.pointerCurrentRaycast = hit;
        pointerData.pointerPressRaycast = hit;

        ExecuteEvents.ExecuteHierarchy(hitObject, pointerData, ExecuteEvents.pointerDownHandler);
        ExecuteEvents.ExecuteHierarchy(hitObject, pointerData, ExecuteEvents.pointerUpHandler);
        GameObject? clickTarget = ExecuteEvents.GetEventHandler<IPointerClickHandler>(hitObject);
        if (clickTarget is null)
        {
            result = $"no click handler at ({command.X}, {command.Y})";
            return false;
        }

        ExecuteEvents.Execute(clickTarget, pointerData, ExecuteEvents.pointerClickHandler);
        result = $"clicked '{clickTarget.name}' at ({command.X}, {command.Y})";
        return true;
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool GetClientRect(nint window, out Rect client);

    [StructLayout(LayoutKind.Sequential)]
    private struct Rect
    {
        internal int Left;
        internal int Top;
        internal int Right;
        internal int Bottom;
    }
}
