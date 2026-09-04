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

    internal static int FindFirstScrollableIndex(IReadOnlyList<bool> scrollable)
    {
        for (int index = 0; index < scrollable.Count; index++)
        {
            if (scrollable[index])
                return index;
        }

        return -1;
    }

    internal static bool TryDispatchScroll(nint window, ScrollCommand command, out string result)
    {
        if (window == 0 || !GetClientRect(window, out Rect client))
        {
            result = $"invalid bounds at ({command.X}, {command.Y})";
            return false;
        }

        int clientWidth = client.Right - client.Left;
        int clientHeight = client.Bottom - client.Top;
        if (!InputBridgeProtocol.TryMapToUnity(
                new ClickCommand(command.RequestId, command.X, command.Y),
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
            position = new Vector2(unityX, unityY),
            scrollDelta = command.Axis == 0
                ? new Vector2(0f, command.Length)
                : new Vector2(command.Length, 0f)
        };
        var raycasts = new Il2CppSystem.Collections.Generic.List<RaycastResult>();
        eventSystem.RaycastAll(pointerData, raycasts);

        var scrollable = new bool[raycasts.Count];
        var scrollTargets = new GameObject?[raycasts.Count];
        for (int index = 0; index < raycasts.Count; index++)
        {
            GameObject? candidate = raycasts[index].gameObject;
            if (candidate is null)
                continue;

            GameObject? target = ExecuteEvents.GetEventHandler<IScrollHandler>(candidate);
            if (target is not null)
            {
                scrollable[index] = true;
                scrollTargets[index] = target;
            }
        }

        int scrollableIndex = FindFirstScrollableIndex(scrollable);
        if (scrollableIndex < 0)
        {
            result = $"no scroll handler at ({command.X}, {command.Y})";
            return false;
        }

        GameObject targetObject = scrollTargets[scrollableIndex]!;
        pointerData.pointerCurrentRaycast = raycasts[scrollableIndex];
        ExecuteEvents.Execute(targetObject, pointerData, ExecuteEvents.scrollHandler);
        result = $"scrolled '{targetObject.name}' at ({command.X}, {command.Y})";
        return true;
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

    internal static bool TryDispatchDrag(nint window, DragCommand command, out string result)
    {
        if (window == 0 || !GetClientRect(window, out Rect client))
        {
            result = $"invalid bounds at ({command.StartX}, {command.StartY})";
            return false;
        }

        int clientWidth = client.Right - client.Left;
        int clientHeight = client.Bottom - client.Top;

        if (!InputBridgeProtocol.TryMapToUnity(
                new ClickCommand(command.RequestId, command.StartX, command.StartY),
                clientWidth, clientHeight, Screen.width, Screen.height,
                out float startX, out float startY) ||
            !InputBridgeProtocol.TryMapToUnity(
                new ClickCommand(command.RequestId, command.EndX, command.EndY),
                clientWidth, clientHeight, Screen.width, Screen.height,
                out float endX, out float endY))
        {
            result = $"invalid bounds for drag ({command.StartX}, {command.StartY}) -> ({command.EndX}, {command.EndY})";
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
            pressPosition = new Vector2(startX, startY),
            position = new Vector2(startX, startY)
        };

        var raycasts = new Il2CppSystem.Collections.Generic.List<RaycastResult>();
        eventSystem.RaycastAll(pointerData, raycasts);

        var draggable = new bool[raycasts.Count];
        var dragTargets = new GameObject?[raycasts.Count];
        for (int index = 0; index < raycasts.Count; index++)
        {
            GameObject? candidate = raycasts[index].gameObject;
            if (candidate is null)
                continue;

            GameObject? target = ExecuteEvents.GetEventHandler<IDragHandler>(candidate);
            if (target is null)
                continue;

            draggable[index] = true;
            dragTargets[index] = target;
        }

        int draggableIndex = FindFirstClickableIndex(draggable);
        if (draggableIndex < 0)
        {
            result = $"no drag handler at ({command.StartX}, {command.StartY})";
            return false;
        }

        RaycastResult hit = raycasts[draggableIndex];
        GameObject dragTarget = dragTargets[draggableIndex]!;
        pointerData.pointerCurrentRaycast = hit;
        pointerData.pointerPressRaycast = hit;
        pointerData.pointerDrag = dragTarget;

        ExecuteEvents.Execute(dragTarget, pointerData, ExecuteEvents.beginDragHandler);

        pointerData.delta = new Vector2(endX - startX, endY - startY);
        pointerData.position = new Vector2(endX, endY);
        ExecuteEvents.Execute(dragTarget, pointerData, ExecuteEvents.dragHandler);

        var dropRaycasts = new Il2CppSystem.Collections.Generic.List<RaycastResult>();
        eventSystem.RaycastAll(pointerData, dropRaycasts);

        GameObject? dropTarget = null;
        for (int index = 0; index < dropRaycasts.Count; index++)
        {
            GameObject? candidate = dropRaycasts[index].gameObject;
            if (candidate is null)
                continue;

            dropTarget = ExecuteEvents.GetEventHandler<IDropHandler>(candidate);
            if (dropTarget is not null)
            {
                pointerData.pointerCurrentRaycast = dropRaycasts[index];
                break;
            }
        }

        string dropDescription;
        if (dropTarget is not null)
        {
            ExecuteEvents.Execute(dropTarget, pointerData, ExecuteEvents.dropHandler);
            dropDescription = $"dropped on '{dropTarget.name}'";
        }
        else
        {
            dropDescription = "no drop handler at destination";
        }

        ExecuteEvents.Execute(dragTarget, pointerData, ExecuteEvents.endDragHandler);

        result = $"dragged '{dragTarget.name}' from ({command.StartX}, {command.StartY}) to ({command.EndX}, {command.EndY}), {dropDescription}";
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
