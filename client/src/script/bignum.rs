use num::{BigInt, Integer, Signed, ToPrimitive, Zero};
use rquickjs::{
    class::Trace,
    function::{Opt, Rest},
    object::Property,
    Class, Ctx, Error, Function, JsLifetime, Result, Value,
};
use std::str::FromStr;

#[derive(Clone, Trace, JsLifetime)]
#[rquickjs::class]
pub(super) struct BigNum {
    #[qjs(skip_trace)]
    man: BigInt,
    #[qjs(skip_trace)]
    exp: BigInt,
}

impl BigNum {
    pub(super) fn from_decimal(text: &str) -> std::result::Result<Self, String> {
        let mut parts = text.split('e').map(str::trim);
        let man_text = parts.next().unwrap_or_default();
        let exp_text = parts.next().unwrap_or("0");
        if parts.next().is_some() {
            return Err(format!("Invalid number format: {text}"));
        }
        let exp_digits = match exp_text.as_bytes().first() {
            Some(b'+') | Some(b'-') => &exp_text[1..],
            _ => exp_text,
        };
        if exp_digits.is_empty() || !exp_digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!("Invalid number format: {text}"));
        }
        let parsed_exp = BigInt::from_str(exp_text.strip_prefix('+').unwrap_or(exp_text))
            .map_err(|_| format!("Invalid number format: {text}"))?;

        let (negative, man_text) = match man_text.as_bytes().first() {
            Some(b'-') => (true, &man_text[1..]),
            Some(b'+') => (false, &man_text[1..]),
            _ => (false, man_text),
        };
        let Some(dot) = man_text.find('.') else {
            if man_text.is_empty() || !man_text.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!("Invalid number format: {text}"));
            }
            let d = man_text;
            let digits = d.trim_start_matches('0');
            if digits.is_empty() {
                return Ok(Self { man: BigInt::zero(), exp: BigInt::zero() });
            }
            let first_nonzero = d.bytes().position(|byte| byte != b'0').unwrap();
            let exponent = parsed_exp.clone()
                + BigInt::from(d.len() as u64)
                - BigInt::from(first_nonzero as u64)
                - BigInt::from(1u8);
            let significant = &digits[..digits.len().min(16)];
            let mut man = BigInt::from_str(significant)
                .map_err(|_| format!("Invalid number format: {text}"))?
                * BigInt::from(10u8).pow((16 - significant.len()) as u32);
            if negative {
                man = -man;
            }
            return Ok(Self { man, exp: exponent });
        };

        if man_text[dot + 1..].contains('.') {
            return Err(format!("Invalid number format: {text}"));
        }
        let d = &man_text[..dot];
        let f = &man_text[dot + 1..];
        if (d.is_empty() && f.is_empty())
            || (!d.is_empty() && !d.bytes().all(|byte| byte.is_ascii_digit()))
            || (!f.is_empty() && !f.bytes().all(|byte| byte.is_ascii_digit()))
            || (d.is_empty() && !f.bytes().next().is_some_and(|byte| byte.is_ascii_digit()))
        {
            return Err(format!("Invalid number format: {text}"));
        }
        let mut raw_digits = String::with_capacity(d.len() + f.len());
        raw_digits.push_str(d);
        raw_digits.push_str(f);
        let digits = raw_digits.trim_start_matches('0');
        if digits.is_empty() {
            return Ok(Self { man: BigInt::zero(), exp: BigInt::zero() });
        }
        let first_nonzero = raw_digits.bytes().position(|byte| byte != b'0').unwrap();
        let exponent = parsed_exp
            + BigInt::from(d.len() as u64)
            - BigInt::from(first_nonzero as u64)
            - BigInt::from(1u8);
        let significant = &digits[..digits.len().min(16)];
        let mut man = BigInt::from_str(significant)
            .map_err(|_| format!("Invalid number format: {text}"))?
            * BigInt::from(10u8).pow((16 - significant.len()) as u32);
        if negative {
            man = -man;
        }
        Ok(Self { man, exp: exponent })
    }

    pub(super) fn decimal(&self) -> String {
        self.string_value(None)
    }

    fn normalize(&mut self) {
        if self.man.is_zero() {
            self.exp = BigInt::zero();
            return;
        }

        let negative = self.man.is_negative();
        let mut digits = self.man.abs().to_string();
        if digits.len() > 16 {
            self.exp += BigInt::from((digits.len() - 16) as u32);
            digits.truncate(16);
        } else if digits.len() < 16 {
            self.exp -= BigInt::from((16 - digits.len()) as u32);
            digits.push_str(&"0".repeat(16 - digits.len()));
        }
        self.man = BigInt::from_str(&digits).unwrap();
        if negative {
            self.man = -self.man.clone();
        }
    }

    fn from_parts(man: BigInt, exp: BigInt) -> Self {
        let mut value = Self { man, exp };
        value.normalize();
        value
    }

    fn string_value(&self, man_len: Option<i32>) -> String {
        let mut digits = self.man.abs().to_string();
        if digits.len() < 16 {
            let mut padded = String::with_capacity(16);
            padded.push_str(&"0".repeat(16 - digits.len()));
            padded.push_str(&digits);
            digits = padded;
        }

        let mut rendered = String::with_capacity(18);
        if self.man.is_negative() {
            rendered.push('-');
        }
        rendered.push(digits.as_bytes()[0] as char);
        rendered.push('.');
        rendered.push_str(&digits[1..]);
        while rendered.ends_with('0') {
            rendered.pop();
        }
        if rendered.ends_with('.') {
            rendered.pop();
        }

        let rendered = match man_len {
            None => rendered,
            Some(0) => String::new(),
            Some(length) => {
                let target = length.max(0) as usize;
                let end = if length >= 0 {
                    target.min(rendered.len())
                } else {
                    rendered.len().saturating_sub(length.unsigned_abs() as usize)
                };
                let mut value = rendered[..end].to_owned();
                if value.len() < target {
                    value.push_str(&"0".repeat(target - value.len()));
                }
                value
            }
        };
        if matches!(man_len, Some(0)) {
            format!("e{}", self.exp)
        } else {
            format!("{}e{}", rendered, self.exp)
        }
    }

    fn from_js_value<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        if let Some(class) = value.as_object().and_then(Class::<Self>::from_object) {
            return Ok(class.try_borrow()?.clone());
        }

        let text = if let Some(number) = value.as_number() {
            if !number.is_finite() {
                return Err(Error::new_from_js_message(
                    "number",
                    "BigNum",
                    format!("Invalid mantissa: {}", if number.is_nan() {
                        "NaN".to_owned()
                    } else if number.is_sign_negative() {
                        "-Infinity".to_owned()
                    } else {
                        "Infinity".to_owned()
                    }),
                ));
            }
            number.to_string()
        } else if let Some(string) = value.as_string() {
            string.to_string()?
        } else if value.is_big_int() {
            let stringify: Function = ctx.globals().get("String")?;
            stringify.call((value,))?
        } else {
            return Err(Error::new_from_js_message(
                value.type_name(),
                "BigNum",
                format!("Invalid type for BigNum: ({})", value.type_name()),
            ));
        };

        Self::from_decimal(&text)
            .map_err(|message| Error::new_from_js_message("string", "BigNum", message))
    }

    fn threshold<'js>(ctx: &Ctx<'js>) -> Result<BigInt> {
        let prototype = Class::<Self>::prototype(ctx)?
            .ok_or_else(|| Error::new_from_js_message("BigNum", "constructor", "BigNum is not installed"))?;
        let constructor: Function = prototype.get("constructor")?;
        Ok(BigInt::from(constructor.get::<_, i64>("NEGLIGIBLE_THRESHOLD")?))
    }

    fn compare(&self, other: &Self) -> i8 {
        if self.is_zero() || other.is_zero() || self.is_neg() != other.is_neg() {
            return self.man.cmp(&other.man) as i8;
        }

        let exponent_order = self.exp.cmp(&other.exp);
        if exponent_order != std::cmp::Ordering::Equal {
            return if self.is_neg() {
                match exponent_order {
                    std::cmp::Ordering::Less => 1,
                    std::cmp::Ordering::Greater => -1,
                    std::cmp::Ordering::Equal => 0,
                }
            } else {
                match exponent_order {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Greater => 1,
                    std::cmp::Ordering::Equal => 0,
                }
            };
        }
        self.man.cmp(&other.man) as i8
    }

    fn add_native(&self, other: &Self, threshold: &BigInt) -> std::result::Result<Self, String> {
        if self.is_zero() {
            return Ok(other.clone());
        }
        if other.is_zero() {
            return Ok(self.clone());
        }

        let base_exp = if self.exp < other.exp { self.exp.clone() } else { other.exp.clone() };
        let self_shift = &self.exp - &base_exp;
        let other_shift = &other.exp - &base_exp;
        if &self_shift > threshold {
            return Ok(self.clone());
        }
        if &other_shift > threshold {
            return Ok(other.clone());
        }
        let Some(self_shift) = self_shift.to_u32() else {
            return Err("exponent difference is too large".to_owned());
        };
        let Some(other_shift) = other_shift.to_u32() else {
            return Err("exponent difference is too large".to_owned());
        };
        Ok(Self::from_parts(
            self.man.clone() * BigInt::from(10u8).pow(self_shift)
                + other.man.clone() * BigInt::from(10u8).pow(other_shift),
            base_exp,
        ))
    }

    fn sub_native(&self, other: &Self, threshold: &BigInt) -> std::result::Result<Self, String> {
        if self.is_zero() {
            return Ok(other.neg());
        }
        if other.is_zero() {
            return Ok(self.clone());
        }

        let base_exp = if self.exp < other.exp { self.exp.clone() } else { other.exp.clone() };
        let self_shift = &self.exp - &base_exp;
        let other_shift = &other.exp - &base_exp;
        if &self_shift > threshold {
            return Ok(self.clone());
        }
        if &other_shift > threshold {
            return Ok(other.neg());
        }
        let Some(self_shift) = self_shift.to_u32() else {
            return Err("exponent difference is too large".to_owned());
        };
        let Some(other_shift) = other_shift.to_u32() else {
            return Err("exponent difference is too large".to_owned());
        };
        Ok(Self::from_parts(
            self.man.clone() * BigInt::from(10u8).pow(self_shift)
                - other.man.clone() * BigInt::from(10u8).pow(other_shift),
            base_exp,
        ))
    }

    fn integer(&self) -> std::result::Result<BigInt, String> {
        let shift = &self.exp - BigInt::from(15u8);
        if shift >= BigInt::zero() {
            let Some(shift) = shift.to_u32() else {
                return Err("integer exponent is too large".to_owned());
            };
            return Ok(self.man.clone() * BigInt::from(10u8).pow(shift));
        }
        let shift = -shift;
        if shift > BigInt::from(16u8) {
            return Ok(if self.is_neg() { -BigInt::from(1u8) } else { BigInt::zero() });
        }
        let divisor = BigInt::from(10u8).pow(shift.to_u32().unwrap());
        let (quotient, remainder) = self.man.div_rem(&divisor);
        Ok(quotient - if self.is_neg() && !remainder.is_zero() {
            BigInt::from(1u8)
        } else {
            BigInt::zero()
        })
    }
}

#[rquickjs::methods]
impl BigNum {
    #[qjs(constructor)]
    pub fn new<'js>(ctx: Ctx<'js>, value: Opt<Value<'js>>) -> Result<Self> {
        Self::from_js_value(&ctx, value.0.filter(|value| !value.is_undefined()).unwrap_or_else(|| Value::new_int(ctx.clone(), 0)))
    }

    #[qjs(get, rename = "mantissa")]
    pub fn mantissa(&self) -> f64 {
        let mut digits = self.man.abs().to_string();
        if digits.len() < 16 {
            let mut padded = String::with_capacity(16);
            padded.push_str(&"0".repeat(16 - digits.len()));
            padded.push_str(&digits);
            digits = padded;
        }
        let mut value = String::with_capacity(18);
        if self.man.is_negative() {
            value.push('-');
        }
        value.push(digits.as_bytes()[0] as char);
        value.push('.');
        value.push_str(&digits[1..]);
        value.parse::<f64>().unwrap()
    }

    #[qjs(get, rename = "exponent")]
    pub fn exponent<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let bigint: Function = ctx.globals().get("BigInt")?;
        bigint.call((self.exp.to_string(),))
    }

    #[qjs(get, rename = "isZero")]
    pub fn is_zero(&self) -> bool {
        self.man.is_zero()
    }

    #[qjs(get, rename = "isNeg")]
    pub fn is_neg(&self) -> bool {
        self.man.is_negative()
    }

    #[qjs(get, rename = "isPos")]
    pub fn is_pos(&self) -> bool {
        !self.is_neg()
    }

    pub fn cmp<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<i8> {
        Ok(self.compare(&Self::from_js_value(&ctx, other)?))
    }

    pub fn lt<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<bool> {
        Ok(self.compare(&Self::from_js_value(&ctx, other)?) < 0)
    }

    pub fn lte<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<bool> {
        Ok(self.compare(&Self::from_js_value(&ctx, other)?) <= 0)
    }

    pub fn gt<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<bool> {
        Ok(self.compare(&Self::from_js_value(&ctx, other)?) > 0)
    }

    pub fn gte<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<bool> {
        Ok(self.compare(&Self::from_js_value(&ctx, other)?) >= 0)
    }

    pub fn eq<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<bool> {
        Ok(self.compare(&Self::from_js_value(&ctx, other)?) == 0)
    }

    pub fn neq<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<bool> {
        Ok(self.compare(&Self::from_js_value(&ctx, other)?) != 0)
    }

    pub fn max<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<Self> {
        let other = Self::from_js_value(&ctx, other)?;
        Ok(if self.compare(&other) >= 0 { self.clone() } else { other })
    }

    pub fn min<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<Self> {
        let other = Self::from_js_value(&ctx, other)?;
        Ok(if self.compare(&other) <= 0 { self.clone() } else { other })
    }

    #[qjs(static, rename = "max")]
    pub fn max_values<'js>(ctx: Ctx<'js>, values: Rest<Value<'js>>) -> Result<Value<'js>> {
        if values.0.is_empty() {
            return Err(Error::new_from_js_message("arguments", "BigNum", "No values provided"));
        }
        let mut selected = values.0[0].clone();
        let mut selected_number = Self::from_js_value(&ctx, selected.clone())?;
        for value in values.0.iter().skip(1) {
            let candidate = Self::from_js_value(&ctx, value.clone())?;
            if selected_number.compare(&candidate) < 0 {
                selected = value.clone();
                selected_number = candidate;
            }
        }
        Ok(selected)
    }

    #[qjs(static, rename = "min")]
    pub fn min_values<'js>(ctx: Ctx<'js>, values: Rest<Value<'js>>) -> Result<Value<'js>> {
        if values.0.is_empty() {
            return Err(Error::new_from_js_message("arguments", "BigNum", "No values provided"));
        }
        let mut selected = values.0[0].clone();
        let mut selected_number = Self::from_js_value(&ctx, selected.clone())?;
        for value in values.0.iter().skip(1) {
            let candidate = Self::from_js_value(&ctx, value.clone())?;
            if selected_number.compare(&candidate) > 0 {
                selected = value.clone();
                selected_number = candidate;
            }
        }
        Ok(selected)
    }

    pub fn sign(&self) -> i8 {
        if self.is_zero() { 0 } else if self.is_neg() { -1 } else { 1 }
    }

    pub fn neg(&self) -> Self {
        Self::from_parts(-self.man.clone(), self.exp.clone())
    }

    pub fn abs(&self) -> Self {
        Self::from_parts(self.man.abs(), self.exp.clone())
    }

    pub fn add<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<Self> {
        let threshold = Self::threshold(&ctx)?;
        self.add_native(&Self::from_js_value(&ctx, other)?, &threshold)
            .map_err(|message| Error::new_from_js_message("BigNum", "number", message))
    }

    pub fn sub<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<Self> {
        let threshold = Self::threshold(&ctx)?;
        self.sub_native(&Self::from_js_value(&ctx, other)?, &threshold)
            .map_err(|message| Error::new_from_js_message("BigNum", "number", message))
    }

    pub fn mul<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<Self> {
        let other = Self::from_js_value(&ctx, other)?;
        Ok(Self::from_parts(
            self.man.clone() * other.man,
            self.exp.clone() + other.exp - BigInt::from(15u8),
        ))
    }

    pub fn div<'js>(&self, ctx: Ctx<'js>, other: Value<'js>) -> Result<Self> {
        let other = Self::from_js_value(&ctx, other)?;
        if other.is_zero() {
            return Err(Error::new_from_js_message("BigNum", "nonzero divisor", "Division by zero"));
        }
        Ok(Self::from_parts(
            self.man.clone() * BigInt::from(10u8).pow(16) / other.man,
            self.exp.clone() - other.exp - BigInt::from(1u8),
        ))
    }

    #[qjs(static)]
    pub fn sum<'js>(ctx: Ctx<'js>, values: Rest<Value<'js>>) -> Result<Self> {
        if values.0.is_empty() {
            return Err(Error::new_from_js_message("arguments", "BigNum", "No values provided"));
        }
        let threshold = Self::threshold(&ctx)?;
        let mut result = Self { man: BigInt::zero(), exp: BigInt::zero() };
        for value in values.0 {
            result = result
                .add_native(&Self::from_js_value(&ctx, value)?, &threshold)
                .map_err(|message| Error::new_from_js_message("BigNum", "number", message))?;
        }
        Ok(result)
    }

    #[qjs(rename = "toString")]
    pub fn to_string<'js>(&self, man_len: Opt<Option<i32>>) -> String {
        self.string_value(man_len.0.flatten())
    }

    #[qjs(rename = "toJSON")]
    pub fn to_json(&self) -> String {
        self.decimal()
    }

    #[qjs(rename = "toNumber")]
    pub fn to_number(&self) -> f64 {
        let mut digits = self.man.abs().to_string();
        if digits.len() < 16 {
            let mut padded = String::with_capacity(16);
            padded.push_str(&"0".repeat(16 - digits.len()));
            padded.push_str(&digits);
            digits = padded;
        }
        let mut value = String::with_capacity(24 + self.exp.to_string().len());
        if self.man.is_negative() {
            value.push('-');
        }
        value.push(digits.as_bytes()[0] as char);
        value.push('.');
        value.push_str(&digits[1..]);
        value.push('e');
        value.push_str(&self.exp.to_string());
        value.parse::<f64>().unwrap_or(if self.is_neg() { f64::NEG_INFINITY } else { f64::INFINITY })
    }

    #[qjs(rename = "toInt")]
    pub fn to_int(&self) -> f64 {
        if self.exp >= BigInt::from(15u8) {
            return self.to_number();
        }
        let integer = self.integer().unwrap_or_else(|_| if self.is_neg() { -BigInt::from(1u8) } else { BigInt::zero() });
        integer.to_string().parse::<f64>().unwrap_or(if self.is_neg() { f64::NEG_INFINITY } else { f64::INFINITY })
    }

    #[qjs(rename = "toBigInt")]
    pub fn to_big_int<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let integer = self.integer()
            .map_err(|message| Error::new_from_js_message("BigNum", "integer", message))?;
        let bigint: Function = ctx.globals().get("BigInt")?;
        bigint.call((integer.to_string(),))
    }
}

pub(super) fn install(ctx: &Ctx<'_>) -> Result<()> {
    Class::<BigNum>::define(&ctx.globals())?;
    let constructor: Function = ctx.globals().get("BigNum")?;
    constructor.prop(
        "ZERO",
        Property::from(Class::instance(
            ctx.clone(),
            BigNum { man: BigInt::zero(), exp: BigInt::zero() },
        )?),
    )?;
    constructor.prop(
        "ONE",
        Property::from(Class::instance(ctx.clone(), BigNum::from_decimal("1").unwrap())?),
    )?;
    constructor.set("NEGLIGIBLE_THRESHOLD", 15i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquickjs::{Context, Runtime};

    #[test]
    fn parser_and_fixed_point_roundtrip_match_script_contract() {
        assert_eq!(BigNum::from_decimal("1.234567890123456789e2").unwrap().decimal(), "1.234567890123456e2");
        assert_eq!(BigNum::from_decimal("-0.00123").unwrap().decimal(), "-1.23e-3");
        assert_eq!(BigNum::from_decimal("+1e+2").unwrap().decimal(), "1e2");
        assert_eq!(BigNum::from_decimal("0").unwrap().decimal(), "0e0");
        assert!(BigNum::from_decimal("1E2").is_err());
        assert!(BigNum::from_decimal("1e").is_err());
        assert!(BigNum::from_decimal("1e1_0").is_err());
        assert!(BigNum::from_decimal("1e++2").is_err());
        assert!(BigNum::from_decimal("1e0x10").is_err());
        assert!(BigNum::from_decimal("1.2.3").is_err());
    }

    #[test]
    fn arithmetic_and_integer_conversions_preserve_fixed_point_rules() {
        let first = BigNum::from_decimal("1.13e2").unwrap();
        let second = BigNum::from_decimal("2.7e1").unwrap();
        let threshold = BigInt::from(15u8);
        assert_eq!(first.add_native(&second, &threshold).unwrap().decimal(), "1.4e2");
        assert_eq!(first.sub_native(&second, &threshold).unwrap().decimal(), "8.6e1");
        assert_eq!(BigNum::from_parts(
            first.man.clone() * second.man.clone(),
            first.exp.clone() + second.exp.clone() - BigInt::from(15u8),
        ).decimal(), "3.051e3");
        assert_eq!(BigNum::from_decimal("-1.2e-1").unwrap().integer().unwrap(), BigInt::from(-1));
        assert_eq!(BigNum::from_decimal("1.2e-1").unwrap().integer().unwrap(), BigInt::zero());
    }

    #[test]
    fn javascript_class_accepts_primitives_and_preserves_instances() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            install(&ctx).unwrap();
            let result: String = ctx.eval(
                r#"(() => {
                    const source = new BigNum("1.23e400");
                    const copy = new BigNum(source);
                    return JSON.stringify([
                        source instanceof BigNum,
                        copy instanceof BigNum,
                        new BigNum(1.13e2).toString(),
                        new BigNum(9007199254740993123456789n).toString(),
                        BigNum.ZERO.isZero,
                        JSON.stringify({value: source}) === '{"value":"1.23e400"}',
                        (() => { const value = new BigNum(2); return BigNum.max(new BigNum(1), value) === value; })(),
                        BigNum.ZERO === BigNum.ZERO,
                        BigNum.ONE === BigNum.ONE,
                    ]);
                })()"#,
            ).unwrap();
            assert_eq!(result, "[true,true,\"1.13e2\",\"9.007199254740993e24\",true,true,true,true,true]");
            let threshold: String = ctx.eval(
                r#"(() => {
                    BigNum.NEGLIGIBLE_THRESHOLD = 0;
                    const limited = new BigNum("1e15").add(1);
                    BigNum.NEGLIGIBLE_THRESHOLD = 15;
                    return `${limited.toString(0)}:${new BigNum("1e9223372036854775808").exponent}`;
                })()"#,
            ).unwrap();
            assert_eq!(threshold, "e15:9223372036854775808");
        });
    }

    #[test]
    fn javascript_constructor_is_callable_and_accepts_map_arguments() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            install(&ctx).unwrap();
            assert!(ctx.eval::<bool, _>(r#"(() => {
                const values = [1, '2', 3n, new BigNum(4)].map(BigNum);
                return values.every(value => value instanceof BigNum && value.constructor === BigNum)
                    && BigNum.sum(...values).toBigInt() === 10n
                    && BigNum('1.13e2').toBigInt() === 113n
                    && new BigNum('1.13e2').toBigInt() === 113n
                    && BigNum(BigNum.ONE) !== BigNum.ONE;
            })()"#).unwrap());
        });
    }

    #[test]
    fn javascript_constructor_defaults_to_zero() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            install(&ctx).unwrap();
            assert!(ctx.eval::<bool, _>(r#"(() => {
                return BigNum().isZero && new BigNum().isZero
                    && BigNum(undefined).isZero && new BigNum(undefined).isZero;
            })()"#).unwrap());
        });
    }

    #[test]
    fn malformed_and_non_finite_inputs_fail() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            install(&ctx).unwrap();
            assert!(ctx.eval::<Value, _>(r#"(() => { try { new BigNum("1E2"); return null; } catch (_) { return 1; } })()"#).unwrap().is_number());
            assert!(ctx.eval::<Value, _>(r#"(() => { try { new BigNum(Infinity); return null; } catch (_) { return 1; } })()"#).unwrap().is_number());
        });
    }
}
