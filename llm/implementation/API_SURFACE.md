# API Surface

## Status Snapshot

- Date: 2026-02-20
- Runtime interfaces: HTTP server contract and Tauri invoke/event contract

## Purpose

Provide a strict contract reference for all callable routes, commands, and runtime event channels.

## Scope

In scope:

- web HTTP contract from `/server.js`
- Tauri command and event contract from `/src-tauri/src/main.rs`

Out of scope:

- implementation internals that do not affect external call surface

## Source Anchors

- `/Users/plebdev/Desktop/code/dicktaint/server.js`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`

## Contract

HTTP interface:

- `GET /api/*` always returns `404` with JSON body:
  - `{"ok": false, "error": "No API routes are enabled in dictation-only mode."}`
- static assets served from `public/`
- SPA fallback returns `index.html` for navigation-like misses
- traversal-invalid paths return `400`

Tauri commands:

- `get_dictation_onboarding() -> DictationOnboardingPayload`
- `get_dictation_trigger() -> DictationTriggerPayload`
- `set_dictation_trigger(trigger: String) -> DictationTriggerPayload`
- `clear_dictation_trigger() -> DictationTriggerPayload`
- `set_focused_field_insert_enabled(enabled: bool) -> FocusedFieldInsertPayload`
- `insert_text_into_focused_field(text: String) -> Result<(), String>`
- `open_whisper_setup_page() -> Result<(), String>`
- `install_dictation_model(model: String) -> DictationModelSelection`
- `delete_dictation_model(model: String) -> DictationModelDeletion`
- `start_native_dictation() -> Result<(), String>`
- `stop_native_dictation() -> Result<String, String>`
- `cancel_native_dictation() -> Result<(), String>`

Command payload notes:

- `DictationOnboardingPayload` includes `focused_field_insert_enabled: bool`
- `DictationTriggerPayload` includes `trigger` and `default_trigger`
- `FocusedFieldInsertPayload` includes `enabled`

Event channels:

- backend to frontend: `dictation:hotkey-triggered` payload `()`
- backend to frontend: `dictation:state-changed` payload `{ state, error?, transcript? }`
- frontend to overlay: `dicktaint://pill-status` payload `{ message, state, visible }`
- allowed `state`: `idle`, `working`, `live`, `ok`, `error`

Environment variables with contract impact:

- `HOST`, `PORT`
- `WHISPER_CLI_PATH`, `WHISPER_MODEL_PATH`
- `DICKTAINT_START_HIDDEN`

## Verification

Re-verify this file when these change:

1. route handling in `/Users/plebdev/Desktop/code/dicktaint/server.js`
2. `tauri::generate_handler!` registrations in `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
3. event names or payload fields in `/Users/plebdev/Desktop/code/dicktaint/public/app.js` and `/Users/plebdev/Desktop/code/dicktaint/public/pill.js`

## Related Docs

- [`WEB_SERVER_MODE.md`](WEB_SERVER_MODE.md)
- [`NATIVE_DESKTOP_DICTATION.md`](NATIVE_DESKTOP_DICTATION.md)
- [`CONFIG_AND_PATH_RESOLUTION.md`](CONFIG_AND_PATH_RESOLUTION.md)
