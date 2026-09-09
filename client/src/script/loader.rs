use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Component, Path, PathBuf},
};

use rquickjs::{
    loader::{ImportAttributes, Loader, Resolver},
    module::Declared,
    Ctx,
    Error,
    Module,
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
        Ok(format!(
            "{}{}{:016x}",
            resolved.to_string_lossy(),
            SOURCE_VERSION_SEPARATOR,
            hasher.finish(),
        ))
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
        let source_path = path
            .split_once(SOURCE_VERSION_SEPARATOR)
            .map_or(path, |(path, _)| path);
        let source = std::fs::read_to_string(source_path)
            .map_err(|error| Error::new_loading_message(source_path, error.to_string()))?;
        Module::declare(ctx.clone(), path, source)
    }
}
