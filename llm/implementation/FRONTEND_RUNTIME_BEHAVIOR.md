# Frontend Runtime Behavior

## Status Snapshot

- Date: 2026-07-30
- Frontend runtime split: `public/app.js` ESM entry loads domain modules under `public/js/`
- Overlay runtime is separate: `public/pill.html` loads `public/pill.js`

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

- `public/app.js` (ESM entry)
- `public/js/platform.js`
- `public/js/events.js`
- `public/js/ui.js`
- `public/js/native-dictation.js`
- `public/js/onboarding/index.js`
- `public/js/constants.js`
- `public/index.html`
- `public/pill.html`
- `public/pill.js`

## Contract

Runtime routing:

- `isFocusedMacDesktopMode()` (`public/js/platform.js`) -> native desktop dictation command path
- web path -> browser speech recognition when supported
- non-mac native desktop -> unsupported desktop messaging path

Setup gate on mac desktop:

- `nativeDictationModelReady` depends on onboarding result for selected model existence + `whisper-cli` availability
- start dictation controls remain disabled until setup ready
- onboarding payload also drives `focusedFieldInsertEnabled` for optional focused-field paste behavior

Native desktop start/stop contract:

- start calls `start_native_dictation`
- stop calls `stop_native_dictation`
- clear calls `cancel_native_dictation` best-effort
- focused-field toggle writes through `set_focused_field_insert_enabled`
- finalized transcript path attempts `insert_text_into_focused_field` only when enabled and when app window is not focused

Browser speech path:

- uses continuous recognition with interim results
- auto-restart timer keeps capture flow between utterance boundaries
- fatal speech errors stop restart loop

Status to overlay mapping:

- `setStatus()` in `public/js/ui.js` calls overlay sync and emits `dicktaint://pill-status`
- overlay listeners and audio-level waveform live in `public/pill.js`

Invariants:

- runtime mode is authoritative for command path selection
- UI controls reflect lock/busy/setup states through `syncControls()` in `public/js/ui.js`

## Verification

Re-verify after `public/js/` or `public/pill.js` changes:

1. mac desktop: onboarding gate, start/stop flow, status updates
2. web mode: speech path and manual input fallback
3. overlay event emission for status changes
4. overlay waveform updates while listening (`dictation:audio-level`)

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`HOTKEY_AND_OVERLAY.md`](HOTKEY_AND_OVERLAY.md)
- [`../context/RUNTIME_MODES.md`](../context/RUNTIME_MODES.md)
