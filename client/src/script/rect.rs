use rquickjs::{class::Trace, Class, Ctx, JsLifetime, Object, Result};

#[derive(Clone, Default, Trace, JsLifetime)]
#[rquickjs::class]
pub(super) struct Rect {
    #[qjs(get, set)]
    pub(super) top: f64,
    #[qjs(get, set)]
    pub(super) left: f64,
    #[qjs(get, set)]
    pub(super) right: f64,
    #[qjs(get, set)]
    pub(super) bottom: f64,
    #[qjs(get, set)]
    pub(super) width: f64,
    #[qjs(get, set)]
    pub(super) height: f64,
}

#[rquickjs::methods]
impl Rect {
    #[qjs(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[qjs(rename = "toJSON")]
    pub fn to_json<'js>(&self, ctx: Ctx<'js>) -> Result<Object<'js>> {
        let value = Object::new(ctx)?;
        for (field, number) in [
            ("top", self.top), ("left", self.left), ("right", self.right),
            ("bottom", self.bottom), ("width", self.width), ("height", self.height),
        ] {
            value.set(field, number)?;
        }
        Ok(value)
    }
}

pub(super) fn install(ctx: &Ctx<'_>) -> Result<()> {
    Class::<Rect>::define(&ctx.globals())
}
