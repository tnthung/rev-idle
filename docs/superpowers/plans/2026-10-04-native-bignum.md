# Native BigNum Implementation Plan

> **For agentic workers:** Use disjoint file ownership and focused checks. Stage only this refactor when complete; do not commit.

**Goal:** Return host-provided BigNum instances for game BigDouble values, replace the script implementation, and generate matching types.

**Architecture:** Port the existing decimal arithmetic to a Rust-backed QuickJS class. Preserve explicit numeric type metadata through state transport and reconstruct instances in each runtime. Script state models and the manual use the same native type.

**Tech Stack:** Rust, rquickjs 0.12.2, num, C#, TypeScript, PowerShell/Cecil.

**Spec:** User's six requested steps and `docs/research/2026-10-04-native-bignum.md`.

## Constraints

- No formatters or unrelated changes. Preserve established coding style and numeric behavior.
- Keep 16 significant decimal digits, truncation toward zero, arbitrary integer exponent, and existing arithmetic/conversion methods.
- Distinguish ordinary strings and objects from native numeric values using explicit transport metadata.
- Keep the method-manual work stashed, including its research, preview, and plan.
- Stage only this refactor in both repositories; no commits. Preserve unrelated `scripts/setup2.ts`, `scripts/unity_loop.ts`, and `scripts/cspell.json` changes.
- Register the native class before module evaluation in both runtimes. Preserve values across host-owned global/UI state boundaries.

## Tasks

- [x] Stash method-manual work with its documents. Stash commit: `95feabc57ceb791723647fa65efa3770b59e03b2`.
- [x] Native class: `client/src/script/bignum.rs`, native arithmetic tests, registration in main/background runtimes.
- [x] Typed transport: plugin serialization and bridge response metadata, client decoding, global/UI value codecs, collision and nested-value tests.
- [x] Script migration: remove the `scripts/lib/utils.ts` implementation and every BigNum import; use the host global; update States Data types and remove redundant state-value conversions while preserving accessors.
- [x] Manual/declarations: expose native BigNum in `protocol/rev.d.ts`, Copy struct, generated artifacts, and the API guide.
- [x] Validate numeric parity, state/slot decoding, nested values, cross-runtime state, declaration types, plugin tests, and manual output. Review the combined change.
- [x] Stage the exact related paths and verify staged diffs in both repositories, without committing.

## Review focus

- Decimal input and integer conversion preserve `1.13e2 == 113`, negative floor, cancellation, and exponent values outside i64.
- Ordinary numeric-looking strings and marker-shaped objects stay ordinary values; nested BigNum leaves remain native instances.
- Host-owned serialization preserves numeric identity across main/background runtimes and UI callbacks without retaining JS handles in shared Rust state.
- Single-key state unwrapping, multi-key shallow freezing, aliases, arrays, and slot nulls retain their contracts.
- Scripts use the host global directly; no utility re-export or JS numeric implementation remains.

## Execution notes

The user already authorized implementation, stashing, and staging. No additional approval checkpoint or worktree/commit workflow is needed. One worker implemented the native numeric class and another migrated script types and UI value storage; the parent owns transport and integration. A bounded architecture consultation settles the transport representation before its implementation.

## Validation

- Rust: 301 tests passed, run serially for the existing process-wide global store tests.
- Plugin: 136 tests passed on net8.0.
- Manual: 8 viewer tests and 53 generator checks passed.
- Focused TypeScript check passed; the full scripts check retains the existing `lib/action.ts:97` empty-array inference error.
- All BigNum imports and utility re-exports removed.
- No game installation or live-game execution performed.
- Final read-only review found no remaining actionable behavioral defects.
