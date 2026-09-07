use std::path::{Component, Path, PathBuf};

use rquickjs::{
    loader::{ImportAttributes, Loader, Resolver},
    module::Declared,
    Ctx,
    Error,
    Module,
};

/// Resolves the relative `import`/`export` specifiers scripts use to share
/// code with sibling files (e.g. `import { Action } from './_unity_loop.js'`).
/// Only relative specifiers are supported; there is no package-style module
/// resolution for this project's scripts.
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

pub(super) struct ScriptModuleResolver;

impl Resolver for ScriptModuleResolver {
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

        let base_dir = Path::new(base).parent().unwrap_or_else(|| Path::new("."));
        let mut resolved = join_normalized(base_dir, name);
        if resolved.extension().is_none() {
            resolved.set_extension("js");
        }
        Ok(resolved.to_string_lossy().into_owned())
    }
}

pub(super) struct ScriptModuleLoader;

impl Loader for ScriptModuleLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        path: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<Module<'js, Declared>> {
        let source = std::fs::read_to_string(path)
            .map_err(|error| Error::new_loading_message(path, error.to_string()))?;
        Module::declare(ctx.clone(), path, source)
    }
}
