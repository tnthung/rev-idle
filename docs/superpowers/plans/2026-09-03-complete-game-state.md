# Complete Game State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 37-value allowlist with complete, path-selectable, mixed-type serialization of all gameplay state reachable from `GameController.data`.

**Architecture:** The plugin resolves and serializes state on Unity's main thread. A reflection-backed serializer traverses eligible gameplay properties and IL2CPP collections, while selected requests resolve only their requested paths. HTTP remains a loopback transport and the Rust client keeps one reusable connection pool while preserving the plugin's JSON value types.

**Tech Stack:** C#/.NET 6, BepInEx IL2CPP, `Utf8JsonWriter`, Rust 2024, reqwest 0.13.4, serde_json, rquickjs.

**Spec:** `docs/superpowers/specs/2026-09-03-complete-game-state-design.md`

## Constraints

- Include every public declared gameplay property except the exact exclusions in the spec.
- Read game objects only from Unity's main thread.
- Preserve the existing aliases and add `DT` and `DTP`.
- Keep the server loopback-only and reuse the existing Rust HTTP client.
- Do not modify or stage `scripts/test1.js`.
- Do not run formatting tools.

### Task 1: Complete serializer and path resolver

**Files:**
- Modify: `plugin/src/StatePayload.cs`
- Modify: `plugin/src/RevolutionIdle.ScoreTelemetry.csproj`
- Modify: `plugin/tests/Program.cs`
- Modify: `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`

- [ ] Add failing tests for mixed scalar types, nested objects, lists, arrays, dictionaries, large integers, non-finite numbers, getter failures, and cycles.
- [ ] Add failing tests for compatibility aliases, `DT`, `DTP`, dotted paths, numeric collection indexes, dictionary keys, and invalid paths.
- [ ] Implement a status result that distinguishes success, invalid path, and serialization failure.
- [ ] Replace the switch allowlist with cached public-property descriptors and direct `Utf8JsonWriter` traversal.
- [ ] Implement the documented scalar conversions, ACTk decryption, IL2CPP collection access, deterministic property order, and pointer-based cycle handling.
- [ ] Resolve selected paths before serializing their values; serialize the full canonical graph only for an empty path list.
- [ ] Run the plugin tests with the installed game directory.
- [ ] Commit as `[Chg] expose complete game state graph`.

### Task 2: HTTP path and status integration

**Files:**
- Modify: `plugin/src/HttpScoreServer.cs`
- Modify: `plugin/src/Plugin.cs`
- Modify: `plugin/tests/Program.cs`

- [ ] Add failing tests showing that no arguments queue an empty path list, arbitrary decoded paths are accepted, invalid paths return 400, unavailable data returns 503, failures return 500, and keep-alive still works.
- [ ] Remove the HTTP allowlist and pass decoded `key` values to the main-thread serializer unchanged.
- [ ] Map serializer statuses to 200, 400, and 500; retain 503 for unavailable `GameController.data` and log unexpected failures.
- [ ] Run the plugin tests and Release build with the installed game directory.
- [ ] Commit as `[Chg] accept complete state paths`.

### Task 3: Preserve mixed JSON in Rust and QuickJS

**Files:**
- Modify: `client/src/telemetry.rs`
- Modify: `client/src/script.rs`

- [ ] Add failing Rust tests for nested objects, arrays, mixed scalar types, exact selected keys, rejected HTTP responses, and a frozen top-level JavaScript result.
- [ ] Change the telemetry boundary to validate an object with `serde_json::Value` and return the original JSON text.
- [ ] Parse that JSON after awaiting the reusable native HTTP client in the `rev.state` wrapper.
- [ ] Run all Cargo tests.
- [ ] Commit as `[Chg] preserve complete state JSON values`.

### Task 4: Generate the exhaustive state reference

**Files:**
- Create: `plugin/generate-state-reference.ps1`
- Create: `plugin/STATE_KEYS.md`
- Modify: `plugin/README.md`

- [ ] Implement a deterministic Mono.Cecil-based generator that starts at `GameData`, recursively lists reachable declared public properties and collection element/value types, applies the serializer exclusions, and lists every alias.
- [ ] Generate `plugin/STATE_KEYS.md` from the installed `Assembly-CSharp.dll` twice and confirm the second run has no diff.
- [ ] Replace the README's curated key table with the path grammar, mixed-type behavior, examples, and a link to the generated reference.
- [ ] Commit as `[Chg] document complete state paths`.

### Task 5: Final verification

- [ ] Run the plugin test executable with `GameDir=E:\SteamLibrary\steamapps\common\Revolution Idle`.
- [ ] Run the plugin Release build with the same `GameDir`.
- [ ] Run all Cargo tests.
- [ ] Run `git diff --check` across the implementation commits.
- [ ] Confirm `scripts/test1.js` is the only remaining working-tree change.
- [ ] Audit the generated reference against the serializer's inclusion and exclusion rules.
