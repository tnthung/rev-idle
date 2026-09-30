use rquickjs::{
    function::Args, Ctx, Function, Object, Persistent,
};

#[derive(Clone)]
pub(super) struct FunctionDescriptor {
    pub(super) source: String,
    pub(super) module_id: Option<String>,
    pub(super) line: Option<i32>,
    pub(super) column: Option<i32>,
}

pub(super) struct FunctionTransfer {
    to_string: Persistent<Function<'static>>,
    file_name: Persistent<Function<'static>>,
    line_number: Persistent<Function<'static>>,
    column_number: Persistent<Function<'static>>,
}

impl FunctionTransfer {
    pub(super) fn new<'js>(ctx: &Ctx<'js>) -> rquickjs::Result<Self> {
        let intrinsics: Object = ctx.eval(
            r#"({
                toString: Object.getOwnPropertyDescriptor(Function.prototype, "toString").value,
                fileName: Object.getOwnPropertyDescriptor(Function.prototype, "fileName").get,
                lineNumber: Object.getOwnPropertyDescriptor(Function.prototype, "lineNumber").get,
                columnNumber: Object.getOwnPropertyDescriptor(Function.prototype, "columnNumber").get,
            })"#,
        )?;
        Ok(Self {
            to_string: Persistent::save(ctx, intrinsics.get::<_, Function>("toString")?),
            file_name: Persistent::save(ctx, intrinsics.get::<_, Function>("fileName")?),
            line_number: Persistent::save(ctx, intrinsics.get::<_, Function>("lineNumber")?),
            column_number: Persistent::save(ctx, intrinsics.get::<_, Function>("columnNumber")?),
        })
    }

    pub(super) fn capture<'js>(
        &self,
        ctx: &Ctx<'js>,
        function: Function<'js>,
    ) -> rquickjs::Result<FunctionDescriptor> {
        let to_string = self.to_string.clone().restore(ctx)?;
        let mut args = Args::new(ctx.clone(), 0);
        args.this(function.clone())?;
        let source: String = args.apply(&to_string)?;

        let file_name = self.file_name.clone().restore(ctx)?;
        let mut args = Args::new(ctx.clone(), 0);
        args.this(function.clone())?;
        let module_id: Option<String> = args.apply(&file_name)?;

        let line_number = self.line_number.clone().restore(ctx)?;
        let mut args = Args::new(ctx.clone(), 0);
        args.this(function.clone())?;
        let line: Option<i32> = args.apply(&line_number)?;

        let column_number = self.column_number.clone().restore(ctx)?;
        let mut args = Args::new(ctx.clone(), 0);
        args.this(function)?;
        let column: Option<i32> = args.apply(&column_number)?;

        Ok(FunctionDescriptor {
            source,
            module_id,
            line,
            column,
        })
    }
}
