use rquickjs::{Class, Ctx, Exception, Function, Object, Value};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct EncodedValue {
    pub(super) value: JsonValue,
    #[serde(rename = "bigNums", default, skip_serializing_if = "Vec::is_empty")]
    pub(super) big_nums: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) colors: Vec<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use rquickjs::CatchResultExt;

    #[test]
    fn typed_values_preserve_native_numbers_and_json_semantics() {
        let runtime = rquickjs::Runtime::new().unwrap();
        let context = rquickjs::Context::full(&runtime).unwrap();
        context.with(|ctx| {
            super::super::bignum::install(&ctx).unwrap();
            super::super::color::install(&ctx).unwrap();
            ctx.globals().set("codec", super::codec(&ctx).unwrap()).unwrap();
            ctx.eval::<(), _>(r#"
                const source = { value: new BigNum('1.25e300'), text: '1.25e300', nested: [new BigNum(-2)], bigNums: [[]] };
                const copy = codec.decode(codec.encode(source));
                if (!(copy.value instanceof BigNum) || !copy.value.eq(source.value)) throw Error('native value');
                if (!(copy.nested[0] instanceof BigNum) || !copy.nested[0].eq(-2)) throw Error('nested value');
                if (copy.text !== source.text || JSON.stringify(copy.bigNums) !== '[[]]') throw Error('ordinary values');
                if (!(codec.decode(codec.encode(BigNum.ONE)) instanceof BigNum)) throw Error('root value');
                if (codec.decode(codec.encode(undefined)) !== null) throw Error('undefined');
                let calls = 0;
                const item = { toJSON() { calls++; return { amount: BigNum.ONE, toJSON() { throw Error('called twice'); } }; } };
                if (!codec.decode(codec.encode(item)).amount.eq(1) || calls !== 1) throw Error('toJSON');
                const circular = {}; circular.self = circular;
                for (const value of [circular, 1n]) {
                    let threw = false;
                    try { codec.encode(value); } catch { threw = true; }
                    if (!threw) throw Error('invalid JSON accepted');
                }
                JSON.parse = () => { throw Error('replaced parse'); };
                JSON.stringify = () => { throw Error('replaced stringify'); };
                if (!codec.decode(codec.encode(source)).value.eq(source.value)) throw Error('captured intrinsics');
            "#).catch(&ctx).unwrap();
        });
    }

    #[test]
    fn malformed_number_metadata_rejects() {
        let runtime = rquickjs::Runtime::new().unwrap();
        let context = rquickjs::Context::full(&runtime).unwrap();
        context.with(|ctx| {
            super::super::bignum::install(&ctx).unwrap();
            super::super::color::install(&ctx).unwrap();
            ctx.globals().set("codec", super::codec(&ctx).unwrap()).unwrap();
            ctx.eval::<(), _>(r#"
                for (const envelope of [
                    {}, { value: '1e3', bigNums: null }, { value: '1e3', bigNums: [[], []] },
                    { value: { x: 1 }, bigNums: [['x']] }, { value: {}, bigNums: [['missing']] },
                    { value: ['1e3'], bigNums: [['01']] }, { value: ['1e3'], bigNums: [['1']] },
                    { value: ['1e3'], bigNums: [[0]] }, { value: 'NaN', bigNums: [[]] },
                ]) {
                    let threw = false;
                    try { codec.decode(JSON.stringify(envelope)); } catch { threw = true; }
                    if (!threw) throw Error('accepted ' + JSON.stringify(envelope));
                }
            "#).catch(&ctx).unwrap();
        });
    }

    #[test]
    fn typed_colors_preserve_fractional_channels_and_ordinary_arrays() {
        let runtime = rquickjs::Runtime::new().unwrap();
        let context = rquickjs::Context::full(&runtime).unwrap();
        context.with(|ctx| {
            super::super::bignum::install(&ctx).unwrap();
            super::super::color::install(&ctx).unwrap();
            ctx.globals().set("codec", super::codec(&ctx).unwrap()).unwrap();
            ctx.eval::<(), _>(r#"
                const color = Color.fromRgb(1.25, 2.5, 3.75, 127.5);
                const source = { color, nested: [color, BigNum.ONE], array: [1, 3, 4, 128], colors: [[]] };
                const copy = codec.decode(codec.encode(source));
                if (!(copy.color instanceof Color) || !copy.color.equals(color)) throw Error('color precision lost');
                if (!(copy.nested[0] instanceof Color) || !copy.nested[1].eq(1)) throw Error('nested native values lost');
                if (!Array.isArray(copy.array) || JSON.stringify(copy.colors) !== '[[]]') throw Error('ordinary arrays changed');
                if (JSON.stringify(color) !== '[1,3,4,128]') throw Error('ordinary JSON changed');
                if (!codec.decode(codec.encode(color)).equals(color)) throw Error('root color lost');
                let calls = 0;
                const item = { toJSON() { calls++; return color; } };
                if (!codec.decode(codec.encode(item)).equals(color) || calls !== 1) throw Error('toJSON color lost');
                if (JSON.stringify(JSON.parse(codec.encode({ a: color, b: color })).colors)
                    !== JSON.stringify(JSON.parse(codec.encode({ b: color, a: color })).colors)) throw Error('unstable metadata order');
                for (const envelope of [
                    { value: [1, 2, 3, 4], colors: null }, { value: [1, 2, 3, 4], colors: [[], []] },
                    { value: [1, 2, 3], colors: [[]] }, { value: [1, 2, 3, '4'], colors: [[]] },
                    { value: [1, 2, 3, null], colors: [[]] }, { value: {}, colors: [['missing']] },
                    { value: [[1, 2, 3, 4]], colors: [['00']] },
                ]) {
                    let threw = false;
                    try { codec.decode(JSON.stringify(envelope)); } catch { threw = true; }
                    if (!threw) throw Error('accepted ' + JSON.stringify(envelope));
                }
                JSON.parse = () => { throw Error('replaced parse'); };
                JSON.stringify = () => { throw Error('replaced stringify'); };
                if (!codec.decode(codec.encode(source)).color.equals(color)) throw Error('captured intrinsics');
            "#).catch(&ctx).unwrap();
        });
    }

    #[test]
    fn native_color_channels_survive_host_json_storage() {
        let runtime = rquickjs::Runtime::new().unwrap();
        let context = rquickjs::Context::full(&runtime).unwrap();
        context.with(|ctx| {
            super::super::color::install(&ctx).unwrap();
            ctx.globals().set("codec", super::codec(&ctx).unwrap()).unwrap();
            let encoded: String = ctx.eval(r#"
                const color = Color.fromRgb(51.248178375505404, 40.25, 20.75, 127.5);
                codec.encode({ color });
            "#).catch(&ctx).unwrap();
            let stored: super::EncodedValue = serde_json::from_str(&encoded).unwrap();
            ctx.globals().set("stored", serde_json::to_string(&stored).unwrap()).unwrap();
            assert!(ctx.eval::<bool, _>("codec.decode(stored).color.equals(color)").catch(&ctx).unwrap());
        });
    }
}

pub(super) fn codec<'js>(ctx: &Ctx<'js>) -> rquickjs::Result<Object<'js>> {
    ctx.eval::<Function, _>(include_str!("value.js"))?.call((
        Function::new(ctx.clone(), |value: Value<'js>| {
            value.as_object().is_some_and(|object| object.instance_of::<super::bignum::BigNum>())
        })?,
        Function::new(ctx.clone(), |value: Class<'js, super::bignum::BigNum>| value.borrow().decimal())?,
        Function::new(ctx.clone(), |ctx: Ctx<'js>, text: String| {
            Class::instance(ctx.clone(), super::bignum::BigNum::from_decimal(&text)
                .map_err(|error| Exception::throw_message(&ctx, &error))?)
        })?,
        Function::new(ctx.clone(), |value: Value<'js>| {
            value.as_object().is_some_and(|object| object.instance_of::<super::color::Color>())
        })?,
        Function::new(ctx.clone(), |value: Class<'js, super::color::Color>| value.borrow().channels.to_vec())?,
        Function::new(ctx.clone(), |ctx: Ctx<'js>, channels: Vec<f64>| {
            let channels = channels.try_into().map_err(|_| Exception::throw_type(&ctx, "Expected four Color channels"))?;
            Class::instance(ctx, super::color::Color::from_channels(channels)?)
        })?,
    ))
}
