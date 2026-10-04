# Native QuickJS `BigNum` feasibility

Date: 2026-10-04

Scope: feasibility research preceding the native refactor. The implementation is tracked in [the plan](../superpowers/plans/2026-10-04-native-bignum.md); current behavior is documented in [the API guide](../js_api_guide.md#bignum).

## Finding

The existing `scripts/lib/utils.ts` `BigNum` can be represented as a Rust-backed QuickJS class with native instances returned from state decoding. `rquickjs` 0.12.2 provides the required class machinery. The state transport must first carry type information, however: the current wire value is JSON and the client currently parses it without distinguishing a BigDouble string from an ordinary string.

## Pinned dependency and native-class surface

`client/Cargo.toml` declares `rquickjs = "0.12.2"` with `full-async`; `client/Cargo.lock` resolves `rquickjs`, `rquickjs-core`, `rquickjs-macro`, and `rquickjs-sys` to 0.12.2. The installed crate source is available under the local Cargo registry.

The 0.12.2 class API supports:

- `#[rquickjs::class]` on a Rust type and `#[rquickjs::methods]` on one implementation block. A method marked `#[qjs(constructor)]` becomes the JavaScript constructor; ordinary methods are placed on the prototype; `#[qjs(static)]` creates constructor methods. The macro documentation also says that `#[class]` supplies the `FromJs`/`IntoJs` pair for a `Class<Self>` instance, and cannot be combined with plain-object `FromJs`/`IntoJs` derives. [rquickjs 0.12.2 macro source](https://docs.rs/rquickjs/0.12.2/rquickjs/attr.class.html); installed source: `C:\Users\hung1\.cargo\registry\src\...\rquickjs-macro-0.12.2\src\lib.rs:164-326`
- `Class::<T>::define(&ctx.globals())` registers a generated constructor, while `Class::instance(ctx, value)` creates a JS object backed by a Rust value. The installed core source shows the opaque Rust value is attached to a QuickJS class object. Source: `...\rquickjs-core-0.12.2\src\class.rs:221-245`; [docs.rs `Class`](https://docs.rs/rquickjs/0.12.2/rquickjs/struct.Class.html)
- Native recognition is typed: `Object::instance_of::<T>()` checks the registered class identity, and `Object::into_class::<T>()`/`Class::from_value` rejects unrelated objects. This is suitable for Rust callbacks that accept a native `BigNum` and for `rev.call` argument validation. Source: `...\rquickjs-core-0.12.2\src\class.rs:362-424`; [docs.rs `Class`](https://docs.rs/rquickjs/0.12.2/rquickjs/struct.Class.html)

The minimal class surface should preserve the current observable operations (`constructor`, `mantissa`, `exponent`, sign/zero predicates, comparisons, `add`/`sub`/`mul`/`div`, and string/integer conversions). The native class should expose only the operations whose semantics are already established. Adding JavaScript operator overloads or a promise-aware API would be a separate design decision.

## Existing `BigNum` contract to preserve

The current implementation stores a `bigint` mantissa and `bigint` exponent (`utils.ts:649-650`). Parsing accepts number/string/bigint/`BigNum`, rejects non-finite numbers, keeps at most 16 significant decimal digits by truncating input (`:655-687`), and normalizes to the same 16-digit representation (`:693-716`). The 16-digit limit is therefore part of the current behavior, not an incidental Rust representation.

Arithmetic is decimal fixed-point arithmetic over those integer fields: comparison is exponent/mantissa based (`:728-737`); addition and subtraction align at the smaller exponent and drop terms beyond `NEGLIGIBLE_THRESHOLD = 15` (`:763-804`); multiplication/division use integer powers of ten and normalize (`:807-820`). `toBigInt()` works from the mantissa and exponent and applies floor semantics for negative fractions (`:848-854`). A native implementation must not route these paths through `f64`, JavaScript `Number`, or game double arithmetic. The public `mantissa` getter is already a lossy `number` projection (`:719-722`), so it should remain explicitly a projection if retained.

Stringification currently produces normalized scientific notation and is the explicit JSON boundary used by `stringify`: the replacer converts `BigNum` to `toString()` (`:827-864`). A native class therefore needs an explicit serialization rule (`toJSON`, a replacer, or a codec) rather than relying on default JSON behavior for a Rust class.

## State and transport consequence

The type information is lost at [the plugin serializer](../../plugin/src/StatePayload.cs#L313): it recognizes the actual `BigDouble` type, then writes only its mantissa/exponent string. [The state handler](../../plugin/src/Plugin.cs#L124) sends that JSON in `StateRes`. This is the point where a typed transport representation can retain the information without guessing from property names or numeric-looking strings.

The current client state path requests a JSON object and serializes it back to a JSON string (`client/src/bridge/state.rs:165-176`). The JS wrapper then performs `parse(await raw(...keys))`, shallow-freezes the result, and returns either the keyed value or the map (`client/src/script/bindings.rs:475-482`). JSON alone cannot tell whether a string such as `1.23e400` is a BigDouble or an ordinary user string. Rust cannot safely infer the type from string content.

To return native instances from `rev.state`, the transport needs an explicit type tag or sidecar metadata, with a recursively defined codec for arrays and objects. The decoder must reconstruct a `BigNum` instance inside each QuickJS runtime; a JS object or handle from the main runtime cannot be shared with the background runtime. The exact envelope/tag spelling remains an API decision. It should be reserved and escaped according to the eventual protocol contract, rather than guessed from scientific-notation text.

The same codec must be used in persistence and cross-runtime paths. The current `stringify` contract should continue to emit the canonical scientific string (or an explicitly versioned wire form) so existing logs/history remain readable. Native-instance detection for future host calls should use the rquickjs class identity (`instance_of`/`into_class`), while strings remain accepted only through an intentional compatibility conversion.

Register the constructor in both runtimes (`client/src/script/session.rs:156-201`, `client/src/script/background.rs:226-265`). `rev.global` currently stores `serde_json::Value` and uses JSON stringify/parse (`client/src/global_state.rs:5-22`, `client/src/script/bindings.rs:588-634`); UI state has similar boundaries (`client/src/script/ui.js:45,135-151`). These need typed reconstruction if they are to preserve native numeric values. Update script declarations and the manual's generated `string // BigDouble` fields to reflect `BigNum` values. The native implementation supplies `BigNum` globally. Remove script imports and the utility implementation; no compatibility re-export is retained.

## Open decisions for a later implementation

1. Choose and document the typed state envelope/sidecar format, including nested values, ordinary strings, `null`, and unsupported special values.
2. Preserve the current constructor inputs and decide how the game's declared `NaN`/`Infinity` BigDouble values map. The current TypeScript constructor rejects non-finite numbers (`utils.ts:662-665`); native conversion must not silently broaden that contract.
3. Define serialization for `JSON.stringify`, `rev.global`, UI state, and any callback/request path before replacing the current string boundary.

These are transport/API decisions; this report does not select them.
