# User Flows

## Status Snapshot

- Date: 2026-10-18
- Flow descriptions match current app behavior in web and desktop paths

## Purpose

Document end-user flow sequences that the implementation currently supports.

## Scope

In scope:

- first-run setup
- button dictation flow
- hold-to-talk flow
- model management flow
- web fallback flow

Out of scope:

- not-yet-built UX variations

## Source Anchors

- `public/index.html`
- `public/app.js` (ESM entry)
- `public/js/events.js`
- `public/js/native-dictation.js`
- `public/js/onboarding/index.js`
- `public/js/ui.js`
- `public/pill.html`
- `public/pill.js`
- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/commands.rs`
- `src-tauri/src/hotkey_overlay/mod.rs`
- `src-tauri/src/state.rs`

## Current State

Desktop first-run (macOS):

1. app quietly checks local speech engine, selected model, hardware profile, and microphone facts
2. a ready returning user opens Dictation; otherwise the first load opens guided setup
3. user installs/selects a model and explicitly allows microphone access, or opens Microphone settings for denied access
4. engine + model + authorized microphone unlock native dictation; optional Accessibility/Input Monitoring never gate it
5. user can open the editable draft while completing setup; later permission refresh does not force navigation or discard the draft

Desktop dictation (button):

1. click start
2. speak
3. click stop
4. transcript returns and appends
5. if focused-field insertion is enabled and another app is focused, transcript is also pasted into that field

Desktop dictation (hold-to-talk):

1. hold `fn`
2. app starts capture
3. release `fn`
4. app stops/transcribes
5. transcript appends
6. if focused-field insertion is enabled and another app is focused, transcript is also pasted into that field

Race control detail:

- if key release occurs during startup, frontend defers stop to avoid lifecycle drop.

Desktop model maintenance:

- user can download/use an installed model or delete one with confirmation in Settings. The recommended model is highlighted; active model deletion selects a valid installed fallback when available.

Desktop settings maintenance:

- user can enable or disable focused-field insertion in Settings; the setting persists per device.
- permission rows offer explicit request/open/recheck actions. Returning to the app rechecks facts and idle hotkey capability without interrupting capture.
- Dictation and Settings share an editable draft. Copy draft and recent-item actions use the clipboard; recent history is memory-only and can be cleared.

Web flow:

- browser speech recognition path when available, manual text entry fallback always available. Browser-provider audio processing may be remote; the desktop local-transcription promise does not extend to browser SpeechRecognition.

## Verification

Re-verify when changing:

1. onboarding and dictation handlers in `public/js/events.js`, `public/js/onboarding/index.js`, and `public/js/native-dictation.js`
2. command implementations in `src-tauri/src/commands.rs`
3. setup UI structure in `public/index.html`
4. overlay status / waveform behavior in `public/pill.js`

## Related Docs

- [`../implementation/FRONTEND_RUNTIME_BEHAVIOR.md`](../implementation/FRONTEND_RUNTIME_BEHAVIOR.md)
- [`../implementation/API_SURFACE.md`](../implementation/API_SURFACE.md)
- [`../workflow/SMOKE_TESTS.md`](../workflow/SMOKE_TESTS.md)
