use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    path::{Component, Path, PathBuf},
    rc::Rc,
};

use oxc::{
    allocator::Allocator,
    codegen::{Codegen, CodegenOptions},
    diagnostics::{Diagnostics, NamedSource},
    parser::Parser,
    semantic::SemanticBuilder,
    span::SourceType,
    transformer::{TransformOptions, Transformer},
};
use oxc_sourcemap::SourceMap;
use rquickjs::{
    loader::{ImportAttributes, Loader, Resolver},
    module::Declared,
    Ctx,
    Error,
    Function,
    Module,
    Object,
};

const SOURCE_VERSION_SEPARATOR: &str = "?rev-source=";

/// Resolves the relative `import`/`export` specifiers scripts use to share
/// code with sibling files (e.g. `import { Action } from './_unity_loop.js'`).
/// Only relative specifiers are supported; there is no package-style module
/// resolution for this project's scripts.
/// The source hash changes the internal module name when the file changes,
/// preventing QuickJS from returning a stale cached namespace.
/// Joins `name` onto `base_dir`, collapsing `.`/`..` components instead of
/// leaving them as literal path segments (plain `Path::join` does not
/// normalize them, which would otherwise produce paths like `scripts\./lib`).
pub(super) fn join_normalized(base_dir: &Path, name: &str) -> PathBuf {
    let mut result = base_dir.to_path_buf();
    for component in Path::new(name).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

struct CompiledModule {
    source: String,
    map: Option<SourceMap<'static>>,
}

/// Keeps generated code and maps for every source version until the session ends.
/// The resolver compiles the same source snapshot that it hashes, so an edit
/// between resolution and loading cannot attach code to the wrong version.
#[derive(Clone, Default)]
pub(super) struct ScriptModules {
    modules: Rc<RefCell<HashMap<String, CompiledModule>>>,
}

impl ScriptModules {
    pub(super) fn insert(&self, name: &str, source: String) -> Result<(), String> {
        if self.modules.borrow().contains_key(name) {
            return Ok(());
        }
        let source_path = name
            .split_once(SOURCE_VERSION_SEPARATOR)
            .map_or(name, |(path, _)| path);
        let compiled = if Path::new(source_path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ts"))
        {
            let check_diagnostics = |diagnostics: Diagnostics| -> Result<(), String> {
                if diagnostics.is_empty() {
                    return Ok(());
                }
                Err(diagnostics
                    .into_iter()
                    .map(|error| error.render_with_source_code(NamedSource::new(source_path, source.as_str())))
                    .collect::<Vec<_>>()
                    .join("\n"))
            };
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &source, SourceType::ts()).parse();
            check_diagnostics(parsed.diagnostics)?;
            let mut program = parsed.program;
            let semantic = SemanticBuilder::new()
                .with_check_syntax_error(true)
                .with_enum_eval(true)
                .build(&program);
            check_diagnostics(semantic.diagnostics)?;
            check_diagnostics(Transformer::new(&allocator, Path::new(source_path), &TransformOptions::default())
                .build_with_scoping(semantic.semantic.into_scoping(), &mut program)
                .diagnostics)?;
            let generated = Codegen::new()
                .with_options(CodegenOptions {
                    source_map_path: Some(PathBuf::from(source_path)),
                    ..CodegenOptions::default()
                })
                .build(&program);
            CompiledModule {
                source: generated.code,
                map: generated.map.map(|map| map.into_owned()),
            }
        } else {
            CompiledModule { source, map: None }
        };
        self.modules.borrow_mut().insert(name.to_owned(), compiled);
        Ok(())
    }

    pub(super) fn install_stack_trace(&self, ctx: &Ctx<'_>) -> rquickjs::Result<()> {
        let modules = self.modules.clone();
        let lookup = Function::new(ctx.clone(), move |path: Option<String>, line: i32, column: i32| -> Option<String> {
            if line <= 0 || column <= 0 {
                return None;
            }
            let modules = modules.borrow();
            let module = modules.get(&path?)?;
            let map = module.map.as_ref()?;
            // QuickJS uses one-based UTF-8 byte columns; source maps use
            // zero-based UTF-16 columns, including for non-ASCII generated code.
            let token = map.lookup_source_view_token(
                &map.generate_lookup_table(),
                (line - 1) as u32,
                module.source.lines().nth((line - 1) as usize)?
                    .get(..(column - 1) as usize)?.encode_utf16().count() as u32,
            )?;
            Some(format!("{}:{}:{}", token.get_source()?, token.get_src_line() + 1, token.get_src_col() + 1))
        })?;
        // Keep QuickJS's trace-only stack format so console and CaughtError do
        // not duplicate the error header. Native and unmapped frames stay visible.
        let prepare: Function = ctx.eval(r#"
            lookup => (_error, frames) => {
                let stack = "";
                for (const frame of frames) {
                    const name = frame.getFunctionName() || "<anonymous>";
                    if (frame.isNative()) {
                        stack += `    at ${name} (native)\n`;
                        continue;
                    }
                    const path = frame.getFileName();
                    const line = frame.getLineNumber();
                    const column = frame.getColumnNumber();
                    const location = lookup(path, line, column)
                        || `${path || "<null>"}${line > 0 ? `:${line}:${column}` : ""}`;
                    stack += frame.getFunction() === null
                        ? `    at ${location}\n`
                        : `    at ${name} (${location})\n`;
                }
                return stack;
            }
        "#)?;
        ctx.globals().get::<_, Object>("Error")?.set(
            "prepareStackTrace",
            prepare.call::<_, Function>((lookup,))?,
        )
    }
}

impl Resolver for ScriptModules {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<String> {
        if !(name.starts_with("./") || name.starts_with("../")) {
            return Err(Error::new_resolving_message(
                base,
                name,
                "only relative imports (starting with ./ or ../) are supported",
            ));
        }

        let base = base
            .split_once(SOURCE_VERSION_SEPARATOR)
            .map_or(base, |(path, _)| path);
        let base_dir = Path::new(base).parent().unwrap_or_else(|| Path::new("."));
        let mut resolved = join_normalized(base_dir, name);
        if resolved.extension().is_none() {
            resolved.set_extension("js");
        }
        let source = std::fs::read(&resolved)
            .map_err(|error| Error::new_resolving_message(base, name, error.to_string()))?;
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        let name = format!(
            "{}{}{:016x}",
            resolved.to_string_lossy(),
            SOURCE_VERSION_SEPARATOR,
            hasher.finish(),
        );
        self.insert(
            &name,
            String::from_utf8(source)
                .map_err(|error| Error::new_loading_message(&name, error.to_string()))?,
        )
        .map_err(|error| Error::new_loading_message(&name, error))?;
        Ok(name)
    }
}

impl Loader for ScriptModules {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        path: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<Module<'js, Declared>> {
        // Release the registry borrow before QuickJS can resolve more imports or
        // invoke the stack hook during declaration.
        let source = self.modules.borrow().get(path)
            .ok_or_else(|| Error::new_loading_message(path, "module source was not resolved"))?
            .source.clone();
        Module::declare(ctx.clone(), path, source)
    }
}
