# Frontend Runtime Behavior

## Status Snapshot

- Date: 2026-02-17
- Frontend runtime split is implemented in `public/app.js`

## Purpose

Define frontend runtime branching and dictation state transitions.

## Scope

In scope:

- runtime detection logic
- onboarding gating logic
- native and browser dictation paths
- status and overlay synchronization

Out of scope:

- backend transcription internals

## Source Anchors

- `/Users/plebdev/Desktop/code/dicktaint/public/app.js`
- `/Users/plebdev/Desktop/code/dicktaint/public/index.html`
- `/Users/plebdev/Desktop/code/dicktaint/public/pill.js`

## Contract

Runtime routing:

- `isFocusedMacDesktopMode()` -> native desktop dictation command path
- web path -> browser speech recognition when supported
- non-mac native desktop -> unsupported desktop messaging path

Setup gate on mac desktop:

- `nativeDictationModelReady` depends on onboarding result for selected model existence + `whisper-cli` availability
- start dictation controls remain disabled until setup ready

Native desktop start/stop contract:

- start calls `start_native_dictation`
- stop calls `stop_native_dictation`
- clear calls `cancel_native_dictation` best-effort
- capture start timestamp is tracked for UX messaging
- transcribe status includes capture duration when available
- low-information transcript heuristic shows explicit quality guidance to user

Browser speech path:

- uses continuous recognition with interim results
- auto-restart timer keeps capture flow between utterance boundaries
- fatal speech errors stop restart loop

Status to overlay mapping:

- `setStatus()` calls overlay sync and emits `dicktaint://pill-status`

Invariants:

- runtime mode is authoritative for command path selection
- UI controls reflect lock/busy/setup states through `syncControls()`

## Verification

Re-verify after `public/app.js` changes:

1. mac desktop: onboarding gate, start/stop flow, status updates
2. web mode: speech path and manual input fallback
3. overlay event emission for status changes

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`HOTKEY_AND_OVERLAY.md`](HOTKEY_AND_OVERLAY.md)
- [`../context/RUNTIME_MODES.md`](../context/RUNTIME_MODES.md)
