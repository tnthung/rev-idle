(function (isBigNum, decimal, createBigNum, isColor, channels, createColor, isRect, rectFields, createRect) {
    const { parse, stringify } = JSON;
    const { keys, create, hasOwn } = Object;
    const { isArray } = Array;
    const { isFinite } = Number;
    const sort = Array.prototype.sort;
    const { apply } = Reflect;
    const tag = Object.prototype.toString;
    const numberValue = Number.prototype.valueOf, stringValue = String.prototype.valueOf;
    const booleanValue = Boolean.prototype.valueOf, bigintValue = BigInt.prototype.valueOf;
    const string = String;
    const NativeWeakSet = WeakSet, NativeSet = Set, NativeTypeError = TypeError;

    return {
        isNative: value => isBigNum(value) || isColor(value) || isRect(value),
        encode(value) {
            const bigNums = [], colors = [], rects = [], seen = new NativeWeakSet();
            function visit(value, path, key) {
                if (isBigNum(value)) {
                    bigNums.push(path);
                    return decimal(value);
                }
                if (isColor(value)) {
                    colors.push(path);
                    return channels(value);
                }
                if (isRect(value)) {
                    rects.push(path);
                    return rectFields(value);
                }
                if (value !== null && (typeof value === 'object' || typeof value === 'bigint')) {
                    const toJSON = value.toJSON;
                    if (typeof toJSON === 'function') value = apply(toJSON, value, [key]);
                }
                if (isBigNum(value)) {
                    bigNums.push(path);
                    return decimal(value);
                }
                if (isColor(value)) {
                    colors.push(path);
                    return channels(value);
                }
                if (isRect(value)) {
                    rects.push(path);
                    return rectFields(value);
                }
                if (value === null || typeof value !== 'object') return value;
                switch (apply(tag, value, [])) {
                    case '[object Number]': return apply(numberValue, value, []);
                    case '[object String]': return apply(stringValue, value, []);
                    case '[object Boolean]': return apply(booleanValue, value, []);
                    case '[object BigInt]': return apply(bigintValue, value, []);
                }
                if (seen.has(value)) throw new NativeTypeError('Value contains a circular reference');
                seen.add(value);
                const result = isArray(value) ? [] : create(null);
                if (isArray(value)) {
                    for (let i = 0; i < value.length; i++) result[i] = visit(value[i], [...path, string(i)], string(i));
                } else {
                    for (const field of keys(value)) {
                        const item = visit(value[field], [...path, field], field);
                        if (typeof item !== 'function' && typeof item !== 'symbol' && item !== undefined) result[field] = item;
                    }
                }
                seen.delete(value);
                return result;
            }
            const json = stringify(visit(value === undefined ? null : value, [], ''));
            if (json === undefined) throw new NativeTypeError('Value is not JSON serializable');
            for (const paths of [bigNums, colors, rects]) {
                apply(sort, paths, [(left, right) => {
                    const a = stringify(left), b = stringify(right);
                    return a < b ? -1 : a > b ? 1 : 0;
                }]);
            }
            return stringify({ value: parse(json), bigNums, colors, rects });
        },
        decode(json) {
            const envelope = parse(json);
            if (envelope === null || typeof envelope !== 'object' || !hasOwn(envelope, 'value')
                || (envelope.bigNums !== undefined && !isArray(envelope.bigNums))
                || (envelope.colors !== undefined && !isArray(envelope.colors))
                || (envelope.rects !== undefined && !isArray(envelope.rects)))
                throw new NativeTypeError('Invalid typed value envelope');
            const seen = new NativeSet();
            for (const [paths, kind] of [[envelope.bigNums ?? [], 'bigNum'], [envelope.colors ?? [], 'color'], [envelope.rects ?? [], 'rect']]) {
                for (const path of paths) {
                    if (!isArray(path) || path.some(part => typeof part !== 'string') || seen.has(stringify(path)))
                        throw new NativeTypeError('Invalid or duplicate native value path');
                    seen.add(stringify(path));
                    let parent = envelope, field = 'value';
                    for (const part of path) {
                        parent = parent[field];
                        if (parent === null || typeof parent !== 'object' || !hasOwn(parent, part)
                            || (isArray(parent) && !/^(0|[1-9][0-9]*)$/.test(part)))
                            throw new NativeTypeError('Native value path does not resolve to a value');
                        field = part;
                    }
                    if (kind === 'rect') {
                        const value = parent[field];
                        if (value === null || typeof value !== 'object' || isArray(value)
                            || ['top', 'left', 'right', 'bottom', 'width', 'height'].some(key => !hasOwn(value, key) || typeof value[key] !== 'number' || !isFinite(value[key])))
                            throw new NativeTypeError('Rect path must identify six numeric fields');
                        parent[field] = createRect(value);
                    } else if (kind === 'color') {
                        if (!isArray(parent[field]) || parent[field].length !== 4
                            || parent[field].some(channel => typeof channel !== 'number' || !isFinite(channel) || channel < 0 || channel > 255))
                            throw new NativeTypeError('Color path must identify RGBA channels');
                        parent[field] = createColor(parent[field]);
                    } else {
                        if (typeof parent[field] !== 'string') throw new NativeTypeError('BigNum path must identify a string');
                        parent[field] = createBigNum(parent[field]);
                    }
                }
            }
            return envelope.value;
        },
    };
})
