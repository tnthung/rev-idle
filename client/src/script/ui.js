(function (host, codec) {
    // Capture host machinery before the user module can change this realm.
    const { create, keys, hasOwn, freeze, setPrototypeOf } = Object;
    const { ownKeys } = Reflect;
    const { isFinite, isInteger } = Number;
    const { isArray } = Array;
    const NativeProxy = Proxy, NativeError = Error, NativeTypeError = TypeError, NativeWeakSet = WeakSet, string = String;
    const elements = new Map();
    const states = new Map();
    const elementGet = elements.get.bind(elements), elementHas = elements.has.bind(elements), elementSet = elements.set.bind(elements);
    const stateGet = states.get.bind(states), stateHas = states.has.bind(states), stateSet = states.set.bind(states);
    const defaults = {
        hidden: false, basedOn: '', text: '', font: '', size: 14, posX: 0, posY: 0,
        lenX: { min: 0 }, lenY: { min: 0 }, alignX: 'left', alignY: 'center',
        color: [0, 0, 0, 0], textColor: [255, 255, 255, 255],
        border: { thickness: 0, color: [255, 255, 255] }, corner: { radius: 0 }, padding: { thickness: 0 },
    };
    const legacyHandlers = { __proto__: null, onClick: true, onHover: true, onLeave: true, onStateUpdate: true };

    function readonly(value, seen = new NativeWeakSet()) {
        if (codec.isBigNum(value)) return value;
        if (value === null || typeof value !== 'object') return value;
        if (seen.has(value)) throw new NativeTypeError('UI value is not JSON serializable');
        seen.add(value);
        const copy = isArray(value) ? [] : create(null);
        const fields = keys(value);
        for (let i = 0; i < fields.length; i++) copy[fields[i]] = readonly(value[fields[i]], seen);
        seen.delete(value);
        freeze(copy);
        return new NativeProxy(copy, {
            set() { throw new NativeTypeError('Replace the whole UI field'); },
            deleteProperty() { throw new NativeTypeError('Replace the whole UI field'); },
            defineProperty() { throw new NativeTypeError('Replace the whole UI field'); },
            setPrototypeOf() { throw new NativeTypeError('Replace the whole UI field'); },
        });
    }
    const defaultKeys = keys(defaults);
    for (let i = 0; i < defaultKeys.length; i++) defaults[defaultKeys[i]] = readonly(defaults[defaultKeys[i]]);

    function ensureLive() {
        if (host.stopped()) throw new NativeError('script session stopped');
    }

    const encode = codec.encode;

    function checkObject(value, field, allowed) {
        if (value === null || typeof value !== 'object' || isArray(value)) throw new NativeTypeError(`Invalid ${field}`);
        const result = create(null), fields = ownKeys(value);
        for (let i = 0; i < fields.length; i++) {
            const key = fields[i];
            let known = false;
            for (let j = 0; j < allowed.length; j++) if (allowed[j] === key) known = true;
            if (typeof key !== 'string' || !known) throw new NativeTypeError(`Unknown ${field} field: ${string(key)}`);
            result[key] = value[key];
        }
        return result;
    }

    function validate(field, value) {
        if (legacyHandlers[field]) throw new NativeTypeError(`${field} is configured through its setter`);
        if (field === 'states') {
            if (value === null || typeof value !== 'object' || isArray(value)) throw new NativeTypeError('Invalid states');
            const copy = create(null), fields = ownKeys(value);
            for (let i = 0; i < fields.length; i++) {
                if (typeof fields[i] !== 'string') throw new NativeTypeError('State keys must be strings');
                copy[fields[i]] = value[fields[i]];
            }
            return readonly(copy);
        }
        if (!hasOwn(defaults, field)) throw new NativeTypeError(`Unknown UI field: ${string(field)}`);
        if (field === 'hidden') {
            if (typeof value !== 'boolean') throw new NativeTypeError('hidden must be a boolean');
            return value;
        }
        if (field === 'basedOn' || field === 'text' || field === 'font') {
            if (typeof value !== 'string') throw new NativeTypeError(`${field} must be a string`);
            return value;
        }
        if (field === 'size') {
            if (typeof value !== 'number' || !isInteger(value) || value <= 0 || value > 2147483647) throw new NativeTypeError('Invalid size');
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
        const allowed = field === 'border' ? ['thickness', 'color']
            : field === 'corner' ? ['radius', 'topLeft', 'topRight', 'bottomLeft', 'bottomRight']
            : field === 'padding' ? ['thickness', 'top', 'right', 'bottom', 'left'] : ['min', 'max'];
        const copy = checkObject(value, field, allowed), fields = keys(copy);
        for (let i = 0; i < fields.length; i++) {
            const key = fields[i], item = copy[key];
            if (field === 'border' && key === 'color') copy[key] = validate('color', item);
            else if (typeof item !== 'number' || !isFinite(item) || item < 0) throw new NativeTypeError(`Invalid ${field}.${key}`);
        }
        if ((field === 'lenX' || field === 'lenY') && copy.max !== undefined && copy.max < (copy.min ?? 0)) throw new NativeTypeError('Maximum length is less than minimum');
        return readonly(copy);
    }

    function prepareDefinition(definition) {
        ensureLive();
        if (definition === null || typeof definition !== 'object' || isArray(definition)) throw new NativeTypeError('UI definition must be an object');
        const values = create(null), fields = ownKeys(definition);
        for (let i = 0; i < fields.length; i++) {
            const field = fields[i];
            if (typeof field !== 'string') throw new NativeTypeError('UI definition fields must be strings');
            values[field] = validate(field, definition[field]);
        }
        return encode(values);
    }

    function record(name, instance) {
        return codec.decode(host.record(name, instance));
    }

    function stateProxy(name, instance) {
        if (stateHas(instance)) return stateGet(instance);
        const proxy = new NativeProxy(create(null), {
            get(_, field) {
                if (typeof field !== 'string') return undefined;
                const value = host.stateGet(name, instance, field);
                return value === undefined ? undefined : codec.decode(value);
            },
            has(_, field) { return typeof field === 'string' && host.stateGet(name, instance, field) !== undefined; },
            ownKeys() { return host.stateKeys(name, instance); },
            getOwnPropertyDescriptor(_, field) {
                if (typeof field !== 'string') return undefined;
                const value = host.stateGet(name, instance, field);
                return value === undefined ? undefined : { configurable: true, enumerable: true, writable: true, value: codec.decode(value) };
            },
            set(_, field, value) {
                ensureLive();
                if (typeof field !== 'string') throw new NativeTypeError('State keys must be strings');
                host.stateSet(name, instance, field, encode(value));
                return true;
            },
            deleteProperty(_, field) {
                ensureLive();
                if (typeof field === 'string') host.stateDelete(name, instance, field);
                return true;
            },
            defineProperty() { throw new NativeTypeError('UI state property descriptors are unsupported'); },
            setPrototypeOf() { throw new NativeTypeError('UI state prototypes are unsupported'); },
            preventExtensions() { throw new NativeTypeError('UI state maps cannot be frozen'); },
        });
        stateSet(instance, proxy);
        return proxy;
    }

    function elementProxy(name, instance) {
        if (elementHas(instance)) return elementGet(instance);
        const eventMethods = create(null), elementCallbacks = create(null);
        const method = event => callback => {
            ensureLive();
            if (callback !== null && typeof callback !== 'function') throw new NativeTypeError(`${event} callback must be a function or null`);
            if (callback === null) {
                host.handlerClear(name, instance, event);
                delete elementCallbacks[event];
                return proxy;
            }
            const previous = elementCallbacks[event];
            if (previous !== undefined && previous.callback === callback) {
                const status = host.handlerStatus(previous.registration);
                if (status === 'pending' || status === 'active') return proxy;
            }
            const registration = host.handlerRegister(name, instance, event, callback);
            elementCallbacks[event] = { callback, registration };
            return proxy;
        };
        eventMethods.setOnClick = method('click');
        eventMethods.setOnHover = method('hover');
        eventMethods.setOnLeave = method('leave');
        eventMethods.setOnStateUpdate = method('stateUpdate');
        eventMethods.update = () => {
            ensureLive();
            host.update(name, instance);
            return proxy;
        };
        const dimension = kind => async relativeTo => {
            ensureLive();
            const position = kind === 'globalX' || kind === 'globalY';
            if (position && relativeTo !== undefined && typeof relativeTo !== 'string') throw new NativeTypeError('relativeTo must be a string');
            const value = await host.measure(name, instance, kind, position ? relativeTo ?? '' : '');
            if (position && value === null) throw new NativeError('UI element basedOn or relativeTo target is inaccessible');
            return value;
        };
        const dimensions = {
            width: dimension('width'),
            height: dimension('height'),
            globalXPos: dimension('globalX'),
            globalYPos: dimension('globalY'),
        };
        const proxy = new NativeProxy(create(null), {
            get(_, field) {
                const values = record(name, instance).values;
                if (field === 'states') return stateProxy(name, instance);
                if (hasOwn(eventMethods, field)) return eventMethods[field];
                if (hasOwn(dimensions, field)) return dimensions[field];
                if (hasOwn(values, field)) return readonly(values[field]);
                return hasOwn(defaults, field) ? defaults[field] : undefined;
            },
            has(_, field) {
                const values = record(name, instance).values;
                return field === 'states' || hasOwn(eventMethods, field) || hasOwn(dimensions, field) || hasOwn(values, field) || hasOwn(defaults, field);
            },
            ownKeys() {
                const values = record(name, instance).values;
                const fields = keys(values);
                fields[fields.length] = 'states';
                return fields;
            },
            getOwnPropertyDescriptor(_, field) {
                if (field === 'states') return { configurable: true, enumerable: true, writable: true, value: stateProxy(name, instance) };
                if (hasOwn(eventMethods, field)) return { configurable: true, enumerable: false, writable: false, value: eventMethods[field] };
                if (hasOwn(dimensions, field)) return { configurable: true, enumerable: false, writable: false, value: dimensions[field] };
                const values = record(name, instance).values;
                return hasOwn(values, field) ? { configurable: true, enumerable: true, writable: true, value: readonly(values[field]) } : undefined;
            },
            set(_, field, value) {
                ensureLive();
                if (field === 'states') {
                    host.setField(name, instance, 'states', encode(validate(field, value)));
                    return true;
                }
                if (hasOwn(eventMethods, field) || hasOwn(dimensions, field)) throw new NativeTypeError('UI methods are read-only');
                host.setField(name, instance, field, encode(validate(field, value)));
                return true;
            },
            deleteProperty(_, field) {
                ensureLive();
                if (hasOwn(eventMethods, field) || hasOwn(dimensions, field)) throw new NativeTypeError('UI methods are read-only');
                if (legacyHandlers[field]) return true;
                if (field === 'states' || hasOwn(defaults, field)) host.deleteField(name, instance, field);
                return true;
            },
            defineProperty() { throw new NativeTypeError('UI property descriptors are unsupported'); },
            setPrototypeOf() { throw new NativeTypeError('UI prototypes are unsupported'); },
            preventExtensions() { throw new NativeTypeError('UI elements cannot be frozen'); },
        });
        elementSet(instance, proxy);
        return proxy;
    }

    const registry = new NativeProxy(setPrototypeOf((name, definition) => {
        if (typeof name !== 'string' || name.length === 0) throw new NativeTypeError('UI names must be nonempty strings');
        if (definition === null) {
            ensureLive();
            host.remove(name);
            return;
        }
        return elementProxy(name, host.define(name, prepareDefinition(definition)));
    }, null), {
        get(_, key) {
            if (typeof key !== 'string') return undefined;
            const instance = host.lookup(key);
            return instance === undefined ? undefined : elementProxy(key, instance);
        },
        has(_, key) { return typeof key === 'string' && host.lookup(key) !== undefined; },
        ownKeys() { return host.names(); },
        getOwnPropertyDescriptor(_, key) {
            if (typeof key !== 'string') return undefined;
            const instance = host.lookup(key);
            return instance === undefined ? undefined : { configurable: true, enumerable: true, writable: false, value: elementProxy(key, instance) };
        },
        set() { throw new NativeTypeError('Create or update UI elements with rev.ui(name, attributes)'); },
        deleteProperty() { throw new NativeTypeError('Remove UI elements with rev.ui(name, null)'); },
        defineProperty() { throw new NativeTypeError('UI property descriptors are unsupported'); },
        setPrototypeOf() { throw new NativeTypeError('UI registry prototypes are unsupported'); },
        preventExtensions() { throw new NativeTypeError('UI registry cannot be frozen'); },
    });

    function element(name, instance) {
        record(name, instance);
        return elementProxy(name, instance);
    }

    return { registry, element };
})
