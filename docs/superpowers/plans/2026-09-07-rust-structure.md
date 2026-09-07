# Rust structure implementation plan

**Goal:** Give Rust responsibilities focused modules and narrow interfaces while preserving existing behavior.

**Architecture:** Keep the existing single-thread Tokio/QuickJS execution and worker lifecycle. Separate application coordination, console editing, script execution and bindings, plugin transport, and Windows operations within the existing crate.

**Tech stack:** Rust, Tokio, rquickjs, reqwest, windows. No dependency changes.

**Design:** User approved the responsibility-based structure proposed in chat and delegated implementation choices.

## Constraints

- Preserve commands, JavaScript API, history/completion, pause ordering, cancellation, global-state lifetime, and QuickJS persistent-root destruction order.
- Follow existing style; never run formatters or mutate Git state.
- Use substantive module interfaces; avoid trivial wrappers, speculative traits, or broad visibility.
- Keep platform workers and lifecycle algorithms intact while relocating their responsibilities.
- User reaffirmed the boundary during implementation: finish the existing responsibility split only; no new features, dependencies, frameworks, or lifecycle redesign.

## Tasks

- [x] Establish baseline Cargo checks and existing test results.
- [x] Move application coordination and shared commands/pause controls into `client/src/app/`; keep `main.rs` as entrypoint.
- [x] Split `client/src/console.rs` into console input, editing, history, completion, and parsing modules. Move relevant tests with their owner.
- [x] Split `client/src/script.rs` into lifecycle, session, module loading, and host bindings. Preserve runtime ownership and existing behavior tests.
- [x] Consolidate plugin HTTP operations in `client/src/bridge/`, keeping QuickJS value conversion in script bindings.
- [x] Split `client/src/window.rs` into discovery/geometry, clipboard, and bridge input responsibilities. Preserve message encoding and resource cleanup.
- [x] Review module boundaries and behavior preservation; resolve concrete findings.
- [x] Run `cargo check --all-targets`, existing tests, and `git diff --check`; record results.

## Validation

Run Cargo commands from `client`. Existing tests cover command parsing, editing/history/completion, JavaScript state and globals, module loading, pause/resume/cancellation, Windows message packing, and worker cleanup. Check availability of port 19841 before running tests that bind it; do not disturb a running game. Review moved code against HEAD to detect accidental behavioral changes. Record any environment-limited checks explicitly.

Baseline: `cargo check --all-targets` passed. `cargo test --all-targets` ran 91 tests: 84 passed, 7 failed. The first state test failed binding port 19841 with Windows error 10048 (`AddrInUse`); six further state tests failed on the poisoned shared lock. Preserve this transport behavior during the refactor and report the environment limitation.

Final verification: source review found no behavioral regressions. All 91 original client test names are retained. cargo check --all-targets passed; the test run excluding the seven baseline port-dependent failures passed 84 client tests and 4 probe tests. Tracked and new files passed whitespace checks. No formatting tools or Git mutations were used.
