using Il2CppInterop.Runtime;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.Events;
using UnityEngine.UI;

namespace RevIdle.ScoreTelemetry;

internal enum ControlIcon
{
    Reload,
    Stop,
    Resume,
    Pause,
    Capture
}

internal enum ControlPixel
{
    Transparent,
    Border,
    Fill
}

internal readonly record struct ControlPresentation(
    ControlIcon ReloadStopIcon,
    bool ReloadStopEnabled,
    ControlIcon ResumePauseIcon,
    bool ResumePauseEnabled,
    bool CaptureEnabled)
{
    public static ControlPresentation From(ControlState? state)
    {
        if (state is null || state.Value.Capture)
            return new(ControlIcon.Reload, false, ControlIcon.Resume, false, false);
        return state.Value.Phase switch
        {
            ScriptPhase.Stopped => new(ControlIcon.Reload, true, ControlIcon.Resume, false, true),
            ScriptPhase.Running => new(ControlIcon.Stop, true, ControlIcon.Pause, true, false),
            ScriptPhase.Paused => new(ControlIcon.Stop, true, ControlIcon.Resume, true, true),
            _ => new(ControlIcon.Reload, false, ControlIcon.Resume, false, false)
        };
    }
}

internal sealed class ControlOverlay : IDisposable
{
    internal const int TooltipFontSize = 16;
    internal const int TooltipWidth = 132;
    internal const int TooltipHeight = 28;

    private readonly GameObject _root;
    private readonly Button _reloadStop;
    private readonly Image _reloadStopIcon;
    private readonly Image _reloadStopImage;
    private readonly Button _resumePause;
    private readonly Image _resumePauseIcon;
    private readonly Image _resumePauseImage;
    private readonly Button _capture;
    private readonly Image _captureImage;
    private readonly Text _tooltip;
    private readonly GameObject _tooltipRoot;
    private readonly Texture2D _texture;
    private readonly Sprite _sprite;
    private readonly Texture2D[] _iconTextures;
    private readonly Sprite[] _icons;
    private readonly Font _font;
    private ControlCommand _reloadStopCommand;
    private ControlCommand _resumePauseCommand;

    public static ControlOverlay? Create(Action<ControlCommand> publish)
    {
        Font? font = null;
        try { font = Resources.GetBuiltinResource<Font>("LegacyRuntime.ttf"); }
        catch { }
        if (font is null)
        {
            foreach (Text text in Resources.FindObjectsOfTypeAll<Text>())
            {
                if (text != null && text.font != null)
                {
                    font = text.font;
                    break;
                }
            }
        }
        return font is null ? null : new ControlOverlay(publish, font);
    }

    private ControlOverlay(Action<ControlCommand> publish, Font font)
    {
        _font = font;
        _texture = new Texture2D(12, 12, TextureFormat.RGBA32, false)
        {
            filterMode = FilterMode.Point,
            wrapMode = TextureWrapMode.Clamp
        };
        for (int y = 0; y < 12; y++)
        for (int x = 0; x < 12; x++)
        {
            _texture.SetPixel(x, y, ButtonPixel(x, y) switch
            {
                ControlPixel.Border => Color.black,
                ControlPixel.Fill => Color.white,
                _ => Color.clear
            });
        }
        _texture.Apply();
        _sprite = Sprite.Create(_texture, new Rect(0, 0, 12, 12), new Vector2(0.5f, 0.5f), 100f, 0, SpriteMeshType.FullRect, new Vector4(4, 4, 4, 4));
        _iconTextures = new Texture2D[5];
        _icons = new Sprite[5];
        foreach (ControlIcon icon in Enum.GetValues<ControlIcon>())
        {
            Texture2D texture = new(16, 16, TextureFormat.RGBA32, false)
            {
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            for (int y = 0; y < 16; y++)
            for (int x = 0; x < 16; x++)
                texture.SetPixel(x, y, IconPixel(icon, x, y) ? Color.white : Color.clear);
            texture.Apply();
            _iconTextures[(int)icon] = texture;
            _icons[(int)icon] = Sprite.Create(texture, new Rect(0, 0, 16, 16), new Vector2(0.5f, 0.5f), 100f);
        }

        _root = new GameObject("RevIdle Script Controls");
        UnityEngine.Object.DontDestroyOnLoad(_root);
        Canvas canvas = _root.AddComponent<Canvas>();
        canvas.renderMode = RenderMode.ScreenSpaceOverlay;
        canvas.sortingOrder = short.MaxValue;
        _root.AddComponent<GraphicRaycaster>();

        GameObject group = new("Buttons", Il2CppType.Of<RectTransform>());
        group.transform.SetParent(_root.transform, false);
        RectTransform groupRect = group.GetComponent<RectTransform>();
        groupRect.anchorMin = new Vector2(1, 0);
        groupRect.anchorMax = new Vector2(1, 0);
        groupRect.pivot = new Vector2(1, 0);
        groupRect.anchoredPosition = Vector2.zero;
        groupRect.sizeDelta = new Vector2(130, 50);

        (Button Button, Image Icon, Image Image) CreateButton(string name, ControlIcon icon, float x, string tooltip)
        {
            GameObject buttonObject = new(name, Il2CppType.Of<RectTransform>());
            buttonObject.transform.SetParent(group.transform, false);
            Image image = buttonObject.AddComponent<Image>();
            image.sprite = _sprite;
            image.type = Image.Type.Sliced;
            Button button = buttonObject.AddComponent<Button>();
            button.targetGraphic = image;
            ColorBlock colors = button.colors;
            colors.normalColor = new Color(69f / 255f, 74f / 255f, 79f / 255f, 1);
            colors.highlightedColor = new Color(82f / 255f, 88f / 255f, 94f / 255f, 1);
            colors.pressedColor = new Color(55f / 255f, 59f / 255f, 63f / 255f, 1);
            colors.disabledColor = new Color(34f / 255f, 34f / 255f, 34f / 255f, 1);
            colors.colorMultiplier = 1;
            button.colors = colors;
            RectTransform rect = buttonObject.GetComponent<RectTransform>();
            rect.anchorMin = Vector2.zero;
            rect.anchorMax = Vector2.zero;
            rect.pivot = Vector2.zero;
            rect.anchoredPosition = new Vector2(x, 10);
            rect.sizeDelta = new Vector2(30, 30);

            GameObject labelObject = new("Icon", Il2CppType.Of<RectTransform>());
            labelObject.transform.SetParent(buttonObject.transform, false);
            Image label = labelObject.AddComponent<Image>();
            label.sprite = _icons[(int)icon];
            label.preserveAspect = true;
            label.raycastTarget = false;
            RectTransform labelRect = labelObject.GetComponent<RectTransform>();
            labelRect.anchorMin = new Vector2(0.5f, 0.5f);
            labelRect.anchorMax = new Vector2(0.5f, 0.5f);
            labelRect.pivot = new Vector2(0.5f, 0.5f);
            labelRect.anchoredPosition = Vector2.zero;
            labelRect.sizeDelta = new Vector2(16, 16);

            EventTrigger trigger = buttonObject.AddComponent<EventTrigger>();
            EventTrigger.Entry enter = new() { eventID = EventTriggerType.PointerEnter };
            enter.callback.AddListener((UnityAction<BaseEventData>)(_ =>
            {
                _tooltip.text = tooltip;
                _tooltipRoot.SetActive(true);
            }));
            trigger.triggers.Add(enter);
            EventTrigger.Entry exit = new() { eventID = EventTriggerType.PointerExit };
            exit.callback.AddListener((UnityAction<BaseEventData>)(_ => _tooltipRoot.SetActive(false)));
            trigger.triggers.Add(exit);
            return (button, label, image);
        }

        GameObject tooltipObject = new("Tooltip", Il2CppType.Of<RectTransform>());
        tooltipObject.transform.SetParent(group.transform, false);
        _tooltipRoot = tooltipObject;
        Image tooltipBackground = tooltipObject.AddComponent<Image>();
        tooltipBackground.sprite = _sprite;
        tooltipBackground.type = Image.Type.Sliced;
        tooltipBackground.color = new Color(34f / 255f, 34f / 255f, 34f / 255f, 0.94f);
        tooltipBackground.raycastTarget = false;
        GameObject tooltipTextObject = new("Text", Il2CppType.Of<RectTransform>());
        tooltipTextObject.transform.SetParent(tooltipObject.transform, false);
        _tooltip = tooltipTextObject.AddComponent<Text>();
        _tooltip.font = _font;
        _tooltip.fontSize = TooltipFontSize;
        _tooltip.alignment = TextAnchor.MiddleCenter;
        _tooltip.color = Color.white;
        _tooltip.raycastTarget = false;
        RectTransform tooltipRect = tooltipObject.GetComponent<RectTransform>();
        tooltipRect.anchorMin = Vector2.zero;
        tooltipRect.anchorMax = Vector2.zero;
        tooltipRect.pivot = Vector2.zero;
        tooltipRect.anchoredPosition = new Vector2(-2, 42);
        tooltipRect.sizeDelta = new Vector2(TooltipWidth, TooltipHeight);
        RectTransform tooltipTextRect = tooltipTextObject.GetComponent<RectTransform>();
        tooltipTextRect.anchorMin = Vector2.zero;
        tooltipTextRect.anchorMax = Vector2.one;
        tooltipTextRect.offsetMin = new Vector2(6, 4);
        tooltipTextRect.offsetMax = new Vector2(-6, -4);
        tooltipObject.SetActive(false);

        (_reloadStop, _reloadStopIcon, _reloadStopImage) = CreateButton("Reload Stop", ControlIcon.Reload, 10, "Reload / Stop");
        (_resumePause, _resumePauseIcon, _resumePauseImage) = CreateButton("Resume Pause", ControlIcon.Resume, 50, "Resume / Pause");
        (_capture, _, _captureImage) = CreateButton("Capture", ControlIcon.Capture, 90, "Capture UI path");
        _reloadStop.onClick.AddListener((UnityAction)(() => publish(_reloadStopCommand)));
        _resumePause.onClick.AddListener((UnityAction)(() => publish(_resumePauseCommand)));
        _capture.onClick.AddListener((UnityAction)(() => publish(ControlCommand.Capture)));
    }

    public void Apply(ControlState? state)
    {
        ControlPresentation presentation = ControlPresentation.From(state);
        _reloadStopCommand = state?.Phase == ScriptPhase.Stopped ? ControlCommand.Reload : ControlCommand.Stop;
        _resumePauseCommand = state?.Phase == ScriptPhase.Paused ? ControlCommand.Resume : ControlCommand.Pause;
        _reloadStopIcon.sprite = _icons[(int)presentation.ReloadStopIcon];
        _reloadStop.interactable = presentation.ReloadStopEnabled;
        _resumePauseIcon.sprite = _icons[(int)presentation.ResumePauseIcon];
        _resumePause.interactable = presentation.ResumePauseEnabled;
        _capture.interactable = presentation.CaptureEnabled;
        _reloadStopImage.raycastTarget = state?.Capture != true;
        _resumePauseImage.raycastTarget = state?.Capture != true;
        _captureImage.raycastTarget = state?.Capture != true;
        if (state?.Capture == true)
            _tooltipRoot.SetActive(false);
    }

    public void Dispose()
    {
        UnityEngine.Object.Destroy(_root);
        UnityEngine.Object.Destroy(_sprite);
        UnityEngine.Object.Destroy(_texture);
        foreach (Sprite icon in _icons)
            UnityEngine.Object.Destroy(icon);
        foreach (Texture2D texture in _iconTextures)
            UnityEngine.Object.Destroy(texture);
    }

    internal static ControlPixel ButtonPixel(int x, int y)
    {
        int cornerX = x < 4 ? 3 - x : x > 7 ? x - 8 : 0;
        int cornerY = y < 4 ? 3 - y : y > 7 ? y - 8 : 0;
        int cornerDistance = cornerX * cornerX + cornerY * cornerY;
        if (cornerDistance > 9)
            return ControlPixel.Transparent;
        return x is 0 or 11 || y is 0 or 11 || cornerDistance > 4 ? ControlPixel.Border : ControlPixel.Fill;
    }

    internal static bool IconPixel(ControlIcon icon, int x, int y)
    {
        int centerX = x * 2 - 15;
        int centerY = y * 2 - 15;
        int distance = centerX * centerX + centerY * centerY;
        return icon switch
        {
            ControlIcon.Reload => distance is >= 64 and <= 121 && !(x >= 10 && y >= 10) ||
                x is >= 10 and <= 13 && y is >= 8 and <= 11 && y - 8 <= 13 - x,
            ControlIcon.Stop => x is >= 4 and <= 11 && y is >= 4 and <= 11 &&
                (x is 4 or 11 || y is 4 or 11),
            ControlIcon.Resume => x is >= 4 and <= 11 &&
                (x == 4 ? y is >= 4 and <= 11 : y == 4 + (x - 4) / 2 || y == 11 - (x - 4) / 2),
            ControlIcon.Pause => (x is 4 or 5 or 10 or 11) && y is >= 4 and <= 11,
            _ => distance is >= 64 and <= 100 ||
                (x is 7 or 8) && (y <= 3 || y >= 12) ||
                (y is 7 or 8) && (x <= 3 || x >= 12)
        };
    }
}
