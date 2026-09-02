# Complete Game State Design

## Goal

Expose every readable gameplay-state value reachable from `GameController.data` through the existing on-demand HTTP and `await rev.state(...)` API, without another manually curated value allowlist.

## Public API

- `await rev.state()` returns the complete canonical nested `GameData` graph.
- `await rev.state("score", "eternity.dtpSpent")` returns a flat object with exactly those requested property names.
- Paths are case-sensitive and use public property names separated by `.`.
- Numeric path segments index arrays and lists. Dictionary segments resolve string, integer, or enum keys.
- The existing 37 aliases remain valid and retain their existing output names.
- `DT` aliases `eternity.dilationTree`.
- `DTP` aliases `eternity.dtpMax`; the unambiguous raw values `dtpBought`, `dtpSpent`, `dtpMax`, `dtpFree`, `nextDtpCost`, and `dilationTree.TotalDTP` remain independently selectable.
- Duplicate requested paths appear once, preserving their first position.
- Unknown paths return HTTP 400. Missing `GameController.data` returns 503. Unexpected getter or serialization failures return 500 and are logged.

Example:

```javascript
const state = await rev.state(
  "DT",
  "DTP",
  "eternity.dtpSpent",
  "eternity.dilationTree.top.0.level",
);
```

## Serialization

`StatePayload` is the single state-policy boundary. It resolves selected paths and writes JSON directly with `Utf8JsonWriter` while running inside `ScoreTicker.Update()` on Unity's main thread.

Values are encoded as follows:

| Game value | JSON representation |
| --- | --- |
| `BigDouble` | Existing invariant mantissa/exponent string |
| String, boolean, null | Native JSON type |
| Safe integer | JSON number |
| Integer outside JavaScript's safe range | Decimal string |
| Finite float, double, decimal | JSON number |
| Non-finite float or double | `"NaN"`, `"Infinity"`, or `"-Infinity"` |
| Enum | Symbolic string |
| ACTk obscured scalar | Decrypted native JSON value |
| Date/time | Round-trip string |
| Array or list | JSON array |
| Dictionary with string, integer, or enum keys | JSON object |
| Gameplay object | JSON object of eligible declared properties |

The serializer caches reflection descriptors by CLR type. IL2CPP lists and arrays use reflected `Count` or `Length` plus indexed `Item`; dictionaries use their reflected enumerator and `Key`/`Value` pair.

## Completeness Boundary

The canonical graph includes every readable public instance property declared by reachable gameplay types. It excludes only:

- generated IL2CPP plumbing such as `Pointer`, `ObjectClass`, `WasCollected`, backing members, and `prop_*` members;
- Unity objects, events, and delegates;
- ownership/back-reference edges such as `Data`, `Controller`, `Parent`, and adjacency properties that point back into an already represented graph;
- repeated native objects already seen during the same serialization.

Cycles are identified by `Il2CppObjectBase.Pointer`, not managed reference identity. Repeated object properties are omitted; repeated collection elements become `null` so indexes remain stable. A getter failure fails the response rather than silently omitting gameplay state.

This definition deliberately excludes runtime implementation objects, not game progress or configuration.

## HTTP and Client Flow

`HttpScoreServer` continues to bind only `127.0.0.1`, use HTTP/1.1 keep-alive, and queue work for the Unity thread. It no longer validates paths against a static allowlist. It transports zero or more decoded `key` query parameters; `StatePayload` performs authoritative validation.

The Rust process continues to own one `reqwest::Client`. It validates that the response is a JSON object and returns the original JSON text. QuickJS parses that text after the await and shallow-freezes the fresh result object.

## Reference Manual

`plugin/STATE_KEYS.md` is generated from the installed `Assembly-CSharp.dll`. It lists every reachable gameplay type and its selectable public properties, plus collection element/value types and the compatibility aliases. The README documents the path grammar and links to this exhaustive reference.

## Validation

- Pure C# fixtures cover mixed scalar types, nested objects, arrays, lists, dictionaries, aliases, indexes, invalid paths, cycles, and failure propagation.
- Existing socket tests cover no-argument transport, selected paths, status mapping, and keep-alive.
- Rust tests cover mixed nested JSON reaching QuickJS, arrays, exact selected keys, promise rejection, and a frozen top-level result.
- Plugin tests and Release build use the installed game assemblies at `E:\SteamLibrary\steamapps\common\Revolution Idle`.
- Cargo tests and `git diff --check` must pass without running formatting tools.
