using System.Runtime.InteropServices;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace RevIdle.ScoreTelemetry;

internal static class UnityUiClickDispatcher
{
    internal static bool TryInvoke(string name, out string result)
    {
        List<Button> matches = new();
        foreach (Button candidateButton in Resources.FindObjectsOfTypeAll<Button>())
        {
            if (candidateButton.gameObject.scene.IsValid() &&
                (candidateButton.name == name || GetPath(candidateButton.gameObject) == name))
                matches.Add(candidateButton);
        }

        if (matches.Count > 1 && matches.Any(button => button.IsActive()))
            matches.RemoveAll(button => !button.IsActive());

        if (matches.Count == 0)
        {
            result = $"invoke target not found: '{name}'";
            return false;
        }
        if (matches.Count != 1)
        {
            result = $"invoke target is ambiguous: '{name}' matches {matches.Count} objects; use a path: {string.Join(", ", matches.Select(button => GetPath(button.gameObject)))}";
            return false;
        }

        Button targetButton = matches[0];
        if (!targetButton.IsInteractable())
        {
            result = $"invoke target is not interactable: '{name}'";
            return false;
        }

        targetButton.onClick.Invoke();
        result = $"invoked '{GetPath(targetButton.gameObject)}'";
        return true;
    }

    internal static bool TryCapture(nint window, int x, int y, out string? name, out string? path, out string result)
    {
        name = null;
        path = null;
        if (window == 0 || !GetClientRect(window, out Rect client))
        {
            result = $"invalid bounds at ({x}, {y})";
            return false;
        }

        if (!InputBridgeProtocol.TryMapToUnity(
                new ClickCommand(1, unchecked((uint)x), unchecked((uint)y)), client.Right - client.Left, client.Bottom - client.Top,
                Screen.width, Screen.height, out float unityX, out float unityY))
        {
            result = $"invalid bounds at ({x}, {y})";
            return false;
        }

        EventSystem? eventSystem = EventSystem.current;
        if (eventSystem is null)
        {
            result = "no EventSystem";
            return false;
        }

        var pointerData = new PointerEventData(eventSystem) { position = new Vector2(unityX, unityY) };
        var raycasts = new Il2CppSystem.Collections.Generic.List<RaycastResult>();
        eventSystem.RaycastAll(pointerData, raycasts);
        for (int index = 0; index < raycasts.Count; index++)
        {
            GameObject? candidate = raycasts[index].gameObject;
            GameObject? target = candidate is null ? null : ExecuteEvents.GetEventHandler<IPointerClickHandler>(candidate);
            if (target is null)
                continue;
            name = target.name;
            path = GetPath(target);
            result = $"captured '{name}' at ({x}, {y})";
            return true;
        }

        result = $"no clickable handler at ({x}, {y})";
        return true;
    }

    private static string GetPath(GameObject target)
    {
        string path = $"{Uri.EscapeDataString(target.name)}[{target.transform.GetSiblingIndex()}]";
        Transform? parent = target.transform.parent;
        while (parent is not null)
        {
            path = $"{Uri.EscapeDataString(parent.name)}[{parent.GetSiblingIndex()}]/{path}";
            parent = parent.parent;
        }
        return $"scene:{target.scene.handle}/{path}";
    }

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
