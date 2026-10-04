use rquickjs::{
    class::Trace,
    function::{Opt, Rest, This},
    Class, Ctx, Error, JsLifetime, Result, Value,
};

fn tuple_components(value: Value<'_>, count: usize, method: &'static str) -> Result<Vec<f64>> {
    let array = value
        .into_object()
        .and_then(|object| object.into_array())
        .ok_or_else(|| Error::new_from_js_message(method, "array", "Expected an array."))?;
    if array.len() < count - 1 {
        return Err(Error::new_from_js_message(
            method,
            "array",
            "Invalid number of components.",
        ));
    }

    let mut values = Vec::with_capacity(count);
    for index in 0..array.len().min(count) {
        let value: Value<'_> = array.get(index)?;
        if value.is_undefined() {
            if index + 1 == count {
                break;
            }
            return Err(Error::new_from_js_message(
                method,
                "array",
                "Missing component.",
            ));
        }
        values.push(value.as_number().ok_or_else(|| {
            Error::new_from_js_message(method, "array", "Invalid color component.")
        })?);
    }
    if values.len() < count - 1 {
        return Err(Error::new_from_js_message(
            method,
            "array",
            "Missing component.",
        ));
    }
    Ok(values)
}

#[derive(Clone, Trace, JsLifetime)]
#[rquickjs::class]
pub(super) struct Color {
    #[qjs(skip_trace)]
    pub(super) channels: [f64; 4],
}

impl Color {
    pub(super) fn from_channels(mut channels: [f64; 4]) -> Result<Self> {
        if channels.iter().any(|channel| !channel.is_finite()) {
            return Err(Error::new_from_js_message(
                "Color",
                "channels",
                "Invalid color channels.",
            ));
        }
        for channel in &mut channels {
            *channel = channel.clamp(0.0, 255.0);
        }
        Ok(Self { channels })
    }

    fn from_hsv_values(hue: f64, saturation: f64, value: f64, alpha: f64) -> Result<Self> {
        if [hue, saturation, value, alpha]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(Error::new_from_js_message(
                "fromHsv",
                "color",
                "Invalid HSV color.",
            ));
        }
        let hue = ((hue % 360.0) + 360.0) % 360.0 / 60.0;
        let saturation = saturation.clamp(0.0, 1.0);
        let value = value.clamp(0.0, 1.0);
        let chroma = value * saturation;
        let x = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
        let rgb = if hue < 1.0 {
            [chroma, x, 0.0]
        } else if hue < 2.0 {
            [x, chroma, 0.0]
        } else if hue < 3.0 {
            [0.0, chroma, x]
        } else if hue < 4.0 {
            [0.0, x, chroma]
        } else if hue < 5.0 {
            [x, 0.0, chroma]
        } else {
            [chroma, 0.0, x]
        };
        Self::from_channels([
            (rgb[0] + value - chroma) * 255.0,
            (rgb[1] + value - chroma) * 255.0,
            (rgb[2] + value - chroma) * 255.0,
            alpha * 255.0,
        ])
    }

    fn from_hsl_values(hue: f64, saturation: f64, lightness: f64, alpha: f64) -> Result<Self> {
        if [hue, saturation, lightness, alpha]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(Error::new_from_js_message(
                "fromHsl",
                "color",
                "Invalid HSL color.",
            ));
        }
        let saturation = saturation.clamp(0.0, 1.0);
        let lightness = lightness.clamp(0.0, 1.0);
        let value = lightness + saturation * lightness.min(1.0 - lightness);
        Self::from_hsv_values(
            hue,
            if value == 0.0 {
                0.0
            } else {
                2.0 * (1.0 - lightness / value)
            },
            value,
            alpha,
        )
    }

    fn from_hwb_values(hue: f64, whiteness: f64, blackness: f64, alpha: f64) -> Result<Self> {
        if [hue, whiteness, blackness, alpha]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(Error::new_from_js_message(
                "fromHwb",
                "color",
                "Invalid HWB color.",
            ));
        }
        let whiteness = whiteness.clamp(0.0, 1.0);
        let blackness = blackness.clamp(0.0, 1.0);
        if whiteness + blackness >= 1.0 {
            let gray = whiteness / (whiteness + blackness) * 255.0;
            return Self::from_channels([gray, gray, gray, alpha * 255.0]);
        }

        let color = Self::from_hsv_values(hue, 1.0, 1.0, 1.0)?;
        Self::from_channels([
            (color.channels[0] / 255.0 * (1.0 - whiteness - blackness) + whiteness) * 255.0,
            (color.channels[1] / 255.0 * (1.0 - whiteness - blackness) + whiteness) * 255.0,
            (color.channels[2] / 255.0 * (1.0 - whiteness - blackness) + whiteness) * 255.0,
            alpha * 255.0,
        ])
    }

    fn hsv_values(&self) -> [f64; 4] {
        let max = self.channels[0]
            .max(self.channels[1])
            .max(self.channels[2]);
        let min = self.channels[0]
            .min(self.channels[1])
            .min(self.channels[2]);
        let delta = max - min;
        let mut hue = 0.0;
        if delta != 0.0 {
            hue = if max == self.channels[0] {
                (self.channels[1] - self.channels[2]) / delta
            } else if max == self.channels[1] {
                2.0 + (self.channels[2] - self.channels[0]) / delta
            } else {
                4.0 + (self.channels[0] - self.channels[1]) / delta
            };
            hue = (hue * 60.0 + 360.0) % 360.0;
        }
        [
            hue,
            if max == 0.0 { 0.0 } else { delta / max },
            max / 255.0,
            self.channels[3] / 255.0,
        ]
    }

    fn hsl_values(&self) -> [f64; 4] {
        let hsv = self.hsv_values();
        let lightness = hsv[2] * (1.0 - hsv[1] / 2.0);
        [
            hsv[0],
            if lightness == 0.0 || lightness == 1.0 {
                0.0
            } else {
                (hsv[2] - lightness) / lightness.min(1.0 - lightness)
            },
            lightness,
            hsv[3],
        ]
    }

    fn linear_values(&self) -> [f64; 4] {
        [
            if self.channels[0] / 255.0 <= 0.04045 {
                self.channels[0] / 255.0 / 12.92
            } else {
                ((self.channels[0] / 255.0 + 0.055) / 1.055).powf(2.4)
            },
            if self.channels[1] / 255.0 <= 0.04045 {
                self.channels[1] / 255.0 / 12.92
            } else {
                ((self.channels[1] / 255.0 + 0.055) / 1.055).powf(2.4)
            },
            if self.channels[2] / 255.0 <= 0.04045 {
                self.channels[2] / 255.0 / 12.92
            } else {
                ((self.channels[2] / 255.0 + 0.055) / 1.055).powf(2.4)
            },
            self.channels[3] / 255.0,
        ]
    }

    fn mix_values(&self, other: &Self, amount: f64) -> Result<Self> {
        if !amount.is_finite() {
            return Err(Error::new_from_js_message(
                "mix",
                "amount",
                "Invalid blend amount.",
            ));
        }
        let amount = amount.clamp(0.0, 1.0);
        Self::from_channels(std::array::from_fn(|index| {
            self.channels[index] + (other.channels[index] - self.channels[index]) * amount
        }))
    }

}

#[rquickjs::methods]
impl Color {
    #[qjs(constructor)]
    pub fn new() -> Result<Self> {
        Err(Error::new_from_js_message(
            "Color",
            "constructor",
            "Color instances can only be created by static factories.",
        ))
    }

    #[qjs(static, rename = "fromRgb")]
    pub fn from_rgb(r: Value<'_>, args: Rest<Value<'_>>) -> Result<Self> {
        if let Some(first) = r.as_number() {
            let second = args.0.first().and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromRgb", "component", "Missing component.")
            })?;
            let third = args.0.get(1).and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromRgb", "component", "Missing component.")
            })?;
            let alpha = match args.0.get(2) {
                None => 255.0,
                Some(value) if value.is_undefined() => 255.0,
                Some(value) => value.as_number().ok_or_else(|| {
                    Error::new_from_js_message("fromRgb", "component", "Invalid color channels.")
                })?,
            };
            return Self::from_channels([first, second, third, alpha]);
        }
        let values = tuple_components(r, 4, "fromRgb")?;
        Self::from_channels([
            values[0],
            values[1],
            values[2],
            values.get(3).copied().unwrap_or(255.0),
        ])
    }

    #[qjs(static, rename = "fromNormalizedRgb")]
    pub fn from_normalized_rgb(r: Value<'_>, args: Rest<Value<'_>>) -> Result<Self> {
        if let Some(first) = r.as_number() {
            let second = args.0.first().and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message(
                    "fromNormalizedRgb",
                    "component",
                    "Missing component.",
                )
            })?;
            let third = args.0.get(1).and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message(
                    "fromNormalizedRgb",
                    "component",
                    "Missing component.",
                )
            })?;
            let alpha = match args.0.get(2) {
                None => 1.0,
                Some(value) if value.is_undefined() => 1.0,
                Some(value) => value.as_number().ok_or_else(|| {
                    Error::new_from_js_message(
                        "fromNormalizedRgb",
                        "component",
                        "Invalid color channels.",
                    )
                })?,
            };
            return Self::from_channels([
                first * 255.0,
                second * 255.0,
                third * 255.0,
                alpha * 255.0,
            ]);
        }
        let values = tuple_components(r, 4, "fromNormalizedRgb")?;
        Self::from_channels([
            values[0] * 255.0,
            values[1] * 255.0,
            values[2] * 255.0,
            values.get(3).copied().unwrap_or(1.0) * 255.0,
        ])
    }

    #[qjs(static, rename = "fromLinearRgb")]
    pub fn from_linear_rgb(r: Value<'_>, args: Rest<Value<'_>>) -> Result<Self> {
        let values = if let Some(first) = r.as_number() {
            let second = args.0.first().and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message(
                    "fromLinearRgb",
                    "component",
                    "Missing component.",
                )
            })?;
            let third = args.0.get(1).and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message(
                    "fromLinearRgb",
                    "component",
                    "Missing component.",
                )
            })?;
            let alpha = match args.0.get(2) {
                None => 1.0,
                Some(value) if value.is_undefined() => 1.0,
                Some(value) => value.as_number().ok_or_else(|| {
                    Error::new_from_js_message(
                        "fromLinearRgb",
                        "component",
                        "Invalid linear RGB color.",
                    )
                })?,
            };
            vec![first, second, third, alpha]
        } else {
            let values = tuple_components(r, 4, "fromLinearRgb")?;
            vec![
                values[0],
                values[1],
                values[2],
                values.get(3).copied().unwrap_or(1.0),
            ]
        };
        if values.iter().any(|value| !value.is_finite()) {
            return Err(Error::new_from_js_message(
                "fromLinearRgb",
                "color",
                "Invalid linear RGB color.",
            ));
        }
        Self::from_channels([
            {
                let value = values[0].clamp(0.0, 1.0);
                255.0
                    * if value <= 0.0031308 {
                        value * 12.92
                    } else {
                        1.055 * value.powf(1.0 / 2.4) - 0.055
                    }
            },
            {
                let value = values[1].clamp(0.0, 1.0);
                255.0
                    * if value <= 0.0031308 {
                        value * 12.92
                    } else {
                        1.055 * value.powf(1.0 / 2.4) - 0.055
                    }
            },
            {
                let value = values[2].clamp(0.0, 1.0);
                255.0
                    * if value <= 0.0031308 {
                        value * 12.92
                    } else {
                        1.055 * value.powf(1.0 / 2.4) - 0.055
                    }
            },
            values[3] * 255.0,
        ])
    }

    #[qjs(static, rename = "fromHsl")]
    pub fn from_hsl(r: Value<'_>, args: Rest<Value<'_>>) -> Result<Self> {
        if let Some(first) = r.as_number() {
            let saturation = args.0.first().and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromHsl", "component", "Missing component.")
            })?;
            let lightness = args.0.get(1).and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromHsl", "component", "Missing component.")
            })?;
            let alpha = match args.0.get(2) {
                None => 1.0,
                Some(value) if value.is_undefined() => 1.0,
                Some(value) => value.as_number().ok_or_else(|| {
                    Error::new_from_js_message("fromHsl", "component", "Invalid HSL color.")
                })?,
            };
            return Self::from_hsl_values(first, saturation, lightness, alpha);
        }
        let values = tuple_components(r, 4, "fromHsl")?;
        Self::from_hsl_values(
            values[0],
            values[1],
            values[2],
            values.get(3).copied().unwrap_or(1.0),
        )
    }

    #[qjs(static, rename = "fromHsv")]
    pub fn from_hsv(r: Value<'_>, args: Rest<Value<'_>>) -> Result<Self> {
        if let Some(first) = r.as_number() {
            let saturation = args.0.first().and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromHsv", "component", "Missing component.")
            })?;
            let value = args.0.get(1).and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromHsv", "component", "Missing component.")
            })?;
            let alpha = match args.0.get(2) {
                None => 1.0,
                Some(value) if value.is_undefined() => 1.0,
                Some(value) => value.as_number().ok_or_else(|| {
                    Error::new_from_js_message("fromHsv", "component", "Invalid HSV color.")
                })?,
            };
            return Self::from_hsv_values(first, saturation, value, alpha);
        }
        let values = tuple_components(r, 4, "fromHsv")?;
        Self::from_hsv_values(
            values[0],
            values[1],
            values[2],
            values.get(3).copied().unwrap_or(1.0),
        )
    }

    #[qjs(static, rename = "fromHwb")]
    pub fn from_hwb(r: Value<'_>, args: Rest<Value<'_>>) -> Result<Self> {
        if let Some(first) = r.as_number() {
            let whiteness = args.0.first().and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromHwb", "component", "Missing component.")
            })?;
            let blackness = args.0.get(1).and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromHwb", "component", "Missing component.")
            })?;
            let alpha = match args.0.get(2) {
                None => 1.0,
                Some(value) if value.is_undefined() => 1.0,
                Some(value) => value.as_number().ok_or_else(|| {
                    Error::new_from_js_message("fromHwb", "component", "Invalid HWB color.")
                })?,
            };
            return Self::from_hwb_values(first, whiteness, blackness, alpha);
        }
        let values = tuple_components(r, 4, "fromHwb")?;
        Self::from_hwb_values(
            values[0],
            values[1],
            values[2],
            values.get(3).copied().unwrap_or(1.0),
        )
    }

    #[qjs(static, rename = "fromCmyk")]
    pub fn from_cmyk(r: Value<'_>, args: Rest<Value<'_>>) -> Result<Self> {
        let values = if let Some(first) = r.as_number() {
            let magenta = args.0.first().and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromCmyk", "component", "Missing component.")
            })?;
            let yellow = args.0.get(1).and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromCmyk", "component", "Missing component.")
            })?;
            let key = args.0.get(2).and_then(Value::as_number).ok_or_else(|| {
                Error::new_from_js_message("fromCmyk", "component", "Missing component.")
            })?;
            let alpha = match args.0.get(3) {
                None => 1.0,
                Some(value) if value.is_undefined() => 1.0,
                Some(value) => value.as_number().ok_or_else(|| {
                    Error::new_from_js_message("fromCmyk", "component", "Invalid CMYK color.")
                })?,
            };
            vec![first, magenta, yellow, key, alpha]
        } else {
            let values = tuple_components(r, 5, "fromCmyk")?;
            vec![
                values[0],
                values[1],
                values[2],
                values[3],
                values.get(4).copied().unwrap_or(1.0),
            ]
        };
        if values.iter().any(|value| !value.is_finite()) {
            return Err(Error::new_from_js_message(
                "fromCmyk",
                "color",
                "Invalid CMYK color.",
            ));
        }
        let key = values[3].clamp(0.0, 1.0);
        Self::from_channels([
            (1.0 - values[0].clamp(0.0, 1.0)) * (1.0 - key) * 255.0,
            (1.0 - values[1].clamp(0.0, 1.0)) * (1.0 - key) * 255.0,
            (1.0 - values[2].clamp(0.0, 1.0)) * (1.0 - key) * 255.0,
            values[4] * 255.0,
        ])
    }

    #[qjs(static, rename = "fromHex")]
    pub fn from_hex(hex: String) -> Result<Self> {
        let mut hex = hex.trim().to_owned();
        if let Some(stripped) = hex.strip_prefix('#') {
            hex = stripped.to_owned();
        }
        if !matches!(hex.len(), 3 | 4 | 6 | 8)
            || !hex.chars().all(|character| character.is_ascii_hexdigit())
        {
            return Err(Error::new_from_js_message(
                "fromHex",
                "hex",
                "Invalid hex color.",
            ));
        }
        if hex.len() <= 4 {
            hex = hex
                .chars()
                .flat_map(|character| [character, character])
                .collect();
        }
        Self::from_channels([
            u8::from_str_radix(&hex[0..2], 16).unwrap() as f64,
            u8::from_str_radix(&hex[2..4], 16).unwrap() as f64,
            u8::from_str_radix(&hex[4..6], 16).unwrap() as f64,
            if hex.len() == 8 {
                u8::from_str_radix(&hex[6..8], 16).unwrap() as f64
            } else {
                255.0
            },
        ])
    }

    #[qjs(static, rename = "fromCss")]
    pub fn from_css(css: String) -> Result<Self> {
        fn css_number(part: &str, hue: bool) -> Option<f64> {
            let number = if hue {
                if let Some(number) = part.strip_suffix("deg") {
                    number
                } else if let Some(number) = part.strip_suffix("grad") {
                    number
                } else if let Some(number) = part.strip_suffix("rad") {
                    number
                } else if let Some(number) = part.strip_suffix("turn") {
                    number
                } else {
                    part
                }
            } else {
                part.strip_suffix('%').unwrap_or(part)
            };

            let bytes = number.as_bytes();
            let mut index = 0;
            if bytes.get(index).is_some_and(|byte| *byte == b'+' || *byte == b'-') {
                index += 1;
            }
            let integer_start = index;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
            let has_integer = index > integer_start;
            let mut has_fraction = false;
            if bytes.get(index) == Some(&b'.') {
                index += 1;
                let fraction_start = index;
                while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                    index += 1;
                }
                has_fraction = index > fraction_start;
            }
            if !has_integer && !has_fraction {
                return None;
            }
            if bytes.get(index) == Some(&b'e') {
                index += 1;
                if bytes.get(index).is_some_and(|byte| *byte == b'+' || *byte == b'-') {
                    index += 1;
                }
                let exponent_start = index;
                while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                    index += 1;
                }
                if index == exponent_start {
                    return None;
                }
            }
            (index == bytes.len()).then(|| number.parse().ok()).flatten()
        }

        let css = css.trim().to_lowercase();
        if css == "transparent" {
            return Self::from_channels([0.0, 0.0, 0.0, 0.0]);
        }
        if css.starts_with('#') {
            return Self::from_hex(css);
        }

        let Some((name, body)) = css.split_once('(') else {
            return Err(Error::new_from_js_message(
                "fromCss",
                "css",
                "Unsupported CSS color.",
            ));
        };
        if !css.ends_with(')')
            || !matches!(name, "rgb" | "rgba" | "hsl" | "hsla" | "hwb")
        {
            return Err(Error::new_from_js_message(
                "fromCss",
                "css",
                "Unsupported CSS color.",
            ));
        }
        let body = &body[..body.len() - 1];
        let legacy = body.contains(',');
        let parts = if legacy {
            if body.contains('/') || name == "hwb" {
                return Err(Error::new_from_js_message(
                    "fromCss",
                    "css",
                    "Invalid CSS color.",
                ));
            }
            body.split(',')
                .map(str::trim)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        } else {
            let separated = body.split('/').collect::<Vec<_>>();
            if separated.len() > 2 || separated.iter().any(|part| part.trim().is_empty()) {
                return Err(Error::new_from_js_message(
                    "fromCss",
                    "css",
                    "Invalid CSS color.",
                ));
            }
            let mut parts = separated[0]
                .trim()
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if parts.len() != 3 {
                return Err(Error::new_from_js_message(
                    "fromCss",
                    "css",
                    "Invalid CSS color.",
                ));
            }
            if let Some(alpha) = separated.get(1) {
                parts.push(alpha.trim().to_owned());
            }
            parts
        };

        let hue = !name.starts_with("rgb");
        if (parts.len() != 3 && parts.len() != 4)
            || parts
                .iter()
                .enumerate()
                .any(|(index, part)| css_number(part, index == 0 && hue).is_none())
        {
            return Err(Error::new_from_js_message(
                "fromCss",
                "css",
                "Invalid CSS color.",
            ));
        }
        if legacy
            && ((name.starts_with("hsl")
                && parts[1..3].iter().any(|part| !part.ends_with('%')))
                || (name.starts_with("rgb")
                    && parts[0..3]
                        .iter()
                        .any(|part| part.ends_with('%') != parts[0].ends_with('%'))))
        {
            return Err(Error::new_from_js_message(
                "fromCss",
                "css",
                "Invalid legacy CSS color.",
            ));
        }

        let values = parts
            .iter()
            .enumerate()
            .map(|(index, part)| css_number(part, index == 0 && hue).unwrap())
            .collect::<Vec<_>>();
        if values.iter().any(|value| !value.is_finite()) {
            return Err(Error::new_from_js_message(
                "fromCss",
                "css",
                "Invalid CSS color.",
            ));
        }
        let alpha = if values.len() == 4 {
            values[3] / if parts[3].ends_with('%') { 100.0 } else { 1.0 }
        } else {
            1.0
        }
        .clamp(0.0, 1.0);
        if name.starts_with("rgb") {
            return Self::from_channels([
                if parts[0].ends_with('%') {
                    values[0] / 100.0 * 255.0
                } else {
                    values[0]
                },
                if parts[1].ends_with('%') {
                    values[1] / 100.0 * 255.0
                } else {
                    values[1]
                },
                if parts[2].ends_with('%') {
                    values[2] / 100.0 * 255.0
                } else {
                    values[2]
                },
                alpha * 255.0,
            ]);
        }
        if name == "hwb" && values[1].max(0.0) + values[2].max(0.0) >= 100.0 {
            let whiteness = values[1].max(0.0);
            let blackness = values[2].max(0.0);
            let gray = whiteness / (whiteness + blackness) * 255.0;
            return Self::from_channels([gray, gray, gray, alpha * 255.0]);
        }
        let hue = values[0]
            * if parts[0].ends_with("turn") {
                360.0
            } else if parts[0].ends_with("grad") {
                0.9
            } else if parts[0].ends_with("rad") {
                180.0 / std::f64::consts::PI
            } else {
                1.0
            };
        if name == "hwb" {
            Self::from_hwb_values(hue, values[1] / 100.0, values[2] / 100.0, alpha)
        } else {
            Self::from_hsl_values(hue, values[1] / 100.0, values[2] / 100.0, alpha)
        }
    }

    pub fn brightness(&self, factor: f64) -> Result<Self> {
        Self::from_channels([
            self.channels[0] * factor,
            self.channels[1] * factor,
            self.channels[2] * factor,
            self.channels[3],
        ])
    }

    pub fn contrast(&self, factor: f64) -> Result<Self> {
        Self::from_channels([
            (self.channels[0] - 127.5) * factor + 127.5,
            (self.channels[1] - 127.5) * factor + 127.5,
            (self.channels[2] - 127.5) * factor + 127.5,
            self.channels[3],
        ])
    }

    pub fn gamma(&self, value: f64) -> Result<Self> {
        if !value.is_finite() || value <= 0.0 {
            return Err(Error::new_from_js_message(
                "gamma",
                "value",
                "Invalid gamma.",
            ));
        }
        Self::from_channels([
            255.0 * (self.channels[0] / 255.0).powf(1.0 / value),
            255.0 * (self.channels[1] / 255.0).powf(1.0 / value),
            255.0 * (self.channels[2] / 255.0).powf(1.0 / value),
            self.channels[3],
        ])
    }

    #[qjs(rename = "rotateHue")]
    pub fn rotate_hue(&self, degrees: f64) -> Result<Self> {
        let hsl = self.hsl_values();
        Self::from_hsl_values(hsl[0] + degrees, hsl[1], hsl[2], hsl[3])
    }

    pub fn saturation(&self, factor: f64) -> Result<Self> {
        let hsl = self.hsl_values();
        Self::from_hsl_values(hsl[0], hsl[1] * factor, hsl[2], hsl[3])
    }

    pub fn lighten(&self, amount: Opt<Option<f64>>) -> Result<Self> {
        let hsl = self.hsl_values();
        Self::from_hsl_values(
            hsl[0],
            hsl[1],
            hsl[2] + amount.0.flatten().unwrap_or(0.1),
            hsl[3],
        )
    }

    pub fn darken(&self, amount: Opt<Option<f64>>) -> Result<Self> {
        let hsl = self.hsl_values();
        Self::from_hsl_values(
            hsl[0],
            hsl[1],
            hsl[2] - amount.0.flatten().unwrap_or(0.1),
            hsl[3],
        )
    }

    pub fn opacity(&self, value: f64) -> Result<Self> {
        Self::from_channels([
            self.channels[0],
            self.channels[1],
            self.channels[2],
            value * 255.0,
        ])
    }

    pub fn fade(&self, factor: f64) -> Result<Self> {
        Self::from_channels([
            self.channels[0],
            self.channels[1],
            self.channels[2],
            self.channels[3] * factor,
        ])
    }

    pub fn mix(&self, other: Class<'_, Color>, amount: Opt<Option<f64>>) -> Result<Self> {
        let other = other.borrow();
        self.mix_values(&other, amount.0.flatten().unwrap_or(0.5))
    }

    pub fn tint(&self, amount: Opt<Option<f64>>) -> Result<Self> {
        self.mix_values(
            &Self {
                channels: [255.0, 255.0, 255.0, self.channels[3]],
            },
            amount.0.flatten().unwrap_or(0.1),
        )
    }

    pub fn shade(&self, amount: Opt<Option<f64>>) -> Result<Self> {
        self.mix_values(
            &Self {
                channels: [0.0, 0.0, 0.0, self.channels[3]],
            },
            amount.0.flatten().unwrap_or(0.1),
        )
    }

    pub fn invert(&self, amount: Opt<Option<f64>>) -> Result<Self> {
        self.mix_values(
            &Self {
                channels: [
                    255.0 - self.channels[0],
                    255.0 - self.channels[1],
                    255.0 - self.channels[2],
                    self.channels[3],
                ],
            },
            amount.0.flatten().unwrap_or(1.0),
        )
    }

    pub fn grayscale(&self, amount: Opt<Option<f64>>) -> Result<Self> {
        let linear = self.linear_values();
        let luminance = linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
        let gray = 255.0
            * if luminance <= 0.0031308 {
                luminance * 12.92
            } else {
                1.055 * luminance.powf(1.0 / 2.4) - 0.055
            };
        self.mix_values(
            &Self {
                channels: [gray, gray, gray, self.channels[3]],
            },
            amount.0.flatten().unwrap_or(1.0),
        )
    }

    pub fn sepia(&self, amount: Opt<Option<f64>>) -> Result<Self> {
        self.mix_values(
            &Self::from_channels([
                self.channels[0] * 0.393
                    + self.channels[1] * 0.769
                    + self.channels[2] * 0.189,
                self.channels[0] * 0.349
                    + self.channels[1] * 0.686
                    + self.channels[2] * 0.168,
                self.channels[0] * 0.272
                    + self.channels[1] * 0.534
                    + self.channels[2] * 0.131,
                self.channels[3],
            ])?,
            amount.0.flatten().unwrap_or(1.0),
        )
    }

    pub fn blend<'js>(
        &self,
        source: Class<'js, Color>,
        mode: Opt<Option<String>>,
        amount: Opt<Option<f64>>,
    ) -> Result<Self> {
        let source = source.borrow();
        let amount = amount.0.flatten().unwrap_or(1.0);
        if !amount.is_finite() {
            return Err(Error::new_from_js_message(
                "blend",
                "amount",
                "Invalid blend amount.",
            ));
        }
        let source_alpha = source.channels[3] / 255.0 * amount.clamp(0.0, 1.0);
        let backdrop_alpha = self.channels[3] / 255.0;
        let alpha = source_alpha + backdrop_alpha * (1.0 - source_alpha);
        let mode = mode.0.flatten().unwrap_or_else(|| "normal".to_owned());
        let mut channels = [0.0; 4];
        for index in 0..3 {
            let backdrop = self.channels[index] / 255.0;
            let foreground = source.channels[index] / 255.0;
            let blended = match mode.as_str() {
                "normal" => foreground,
                "multiply" => backdrop * foreground,
                "screen" => backdrop + foreground - backdrop * foreground,
                "overlay" => {
                    if backdrop <= 0.5 {
                        2.0 * backdrop * foreground
                    } else {
                        1.0 - 2.0 * (1.0 - backdrop) * (1.0 - foreground)
                    }
                }
                "darken" => backdrop.min(foreground),
                "lighten" => backdrop.max(foreground),
                "color-dodge" => {
                    if backdrop == 0.0 {
                        0.0
                    } else if foreground == 1.0 {
                        1.0
                    } else {
                        (backdrop / (1.0 - foreground)).min(1.0)
                    }
                }
                "color-burn" => {
                    if backdrop == 1.0 {
                        1.0
                    } else if foreground == 0.0 {
                        0.0
                    } else {
                        1.0 - ((1.0 - backdrop) / foreground).min(1.0)
                    }
                }
                "hard-light" => {
                    if foreground <= 0.5 {
                        2.0 * backdrop * foreground
                    } else {
                        1.0 - 2.0 * (1.0 - backdrop) * (1.0 - foreground)
                    }
                }
                "soft-light" => {
                    if foreground <= 0.5 {
                        backdrop - (1.0 - 2.0 * foreground) * backdrop * (1.0 - backdrop)
                    } else {
                        backdrop
                            + (2.0 * foreground - 1.0)
                                * (if backdrop <= 0.25 {
                                    ((16.0 * backdrop - 12.0) * backdrop + 4.0) * backdrop
                                } else {
                                    backdrop.sqrt()
                                } - backdrop)
                    }
                }
                "difference" => (backdrop - foreground).abs(),
                "exclusion" => backdrop + foreground - 2.0 * backdrop * foreground,
                _ => {
                    return Err(Error::new_from_js_message(
                        "blend",
                        "mode",
                        "Invalid blend mode.",
                    ));
                }
            };
            channels[index] = if alpha == 0.0 {
                0.0
            } else {
                255.0
                    * ((1.0 - source_alpha) * backdrop_alpha * backdrop
                        + source_alpha
                            * ((1.0 - backdrop_alpha) * foreground + backdrop_alpha * blended))
                    / alpha
            };
        }
        channels[3] = alpha * 255.0;
        Self::from_channels(channels)
    }

    pub fn over<'js>(this: This<Class<'js, Color>>, background: Class<'js, Color>) -> Result<Self> {
        background.borrow().blend(this.0, Opt(None), Opt(None))
    }

    pub fn luminance(&self) -> f64 {
        let linear = self.linear_values();
        linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
    }

    #[qjs(rename = "contrastRatio")]
    pub fn contrast_ratio(&self, other: Class<'_, Color>) -> Result<f64> {
        let first = self.luminance();
        let second = other.borrow().luminance();
        Ok((first.max(second) + 0.05) / (first.min(second) + 0.05))
    }

    #[qjs(rename = "textColor")]
    pub fn text_color(&self) -> Result<Self> {
        if self.luminance() > 0.0525_f64.sqrt() - 0.05 {
            Self::from_channels([0.0, 0.0, 0.0, 255.0])
        } else {
            Self::from_channels([255.0, 255.0, 255.0, 255.0])
        }
    }

    pub fn equals(&self, other: Class<'_, Color>, tolerance: Opt<Option<f64>>) -> Result<bool> {
        let tolerance = tolerance.0.flatten().unwrap_or(0.0);
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(Error::new_from_js_message(
                "equals",
                "tolerance",
                "Invalid color tolerance.",
            ));
        }
        let other = other.borrow();
        Ok(self
            .channels
            .iter()
            .zip(other.channels)
            .all(|(first, second)| (first - second).abs() <= tolerance))
    }

    pub fn complement(&self) -> Result<Self> {
        let hsl = self.hsl_values();
        Self::from_hsl_values(hsl[0] + 180.0, hsl[1], hsl[2], hsl[3])
    }

    pub fn analogous<'js>(
        this: This<Class<'js, Color>>,
        ctx: Ctx<'js>,
        angle: Opt<Option<f64>>,
    ) -> Result<Vec<Class<'js, Color>>> {
        let original = this.0.clone();
        let color = original.borrow().clone();
        let angle = angle.0.flatten().unwrap_or(30.0);
        Ok(vec![
            Class::instance(ctx.clone(), color.rotate_hue(-angle)?)?,
            original.clone(),
            Class::instance(ctx, color.rotate_hue(angle)?)?,
        ])
    }

    pub fn triadic<'js>(
        this: This<Class<'js, Color>>,
        ctx: Ctx<'js>,
    ) -> Result<Vec<Class<'js, Color>>> {
        let original = this.0.clone();
        let color = original.borrow().clone();
        Ok(vec![
            original.clone(),
            Class::instance(ctx.clone(), color.rotate_hue(120.0)?)?,
            Class::instance(ctx, color.rotate_hue(240.0)?)?,
        ])
    }

    pub fn tetradic<'js>(
        this: This<Class<'js, Color>>,
        ctx: Ctx<'js>,
    ) -> Result<Vec<Class<'js, Color>>> {
        let original = this.0.clone();
        let color = original.borrow().clone();
        Ok(vec![
            original.clone(),
            Class::instance(ctx.clone(), color.rotate_hue(90.0)?)?,
            Class::instance(ctx.clone(), color.rotate_hue(180.0)?)?,
            Class::instance(ctx, color.rotate_hue(270.0)?)?,
        ])
    }

    #[qjs(rename = "splitComplementary")]
    pub fn split_complementary<'js>(
        this: This<Class<'js, Color>>,
        ctx: Ctx<'js>,
        angle: Opt<Option<f64>>,
    ) -> Result<Vec<Class<'js, Color>>> {
        let original = this.0.clone();
        let color = original.borrow().clone();
        let angle = angle.0.flatten().unwrap_or(30.0);
        Ok(vec![
            original.clone(),
            Class::instance(ctx.clone(), color.rotate_hue(180.0 - angle)?)?,
            Class::instance(ctx, color.rotate_hue(180.0 + angle)?)?,
        ])
    }

    #[qjs(rename = "toRgb")]
    pub fn to_rgb(&self) -> Vec<f64> {
        self.channels.map(f64::round).to_vec()
    }

    #[qjs(rename = "toRbg")]
    pub fn to_rbg(&self) -> Vec<f64> {
        self.to_rgb()
    }

    #[qjs(rename = "toNormalizedRgb")]
    pub fn to_normalized_rgb(&self) -> Vec<f64> {
        [
            self.channels[0] / 255.0,
            self.channels[1] / 255.0,
            self.channels[2] / 255.0,
            self.channels[3] / 255.0,
        ]
        .to_vec()
    }

    #[qjs(rename = "toLinearRgb")]
    pub fn to_linear_rgb(&self) -> Vec<f64> {
        self.linear_values().to_vec()
    }

    #[qjs(rename = "toHsv")]
    pub fn to_hsv(&self) -> Vec<f64> {
        self.hsv_values().to_vec()
    }

    #[qjs(rename = "toHsl")]
    pub fn to_hsl(&self) -> Vec<f64> {
        self.hsl_values().to_vec()
    }

    #[qjs(rename = "toHwb")]
    pub fn to_hwb(&self) -> Vec<f64> {
        let hsv = self.hsv_values();
        vec![
            hsv[0],
            self.channels[0]
                .min(self.channels[1])
                .min(self.channels[2])
                / 255.0,
            1.0 - self.channels[0]
                .max(self.channels[1])
                .max(self.channels[2])
                / 255.0,
            self.channels[3] / 255.0,
        ]
    }

    #[qjs(rename = "toCmyk")]
    pub fn to_cmyk(&self) -> Vec<f64> {
        let key = 1.0
            - self.channels[0]
                .max(self.channels[1])
                .max(self.channels[2])
                / 255.0;
        if key == 1.0 {
            return vec![0.0, 0.0, 0.0, 1.0, self.channels[3] / 255.0];
        }
        vec![
            (1.0 - self.channels[0] / 255.0 - key) / (1.0 - key),
            (1.0 - self.channels[1] / 255.0 - key) / (1.0 - key),
            (1.0 - self.channels[2] / 255.0 - key) / (1.0 - key),
            key,
            self.channels[3] / 255.0,
        ]
    }

    #[qjs(rename = "toHex")]
    pub fn to_hex(&self, include_alpha: Opt<Option<bool>>) -> String {
        let mut result = String::from("#");
        for channel in self
            .channels
            .iter()
            .take(if include_alpha.0.flatten().unwrap_or(false) {
                4
            } else {
                3
            })
        {
            result.push_str(&format!("{:02x}", channel.round() as u8));
        }
        result
    }

    #[qjs(rename = "toCss")]
    pub fn to_css(&self, format: Opt<Option<String>>) -> Result<String> {
        fn rounded(value: f64) -> f64 {
            // Components are nonnegative and at most 360. Round the exact binary
            // value like Number.toFixed(6), without first rounding value * 1e6.
            let bits = value.to_bits();
            let shift = 1069 - ((bits >> 52) & 0x7ff);
            if shift >= 128 { return 0.0; }
            let scaled = (((bits & ((1u64 << 52) - 1)) | (1u64 << 52)) as u128) * 15625;
            ((scaled + (1u128 << (shift - 1))) >> shift) as f64 / 1_000_000.0
        }

        let format = format.0.flatten();
        match format.as_deref().unwrap_or("rgb") {
            "hex" => Ok(self.to_hex(Some(Some(self.channels[3] < 255.0)).into())),
            "rgb" => {
                let rgb = self.to_rgb();
                Ok(format!(
                    "rgba({}, {}, {}, {})",
                    rgb[0], rgb[1], rgb[2], rounded(self.channels[3] / 255.0)
                ))
            }
            "hsl" | "hwb" => {
                let components = if format.as_deref() == Some("hsl") {
                    self.hsl_values()
                } else {
                    self.to_hwb().try_into().unwrap()
                };
                Ok(format!(
                    "{}({} {}% {}% / {})",
                    format.as_deref().unwrap(),
                    rounded(components[0]),
                    rounded(components[1] * 100.0),
                    rounded(components[2] * 100.0),
                    rounded(components[3])
                ))
            }
            _ => Err(Error::new_from_js_message(
                "toCss",
                "format",
                "Invalid CSS color format.",
            )),
        }
    }

    #[qjs(rename = "toString")]
    pub fn to_string(&self) -> String {
        self.to_hex(Some(Some(self.channels[3] < 255.0)).into())
    }

    #[qjs(rename = "toJSON")]
    pub fn to_json(&self) -> Vec<f64> {
        self.to_rgb()
    }
}

pub(super) fn install(ctx: &Ctx<'_>) -> Result<()> {
    Class::<Color>::define(&ctx.globals())
}

#[cfg(test)]
mod tests {
    use rquickjs::{Context, Runtime};

    #[test]
    fn factories_accept_tuples_and_positional_components() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            super::install(&ctx).unwrap();
            let result: String = ctx
                .eval(
                    r#"(() => {
                        const tuple = Color.fromRgb([1.25, 2.5, 3.75, 4]);
                        const positional = Color.fromRgb(1.25, 2.5, 3.75, 4);
                        const mapped = [[10, 20, 30]].map(Color.fromRgb)[0];
                        return JSON.stringify([
                            tuple instanceof Color,
                            tuple.equals(positional),
                            mapped.toRgb(),
                            tuple.toJSON(),
                            (() => { try { new Color(); return false; } catch (_) { return true; } })(),
                        ]);
                    })()"#,
                )
                .unwrap();
            assert_eq!(result, "[true,true,[10,20,30,255],[1,3,4,4],true]");
        });
    }

    #[test]
    fn css_grammar_and_linear_conversion_preserve_contract() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            super::install(&ctx).unwrap();
            let result: String = ctx
                .eval(
                    r#"(() => {
                        const legacy = Color.fromCss('rgba(255, 0, 0, 50%)');
                        const modern = Color.fromCss('hsl(120 100 50 / 25%)');
                        const bare = Color.fromCss('hsl(120 1 0.5)');
                        const wide = Color.fromCss('hwb(0 150% 50%)');
                        const linear = Color.fromLinearRgb([0, 0.5, 1, 0.25]);
                        return JSON.stringify([
                            legacy.toRgb(), modern.toRgb(),
                            bare.toHsl().slice(1, 3), wide.toRgb(),
                            linear.toRgb(),
                            (() => { try { Color.fromCss('hsl(0, 1, 50%)'); return false; } catch (_) { return true; } })(),
                        ]);
                    })()"#,
                )
                .unwrap();
            assert_eq!(
                result,
                "[[255,0,0,128],[0,255,0,64],[0.009999999999999766,0.005],[191,191,191,255],[0,188,255,64],true]"
            );
        });
    }

    #[test]
    fn blend_modes_and_palette_methods_cover_native_surface() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            super::install(&ctx).unwrap();
            assert!(ctx
                .eval::<bool, _>(
                    r#"(() => {
                        const base = Color.fromRgb(80, 120, 160, 200);
                        const source = Color.fromRgb(200, 100, 40, 180);
                        const modes = ['normal', 'multiply', 'screen', 'overlay', 'darken', 'lighten',
                            'color-dodge', 'color-burn', 'hard-light', 'soft-light', 'difference', 'exclusion'];
                        const blends = modes.map(mode => base.blend(source, mode).toRgb());
                        const palette = base.analogous();
                        return blends.length === 12
                            && blends.every(value => value.length === 4)
                            && palette[1] === base
                            && base.triadic()[0] === base
                            && base.tetradic()[0] === base
                            && base.splitComplementary()[0] === base;
                    })()"#,
                )
                .unwrap());
        });
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            super::install(&ctx).unwrap();
            assert!(ctx
                .eval::<bool, _>(
                    r#"(() => {
                        const rejects = [
                            () => Color.fromHex('##fff'),
                            () => Color.fromCss('rgb(10%, 20, 30%)'),
                            () => Color.fromCss('rgb(1e, 2, 3)'),
                            () => Color.fromRgb(NaN, 0, 0),
                            () => Color.fromHsl(0, 1, 0.5).blend(Color.fromRgb(0, 0, 0), 'unknown'),
                            () => Color.fromRgb(0, 0, 0).gamma(0),
                        ];
                        return rejects.every(fn => { try { fn(); return false; } catch (_) { return true; } });
                    })()"#,
                )
                .unwrap());
        });
    }
}
