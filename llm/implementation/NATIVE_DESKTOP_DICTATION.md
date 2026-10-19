# Native Desktop Dictation

## Status Snapshot

- Date: 2026-10-18
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

- frontend presentation beyond native capture ownership and cancellation ordering

## Source Anchors

- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/` domain modules (`audio`, `commands`, `hotkey_overlay`, `insert`, `models`, `state`, `transcribe`, `whisper_cli`)
- `src-tauri/src/dictation_session.rs`
- `public/js/native-dictation.js`
- `public/js/events.js`
- `src-tauri/Cargo.toml`

## Contract

Pipeline sequence:

1. `start_native_dictation` reserves a session before dispatching its worker, then validates model + CLI readiness and active state. The UI requires an authorized microphone before starting; native capture still validates actual access and device opening.
2. backend spawns recording thread and opens microphone stream.
3. `stop_native_dictation` stops capture and joins thread.
4. captured samples are resampled to 16 kHz mono if required.
5. temp WAV is written.
6. `whisper-cli` runs with `-m`, `-f`, `-l en`, `-otxt`, `-nt`, `-of`.
7. transcript txt output is read.
8. artifact tokens are removed.
9. cleaned transcript is returned.

Stop and Clear invalidate pending startup as well as established capture. Button, focused Fn, backend hold/toggle, and tray actions share the reservation boundary. Cancellation never waits for microphone authorization or the startup gate on the main thread. Device opening occurs outside the short intent lock; a canceled device-open result is stopped and joined instead of being published. Publication, pending cancellation, and the listening event are ordered by the intent lock. Background UI synchronization is dispatched to the main thread without waiting under that lock.

The optional frontend `request_id` correlates listening, cancellation idle events, and command completion with a specific start action. Stop/Clear invalidate that request locally. If a canceled start reaches the backend only after the initial cancellation found no reservation, its successful completion triggers a second cancellation scoped to that request. This cleanup releases its capture while preserving any unrelated frontend request or native Fn capture. A delayed start may briefly open the device before its completion is acknowledged and cleanup runs; local invalidation does not forcibly interrupt an IPC delivery or device initializer.

Subsequent frontend starts wait for the preceding start and its scoped cleanup. Clear also adds its unscoped cancellation acknowledgement to that ordering barrier, so a delayed Clear cannot cancel the next local recording. Late completions and events cannot revive a canceled UI session or replace a newer one. Session IDs still govern transcript/event ordering. A rejected duplicate focused/global Fn start joins the existing capture and does not run stale-success cleanup.

Capture details:

- input sample formats handled: `f32`, `i16`, `u16`
- channel input is downmixed to mono
- startup timeout for stream init: 5 seconds
- audio I/O uses path-patched `cpal` (`vendor/cpal`) — macOS CoreAudio retain-cycle / idle-mic leak fix; see [`../../vendor/cpal/VENDOR.md`](../../vendor/cpal/VENDOR.md)

Sidecar binaries (bundled `whisper-cli-*` real vs placeholder): [`../../src-tauri/binaries/README.md`](../../src-tauri/binaries/README.md)

Normalization details:

- strips token markers: `BLANK_AUDIO`, `NOISE`, `MUSIC`, `SILENCE`
- if cleaned text is empty, returns no-speech error

Concurrency invariants:

- only one active recording at a time
- second start while active returns `Dictation already running.`
- cancel path is safe when idle
- permission observation/recheck preserves active or starting capture; listener re-arming is deferred until idle
- audio capture/transcription stays local to Rust/cpal + Whisper; model download is a separate network operation

## Verification

Run after native pipeline changes:

1. `bun run test:rust`
2. manual mac desktop dictation smoke (`start -> speak -> stop`)
3. `bun run whisper:smoke` for sidecar pipeline signal

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`MODEL_MANAGEMENT.md`](MODEL_MANAGEMENT.md)
- [`CONFIG_AND_PATH_RESOLUTION.md`](CONFIG_AND_PATH_RESOLUTION.md)
