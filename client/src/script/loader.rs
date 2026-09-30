use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    path::{Component, Path, PathBuf},
    rc::Rc,
};

use oxc::{
    allocator::{Allocator, CloneIn},
    ast::ast::{Expression, ImportDeclarationSpecifier, Statement},
    codegen::{Codegen, CodegenOptions},
    diagnostics::{Diagnostics, NamedSource},
    parser::Parser,
    semantic::SemanticBuilder,
    span::SourceType,
    str::Str,
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
use uuid::Uuid;

use super::transfer::FunctionDescriptor;

const SOURCE_VERSION_SEPARATOR: &str = "?rev-source=";
const STATIC_IMPORT_PREFIX: &str = "?rev-static=";
const TRANSFER_MODULE_SUFFIX: &str = "?rev-transfer=";

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ModuleImports {
    pub(super) module_id: String,
    pub(super) resolution_base: String,
    pub(super) declarations: Vec<String>,
}

#[derive(Clone)]
struct StaticEdge {
    owner_module_id: String,
    original_specifier: String,
    token: String,
    resolved_id: Option<String>,
}

struct CompiledModule {
    source: String,
    map: Option<SourceMap<'static>>,
    imports: ModuleImports,
}

/// Keeps generated code and maps for every source version until the session ends.
/// The resolver compiles the same source snapshot that it hashes, so an edit
/// between resolution and loading cannot attach code to the wrong version.
#[derive(Clone, Default)]
pub(super) struct ScriptModules {
    modules: Rc<RefCell<HashMap<String, CompiledModule>>>,
    static_edges: Rc<RefCell<HashMap<String, StaticEdge>>>,
}

impl ScriptModules {
    pub(super) fn insert(&self, name: &str, source: String) -> Result<(), String> {
        if self.modules.borrow().contains_key(name) {
            return Ok(());
        }
        let source_path = name
            .split_once(SOURCE_VERSION_SEPARATOR)
            .map_or(name, |(path, _)| path);
        let is_typescript = Path::new(source_path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ts"));
        let source_type = if is_typescript {
            SourceType::ts().with_module(true)
        } else {
            SourceType::mjs()
        };
        let check_diagnostics = |diagnostics: Diagnostics| -> Result<(), String> {
            if diagnostics.is_empty() {
                return Ok(());
            }
            Err(diagnostics
                .into_iter()
                .map(|error| {
                    error.render_with_source_code(NamedSource::new(source_path, source.as_str()))
                })
                .collect::<Vec<_>>()
                .join("\n"))
        };
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &source, source_type).parse();
        check_diagnostics(parsed.diagnostics)?;
        let mut program = parsed.program;
        if is_typescript {
            let semantic = SemanticBuilder::new()
                .with_check_syntax_error(true)
                .with_enum_eval(true)
                .build(&program);
            check_diagnostics(semantic.diagnostics)?;
            check_diagnostics(
                Transformer::new(
                    &allocator,
                    Path::new(source_path),
                    &TransformOptions::default(),
                )
                .build_with_scoping(semantic.semantic.into_scoping(), &mut program)
                .diagnostics,
            )?;
        }

        let mut edges = Vec::new();
        for statement in &mut program.body {
            let source_literal = match statement {
                Statement::ImportDeclaration(declaration) => Some(&mut declaration.source),
                Statement::ExportFromDeclaration(declaration) => Some(&mut declaration.source),
                Statement::ExportAllDeclaration(declaration) => Some(&mut declaration.source),
                _ => None,
            };
            let Some(source_literal) = source_literal else {
                continue;
            };
            let original_specifier = source_literal.value.to_string();
            let token = format!("{STATIC_IMPORT_PREFIX}{}", Uuid::new_v4());
            source_literal.value = Str::from(allocator.alloc_str(&token));
            source_literal.raw = None;
            edges.push(StaticEdge {
                owner_module_id: name.to_owned(),
                original_specifier,
                token,
                resolved_id: None,
            });
        }

        let mut declarations = Vec::new();
        for statement in &program.body {
            let Statement::ImportDeclaration(declaration) = statement else {
                continue;
            };
            if declaration.import_kind.is_type()
                || declaration.specifiers.as_ref().is_some_and(|specifiers| {
                    !specifiers.iter().any(|specifier| {
                        match specifier {
                            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                                !specifier.import_kind.is_type()
                            }
                            ImportDeclarationSpecifier::ImportDefaultSpecifier(_)
                            | ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => true,
                        }
                    })
                })
            {
                continue;
            }
            let mut declaration_program = program.clone_in(&allocator);
            declaration_program.comments.clear();
            declaration_program.hashbang = None;
            declaration_program.directives.clear();
            declaration_program.body.clear();
            declaration_program.body.push(statement.clone_in(&allocator));
            declarations.push(Codegen::new().build(&declaration_program).code);
        }

        let generated = Codegen::new()
            .with_options(CodegenOptions {
                source_map_path: Some(PathBuf::from(source_path)),
                ..CodegenOptions::default()
            })
            .build(&program);
        self.modules.borrow_mut().insert(
            name.to_owned(),
            CompiledModule {
                source: generated.code,
                map: generated.map.map(|map| map.into_owned()),
                imports: ModuleImports {
                    module_id: name.to_owned(),
                    resolution_base: source_path.to_owned(),
                    declarations,
                },
            },
        );
        let mut static_edges = self.static_edges.borrow_mut();
        for edge in edges {
            static_edges.insert(edge.token.clone(), edge);
        }
        Ok(())
    }

    pub(super) fn imports(&self, id: &str) -> Option<ModuleImports> {
        self.modules.borrow().get(id).map(|module| module.imports.clone())
    }

    pub(super) fn transfer_module(
        &self,
        registration: Uuid,
        function: &FunctionDescriptor,
    ) -> Result<String, String> {
        let module_id = function
            .module_id
            .as_ref()
            .ok_or_else(|| "function has no defining module origin".to_owned())?;
        let imports = self
            .imports(module_id)
            .ok_or_else(|| format!("defining module `{module_id}` is not retained"))?;
        let allocator = Allocator::default();
        let expression_source = format!("({})", function.source);
        let parsed_expression = Parser::new(&allocator, &expression_source, SourceType::mjs()).parse();
        let mut function_source = None;
        if parsed_expression.diagnostics.is_empty() {
            if let Some(Statement::ExpressionStatement(statement)) = parsed_expression.program.body.first()
            {
                let mut expression = &statement.expression;
                while let Expression::ParenthesizedExpression(parenthesized) = expression {
                    expression = &parenthesized.expression;
                }
                if matches!(
                    expression,
                    Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
                ) {
                    function_source = Some(format!("export default ({});", function.source));
                }
            }
        }
        let function_source = if let Some(function_source) = function_source {
            function_source
        } else {
            let method_wrapper = format!("({{{}}})", function.source);
            let parsed_method = Parser::new(&allocator, &method_wrapper, SourceType::mjs()).parse();
            let Some(Statement::ExpressionStatement(statement)) = parsed_method.program.body.first()
            else {
                return Err(format!(
                    "cannot reconstruct function from {}:{}:{}: source is not a function expression or ordinary object method",
                    module_id,
                    function.line.unwrap_or(0),
                    function.column.unwrap_or(0),
                ));
            };
            let mut expression = &statement.expression;
            while let Expression::ParenthesizedExpression(parenthesized) = expression {
                expression = &parenthesized.expression;
            }
            let Expression::ObjectExpression(object) = expression else {
                return Err(format!(
                    "cannot reconstruct function from {}:{}:{}: source is not a function expression or ordinary object method",
                    module_id,
                    function.line.unwrap_or(0),
                    function.column.unwrap_or(0),
                ));
            };
            if object.properties.len() != 1 {
                return Err(format!(
                    "cannot reconstruct function from {}:{}:{}: method source has no single property",
                    module_id,
                    function.line.unwrap_or(0),
                    function.column.unwrap_or(0),
                ));
            }
            let Some(property) = object.properties.first() else {
                return Err(format!(
                    "cannot reconstruct function from {}:{}:{}: method source has no property",
                    module_id,
                    function.line.unwrap_or(0),
                    function.column.unwrap_or(0),
                ));
            };
            let oxc::ast::ast::ObjectPropertyKind::ObjectProperty(property) = property else {
                return Err(format!(
                    "cannot reconstruct function from {}:{}:{}: spread methods are unsupported",
                    module_id,
                    function.line.unwrap_or(0),
                    function.column.unwrap_or(0),
                ));
            };
            if !property.method {
                return Err(format!(
                    "cannot reconstruct function from {}:{}:{}: source is not an ordinary object method",
                    module_id,
                    function.line.unwrap_or(0),
                    function.column.unwrap_or(0),
                ));
            }
            let property_access = match &property.key {
                oxc::ast::ast::PropertyKey::StaticIdentifier(identifier) => {
                    format!(".{}", identifier.name)
                }
                oxc::ast::ast::PropertyKey::StringLiteral(literal) => {
                    format!(
                        "[{}]",
                        serde_json::to_string(literal.value.as_str())
                            .map_err(|error| error.to_string())?
                    )
                }
                _ => {
                    return Err(format!(
                        "cannot reconstruct function from {}:{}:{}: computed method keys are unsupported",
                        module_id,
                        function.line.unwrap_or(0),
                        function.column.unwrap_or(0),
                    ));
                }
            };
            format!(
                "const __rev_transfer_method = {{{}}}; export default __rev_transfer_method{};",
                function.source, property_access
            )
        };
        let generated_id = format!("{module_id}{TRANSFER_MODULE_SUFFIX}{registration}");
        if !self.modules.borrow().contains_key(&generated_id) {
            let generated_imports = ModuleImports {
                module_id: generated_id.clone(),
                resolution_base: imports.resolution_base,
                declarations: imports.declarations,
            };
            let source = if generated_imports.declarations.is_empty() {
                function_source
            } else {
                format!("{}\n{}", generated_imports.declarations.join("\n"), function_source)
            };
            self.modules.borrow_mut().insert(
                generated_id.clone(),
                CompiledModule {
                    source,
                    map: None,
                    imports: generated_imports,
                },
            );
        }
        Ok(generated_id)
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
        // Keep QuickJS's trace-only stack format so console and CaughtError
        // do not duplicate the error header. Native and unmapped frames stay visible.
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
        if name.starts_with(STATIC_IMPORT_PREFIX) {
            let edge = self.static_edges.borrow().get(name).cloned();
            let Some(edge) = edge else {
                return Err(Error::new_resolving_message(
                    base,
                    name,
                    "unknown retained static import token",
                ));
            };
            if let Some(resolved_id) = edge.resolved_id {
                return Ok(resolved_id);
            }
            let owner_resolution_base = self
                .modules
                .borrow()
                .get(&edge.owner_module_id)
                .map(|module| module.imports.resolution_base.clone())
                .unwrap_or_else(|| {
                    edge.owner_module_id
                        .split_once(SOURCE_VERSION_SEPARATOR)
                        .map_or_else(|| edge.owner_module_id.clone(), |(path, _)| path.to_owned())
                });
            let base_dir = Path::new(&owner_resolution_base)
                .parent()
                .unwrap_or_else(|| Path::new("."));
            let mut resolved = join_normalized(base_dir, &edge.original_specifier);
            if resolved.extension().is_none() {
                resolved.set_extension("js");
            }
            let source = std::fs::read(&resolved).map_err(|error| {
                Error::new_resolving_message(
                    &edge.owner_module_id,
                    &edge.original_specifier,
                    error.to_string(),
                )
            })?;
            let mut hasher = DefaultHasher::new();
            source.hash(&mut hasher);
            let resolved_id = format!(
                "{}{}{:016x}",
                resolved.to_string_lossy(),
                SOURCE_VERSION_SEPARATOR,
                hasher.finish(),
            );
            self.insert(
                &resolved_id,
                String::from_utf8(source)
                    .map_err(|error| Error::new_loading_message(&resolved_id, error.to_string()))?,
            )
            .map_err(|error| Error::new_loading_message(&resolved_id, error))?;
            if let Some(edge) = self.static_edges.borrow_mut().get_mut(name) {
                edge.resolved_id = Some(resolved_id.clone());
            }
            return Ok(resolved_id);
        }
        if name.starts_with('?') {
            return Err(Error::new_resolving_message(
                base,
                name,
                "unknown private module token",
            ));
        }
        if !(name.starts_with("./") || name.starts_with("../")) {
            return Err(Error::new_resolving_message(
                base,
                name,
                "only relative imports (starting with ./ or ../) are supported",
            ));
        }

        let resolution_base = self
            .modules
            .borrow()
            .get(base)
            .map(|module| module.imports.resolution_base.clone())
            .unwrap_or_else(|| {
                base.split_once(SOURCE_VERSION_SEPARATOR)
                    .map_or_else(|| base.to_owned(), |(path, _)| path.to_owned())
            });
        let base_dir = Path::new(&resolution_base)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let mut resolved = join_normalized(base_dir, name);
        if resolved.extension().is_none() {
            resolved.set_extension("js");
        }
        let source = std::fs::read(&resolved)
            .map_err(|error| Error::new_resolving_message(&resolution_base, name, error.to_string()))?;
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        let resolved_id = format!(
            "{}{}{:016x}",
            resolved.to_string_lossy(),
            SOURCE_VERSION_SEPARATOR,
            hasher.finish(),
        );
        self.insert(
            &resolved_id,
            String::from_utf8(source)
                .map_err(|error| Error::new_loading_message(&resolved_id, error.to_string()))?,
        )
        .map_err(|error| Error::new_loading_message(&resolved_id, error))?;
        Ok(resolved_id)
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
        let source = self
            .modules
            .borrow()
            .get(path)
            .ok_or_else(|| Error::new_loading_message(path, "module source was not resolved"))?
            .source
            .clone();
        Module::declare(ctx.clone(), path, source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquickjs::{Context, Runtime};

    #[test]
    fn two_runtime_loader_static_graph_keeps_versions() {
        let root = std::env::temp_dir().join(format!("rev-idle-loader-static-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let entry = root.join("entry.js");
        let barrel = root.join("barrel.js");
        let dependency = root.join("dependency.js");
        std::fs::write(&dependency, "export const value = 1;").unwrap();
        std::fs::write(
            &barrel,
            "import { value } from \"./dependency.js\"; export { value };",
        )
        .unwrap();
        std::fs::write(
            &entry,
            "import { value } from \"./barrel.js\"; export default value;",
        )
        .unwrap();
        let entry_id = entry.to_string_lossy().into_owned();
        let modules = ScriptModules::default();
        modules
            .insert(&entry_id, std::fs::read_to_string(&entry).unwrap())
            .unwrap();
        let entry_token = modules
            .static_edges
            .borrow()
            .values()
            .find(|edge| edge.owner_module_id == entry_id)
            .map(|edge| edge.token.clone())
            .unwrap();
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let mut main_modules = modules.clone();
            let barrel_id = Resolver::resolve(&mut main_modules, &ctx, &entry_id, &entry_token, None).unwrap();
            let barrel_token = modules
                .static_edges
                .borrow()
                .values()
                .find(|edge| edge.owner_module_id == barrel_id)
                .map(|edge| edge.token.clone())
                .unwrap();
            let dependency_id = Resolver::resolve(&mut main_modules, &ctx, &barrel_id, &barrel_token, None).unwrap();

            std::fs::write(&barrel, "import { value } from \"./dependency.js\"; export { value };").unwrap();
            std::fs::write(&dependency, "export const value = 2;").unwrap();

            let mut background_modules = modules.clone();
            assert_eq!(
                Resolver::resolve(&mut background_modules, &ctx, &entry_id, &entry_token, None).unwrap(),
                barrel_id,
            );
            assert_eq!(
                Resolver::resolve(&mut background_modules, &ctx, &barrel_id, &barrel_token, None).unwrap(),
                dependency_id,
            );
            assert_ne!(
                Resolver::resolve(&mut background_modules, &ctx, &barrel_id, "./dependency.js", None).unwrap(),
                dependency_id,
            );
        });
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn two_runtime_loader_import_forms_and_maps() {
        let root = std::env::temp_dir().join(format!("rev-idle-loader-imports-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let typescript = root.join("entry.ts");
        let javascript = root.join("entry.js");
        let typescript_id = typescript.to_string_lossy().into_owned();
        let javascript_id = javascript.to_string_lossy().into_owned();
        std::fs::write(
            &typescript,
            r#"
                import Default, { value as alias, type TypeOnly } from "./dependency.ts";
                import * as namespace from "./dependency.ts";
                import "./side.js";
                import type { TypeOnly as Only } from "./types.ts";
                export default [Default, alias, namespace.value];
            "#,
        )
        .unwrap();
        std::fs::write(&javascript, "import \"./side.js\"; export default 1;").unwrap();
        let modules = ScriptModules::default();
        modules
            .insert(&typescript_id, std::fs::read_to_string(&typescript).unwrap())
            .unwrap();
        modules
            .insert(&javascript_id, std::fs::read_to_string(&javascript).unwrap())
            .unwrap();
        let typescript_imports = modules.imports(&typescript_id).unwrap();
        assert_eq!(typescript_imports.declarations.len(), 3);
        assert!(typescript_imports
            .declarations
            .iter()
            .all(|declaration| declaration.contains(STATIC_IMPORT_PREFIX)));
        assert!(typescript_imports
            .declarations
            .iter()
            .all(|declaration| !declaration.contains("TypeOnly") && !declaration.contains("Only")));
        assert!(modules
            .modules
            .borrow()
            .get(&typescript_id)
            .and_then(|module| module.map.as_ref())
            .is_some());
        assert!(modules
            .modules
            .borrow()
            .get(&javascript_id)
            .and_then(|module| module.map.as_ref())
            .is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn loader_transfer_module_reconstructs_function_and_method_sources() {
        let modules = ScriptModules::default();
        let module_id = "scripts/defining.js";
        modules
            .insert(
                module_id,
                "import { value } from \"./dependency.js\"; export const callback = value;".to_owned(),
            )
            .unwrap();
        let function = FunctionDescriptor {
            source: "(value) => value + 1".to_owned(),
            module_id: Some(module_id.to_owned()),
            line: Some(4),
            column: Some(8),
        };
        let first = modules
            .transfer_module(Uuid::new_v4(), &function)
            .unwrap();
        let second = modules
            .transfer_module(Uuid::new_v4(), &function)
            .unwrap();
        assert_ne!(first, second);
        let first_source = modules.modules.borrow().get(&first).unwrap().source.clone();
        assert!(first_source.contains(STATIC_IMPORT_PREFIX));
        assert!(first_source.contains("export default ((value) => value + 1);"));
        let method = FunctionDescriptor {
            source: "async handleClick(value) { return value + 1; }".to_owned(),
            module_id: Some(module_id.to_owned()),
            line: Some(5),
            column: Some(8),
        };
        let method_id = modules
            .transfer_module(Uuid::new_v4(), &method)
            .unwrap();
        let method_source = modules.modules.borrow().get(&method_id).unwrap().source.clone();
        assert!(method_source.contains("__rev_transfer_method.handleClick"));
    }
}
