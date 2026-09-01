# Task 5 implementation report

## Changed files

- `client/src/main.rs`
- `client/src/script.rs`

## TDD evidence

- RED: `cargo check --manifest-path client/Cargo.toml` failed at the `script::run` call because the required fourth `Arc<AtomicBool>` argument was missing.
- GREEN: after wiring the shared gate and hotkey lifecycle, `cargo check --manifest-path client/Cargo.toml` exited successfully.

## Validation

- `cargo test --manifest-path client/Cargo.toml --target-dir C:\Users\user\AppData\Local\Temp\rev-idle-task5-target`: 34 passed, 0 failed.
- `cargo check --manifest-path client/Cargo.toml`: passed.
- `git diff --check`: passed; Git emitted only existing line-ending normalization warnings.

## Notes

The default worktree Cargo target directory returned Access Denied for the build lock during the test run, so tests used the temporary target directory shown above. No source formatter was run.

## Fix Round 1

- Changed `hotkey` back to a private module.
- Removed early `?` returns from `tokio::select!` task arms so all selected-task outcomes flow through unconditional task abort and hotkey shutdown.
- Added pure `finish_shutdown` tests covering cleanup-error propagation and selected-task error preservation.
- RED: focused test failed to compile because `finish_shutdown` was not yet defined.
- GREEN: focused test passed after implementing the seam.
- Full `cargo test` validation: 36 passed, 0 failed.
- `cargo check`: passed.
- `git diff --check`: passed.
