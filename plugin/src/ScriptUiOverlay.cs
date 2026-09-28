using Il2CppInterop.Runtime;
using Il2CppInterop.Runtime.Injection;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.Events;
using UnityEngine.UI;

namespace RevIdle.ScoreTelemetry;

internal readonly struct ScriptUiRect
{
    internal ScriptUiRect(float xMin, float yMin, float width, float height)
    {
        this.xMin = xMin;
        this.yMin = yMin;
        this.width = width;
        this.height = height;
    }

    internal float xMin { get; }
    internal float yMin { get; }
    internal float width { get; }
    internal float height { get; }
    internal float xMax => xMin + width;
    internal float yMax => yMin + height;
}

internal readonly struct ScriptUiTextMeasurement
{
    internal ScriptUiTextMeasurement(float width, float height, float minY, float maxY)
    {
        Width = width;
        Top = MathF.Ceiling(MathF.Max(0, maxY));
        Bottom = MathF.Ceiling(MathF.Max(height, -minY)) - height;
        Height = height + Top + Bottom;
    }

    internal float Width { get; }
    internal float Height { get; }
    internal float Top { get; }
    internal float Bottom { get; }
}

internal readonly struct ScriptUiRadii
{
    internal ScriptUiRadii(float topLeft, float topRight, float bottomLeft, float bottomRight)
    {
        TopLeft = topLeft;
        TopRight = topRight;
        BottomLeft = bottomLeft;
        BottomRight = bottomRight;
    }

    internal float TopLeft { get; }
    internal float TopRight { get; }
    internal float BottomLeft { get; }
    internal float BottomRight { get; }
}

internal readonly struct ScriptUiLayout
{
    internal ScriptUiLayout(ScriptUiRect box, ScriptUiRect content, ScriptUiRect outline, ScriptUiRadii radii, ScriptUiRadii outlineRadii, float borderThickness)
    {
        Box = box;
        Content = content;
        Outline = outline;
        Radii = radii;
        OutlineRadii = outlineRadii;
        BorderThickness = borderThickness;
    }

    internal ScriptUiRect Box { get; }
    internal ScriptUiRect Content { get; }
    internal ScriptUiRect Outline { get; }
    internal ScriptUiRadii Radii { get; }
    internal ScriptUiRadii OutlineRadii { get; }
    internal float BorderThickness { get; }
}

internal static class ScriptUiGeometry
{
    private const float MaxLayoutValue = 1_000_000;

    internal static ScriptUiLayout Calculate(
        ScriptUiElementState element,
        float viewportWidth,
        float viewportHeight,
        float measuredTextWidth,
        float measuredTextHeight)
    {
        float leftPadding = (float)element.Padding.Left;
        float rightPadding = (float)element.Padding.Right;
        float topPadding = (float)element.Padding.Top;
        float bottomPadding = (float)element.Padding.Bottom;
        float width = ResolveLength(element.LenX, MathF.Max(0, measuredTextWidth) + leftPadding + rightPadding);
        float height = ResolveLength(element.LenY, MathF.Max(0, measuredTextHeight) + topPadding + bottomPadding);
        float left = Limit(element.PosX >= 0 ? (double)element.PosX : viewportWidth + element.PosX - width);
        float top = Limit(element.PosY >= 0 ? (double)element.PosY : viewportHeight + element.PosY - height);
        ScriptUiRect box = new(left, top, width, height);
        float half = MathF.Min(width, height) / 2;
        ScriptUiRadii radii = new(
            MathF.Min((float)element.Corner.TopLeft, half),
            MathF.Min((float)element.Corner.TopRight, half),
            MathF.Min((float)element.Corner.BottomLeft, half),
            MathF.Min((float)element.Corner.BottomRight, half));
        float border = Limit(element.Border.Thickness);
        ScriptUiRadii outlineRadii = new(
            radii.TopLeft == 0 ? 0 : radii.TopLeft + border,
            radii.TopRight == 0 ? 0 : radii.TopRight + border,
            radii.BottomLeft == 0 ? 0 : radii.BottomLeft + border,
            radii.BottomRight == 0 ? 0 : radii.BottomRight + border);
        float outlineWidth = width > 0 && height > 0 ? width + border * 2 : 0;
        float outlineHeight = width > 0 && height > 0 ? height + border * 2 : 0;
        return new(
            box,
            new ScriptUiRect(Limit(left + leftPadding), Limit(top + topPadding), Limit(MathF.Max(0, width - leftPadding - rightPadding)), Limit(MathF.Max(0, height - topPadding - bottomPadding))),
            new ScriptUiRect(left - border, top - border, outlineWidth, outlineHeight),
            radii,
            outlineRadii,
            border);
    }

    internal static bool Contains(ScriptUiLayout layout, float x, float y)
        => Contains(layout.Box, layout.Radii, x, y);

    internal const int BoundaryPointCount = 4 * 33;

    internal static (float X, float Y) BoundaryPoint(ScriptUiRect box, ScriptUiRadii radii, int index)
    {
        int corner = index / 33;
        float radius = corner switch
        {
            0 => radii.TopLeft,
            1 => radii.TopRight,
            2 => radii.BottomRight,
            _ => radii.BottomLeft
        };
        float angle = (corner + 2 + index % 33 / 32f) * MathF.PI / 2;
        float centerX = corner is 0 or 3 ? box.xMin + radius : box.xMax - radius;
        float centerY = corner < 2 ? box.yMin + radius : box.yMax - radius;
        return (centerX + MathF.Cos(angle) * radius, centerY + MathF.Sin(angle) * radius);
    }

    internal static bool Contains(ScriptUiRect box, ScriptUiRadii radii, float x, float y)
    {
        if (box.width <= 0 || box.height <= 0 || x < box.xMin || x > box.xMax || y < box.yMin || y > box.yMax)
            return false;
        if (x < box.xMin + radii.TopLeft && y < box.yMin + radii.TopLeft)
            return WithinCorner(x, y, box.xMin + radii.TopLeft, box.yMin + radii.TopLeft, radii.TopLeft);
        if (x > box.xMax - radii.TopRight && y < box.yMin + radii.TopRight)
            return WithinCorner(x, y, box.xMax - radii.TopRight, box.yMin + radii.TopRight, radii.TopRight);
        if (x < box.xMin + radii.BottomLeft && y > box.yMax - radii.BottomLeft)
            return WithinCorner(x, y, box.xMin + radii.BottomLeft, box.yMax - radii.BottomLeft, radii.BottomLeft);
        if (x > box.xMax - radii.BottomRight && y > box.yMax - radii.BottomRight)
            return WithinCorner(x, y, box.xMax - radii.BottomRight, box.yMax - radii.BottomRight, radii.BottomRight);
        return true;
    }

    private static float ResolveLength(ScriptUiLengthState length, float preferred)
    {
        if (length.Fixed is double fixedLength)
            return Limit(fixedLength);
        float result = MathF.Max((float)length.Min, preferred);
        if (length.Max is double max)
            result = MathF.Min(result, (float)max);
        return Limit(result);
    }

    private static float Limit(double value)
        => double.IsNaN(value) ? 0 : value > MaxLayoutValue ? MaxLayoutValue : value < -MaxLayoutValue ? -MaxLayoutValue : (float)value;

    private static bool WithinCorner(float x, float y, float centerX, float centerY, float radius)
    {
        if (radius <= 0)
            return true;
        float dx = x - centerX;
        float dy = y - centerY;
        return dx * dx + dy * dy <= radius * radius;
    }
}

internal static class ScriptUiPointerPairing
{
    internal static bool MatchesRelease(
        ScriptUiPointer pointer,
        Guid sessionId,
        Guid pressedInstanceId,
        ulong pressedEventsVersion,
        Guid liveInstanceId,
        ulong liveEventsVersion,
        ScriptUiLayout liveLayout)
        => pointer.Phase == "up" &&
           pointer.SessionId == sessionId &&
           pressedInstanceId == liveInstanceId &&
           pressedEventsVersion == liveEventsVersion &&
           ScriptUiGeometry.Contains(liveLayout, pointer.X, pointer.Y);

    internal static bool MatchesRelease(
        ScriptUiPointer pointer,
        Guid sessionId,
        Guid pressedInstanceId,
        ulong pressedEventsVersion,
        Guid liveInstanceId,
        ulong liveEventsVersion,
        ScriptUiLayout liveLayout,
        float x,
        float y)
        => pointer.Phase == "up" &&
           pointer.SessionId == sessionId &&
           pressedInstanceId == liveInstanceId &&
           pressedEventsVersion == liveEventsVersion &&
           ScriptUiGeometry.Contains(liveLayout, x, y);
}

internal sealed class ScriptUiImage : Image
{
    private ScriptUiRect _outer;
    private ScriptUiRadii _outerRadii;
    private ScriptUiRect? _inner;
    private ScriptUiRadii _innerRadii;

    public ScriptUiImage(IntPtr pointer) : base(pointer)
    {
    }

    internal Func<Vector2, bool>? HitTest { get; set; }

    internal void SetGeometry(ScriptUiRect outer, ScriptUiRadii outerRadii, ScriptUiRect? inner = null, ScriptUiRadii innerRadii = default)
    {
        if (_outer.Equals(outer) && _outerRadii.Equals(outerRadii) && _inner.Equals(inner) && _innerRadii.Equals(innerRadii))
            return;
        _outer = outer;
        _outerRadii = outerRadii;
        _inner = inner;
        _innerRadii = innerRadii;
        SetVerticesDirty();
    }

    public override void OnPopulateMesh(VertexHelper vertices)
    {
        vertices.Clear();
        if (_outer.width <= 0 || _outer.height <= 0 || _inner is ScriptUiRect same && same.Equals(_outer))
            return;
        Rect rect = GetPixelAdjustedRect();
        if (_inner is null)
            vertices.AddVert(new Vector3(rect.center.x, rect.center.y, 0), color, Vector2.zero);
        for (int index = 0; index < ScriptUiGeometry.BoundaryPointCount; index++)
        {
            (float x, float y) = ScriptUiGeometry.BoundaryPoint(_outer, _outerRadii, index);
            vertices.AddVert(new Vector3(rect.xMin + x, rect.yMax - y, 0), color, Vector2.zero);
            if (_inner is ScriptUiRect inner)
            {
                (float innerX, float innerY) = ScriptUiGeometry.BoundaryPoint(inner, _innerRadii, index);
                vertices.AddVert(new Vector3(rect.xMin + innerX, rect.yMax - innerY, 0), color, Vector2.zero);
                int next = (index + 1) % ScriptUiGeometry.BoundaryPointCount;
                vertices.AddTriangle(index * 2, next * 2, index * 2 + 1);
                vertices.AddTriangle(next * 2, next * 2 + 1, index * 2 + 1);
            }
            else
            {
                vertices.AddTriangle(0, index + 1, (index + 1) % ScriptUiGeometry.BoundaryPointCount + 1);
            }
        }
    }

    public override bool IsRaycastLocationValid(Vector2 screenPoint, Camera eventCamera)
        => HitTest is null || HitTest(new Vector2(screenPoint.x, Screen.height - screenPoint.y));
}

internal sealed class ScriptUiOverlay : IDisposable
{
    internal const int SortingOrder = 32766;
    internal const int FontSize = 14;

    private readonly GameObject _root;
    private readonly Canvas _canvas;
    private Font? _font;
    private TextGenerator? _textGenerator;
    private readonly Dictionary<string, Font?> _scriptFonts = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<(string Text, Font Font), ScriptUiTextMeasurement> _textMeasurements = new();
    private ulong _textMeasurementRevision;
    private readonly Func<ScriptUiEvent, bool> _sendEvent;
    private readonly Action<string>? _log;
    private readonly List<ElementView> _elements = new();
    private readonly Dictionary<Guid, ElementView> _byInstance = new();
    private readonly Dictionary<ulong, PressedElement> _pressed = new();
    private static bool _scriptUiImageRegistered;
    private Guid? _sessionId;
    private bool _eventsEnabled;
    private int _viewportWidth;
    private int _viewportHeight;
    private bool _disposed;

    private readonly struct PressedElement
    {
        internal PressedElement(Guid instanceId, ulong eventsVersion)
        {
            InstanceId = instanceId;
            EventsVersion = eventsVersion;
        }

        internal Guid InstanceId { get; }
        internal ulong EventsVersion { get; }
    }

    private ScriptUiOverlay(Func<ScriptUiEvent, bool> sendEvent, Action<string>? log, Font? font)
    {
        _sendEvent = sendEvent ?? throw new ArgumentNullException(nameof(sendEvent));
        _log = log;
        _font = font;
        _root = new GameObject("RevIdle Script UI", Il2CppType.Of<RectTransform>());
        UnityEngine.Object.DontDestroyOnLoad(_root);
        _canvas = _root.AddComponent<Canvas>();
        _canvas.renderMode = RenderMode.ScreenSpaceOverlay;
        _canvas.sortingOrder = SortingOrder;
        _root.AddComponent<GraphicRaycaster>();
    }

    internal static ScriptUiOverlay? Create(Func<ScriptUiEvent, bool> sendEvent, Action<string>? log = null)
    {
        Font? font = FindFont(log);
        try
        {
            if (!_scriptUiImageRegistered)
            {
                ClassInjector.RegisterTypeInIl2Cpp<ScriptUiImage>();
                _scriptUiImageRegistered = true;
            }
            return new ScriptUiOverlay(sendEvent, log, font);
        }
        catch (Exception exception)
        {
            try { log?.Invoke($"Script UI root creation failed: {exception.Message}"); } catch { }
            return null;
        }
    }

    private static Font? FindFont(Action<string>? log)
    {
        Font? font = null;
        try { font = Resources.GetBuiltinResource<Font>("LegacyRuntime.ttf"); }
        catch { }
        if (font == null)
        {
            try
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
            catch (Exception exception)
            {
                try { log?.Invoke($"Script UI font lookup failed: {exception.Message}"); } catch { }
            }
        }
        return font;
    }

    internal void Apply(ScriptUiSnapshot? snapshot, bool connectionAvailable, bool capture)
    {
        if (_disposed)
            return;
        bool eventsEnabled = connectionAvailable && !capture && snapshot?.SessionId is not null;
        if (!eventsEnabled)
            _pressed.Clear();
        _eventsEnabled = eventsEnabled;
        if (snapshot?.SessionId is not Guid sessionId)
        {
            _sessionId = null;
            ClearElements();
            return;
        }
        if (_sessionId is not Guid previousSession || previousSession != sessionId)
        {
            _pressed.Clear();
            ClearElements();
        }
        _sessionId = sessionId;
        if (_font == null)
        {
            _font = FindFont(_log);
        }
        if (_font == null)
        {
            _eventsEnabled = false;
            foreach (ElementView view in _elements)
                view.SetEventsEnabled(false);
            Report("Script UI cannot measure text because no Unity font is available; retrying.");
            return;
        }
        int width = Screen.width;
        int height = Screen.height;
        if (width <= 0 || height <= 0)
            return;
        if (_textMeasurementRevision != snapshot.Revision)
        {
            _textMeasurements.Clear();
            _textMeasurementRevision = snapshot.Revision;
        }
        Font?[] fonts = new Font?[snapshot.Elements.Length];
        ScriptUiTextMeasurement[] measurements = new ScriptUiTextMeasurement[snapshot.Elements.Length];
        HashSet<string> requestedFonts = new(StringComparer.OrdinalIgnoreCase);
        for (int index = 0; index < snapshot.Elements.Length; index++)
        {
            ScriptUiElementState element = snapshot.Elements[index];
            fonts[index] = ResolveFont(element.Font);
            if (element.Font.Length > 0)
                requestedFonts.Add(element.Font);
            if (fonts[index] == null || !TryMeasure(element.Text, fonts[index], out measurements[index]))
            {
                _font = null;
                _eventsEnabled = false;
                ClearPendingPointers();
                foreach (ElementView view in _elements)
                    view.SetEventsEnabled(false);
                return;
            }
        }
        _viewportWidth = width;
        _viewportHeight = height;
        HashSet<Guid> retained = new();
        List<ElementView> next = new(snapshot.Elements.Length);
        for (int index = 0; index < snapshot.Elements.Length; index++)
        {
            ScriptUiElementState element = snapshot.Elements[index];
            if (element.InstanceId == Guid.Empty)
                continue;
            ElementView? view = _byInstance.TryGetValue(element.InstanceId, out ElementView? existing) ? existing : null;
            if (view is null)
            {
                view = new ElementView(_root.transform, fonts[index]!, _sendEvent, _log);
                _byInstance[element.InstanceId] = view;
            }
            view.SetSession(sessionId);
            view.Apply(element, width, height, measurements[index], fonts[index]!);
            view.SetEventsEnabled(_eventsEnabled);
            view.Root.transform.SetSiblingIndex(index);
            retained.Add(element.InstanceId);
            next.Add(view);
        }
        foreach ((Guid instanceId, ElementView view) in _byInstance.ToArray())
        {
            if (!retained.Contains(instanceId))
            {
                view.Dispose();
                _byInstance.Remove(instanceId);
            }
        }
        _elements.Clear();
        _elements.AddRange(next);
        foreach ((string family, Font? font) in _scriptFonts.ToArray())
        {
            if (requestedFonts.Contains(family))
                continue;
            _scriptFonts.Remove(family);
            if (font != null)
                UnityEngine.Object.Destroy(font);
        }
    }

    internal bool HandlePointer(ScriptUiPointer pointer)
        => HandlePointer(pointer, Screen.width, Screen.height);

    internal bool HandlePointer(ScriptUiPointer pointer, long generation)
        => HandlePointer(pointer);

    internal bool HandlePointer(ScriptUiPointer pointer, int viewportWidth, int viewportHeight)
    {
        if (_disposed || !_eventsEnabled ||
            _sessionId is not Guid sessionId || pointer.SessionId != sessionId ||
            pointer.Width <= 0 || pointer.Height <= 0 || viewportWidth <= 0 || viewportHeight <= 0)
        {
            if (pointer.Phase == "up")
                _pressed.Remove(pointer.PressId);
            return false;
        }
        if (viewportWidth != _viewportWidth || viewportHeight != _viewportHeight)
        {
            _viewportWidth = viewportWidth;
            _viewportHeight = viewportHeight;
            foreach (ElementView view in _elements)
                view.Reflow(_viewportWidth, _viewportHeight);
        }
        if (pointer.Phase is not ("down" or "up"))
            return false;
        float x = (float)((double)pointer.X * viewportWidth / pointer.Width);
        float y = (float)((double)pointer.Y * viewportHeight / pointer.Height);
        if (pointer.Phase == "down")
        {
            _pressed.Clear();
            ElementView? view = FindHit(x, y);
            if (view is not null && view.HasEvents)
                _pressed[pointer.PressId] = new PressedElement(view.InstanceId, view.EventsVersion);
            return true;
        }
        if (!_pressed.Remove(pointer.PressId, out PressedElement pressed))
            return true;
        ElementView? release = FindHit(x, y);
        if (release is not null &&
            ScriptUiPointerPairing.MatchesRelease(pointer, sessionId, pressed.InstanceId, pressed.EventsVersion, release.InstanceId, release.EventsVersion, release.Layout, x, y) &&
            release.HasEvent("click"))
            release.Dispatch("click");
        return true;
    }

    internal bool HitTestForTest(Guid instanceId, float x, float y)
        => _byInstance.TryGetValue(instanceId, out ElementView? view) && ScriptUiGeometry.Contains(view.Layout, x, y);

    internal void ClearPendingPointers()
    {
        _pressed.Clear();
        foreach (ElementView view in _elements)
            view.ClearPendingPointers();
    }

    private ElementView? FindHit(float x, float y)
    {
        for (int index = _elements.Count - 1; index >= 0; index--)
        {
            ElementView view = _elements[index];
            if (view.HasEvents && ScriptUiGeometry.Contains(view.Layout, x, y))
                return view;
        }
        return null;
    }

    private Font? ResolveFont(string requested)
    {
        if (requested.Length == 0)
            return _font;
        if (_scriptFonts.TryGetValue(requested, out Font? cached))
            return cached != null ? cached : _font;
        Font? resolved = null;
        try
        {
            string? canonical = Font.GetOSInstalledFontNames()
                .FirstOrDefault(name => string.Equals(name, requested, StringComparison.OrdinalIgnoreCase));
            if (canonical is not null)
            {
                // The factory overloads depend on stripped constructors in the game's IL2CPP bindings.
                resolved = new Font(IL2CPP.il2cpp_object_new(Il2CppClassPointerStore<Font>.NativeClassPtr));
                Font.Internal_CreateDynamicFont(resolved, new[] { canonical }, FontSize);
                if (resolved != null)
                    resolved.hideFlags |= HideFlags.DontUnloadUnusedAsset;
            }
        }
        catch (Exception exception)
        {
            if (resolved != null)
                UnityEngine.Object.Destroy(resolved);
            _scriptFonts[requested] = null;
            Report($"Script UI font '{requested}' could not be loaded: {exception.Message}; using the fallback font.");
            return _font;
        }
        _scriptFonts[requested] = resolved;
        if (resolved == null)
            Report($"Script UI font '{requested}' is unavailable; using the fallback font.");
        return resolved != null ? resolved : _font;
    }

    private bool TryMeasure(string text, Font? font, out ScriptUiTextMeasurement measured)
    {
        measured = default;
        if (string.IsNullOrEmpty(text))
            return true;
        if (font == null)
            return false;
        if (_textMeasurements.TryGetValue((text, font), out measured))
            return true;
        try
        {
            TextGenerator generator = _textGenerator ??= new();
            TextGenerationSettings settings = new()
            {
                font = font,
                fontSize = FontSize,
                lineSpacing = 1,
                scaleFactor = 1,
                textAnchor = TextAnchor.UpperLeft,
                alignByGeometry = false,
                resizeTextForBestFit = false,
                resizeTextMinSize = FontSize,
                resizeTextMaxSize = FontSize,
                verticalOverflow = VerticalWrapMode.Overflow,
                horizontalOverflow = HorizontalWrapMode.Overflow,
                updateBounds = true,
                richText = false,
                pivot = new Vector2(0, 1),
                generationExtents = Vector2.zero
            };
            float width = generator.GetPreferredWidth(text, settings);
            float height = generator.GetPreferredHeight(text, settings);
            if (!float.IsFinite(width) || !float.IsFinite(height))
                throw new InvalidOperationException("Font measurement returned a non-finite size.");
            // Preferred size describes line metrics; glyphs can extend beyond that box.
            settings.generationExtents = new Vector2(width, height);
            settings.updateBounds = false;
            if (!generator.Populate(text, settings))
                throw new InvalidOperationException("Font geometry could not be generated.");
            float minY = -height;
            float maxY = 0;
            var vertices = generator.verts;
            for (int index = 0, count = generator.vertexCount; index + 3 < count; index += 4)
            {
                Vector3 first = vertices[index].position;
                Vector3 opposite = vertices[index + 2].position;
                if (first.x == opposite.x || first.y == opposite.y)
                    continue;
                minY = MathF.Min(minY, MathF.Min(first.y, opposite.y));
                maxY = MathF.Max(maxY, MathF.Max(first.y, opposite.y));
            }
            measured = new ScriptUiTextMeasurement(width, height, minY, maxY);
            if (!float.IsFinite(measured.Height))
                throw new InvalidOperationException("Font geometry returned a non-finite size.");
            _textMeasurements[(text, font)] = measured;
            return true;
        }
        catch (Exception exception)
        {
            _textGenerator = null;
            Report($"Script UI text measurement failed: {exception.Message}");
            return false;
        }
    }

    private void ClearElements()
    {
        foreach (ElementView view in _elements)
            view.Dispose();
        _elements.Clear();
        _byInstance.Clear();
        _textMeasurements.Clear();
        foreach (Font? font in _scriptFonts.Values)
            if (font != null)
                UnityEngine.Object.Destroy(font);
        _scriptFonts.Clear();
    }

    private void Report(string message)
    {
        try { _log?.Invoke(message); } catch { }
    }

    public void Dispose()
    {
        if (_disposed)
            return;
        _disposed = true;
        _textGenerator = null;
        _pressed.Clear();
        ClearElements();
        UnityEngine.Object.Destroy(_root);
    }

    private sealed class ElementView : IDisposable
    {
        private readonly GameObject _object;
        private readonly RectTransform _rect;
        private readonly GameObject _outlineObject;
        private readonly ScriptUiImage _outline;
        private readonly GameObject _backgroundObject;
        private readonly ScriptUiImage _background;
        private readonly Image _fill;
        private readonly Mask _mask;
        private readonly GameObject _contentObject;
        private readonly RectTransform _contentRect;
        private readonly RectMask2D _contentMask;
        private readonly Text _text;
        private readonly RectTransform _textRect;
        private readonly Func<ScriptUiEvent, bool> _sendEvent;
        private readonly Action<string>? _log;
        private readonly EventTrigger _trigger;
        private ScriptUiElementState _state = null!;
        private ScriptUiTextMeasurement _measuredText;
        private bool _eventsEnabled;
        private bool _nativePressed;
        private bool _nativeInside;
        private bool _disposed;

        internal ElementView(Transform parent, Font font, Func<ScriptUiEvent, bool> sendEvent, Action<string>? log)
        {
            _sendEvent = sendEvent;
            _log = log;
            _object = new GameObject("Script UI Element", Il2CppType.Of<RectTransform>());
            _object.transform.SetParent(parent, false);
            _rect = _object.GetComponent<RectTransform>();
            _rect.anchorMin = Vector2.zero;
            _rect.anchorMax = Vector2.zero;
            _rect.pivot = new Vector2(0, 1);
            _outlineObject = new GameObject("Outline", Il2CppType.Of<RectTransform>());
            _outlineObject.transform.SetParent(_object.transform, false);
            _outline = _outlineObject.AddComponent<ScriptUiImage>();
            _outline.raycastTarget = false;
            _outlineObject.GetComponent<RectTransform>().anchorMin = Vector2.zero;
            _outlineObject.GetComponent<RectTransform>().anchorMax = Vector2.zero;
            _outlineObject.GetComponent<RectTransform>().pivot = new Vector2(0.5f, 0.5f);
            _backgroundObject = new GameObject("Background", Il2CppType.Of<RectTransform>());
            _backgroundObject.transform.SetParent(_object.transform, false);
            ScriptUiImage background = _backgroundObject.AddComponent<ScriptUiImage>();
            background.HitTest = point => ScriptUiGeometry.Contains(Layout, point.x, point.y);
            _background = background;
            _background.raycastTarget = false;
            _mask = _backgroundObject.AddComponent<Mask>();
            _mask.showMaskGraphic = false;
            _backgroundObject.GetComponent<RectTransform>().anchorMin = Vector2.zero;
            _backgroundObject.GetComponent<RectTransform>().anchorMax = Vector2.one;
            _backgroundObject.GetComponent<RectTransform>().offsetMin = Vector2.zero;
            _backgroundObject.GetComponent<RectTransform>().offsetMax = Vector2.zero;
            GameObject fillObject = new("Fill", Il2CppType.Of<RectTransform>());
            fillObject.transform.SetParent(_backgroundObject.transform, false);
            _fill = fillObject.AddComponent<Image>();
            _fill.raycastTarget = false;
            RectTransform fillRect = fillObject.GetComponent<RectTransform>();
            fillRect.anchorMin = Vector2.zero;
            fillRect.anchorMax = Vector2.one;
            fillRect.offsetMin = Vector2.zero;
            fillRect.offsetMax = Vector2.zero;
            _contentObject = new GameObject("Content", Il2CppType.Of<RectTransform>());
            _contentObject.transform.SetParent(_backgroundObject.transform, false);
            _contentRect = _contentObject.GetComponent<RectTransform>();
            _contentMask = _contentObject.AddComponent<RectMask2D>();
            _text = new GameObject("Text", Il2CppType.Of<RectTransform>()).AddComponent<Text>();
            _text.transform.SetParent(_contentObject.transform, false);
            _text.font = font;
            _text.fontSize = FontSize;
            _text.lineSpacing = 1;
            _text.alignment = TextAnchor.MiddleLeft;
            _text.horizontalOverflow = HorizontalWrapMode.Overflow;
            _text.verticalOverflow = VerticalWrapMode.Overflow;
            _text.supportRichText = false;
            _text.raycastTarget = false;
            _textRect = _text.GetComponent<RectTransform>();
            _textRect.anchorMin = Vector2.zero;
            _textRect.anchorMax = Vector2.one;
            _textRect.pivot = new Vector2(0, 1);
            _textRect.offsetMin = Vector2.zero;
            _textRect.offsetMax = Vector2.zero;
            _trigger = _backgroundObject.AddComponent<EventTrigger>();
        }

        internal GameObject Root => _object;
        internal Guid InstanceId => _state.InstanceId;
        internal ulong EventsVersion => _state.EventsVersion;
        internal ScriptUiLayout Layout { get; private set; }
        internal bool HasEvents => _state.Events is { Length: > 0 };

        internal void Apply(ScriptUiElementState state, int viewportWidth, int viewportHeight, ScriptUiTextMeasurement measuredText, Font font)
        {
            bool eventsChanged = _state is null ||
                _state.EventsVersion != state.EventsVersion ||
                !_state.Events.SequenceEqual(state.Events);
            _state = state;
            _measuredText = measuredText;
            Layout = ScriptUiGeometry.Calculate(state, viewportWidth, viewportHeight, measuredText.Width, measuredText.Height);
            _text.text = state.Text;
            _text.font = font;
            _text.alignment = (state.AlignX, state.AlignY) switch
            {
                ("left", "top") => TextAnchor.UpperLeft,
                ("center", "top") => TextAnchor.UpperCenter,
                ("right", "top") => TextAnchor.UpperRight,
                ("left", "center") => TextAnchor.MiddleLeft,
                ("center", "center") => TextAnchor.MiddleCenter,
                ("right", "center") => TextAnchor.MiddleRight,
                ("left", "bottom") => TextAnchor.LowerLeft,
                ("center", "bottom") => TextAnchor.LowerCenter,
                ("right", "bottom") => TextAnchor.LowerRight,
                _ => TextAnchor.MiddleLeft
            };
            _text.color = new Color32(state.TextColor[0], state.TextColor[1], state.TextColor[2], state.TextColor[3]);
            _background.color = Color.white;
            _fill.color = new Color32(state.Color[0], state.Color[1], state.Color[2], state.Color[3]);
            _outline.color = new Color32(state.Border.Color[0], state.Border.Color[1], state.Border.Color[2], state.Border.Color[3]);
            _text.gameObject.SetActive(Layout.Content.width > 0 && Layout.Content.height > measuredText.Top + measuredText.Bottom && state.Text.Length > 0);
            _object.SetActive(Layout.Box.width > 0 && Layout.Box.height > 0);
            _background.SetGeometry(new ScriptUiRect(0, 0, Layout.Box.width, Layout.Box.height), Layout.Radii);
            _outline.SetGeometry(new ScriptUiRect(0, 0, Layout.Outline.width, Layout.Outline.height), Layout.OutlineRadii, new ScriptUiRect(Layout.BorderThickness, Layout.BorderThickness, Layout.Box.width, Layout.Box.height), Layout.Radii);
            UpdateTransforms(viewportWidth, viewportHeight);
            if (eventsChanged)
            {
                _nativePressed = false;
                _nativeInside = false;
                BindEvents();
            }
        }

        internal void Reflow(int viewportWidth, int viewportHeight)
        {
            if (_disposed)
                return;
            Layout = ScriptUiGeometry.Calculate(_state, viewportWidth, viewportHeight, _measuredText.Width, _measuredText.Height);
            _object.SetActive(Layout.Box.width > 0 && Layout.Box.height > 0);
            _background.SetGeometry(new ScriptUiRect(0, 0, Layout.Box.width, Layout.Box.height), Layout.Radii);
            _outline.SetGeometry(new ScriptUiRect(0, 0, Layout.Outline.width, Layout.Outline.height), Layout.OutlineRadii, new ScriptUiRect(Layout.BorderThickness, Layout.BorderThickness, Layout.Box.width, Layout.Box.height), Layout.Radii);
            UpdateTransforms(viewportWidth, viewportHeight);
        }

        internal bool HasEvent(string name)
            => _state.Events.Contains(name, StringComparer.Ordinal);

        internal void SetEventsEnabled(bool enabled)
        {
            _eventsEnabled = enabled && HasEvents;
            _background.raycastTarget = _eventsEnabled;
            if (!_eventsEnabled)
            {
                _nativePressed = false;
                _nativeInside = false;
            }
        }

        internal void ClearPendingPointers()
        {
            _nativePressed = false;
            _nativeInside = false;
        }

        internal void Dispatch(string eventName)
        {
            if (_disposed || !_eventsEnabled || !HasEvent(eventName))
                return;
            try
            {
                _sendEvent(new ScriptUiEvent(_stateInstanceSessionId, _state.Id, _state.InstanceId, _state.EventsVersion, eventName));
            }
            catch (Exception exception)
            {
                try { _log?.Invoke($"Script UI event dispatch failed for '{_state.Id}': {exception.Message}"); } catch { }
            }
        }

        private Guid _stateInstanceSessionId;

        internal void SetSession(Guid sessionId)
            => _stateInstanceSessionId = sessionId;

        private void BindEvents()
        {
            _trigger.triggers.Clear();
            EventTrigger.Entry enter = new() { eventID = EventTriggerType.PointerEnter };
            enter.callback.AddListener((UnityAction<BaseEventData>)(data =>
            {
                if (_eventsEnabled && data.TryCast<PointerEventData>() is { } pointer && IsNativeHit(pointer))
                {
                    _nativeInside = true;
                    Dispatch("hover");
                }
            }));
            _trigger.triggers.Add(enter);
            EventTrigger.Entry leave = new() { eventID = EventTriggerType.PointerExit };
            leave.callback.AddListener((UnityAction<BaseEventData>)(_ =>
            {
                bool wasInside = _nativeInside;
                _nativeInside = false;
                if (wasInside)
                    Dispatch("leave");
            }));
            _trigger.triggers.Add(leave);
            EventTrigger.Entry down = new() { eventID = EventTriggerType.PointerDown };
            down.callback.AddListener((UnityAction<BaseEventData>)(data =>
            {
                if (_eventsEnabled && data.TryCast<PointerEventData>() is { } pointer && pointer.button == PointerEventData.InputButton.Left && IsNativeHit(pointer))
                    _nativePressed = true;
            }));
            _trigger.triggers.Add(down);
            EventTrigger.Entry up = new() { eventID = EventTriggerType.PointerUp };
            up.callback.AddListener((UnityAction<BaseEventData>)(data =>
            {
                bool wasPressed = _nativePressed;
                _nativePressed = false;
                if (wasPressed && _eventsEnabled && data.TryCast<PointerEventData>() is { } pointer && pointer.button == PointerEventData.InputButton.Left && IsNativeHit(pointer))
                    Dispatch("click");
            }));
            _trigger.triggers.Add(up);
        }

        private bool IsNativeHit(PointerEventData pointer)
            => ScriptUiGeometry.Contains(Layout, pointer.position.x, Screen.height - pointer.position.y);

        private void UpdateTransforms(int viewportWidth, int viewportHeight)
        {
            _rect.sizeDelta = new Vector2(Layout.Box.width, Layout.Box.height);
            _rect.anchoredPosition = new Vector2(Layout.Box.xMin, viewportHeight - Layout.Box.yMin);
            RectTransform outlineRect = _outlineObject.GetComponent<RectTransform>();
            outlineRect.anchorMin = new Vector2(0, 1);
            outlineRect.anchorMax = new Vector2(0, 1);
            outlineRect.sizeDelta = new Vector2(Layout.Outline.width, Layout.Outline.height);
            outlineRect.anchoredPosition = new Vector2(Layout.Box.width / 2, -Layout.Box.height / 2);
            _contentRect.anchorMin = new Vector2(0, 1);
            _contentRect.anchorMax = new Vector2(0, 1);
            _contentRect.pivot = new Vector2(0, 1);
            _contentRect.anchoredPosition = new Vector2(Layout.Content.xMin - Layout.Box.xMin, -(Layout.Content.yMin - Layout.Box.yMin));
            _contentRect.sizeDelta = new Vector2(Layout.Content.width, Layout.Content.height);
            _textRect.offsetMin = new Vector2(0, _measuredText.Bottom);
            _textRect.offsetMax = new Vector2(0, -_measuredText.Top);
        }

        public void Dispose()
        {
            if (_disposed)
                return;
            _disposed = true;
            UnityEngine.Object.Destroy(_object);
        }
    }
}
