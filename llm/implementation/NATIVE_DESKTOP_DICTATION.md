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
4. captured samples are moved out of recording state without clone.
5. audio is sanitized before inference:
   - resample to 16 kHz mono if required
   - dominant-channel capture preference for multi-channel input frames
   - DC offset removal
   - edge silence trimming with small speech padding
   - gain normalization for very quiet/very loud input
6. preflight speech guards run (minimum duration, RMS, peak checks).
7. temp WAV is written.
8. `whisper-cli` fast pass runs with `-m`, `-f`, `-l en`, `-t`, `-bs`, `-bo`, `-otxt`, `-nt`, `-np`, `-of`.
9. transcript txt output is read and cleaned.
10. if transcript quality heuristics are low-confidence, an accurate decode retry runs and better result wins.
11. cleaned transcript is returned.

Capture details:

- input sample formats handled: `f32`, `i16`, `u16`
- channel input is collapsed to mono with dominant channel preference
- startup timeout for stream init: 5 seconds

Normalization details:

- strips token markers: `BLANK_AUDIO`, `NOISE`, `MUSIC`, `SILENCE`
- if cleaned text is empty, returns no-speech error
- very short or very low-energy captures return no-speech/too-short guard errors

Decode strategy:

- thread count is auto-derived from available cores and clamped to practical bounds
- first pass favors speed (`beam=2`, `best-of=2`)
- retry pass favors accuracy (`beam=5`, `best-of=5`) on low-information outputs

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
