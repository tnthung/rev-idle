using System.Runtime.InteropServices;
using UnityEngine;
using UnityEngine.EventSystems;

namespace RevIdle.ScoreTelemetry;

internal static class UnityUiClickDispatcher
{
    internal static int FindFirstClickableIndex(IReadOnlyList<bool> clickable)
    {
        for (int index = 0; index < clickable.Count; index++)
        {
            if (clickable[index])
                return index;
        }

        return -1;
    }

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

        string firstHitName = "<none>";
        if (raycasts.Count > 0)
        {
            GameObject? firstHitObject = raycasts[0].gameObject;
            firstHitName = firstHitObject == null ? "<null>" : firstHitObject.name;
        }

        var clickable = new bool[raycasts.Count];
        var clickTargets = new GameObject?[raycasts.Count];
        for (int index = 0; index < raycasts.Count; index++)
        {
            RaycastResult candidate = raycasts[index];
            GameObject? candidateObject = candidate.gameObject;
            if (candidateObject == null)
                continue;

            GameObject? candidateTarget = ExecuteEvents.GetEventHandler<IPointerClickHandler>(candidateObject);
            if (candidateTarget is null)
                continue;

            clickable[index] = true;
            clickTargets[index] = candidateTarget;
        }

        int clickableIndex = FindFirstClickableIndex(clickable);
        if (clickableIndex < 0)
        {
            result = $"no clickable handler: raycasts={raycasts.Count}, first='{firstHitName}' at ({command.X}, {command.Y})";
            return false;
        }

        RaycastResult hit = raycasts[clickableIndex];
        GameObject hitObject = hit.gameObject!;
        GameObject clickTarget = clickTargets[clickableIndex]!;
        pointerData.pointerCurrentRaycast = hit;
        pointerData.pointerPressRaycast = hit;

        ExecuteEvents.ExecuteHierarchy(hitObject, pointerData, ExecuteEvents.pointerDownHandler);
        ExecuteEvents.ExecuteHierarchy(hitObject, pointerData, ExecuteEvents.pointerUpHandler);
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
