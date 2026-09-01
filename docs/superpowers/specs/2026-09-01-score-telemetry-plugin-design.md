# Score Telemetry Plugin Design

## Goal

Create a proof-of-concept Revolution Idle plugin that publishes the current Score to a local UDP listener. The repository root is reserved for a future client, so all plugin source lives under `plugin/`.

## Runtime

The plugin targets BepInEx 6 for the Windows x64 IL2CPP build of Revolution Idle. It runs on Unity's main thread and reads `GameController.data.score` without changing game state.

The installed game uses Unity 2022.3.62f3 and IL2CPP metadata v31. Its BepInEx setup therefore needs the compatible patched Cpp2IL toolchain before it can generate interop assemblies.

## Behavior

- BepInEx configuration key: `[Network] Port`.
- Default port: `19841`.
- Port `0` disables publishing.
- Any other value outside `1..65535` is treated as disabled.
- Destination: `127.0.0.1:<Port>` only.
- Frequency: one datagram every 50 ms using unscaled Unity time.
- Payload: UTF-8 JSON containing only `score`, for example `{"score":"1.2345678901234567e123"}`.
- Score uses invariant round-trip mantissa formatting plus its decimal exponent so it remains machine-readable without converting through `double`.
- Missing game state and all read, serialization, and UDP errors are silently ignored.

## Structure

- `plugin/src/Plugin.cs`: BepInEx lifecycle, configuration, timer, and access to live game state.
- `plugin/src/ScorePayload.cs`: deterministic score-string and JSON encoding.
- `plugin/src/UdpScorePublisher.cs`: one-way localhost UDP sending with silent failures.
- `plugin/tests/`: focused tests for payload encoding, port handling, and UDP delivery.
- `plugin/README.md`: build, install, configure, receive, and uninstall instructions.

There is no UI, receiver, retry queue, acknowledgement, logging for routine failures, state mutation, scripting runtime, or mouse automation.

## Verification

Development follows a red-green test cycle for the framework-independent payload and networking code. Completion requires a clean test run, a Release build against generated IL2CPP interop assemblies, plugin load confirmation in BepInEx, and a localhost UDP packet whose JSON score matches the live game state representation.

## Installation Safety

Installation adds BepInEx files beside the game executable and the plugin DLL under `BepInEx/plugins/`. It does not delete or edit Revolution Idle saves. Uninstallation consists of removing the installed mod-loader files and plugin; Steam's game files remain otherwise untouched.
