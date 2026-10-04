use rquickjs::{CatchResultExt, Context, Runtime};

#[test]
fn native_colors_match_existing_script_results() {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    context.with(|ctx| {
        super::color::install(&ctx).unwrap();
        let mut failures = Vec::new();
        for (source, expected) in [
            (r#"Color.fromRgb(-1, 300, 2.5).toRgb()"#, r#"[0,255,3,255]"#),
            (r#"Color.fromRgb([1, 2, 3, undefined]).toRgb()"#, r#"[1,2,3,255]"#),
            (r#"Color.fromNormalizedRgb([1, .5, 0, .25]).toRgb()"#, r#"[255,128,0,64]"#),
            (r#"Color.fromLinearRgb(.001, .5, 1, .25).toRgb()"#, r#"[3,188,255,64]"#),
            (r#"Color.fromHsl([480, 1, .5, .5]).toRgb()"#, r#"[0,255,0,128]"#),
            (r#"Color.fromHsv(-60, 1, 1, .5).toRgb()"#, r#"[255,0,255,128]"#),
            (r#"Color.fromHwb([240, .25, .25, .5]).toRgb()"#, r#"[64,64,191,128]"#),
            (r#"Color.fromCmyk(0, 1, 1, .5, .25).toRgb()"#, r#"[128,0,0,64]"#),
            (r#"Color.fromHex('abc').toRgb()"#, r#"[170,187,204,255]"#),
            (r#"Color.fromHex('#abcd').toRgb()"#, r#"[170,187,204,221]"#),
            (r#"Color.fromHex('#123456').toRgb()"#, r#"[18,52,86,255]"#),
            (r#"Color.fromHex('#12345678').toRgb()"#, r#"[18,52,86,120]"#),
            (r#"Color.fromCss('transparent').toRgb()"#, r#"[0,0,0,0]"#),
            (r#"Color.fromCss('rgb(50% 0% 100%)').toRgb()"#, r#"[128,0,255,255]"#),
            (r#"Color.fromCss('hsl(.5turn 100% 50% / .5)').toRgb()"#, r#"[0,255,255,128]"#),
            (r#"Color.fromCss('hsl(200grad, 100%, 50%)').toRgb()"#, r#"[0,255,255,255]"#),
            (r#"Color.fromCss('hsl(3.141592653589793rad 100% 50%)').toRgb()"#, r#"[0,255,255,255]"#),
            (r#"Color.fromCss('hwb(0 150% 50%)').toRgb()"#, r#"[191,191,191,255]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).brightness(.7).toRgb()"#, r#"[56,84,112,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).contrast(.5).toRgb()"#, r#"[104,124,144,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).gamma(2).toRgb()"#, r#"[143,175,202,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).rotateHue(90).toRgb()"#, r#"[160,80,160,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).saturation(.5).toRgb()"#, r#"[100,120,140,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).lighten().toRgb()"#, r#"[109,146,182,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).darken().toRgb()"#, r#"[63,95,126,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).opacity(.5).toRgb()"#, r#"[80,120,160,128]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).fade(.5).toRgb()"#, r#"[80,120,160,100]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).tint().toRgb()"#, r#"[98,134,170,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).shade().toRgb()"#, r#"[72,108,144,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).invert().toRgb()"#, r#"[175,135,95,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).grayscale().toRgb()"#, r#"[117,117,117,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).sepia().toRgb()"#, r#"[154,137,107,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).mix(Color.fromRgb(200, 100, 40, 180)).toRgb()"#, r#"[140,110,100,190]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).over(Color.fromRgb(200, 100, 40, 180)).toRgb()"#, r#"[100,117,140,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).complement().toRgb()"#, r#"[160,120,80,200]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).textColor().toRgb()"#, r#"[255,255,255,255]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'normal').toRgb()"#, r#"[170,105,70,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'multiply').toRgb()"#, r#"[89,74,61,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'screen').toRgb()"#, r#"[181,148,149,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'overlay').toRgb()"#, r#"[126,101,102,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'darken').toRgb()"#, r#"[100,105,70,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'lighten').toRgb()"#, r#"[170,117,140,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'color-dodge').toRgb()"#, r#"[203,163,158,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'color-burn').toRgb()"#, r#"[71,46,46,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'hard-light').toRgb()"#, r#"[158,101,76,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'soft-light').toRgb()"#, r#"[121,109,116,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'difference').toRgb()"#, r#"[123,58,117,239]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).blend(Color.fromRgb(200, 100, 40, 180), 'exclusion').toRgb()"#, r#"[144,120,134,239]"#),
            (r#"Color.fromRgb(0, 0, 0, 0).blend(Color.fromRgb(255, 255, 255, 0)).toRgb()"#, r#"[0,0,0,0]"#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toNormalizedRgb()"#, r#"[1,0,0,0.5]"#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toLinearRgb()"#, r#"[1,0,0,0.5]"#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toHsl()"#, r#"[0,1,0.5,0.5]"#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toHsv()"#, r#"[0,1,1,0.5]"#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toHwb()"#, r#"[0,0,0,0.5]"#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toCmyk()"#, r#"[0,1,1,0,0.5]"#),
            (r#"Color.fromRgb(0, 0, 0).toCmyk()"#, r#"[0,0,0,1,1]"#),
            (r#"Color.fromRgb(0, 0, 0).contrastRatio(Color.fromRgb(255, 255, 255))"#, r#"21"#),
            (r#"Color.fromRgb(0, 0, 0).luminance()"#, r#"0"#),
            (r#"Color.fromRgb(0, 0, 0).textColor().toRgb()"#, r#"[255,255,255,255]"#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toCss('rgb')"#, r#""rgba(255, 0, 0, 0.5)""#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toCss('hsl')"#, r#""hsl(0 100% 50% / 0.5)""#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toCss('hwb')"#, r#""hwb(0 0% 0% / 0.5)""#),
            (r#"Color.fromRgb(255, 0, 0, 127.5).toCss('hex')"#, r##""#ff000080""##),
            (r#"Color.fromRgb(0, 0, 0).opacity(.0078125).toCss()"#, r#""rgba(0, 0, 0, 0.007813)""#),
            (r#"Color.fromRgb(0, 0, 0).opacity(.0000005).toCss()"#, r#""rgba(0, 0, 0, 0)""#),
            (r#"Color.fromRgb(200, 200, 200).sepia(.5).toRgb()"#, r#"[228,220,194,255]"#),
            (r#"Color.fromRgb(80, 120, 160, 200).over(Color.fromRgb(200, 100, 40, 180)).equals(Color.fromRgb(200, 100, 40, 180).blend(Color.fromRgb(80, 120, 160, 200)))"#, r#"true"#),
            (r#"Color.fromHwb(120.5, .13, .27, .41).toNormalizedRgb()"#, r#"[0.13,0.73,0.13499999999999998,0.41]"#),
        ] {
            let actual: String = ctx.eval(format!("JSON.stringify({source})")).catch(&ctx).unwrap();
            if actual != expected { failures.push(format!("{source}: expected {expected}, received {actual}")); }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    });
}
