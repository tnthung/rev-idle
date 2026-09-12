using Il2CppInterop.Runtime;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.Events;
using UnityEngine.UI;

namespace RevIdle.ScoreTelemetry;

internal enum ControlIcon
{
    Menu,
    Reload,
    Stop,
    Resume,
    Pause,
    Capture,
    Lock
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
    bool CaptureEnabled,
    bool MenuEnabled)
{
    public static ControlPresentation From(ControlState? state) => From(state, locked: false);

    public static ControlPresentation From(ControlState? state, bool locked)
    {
        if (state is null || state.Value.Capture)
            return new(ControlIcon.Reload, false, ControlIcon.Resume, false, false, false);
        ControlPresentation presentation = state.Value.Phase switch
        {
            ScriptPhase.Stopped => new(ControlIcon.Reload, true, ControlIcon.Resume, false, true, true),
            ScriptPhase.Running => new(ControlIcon.Stop, true, ControlIcon.Pause, true, false, true),
            ScriptPhase.Paused => new(ControlIcon.Stop, true, ControlIcon.Resume, true, true, true),
            _ => new(ControlIcon.Reload, false, ControlIcon.Resume, false, false, true)
        };
        presentation = presentation with { MenuEnabled = presentation.MenuEnabled && state.Value.Scripts?.Count > 0 };
        return locked ? presentation with { ResumePauseIcon = ControlIcon.Lock, CaptureEnabled = false, MenuEnabled = false } : presentation;
    }
}

internal readonly record struct ScriptSelection(string Path, bool Locked);

internal sealed class ScriptMenuController
{
    private static readonly TimeSpan Window = TimeSpan.FromMilliseconds(200);
    private readonly Action<ScriptSelection> _publish;
    private DateTime? _pendingAt;
    private string? _pendingPath;

    internal ScriptMenuController(Action<ScriptSelection> publish)
    {
        _publish = publish;
    }

    internal bool Open { get; private set; }

    internal void Toggle()
    {
        if (Open)
            Close();
        else
            Open = true;
    }

    internal void Click(DateTime now, string path)
    {
        if (_pendingAt is DateTime pendingAt &&
            _pendingPath == path &&
            now - pendingAt >= TimeSpan.Zero &&
            now - pendingAt <= Window)
        {
            _pendingAt = null;
            _pendingPath = null;
            Open = false;
            _publish(new ScriptSelection(path, true));
            return;
        }
        _pendingAt = now;
        _pendingPath = path;
    }

    internal void Flush(DateTime now)
    {
        if (_pendingAt is not DateTime pendingAt || now - pendingAt < Window)
            return;
        string? path = _pendingPath;
        _pendingAt = null;
        _pendingPath = null;
        Open = false;
        if (path is not null)
            _publish(new ScriptSelection(path, false));
    }

    internal void Close()
    {
        Open = false;
        _pendingAt = null;
        _pendingPath = null;
    }
}

internal sealed class ControlDebounce
{
    private static readonly TimeSpan Window = TimeSpan.FromMilliseconds(200);
    private readonly Action<ControlCommand> _publish;
    private DateTime? _pendingAt;
    private ControlCommand? _pendingSingle;

    internal ControlDebounce(Action<ControlCommand> publish)
    {
        _publish = publish;
    }

    internal void Click(DateTime now, ControlCommand? single, ControlCommand doubleClick)
    {
        if (_pendingAt is DateTime pendingAt)
        {
            TimeSpan elapsed = now - pendingAt;
            if (elapsed >= TimeSpan.Zero && elapsed <= Window)
            {
                _pendingAt = null;
                _pendingSingle = null;
                _publish(doubleClick);
                return;
            }
            Flush(now);
        }
        _pendingAt = now;
        _pendingSingle = single;
    }

    internal void Flush(DateTime now)
    {
        if (_pendingAt is not DateTime pendingAt || now - pendingAt < Window)
            return;
        ControlCommand? single = _pendingSingle;
        _pendingAt = null;
        _pendingSingle = null;
        if (single is ControlCommand command)
            _publish(command);
    }

    internal void Reset()
    {
        _pendingAt = null;
        _pendingSingle = null;
    }
}

internal sealed class ControlOverlay : IDisposable
{
    internal const int TooltipFontSize = 16;
    internal const int TooltipWidth = 132;
    internal const int TooltipHeight = 28;

    private readonly GameObject _root;
    private readonly Button _menu;
    private readonly Image _menuImage;
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
    private readonly GameObject _menuRoot;
    private readonly GameObject _dismissRoot;
    private readonly List<GameObject> _menuItems = new();
    private readonly List<string> _scripts = new();
    private readonly Texture2D _texture;
    private readonly Sprite _sprite;
    private readonly Texture2D[] _iconTextures;
    private readonly Sprite[] _icons;
    private readonly Font _font;
    private ControlCommand _reloadStopCommand;
    private ControlCommand _resumePauseCommand;

    // Lock is owned by the client (Rust) process, which is the only thing
    // that actually blocks input; this just mirrors the last StateUpdate so
    // click handlers know whether a double-click should enter or exit lock.
    // It is never set independently of Apply(ControlState?).
    private bool _locked;
    private readonly ScriptMenuController _menuController;
    private readonly ControlDebounce _reloadStopDebounce;
    private readonly ControlDebounce _resumePauseDebounce;

    public static ControlOverlay? Create(Action<ControlCommand> publish, Action<string, bool> load)
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
        return font is null ? null : new ControlOverlay(publish, load, font);
    }

    private ControlOverlay(Action<ControlCommand> publish, Action<string, bool> load, Font font)
    {
        _font = font;
        _menuController = new ScriptMenuController(selection => load(selection.Path, selection.Locked));
        _reloadStopDebounce = new ControlDebounce(publish);
        _resumePauseDebounce = new ControlDebounce(publish);
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
        int iconCount = Enum.GetValues<ControlIcon>().Length;
        _iconTextures = new Texture2D[iconCount];
        _icons = new Sprite[iconCount];
        foreach (ControlIcon icon in Enum.GetValues<ControlIcon>())
        {
            Texture2D texture = new(16, 16, TextureFormat.RGBA32, false)
            {
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            texture.hideFlags = HideFlags.DontUnloadUnusedAsset;
            for (int y = 0; y < 16; y++)
            for (int x = 0; x < 16; x++)
                texture.SetPixel(x, y, IconPixel(icon, x, y) ? Color.white : Color.clear);
            texture.Apply();
            _iconTextures[(int)icon] = texture;
            _icons[(int)icon] = Sprite.Create(texture, new Rect(0, 0, 16, 16), new Vector2(0.5f, 0.5f), 100f);
            _icons[(int)icon].hideFlags = HideFlags.DontUnloadUnusedAsset;
        }

        _root = new GameObject("RevIdle Script Controls");
        UnityEngine.Object.DontDestroyOnLoad(_root);
        Canvas canvas = _root.AddComponent<Canvas>();
        canvas.renderMode = RenderMode.ScreenSpaceOverlay;
        canvas.sortingOrder = short.MaxValue;
        _root.AddComponent<GraphicRaycaster>();

        _dismissRoot = new GameObject("Script Menu Dismiss", Il2CppType.Of<RectTransform>());
        _dismissRoot.transform.SetParent(_root.transform, false);
        Image dismissImage = _dismissRoot.AddComponent<Image>();
        dismissImage.color = Color.clear;
        Button dismiss = _dismissRoot.AddComponent<Button>();
        dismiss.targetGraphic = dismissImage;
        dismiss.onClick.AddListener((UnityAction)_menuController.Close);
        RectTransform dismissRect = _dismissRoot.GetComponent<RectTransform>();
        dismissRect.anchorMin = Vector2.zero;
        dismissRect.anchorMax = Vector2.one;
        dismissRect.offsetMin = Vector2.zero;
        dismissRect.offsetMax = Vector2.zero;
        _dismissRoot.SetActive(false);

        GameObject group = new("Buttons", Il2CppType.Of<RectTransform>());
        group.transform.SetParent(_root.transform, false);
        RectTransform groupRect = group.GetComponent<RectTransform>();
        groupRect.anchorMin = new Vector2(1, 0);
        groupRect.anchorMax = new Vector2(1, 0);
        groupRect.pivot = new Vector2(1, 0);
        groupRect.anchoredPosition = Vector2.zero;
        groupRect.sizeDelta = new Vector2(168, 46);

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
            colors.selectedColor = new Color(69f / 255f, 74f / 255f, 79f / 255f, 1);
            var disabledColor = CaptureDisabledColor(false);
            colors.disabledColor = new Color(
                disabledColor.Red,
                disabledColor.Green,
                disabledColor.Blue,
                disabledColor.Alpha);
            colors.colorMultiplier = 1;
            button.colors = colors;
            RectTransform rect = buttonObject.GetComponent<RectTransform>();
            rect.anchorMin = Vector2.zero;
            rect.anchorMax = Vector2.zero;
            rect.pivot = Vector2.zero;
            rect.anchoredPosition = new Vector2(x, 8);
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

        void HandleResumePauseClick()
        {
            _menuController.Close();
            DateTime now = DateTime.UtcNow;
            if (_locked)
            {
                _resumePauseDebounce.Click(now, null, ControlCommand.Pause);
                return;
            }
            _resumePauseDebounce.Click(
                now,
                _resumePauseCommand,
                _resumePauseCommand == ControlCommand.Pause ? ControlCommand.Lock : ControlCommand.ResumeLocked);
        }

        void HandleReloadStopClick()
        {
            _menuController.Close();
            if (_locked)
            {
                _reloadStopDebounce.Click(DateTime.UtcNow, null, ControlCommand.Stop);
                return;
            }
            if (_reloadStopCommand == ControlCommand.Stop)
            {
                publish(ControlCommand.Stop);
                return;
            }
            _reloadStopDebounce.Click(
                DateTime.UtcNow,
                ControlCommand.Reload,
                ControlCommand.ReloadLocked);
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

        _menuRoot = new GameObject("Script Menu", Il2CppType.Of<RectTransform>());
        _menuRoot.transform.SetParent(group.transform, false);
        RectTransform menuRect = _menuRoot.GetComponent<RectTransform>();
        menuRect.anchorMin = Vector2.zero;
        menuRect.anchorMax = Vector2.zero;
        menuRect.pivot = new Vector2(1, 0);
        menuRect.anchoredPosition = new Vector2(5, 8);
        menuRect.sizeDelta = new Vector2(232, 0);
        _menuRoot.SetActive(false);

        (_menu, _, _menuImage) = CreateButton("Script Menu", ControlIcon.Menu, 9, "Scripts");
        (_reloadStop, _reloadStopIcon, _reloadStopImage) = CreateButton("Reload Stop", ControlIcon.Reload, 49, "Reload / Stop");
        (_resumePause, _resumePauseIcon, _resumePauseImage) = CreateButton("Resume Pause", ControlIcon.Resume, 89, "Resume / Pause");
        (_capture, _, _captureImage) = CreateButton("Capture", ControlIcon.Capture, 129, "Capture UI path");
        _menu.onClick.AddListener((UnityAction)_menuController.Toggle);
        _reloadStop.onClick.AddListener((UnityAction)HandleReloadStopClick);
        _resumePause.onClick.AddListener((UnityAction)HandleResumePauseClick);
        _capture.onClick.AddListener((UnityAction)(() =>
        {
            _menuController.Close();
            publish(ControlCommand.Capture);
        }));
    }

    public void Apply(ControlState? state)
    {
        DateTime now = DateTime.UtcNow;
        bool lockedChanged = _locked != (state?.Locked == true);
        ControlCommand reloadStopCommand = state?.Phase == ScriptPhase.Stopped ? ControlCommand.Reload : ControlCommand.Stop;
        ControlCommand resumePauseCommand = state?.Phase == ScriptPhase.Paused ? ControlCommand.Resume : ControlCommand.Pause;
        if (lockedChanged || _reloadStopCommand != reloadStopCommand)
            _reloadStopDebounce.Reset();
        else
            _reloadStopDebounce.Flush(now);
        if (lockedChanged || _resumePauseCommand != resumePauseCommand)
            _resumePauseDebounce.Reset();
        else
            _resumePauseDebounce.Flush(now);
        _locked = state?.Locked == true;
        ControlPresentation presentation = ControlPresentation.From(state, _locked);
        IReadOnlyList<string> scripts = state?.Scripts ?? Array.Empty<string>();
        bool scriptsChanged = !_scripts.SequenceEqual(scripts);
        if (!presentation.MenuEnabled || scriptsChanged)
            _menuController.Close();
        else
            _menuController.Flush(now);
        if (scriptsChanged)
        {
            foreach (GameObject item in _menuItems)
            {
                item.SetActive(false);
                UnityEngine.Object.Destroy(item);
            }
            _menuItems.Clear();
            _scripts.Clear();
            _scripts.AddRange(scripts);
            _menuRoot.GetComponent<RectTransform>().sizeDelta = new Vector2(232, scripts.Count * 30);
            for (int index = 0; index < scripts.Count; index++)
            {
                string path = scripts[index];
                GameObject itemObject = new($"Script {index}", Il2CppType.Of<RectTransform>());
                itemObject.transform.SetParent(_menuRoot.transform, false);
                Image itemImage = itemObject.AddComponent<Image>();
                itemImage.sprite = _sprite;
                itemImage.type = Image.Type.Sliced;
                Button item = itemObject.AddComponent<Button>();
                item.targetGraphic = itemImage;
                ColorBlock itemColors = item.colors;
                itemColors.normalColor = new Color(69f / 255f, 74f / 255f, 79f / 255f, 1);
                itemColors.highlightedColor = new Color(82f / 255f, 88f / 255f, 94f / 255f, 1);
                itemColors.pressedColor = new Color(55f / 255f, 59f / 255f, 63f / 255f, 1);
                itemColors.selectedColor = itemColors.normalColor;
                item.colors = itemColors;
                RectTransform itemRect = itemObject.GetComponent<RectTransform>();
                itemRect.anchorMin = Vector2.zero;
                itemRect.anchorMax = Vector2.zero;
                itemRect.pivot = Vector2.zero;
                itemRect.anchoredPosition = new Vector2(0, (scripts.Count - index - 1) * 30);
                itemRect.sizeDelta = new Vector2(232, 30);

                GameObject textObject = new("Text", Il2CppType.Of<RectTransform>());
                textObject.transform.SetParent(itemObject.transform, false);
                Text text = textObject.AddComponent<Text>();
                text.font = _font;
                text.fontSize = 14;
                text.alignment = TextAnchor.MiddleLeft;
                text.color = Color.white;
                text.raycastTarget = false;
                text.text = Path.GetFileName(path);
                RectTransform textRect = textObject.GetComponent<RectTransform>();
                textRect.anchorMin = Vector2.zero;
                textRect.anchorMax = Vector2.one;
                textRect.offsetMin = new Vector2(8, 0);
                textRect.offsetMax = new Vector2(-8, 0);
                item.onClick.AddListener((UnityAction)(() => _menuController.Click(DateTime.UtcNow, path)));
                _menuItems.Add(itemObject);
            }
        }
        _menu.interactable = presentation.MenuEnabled;
        _menuRoot.SetActive(_menuController.Open);
        _dismissRoot.SetActive(_menuController.Open);
        _reloadStopCommand = reloadStopCommand;
        _resumePauseCommand = resumePauseCommand;
        _reloadStopIcon.sprite = _icons[(int)presentation.ReloadStopIcon];
        _reloadStop.interactable = presentation.ReloadStopEnabled;
        _resumePauseIcon.sprite = _icons[(int)presentation.ResumePauseIcon];
        _resumePause.interactable = presentation.ResumePauseEnabled;
        var disabledColor = CaptureDisabledColor(state?.Capture == true);
        ColorBlock captureColors = _capture.colors;
        captureColors.disabledColor = new Color(
            disabledColor.Red,
            disabledColor.Green,
            disabledColor.Blue,
            disabledColor.Alpha);
        _capture.colors = captureColors;
        _capture.interactable = presentation.CaptureEnabled;
        _reloadStopImage.raycastTarget = state?.Capture != true && (presentation.ReloadStopEnabled || !_menuController.Open);
        _resumePauseImage.raycastTarget = state?.Capture != true && (presentation.ResumePauseEnabled || !_menuController.Open);
        _captureImage.raycastTarget = state?.Capture != true && (presentation.CaptureEnabled || !_menuController.Open);
        _menuImage.raycastTarget = state?.Capture != true;
        if (state?.Capture == true || _locked || _menuController.Open)
            _tooltipRoot.SetActive(false);
    }

    internal static (float Red, float Green, float Blue, float Alpha) CaptureDisabledColor(bool captureActive) =>
        captureActive
            ? (1, 0, 0, 1)
            : (34f / 255f, 34f / 255f, 34f / 255f, 1);

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
            ControlIcon.Menu => x is >= 3 and <= 12 && y is 4 or 7 or 10,
            ControlIcon.Reload => distance is >= 64 and <= 121 && !(x >= 10 && y >= 10) ||
                x is >= 10 and <= 13 && y is >= 8 and <= 11 && y - 8 <= 13 - x,
            ControlIcon.Stop => x is >= 4 and <= 11 && y is >= 4 and <= 11 &&
                (x is 4 or 11 || y is 4 or 11),
            ControlIcon.Resume => x is >= 4 and <= 11 &&
                (x == 4 ? y is >= 4 and <= 11 : y == 4 + (x - 4) / 2 || y == 11 - (x - 4) / 2),
            ControlIcon.Pause => (x is 4 or 5 or 10 or 11) && y is >= 4 and <= 11,
            ControlIcon.Lock => x is >= 3 and <= 12 && y is >= 1 and <= 7 ||
                (x is 5 or 10) && y is >= 7 and <= 12 ||
                x is >= 5 and <= 10 && y is >= 11 and <= 12,
            _ => distance is >= 64 and <= 100 ||
                (x is 7 or 8) && (y <= 3 || y >= 12) ||
                (y is 7 or 8) && (x <= 3 || x >= 12)
        };
    }
}
