# Background Runtime + `fn` Hotkey MVP

Status snapshot: implemented as of 2026-02-13.
Platform focus for this snapshot: macOS desktop.

This document describes the current desktop MVP behavior for:
- keeping the app process alive after closing the window
- hold-to-talk dictation with the keyboard `fn` key (macOS)

Companion runtime contract reference:
- [`docs/api-surface.md`](api-surface.md)

## Goals

- Launch the app normally with a visible window.
- Let users close (`X`) the window without quitting the process.
- Allow quick dictation hold-to-talk while the window is hidden.

## Current Behavior

### Startup and window lifecycle

- App launches visible by default.
- Closing the main window does **not** quit the app; it hides the window.
- On macOS app reopen events, the main window is shown/focused again.

Config override:
- `DICKTAINT_START_HIDDEN=1` starts the app hidden.
- `DICKTAINT_START_HIDDEN=0` (or unset) starts visible.

Implementation:
- `src-tauri/src/main.rs`:
  - `should_start_hidden`
  - `.on_window_event(...)` close-request hide behavior
  - `RunEvent::Reopen` handling on macOS

### `fn` key hold-to-talk (macOS)

- A native macOS event monitor listens for `flagsChanged` events.
- Backend emits `dicktaint://fn-state` with `pressed=true|false` as function-key modifier state changes.
- Frontend listens for that event:
  - `pressed=true` -> starts native dictation capture
  - `pressed=false` -> stops capture and transcribes

Fallback while window is focused:
- frontend also listens for `keydown`/`keyup` `Fn` / `F19`.

Event payloads:

- backend event (`dicktaint://fn-state`):
  - `{ "pressed": true | false }`
- overlay status event (`dicktaint://pill-status`):
  - `{ "message": string, "state": "idle|working|live|ok|error", "visible": boolean }`

Hotkey feedback UI:
- app renders a dedicated native overlay window per detected monitor (up to 6 monitors)
- each overlay is transparent with rounded edges, always-on-top, visible across macOS workspaces, and click-through
- pill states: idle, working, live, ok, error
- examples: `Hold fn to dictate`, `Listening - release fn to stop`, `Transcribing...`

Implementation:
- `src-tauri/src/main.rs`:
  - `register_fn_global_hotkey_monitor`
  - `create_pill_overlay_windows`
  - `dicktaint://fn-state` event emission
- `public/app.js`:
  - `applyNativeFnHoldState`
  - `handleNativeFnStateEvent`
  - Tauri event listener for `dicktaint://fn-state`
  - overlay updates via `dicktaint://pill-status`
- `src-tauri/tauri.conf.json` + `src-tauri/Cargo.toml`:
  - macOS private API enabled so transparent rounded overlay edges render correctly

## Onboarding Interaction

- Hotkey wiring is active regardless of onboarding state.
- If onboarding is incomplete (no model or no whisper-cli), hotkey start attempts fail with the same existing validation errors shown in UI.
- After onboarding is complete, closing the window and using `fn` should continue to work without reopening the UI.

## Hold-State Sequence (Current)

1. `fn` down
2. frontend marks hold active and triggers `start_native_dictation`
3. if key is released before mic startup completes, frontend queues a deferred stop (`nativeFnStopRequested`)
4. once start settles, queued stop is applied
5. `fn` up while dictating triggers `stop_native_dictation`
6. frontend appends returned transcript and updates status

This sequence avoids dropping quick tap/hold interactions when microphone startup is slower than key transitions.

## Permissions and OS Notes

macOS global `fn` capture may require:
- Input Monitoring permission
- Accessibility permission

In `tauri:dev`, permissions typically apply to Terminal (or your shell host).

If permission is missing:
- app logs a warning
- focused-window fallback (`Fn`/`F19` keydown) may still work
- true background hold-to-talk may not fire

## Limitations (MVP)

- Global `fn` hotkey path is macOS-only in this implementation.
- No tray/menu controls yet (hide/show/quit are not exposed via tray).
- No explicit onboarding-complete gate for enabling hotkey monitor (it is always registered).
- No dedicated user-facing setting yet for hotkey remapping.
- Overlay windows are created at startup and do not yet live-reflow on monitor plug/unplug changes.
- Overlay window creation is capped at 6 monitors per process (`MAX_PILL_WINDOWS`).

## Quick Manual Test

1. Run `bun run tauri:dev`.
2. Complete model setup in onboarding.
3. Start/stop once from UI to confirm dictation health.
4. Close window using `X`.
5. Press and hold `fn` (or `F19`) and speak.
6. Release `fn` to stop and transcribe.
7. Reopen app from Dock and verify transcript updated.
