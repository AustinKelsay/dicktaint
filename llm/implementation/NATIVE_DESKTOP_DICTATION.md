# Native Desktop Dictation

## Status Snapshot

- Date: 2026-02-17
- Desktop dictation backend is implemented in Rust and wired through Tauri commands

## Purpose

Document exact native dictation pipeline behavior from mic capture to cleaned transcript response.

## Scope

In scope:

- recording lifecycle
- audio preparation
- whisper-cli invocation behavior
- normalization and error handling shape

Out of scope:

- frontend UI branch logic

## Source Anchors

- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/Cargo.toml`

## Contract

Pipeline sequence:

1. `start_native_dictation` validates model + CLI readiness and active state.
2. backend spawns recording thread and opens microphone stream.
3. `stop_native_dictation` stops capture and joins thread.
4. captured samples are resampled to 16 kHz mono if required.
5. temp WAV is written.
6. `whisper-cli` runs with `-m`, `-f`, `-l en`, `-otxt`, `-nt`, `-of`.
7. transcript txt output is read.
8. artifact tokens are removed.
9. cleaned transcript is returned.

Capture details:

- input sample formats handled: `f32`, `i16`, `u16`
- channel input is downmixed to mono
- startup timeout for stream init: 5 seconds

Normalization details:

- strips token markers: `BLANK_AUDIO`, `NOISE`, `MUSIC`, `SILENCE`
- if cleaned text is empty, returns no-speech error

Concurrency invariants:

- only one active recording at a time
- second start while active returns `Dictation already running.`
- cancel path is safe when idle

## Verification

Run after native pipeline changes:

1. `bun run test:rust`
2. manual mac desktop dictation smoke (`start -> speak -> stop`)
3. `bun run whisper:smoke` for sidecar pipeline signal

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`MODEL_MANAGEMENT.md`](MODEL_MANAGEMENT.md)
- [`CONFIG_AND_PATH_RESOLUTION.md`](CONFIG_AND_PATH_RESOLUTION.md)
