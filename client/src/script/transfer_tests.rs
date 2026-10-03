use super::transfer::FunctionTransfer;
use rquickjs::{Context, Function, Module, Object, Runtime};

#[test]
fn captures_function_source_forms_and_ignores_spoofed_intrinsics() {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    context.with(|ctx| {
        let transfer = FunctionTransfer::new(&ctx).unwrap();
        ctx.eval::<(), _>(
            r#"
                Function.prototype.toString = () => "spoofed prototype source";
                Object.defineProperty(Function.prototype, "fileName", { get: () => "spoofed prototype file" });
                Object.defineProperty(Function.prototype, "lineNumber", { get: () => 99 });
                Object.defineProperty(Function.prototype, "columnNumber", { get: () => 99 });
            "#,
        )
        .unwrap();
        let forms: Object = ctx
            .eval(
                r#"({
                    ordinary: function ordinary(value) { return value + 1; },
                    asynchronous: async function asynchronous(value) { return value + 2; },
                    arrow: value => value + 3,
                    method: ({ method(value) { return value + 4; } }).method,
                })"#,
            )
            .unwrap();
        for name in ["ordinary", "asynchronous", "arrow", "method"] {
            let function: Function = forms.get(name).unwrap();
            let descriptor = transfer.capture(&ctx, function).unwrap();
            assert!(!descriptor.source.contains("spoofed prototype source"));
            assert_eq!(descriptor.module_id, Some("eval_script".to_owned()));
            assert!(descriptor.line.is_some());
            assert!(descriptor.column.is_some());
        }
    });
}

#[test]
fn own_function_properties_do_not_override_captured_metadata() {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    context.with(|ctx| {
        let transfer = FunctionTransfer::new(&ctx).unwrap();
        let function: Function = ctx
            .eval("globalThis.original = function original() { return 1; }")
            .unwrap();
        ctx.eval::<(), _>(
            r#"
                Object.defineProperty(original, "fileName", { value: "owned spoof" });
                original.toString = () => "owned spoof source";
            "#,
        )
        .unwrap();
        let descriptor = transfer.capture(&ctx, function).unwrap();
        assert!(descriptor.source.contains("function original"));
        assert_eq!(descriptor.module_id, Some("eval_script".to_owned()));
        assert_ne!(descriptor.module_id, Some("owned spoof".to_owned()));
    });
}

#[test]
fn captures_defining_module_origin_for_a_function_export() {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    context.with(|ctx| {
        let transfer = FunctionTransfer::new(&ctx).unwrap();
        Module::declare(ctx.clone(), "scripts/defining.js", "export function exported() { return 42; }").unwrap();
        Module::evaluate(ctx.clone(), "scripts/registering.js", "import { exported } from './defining.js'; globalThis.imported = exported;")
            .unwrap().finish::<()>().unwrap();
        let descriptor = transfer.capture(&ctx, ctx.globals().get("imported").unwrap()).unwrap();
        assert_eq!(descriptor.module_id, Some("scripts/defining.js".to_owned()));
        assert!(descriptor.source.contains("function exported"));
    });
}

#[test]
fn two_runtime_transfer_replays_imports_and_keeps_dynamic_imports_fresh() {
    use super::loader::ScriptModules;
    use rquickjs::{loader::Loader, CatchResultExt, Promise};

    let root = std::env::temp_dir().join(format!("rev-idle-transfer-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let dependency = root.join("dependency.ts");
    let barrel = root.join("barrel.ts");
    let entry = root.join("entry.ts");
    std::fs::write(&dependency, r#"
        globalThis.importCount = (globalThis.importCount || 0) + 1;
        const privateValue = 1;
        export enum Kind { Answer = 40 }
        export class Box { value = 1; }
        export function helper() { return privateValue; }
    "#).unwrap();
    std::fs::write(&barrel, "export { Kind, Box, helper } from './dependency.ts';").unwrap();
    let modules = ScriptModules::default();
    modules.insert(&entry.to_string_lossy(), r#"
        import { Kind as K, Box, helper } from './barrel.ts';
        const localOnly = 9;
        export function imported() { return K.Answer + new Box().value + helper(); }
        export function missing() { return localOnly; }
        export function nested() { return () => sameFile(); }
        export enum LocalKind { Answer = 7 }
        export class LocalBox { value = 4; }
        const aliasedValue = 2;
        const { destructured } = { destructured: 3 };
        export { helper as localHelper, aliasedValue as "renamed-value", destructured as renamedDestructured };
        export function sameFile() {
            return aliasedValue + destructured + new LocalBox().value + LocalKind.Answer + helper();
        }
        export async function fresh() { return (await import('./dependency.ts')).helper(); }
        export const method = { run() { return helper(); } }.run;
        export const asynchronous = async () => helper();
    "#.to_owned()).unwrap();
    let main = Runtime::new().unwrap();
    main.set_loader(modules.clone(), modules.clone());
    let main_context = Context::full(&main).unwrap();
    let functions = main_context.with(|ctx| {
        let transfer = FunctionTransfer::new(&ctx).unwrap();
        let (module, ready) = modules.clone().load(&ctx, &entry.to_string_lossy(), None).unwrap().eval().unwrap();
        ready.finish::<()>().unwrap();
        assert_eq!(module.get::<_, Function>("imported").unwrap().call::<_, i32>(()).unwrap(), 42);
        assert_eq!(module.get::<_, Function>("sameFile").unwrap().call::<_, i32>(()).unwrap(), 17);
        assert_eq!(ctx.globals().get::<_, i32>("importCount").unwrap(), 1);
        ["imported", "missing", "nested", "sameFile", "fresh", "method", "asynchronous"].map(|name| {
            (name, transfer.capture(&ctx, module.get(name).unwrap()).unwrap())
        })
    });
    std::fs::write(&dependency, "export function helper() { return 2; }").unwrap();
    std::fs::write(&barrel, "throw new Error('edited barrel must not replace the retained graph');").unwrap();
    std::fs::write(&entry, "throw new Error('edited entry must not replace the retained graph');").unwrap();
    let background = Runtime::new().unwrap();
    background.set_loader(modules.clone(), modules.clone());
    let background_context = Context::full(&background).unwrap();
    background_context.with(|ctx| {
        let transfer = FunctionTransfer::new(&ctx).unwrap();
        for (name, descriptor) in functions {
            let id = modules.transfer_module(uuid::Uuid::new_v4(), &descriptor).unwrap();
            let (module, ready) = modules.clone().load(&ctx, &id, None).unwrap().eval().unwrap();
            ready.finish::<()>().unwrap();
            let function: Function = module.get("default").unwrap();
            match name {
                "imported" => assert_eq!(function.call::<_, i32>(()).unwrap(), 42),
                "missing" => assert!(function.call::<_, i32>(()).catch(&ctx).unwrap_err().to_string().contains("localOnly")),
                "nested" => {
                    let nested = transfer.capture(&ctx, function.call(()).unwrap()).unwrap();
                    let id = modules.transfer_module(uuid::Uuid::new_v4(), &nested).unwrap();
                    let (module, ready) = modules.clone().load(&ctx, &id, None).unwrap().eval().unwrap();
                    ready.finish::<()>().unwrap();
                    assert_eq!(module.get::<_, Function>("default").unwrap().call::<_, i32>(()).unwrap(), 17);
                }
                "sameFile" => assert_eq!(function.call::<_, i32>(()).unwrap(), 17),
                "fresh" => assert_eq!(function.call::<_, Promise>(()).unwrap().finish::<i32>().unwrap(), 2),
                "method" => assert_eq!(function.call::<_, i32>(()).unwrap(), 1),
                "asynchronous" => assert_eq!(function.call::<_, Promise>(()).unwrap().finish::<i32>().unwrap(), 1),
                _ => unreachable!(),
            }
        }
        assert_eq!(ctx.globals().get::<_, i32>("importCount").unwrap(), 1);
        let native = transfer.capture(&ctx, ctx.eval("Math.max").unwrap()).unwrap();
        assert!(modules.transfer_module(uuid::Uuid::new_v4(), &native).is_err());
    });
    std::fs::remove_file(dependency).unwrap();
    std::fs::remove_file(barrel).unwrap();
    std::fs::remove_file(entry).unwrap();
    std::fs::remove_dir(root).unwrap();
}
