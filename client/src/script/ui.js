(function (publish, identity, stopped, sessionId, measure) {
    // Capture host machinery before the user module can change its realm.
    const { create, keys, hasOwn, freeze, setPrototypeOf } = Object;
    const { ownKeys, apply } = Reflect;
    const { isFinite, isInteger } = Number;
    const { isArray } = Array;
    const { stringify } = JSON;
    const NativeProxy = Proxy, NativeError = Error, NativeTypeError = TypeError, string = String;
    const elements = new Map();
    const get = elements.get.bind(elements), has = elements.has.bind(elements);
    const set = elements.set.bind(elements), remove = elements.delete.bind(elements), each = elements.forEach.bind(elements);
    const handlers = { onHover: 'hover', onLeave: 'leave', onClick: 'click' };
    const defaults = {
        hidden: false, text: '', font: '', posX: 0, posY: 0, lenX: { min: 0 }, lenY: { min: 0 },
        alignX: 'left', alignY: 'center',
        color: [0, 0, 0, 0], textColor: [255, 255, 255, 255],
        border: { thickness: 0, color: [255, 255, 255, 255] },
        corner: { radius: 0 }, padding: { thickness: 0 },
    };

    function readonly(value) {
        if (value === null || typeof value !== 'object') return value;
        const copy = isArray(value) ? [] : create(null);
        const fields = keys(value);
        for (let i = 0; i < fields.length; i++) copy[fields[i]] = readonly(value[fields[i]]);
        freeze(copy);
        return new NativeProxy(copy, {
            set() { throw new NativeTypeError('Replace the whole UI field'); },
            deleteProperty() { throw new NativeTypeError('Replace the whole UI field'); },
            defineProperty() { throw new NativeTypeError('Replace the whole UI field'); },
            setPrototypeOf() { throw new NativeTypeError('Replace the whole UI field'); },
        });
    }
    const defaultKeys = keys(defaults), handlerKeys = keys(handlers);
    for (let i = 0; i < defaultKeys.length; i++) defaults[defaultKeys[i]] = readonly(defaults[defaultKeys[i]]);

    function validate(field, value) {
        if (hasOwn(handlers, field)) {
            if (typeof value !== 'function') throw new NativeTypeError(`${field} must be a function`);
            return value;
        }
        if (!hasOwn(defaults, field)) throw new NativeTypeError(`Unknown UI field: ${string(field)}`);
        if (field === 'hidden') {
            if (typeof value !== 'boolean') throw new NativeTypeError('hidden must be a boolean');
            return value;
        }
        if (field === 'text' || field === 'font') {
            if (typeof value !== 'string') throw new NativeTypeError(`${field} must be a string`);
            return value;
        }
        if (field === 'alignX') {
            if (value !== 'left' && value !== 'center' && value !== 'right') throw new NativeTypeError('Invalid alignX');
            return value;
        }
        if (field === 'alignY') {
            if (value !== 'top' && value !== 'center' && value !== 'bottom') throw new NativeTypeError('Invalid alignY');
            return value;
        }
        if (field === 'posX' || field === 'posY' || ((field === 'lenX' || field === 'lenY') && typeof value === 'number')) {
            if (typeof value !== 'number' || !isFinite(value) || ((field === 'lenX' || field === 'lenY') && value < 0)) throw new NativeTypeError(`Invalid ${field}`);
            return value;
        }
        if (field === 'color' || field === 'textColor') {
            if (!isArray(value) || (value.length !== 3 && value.length !== 4)) throw new NativeTypeError(`Invalid ${field}`);
            const copy = [];
            for (let i = 0; i < value.length; i++) {
                const channel = value[i];
                if (!isInteger(channel) || channel < 0 || channel > 255) throw new NativeTypeError(`Invalid ${field} channel`);
                copy[i] = channel;
            }
            return readonly(copy);
        }
        if (value === null || typeof value !== 'object' || isArray(value)) throw new NativeTypeError(`Invalid ${field}`);
        const allowed = field === 'border' ? ['thickness', 'color']
            : field === 'corner' ? ['radius', 'topLeft', 'topRight', 'bottomLeft', 'bottomRight']
            : field === 'padding' ? ['thickness', 'top', 'right', 'bottom', 'left'] : ['min', 'max'];
        const copy = create(null), fields = ownKeys(value);
        for (let i = 0; i < fields.length; i++) {
            const key = fields[i];
            let known = false;
            for (let j = 0; j < allowed.length; j++) if (allowed[j] === key) known = true;
            if (!known) throw new NativeTypeError(`Unknown ${field} field: ${string(key)}`);
            const item = value[key];
            if (field === 'border' && key === 'color') copy[key] = validate('color', item);
            else {
                if (typeof item !== 'number' || !isFinite(item) || item < 0) throw new NativeTypeError(`Invalid ${field}.${key}`);
                copy[key] = item;
            }
        }
        if ((field === 'lenX' || field === 'lenY') && copy.max !== undefined && copy.max < (copy.min ?? 0)) throw new NativeTypeError('Maximum length is less than minimum');
        return readonly(copy);
    }

    function equal(left, right) {
        if (left === right) return true;
        if (!left || !right || typeof left !== 'object' || typeof right !== 'object') return false;
        const fields = keys(left);
        if (fields.length !== keys(right).length) return false;
        for (let i = 0; i < fields.length; i++) {
            const key = fields[i];
            if (!hasOwn(right, key) || !equal(left[key], right[key])) return false;
        }
        return true;
    }

    function changed(changedKey, replacement) {
        const states = setPrototypeOf([], null);
        function append(element, id) {
            const values = { __proto__: null, ...defaults, ...element.values };
            const lengths = create(null), axes = ['lenX', 'lenY'];
            for (let i = 0; i < axes.length; i++) {
                const axis = axes[i];
                const length = values[axis];
                lengths[axis] = typeof length === 'number' ? { __proto__: null, fixed: length, min: 0, max: null }
                    : { __proto__: null, fixed: null, min: length.min ?? 0, max: length.max ?? null };
            }
            const corner = create(null), padding = create(null);
            const corners = ['topLeft', 'topRight', 'bottomLeft', 'bottomRight'], edges = ['top', 'right', 'bottom', 'left'];
            for (let i = 0; i < corners.length; i++) corner[corners[i]] = values.corner[corners[i]] ?? values.corner.radius ?? 0;
            for (let i = 0; i < edges.length; i++) padding[edges[i]] = values.padding[edges[i]] ?? values.padding.thickness ?? 0;
            const colors = create(null), colorNames = ['color', 'textColor'];
            for (let i = 0; i < colorNames.length; i++) {
                const color = values[colorNames[i]];
                colors[colorNames[i]] = setPrototypeOf([color[0], color[1], color[2], color[3] ?? 255], null);
            }
            const borderColor = values.border.color ?? defaults.border.color;
            const events = setPrototypeOf([], null);
            for (let i = 0; i < handlerKeys.length; i++) if (element.values[handlerKeys[i]]) events[events.length] = handlers[handlerKeys[i]];
            states[states.length] = { __proto__: null, id, instanceId: element.instanceId, eventsVersion: element.eventsVersion,
                hidden: values.hidden, text: values.text, font: values.font, alignX: values.alignX, alignY: values.alignY, posX: values.posX, posY: values.posY, ...lengths, ...colors,
                border: { __proto__: null, thickness: values.border.thickness ?? 0, color: setPrototypeOf([borderColor[0], borderColor[1], borderColor[2], borderColor[3] ?? 255], null) },
                corner, padding, events };
        }
        each((element, id) => {
            if (id !== changedKey) append(element, id);
            else if (replacement) append(replacement, id);
        });
        if (replacement && !has(changedKey)) append(replacement, changedKey);
        publish(stringify(states));
    }

    const registry = new NativeProxy(create(null), {
        get(_, key) { return get(key)?.proxy; },
        has(_, key) { return has(key); },
        ownKeys() { const names = []; each((_, key) => { names[names.length] = key; }); return names; },
        getOwnPropertyDescriptor(_, key) {
            return has(key) ? { configurable: true, enumerable: true, writable: true, value: get(key).proxy } : undefined;
        },
        set(_, key, definition) {
            if (stopped()) throw new NativeError('script session stopped');
            if (typeof key !== 'string' || key.length === 0) throw new NativeTypeError('UI names must be nonempty strings');
            if (definition === null || typeof definition !== 'object' || isArray(definition)) throw new NativeTypeError('UI definition must be an object');
            let values = create(null);
            const fields = ownKeys(definition);
            for (let i = 0; i < fields.length; i++) values[fields[i]] = validate(fields[i], definition[fields[i]]);
            if (stopped()) throw new NativeError('script session stopped');
            const element = { values, instanceId: identity(), eventsVersion: 1, proxy: null };
            async function dimension(width) {
                if (stopped()) throw new NativeError('script session stopped');
                if (get(key) !== element) throw new NativeError('UI element no longer exists');
                const size = await measure(key, element.instanceId, width);
                if (stopped()) throw new NativeError('script session stopped');
                if (get(key) !== element) throw new NativeError('UI element no longer exists');
                return size;
            }
            const methods = { __proto__: null, width: () => dimension(true), height: () => dimension(false) };
            element.proxy = new NativeProxy(create(null), {
                get(_, field) { return hasOwn(methods, field) ? methods[field] : hasOwn(values, field) ? values[field] : hasOwn(defaults, field) ? defaults[field] : undefined; },
                has(_, field) { return hasOwn(methods, field) || hasOwn(values, field) || hasOwn(defaults, field); },
                ownKeys() { return keys(values); },
                getOwnPropertyDescriptor(_, field) {
                    if (hasOwn(methods, field)) return { configurable: true, enumerable: false, writable: false, value: methods[field] };
                    return hasOwn(values, field) ? { configurable: true, enumerable: true, writable: true, value: values[field] } : undefined;
                },
                set(_, field, value) {
                    if (stopped()) throw new NativeError('script session stopped');
                    if (get(key) !== element) throw new NativeError('UI element no longer exists');
                    const next = validate(field, value);
                    if (stopped()) throw new NativeError('script session stopped');
                    if (get(key) !== element) throw new NativeError('UI element no longer exists');
                    if (equal(hasOwn(values, field) ? values[field] : defaults[field], next)) return true;
                    const nextValues = { __proto__: null, ...values, [field]: next };
                    const eventsVersion = element.eventsVersion + (hasOwn(handlers, field) ? 1 : 0);
                    changed(key, { ...element, values: nextValues, eventsVersion });
                    element.values = values = nextValues;
                    element.eventsVersion = eventsVersion;
                    return true;
                },
                deleteProperty(_, field) {
                    if (stopped()) throw new NativeError('script session stopped');
                    if (get(key) !== element) throw new NativeError('UI element no longer exists');
                    if (hasOwn(methods, field)) throw new NativeTypeError('UI methods are read-only');
                    if (!hasOwn(values, field)) return true;
                    const nextValues = { __proto__: null, ...values };
                    delete nextValues[field];
                    const eventsVersion = element.eventsVersion + (hasOwn(handlers, field) ? 1 : 0);
                    changed(key, { ...element, values: nextValues, eventsVersion });
                    element.values = values = nextValues;
                    element.eventsVersion = eventsVersion;
                    return true;
                },
                defineProperty() { throw new NativeTypeError('UI property descriptors are unsupported'); },
                setPrototypeOf() { throw new NativeTypeError('UI prototypes are unsupported'); },
                preventExtensions() { throw new NativeTypeError('UI elements cannot be frozen'); },
            });
            changed(key, element);
            set(key, element);
            return true;
        },
        deleteProperty(_, key) {
            if (stopped()) throw new NativeError('script session stopped');
            if (has(key)) { changed(key, null); remove(key); }
            return true;
        },
        defineProperty() { throw new NativeTypeError('UI property descriptors are unsupported'); },
        setPrototypeOf() { throw new NativeTypeError('UI prototypes are unsupported'); },
        preventExtensions() { throw new NativeTypeError('UI registry cannot be frozen'); },
    });

    return { registry, dispatch(event) {
        if (stopped() || event.sessionId !== sessionId) return;
        const element = get(event.elementId);
        if (!element || element.instanceId !== event.instanceId || element.eventsVersion !== event.eventsVersion) return;
        for (let i = 0; i < handlerKeys.length; i++) {
            const field = handlerKeys[i];
            if (handlers[field] === event.event && element.values[field]) return apply(element.values[field], undefined, []);
        }
    } };
})
