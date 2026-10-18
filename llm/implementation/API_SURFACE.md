# API Surface

## Status Snapshot

- Date: 2026-10-18
- Runtime interfaces: HTTP server contract and Tauri invoke/event contract

## Purpose

Provide a strict contract reference for all callable routes, commands, and runtime event channels.

## Scope

In scope:

- web HTTP contract from `/server.js`
- Tauri command and event contract from `src-tauri/src/commands.rs` / `src-tauri/src/hotkey_overlay/mod.rs`

Out of scope:

- implementation internals that do not affect external call surface

## Source Anchors

- `server.js`
- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/commands.rs`
- `src-tauri/src/permissions.rs`
- `src-tauri/src/hotkey_overlay/mod.rs`
- `src-tauri/src/state.rs`
- `public/js/constants.js`
- `public/js/events.js`
- `public/js/ui.js`
- `public/pill.js`

## Contract

HTTP interface:

- `GET /api/*` always returns `404` with JSON body:
  - `{"ok": false, "error": "No API routes are enabled in dictation-only mode."}`
- static assets served from `public/`
- SPA fallback returns `index.html` for navigation-like misses
- traversal-invalid paths return `400`

Tauri commands:

- `get_dictation_onboarding() -> Result<DictationOnboardingPayload, String>`
- `get_macos_permissions() -> MacosPermissions` (quiet snapshot)
- `request_microphone_permission() -> Result<MacosPermissions, String>` (explicit request)
- `open_macos_permission_settings(permission: PermissionPane) -> Result<(), String>`
- `recheck_macos_permissions() -> Result<MacosPermissions, String>` (quiet snapshot + recording-safe optional Fn refresh)
- `get_dictation_trigger() -> Result<DictationTriggerPayload, String>`
- `set_dictation_trigger(trigger: String) -> Result<DictationTriggerPayload, String>`
- `clear_dictation_trigger() -> Result<DictationTriggerPayload, String>`
- `set_pill_visibility_mode(mode: String) -> Result<BackgroundUiPreferencesPayload, String>`
- `set_menu_bar_mode(mode: String) -> Result<BackgroundUiPreferencesPayload, String>`
- `set_close_action(action: String) -> Result<BackgroundUiPreferencesPayload, String>`
- `set_preferred_input_device(device_name: Option<String>) -> Result<Option<String>, String>`
- `set_focused_field_insert_enabled(enabled: bool) -> Result<FocusedFieldInsertPayload, String>`
- `insert_text_into_focused_field(text: String) -> Result<(), String>`
- `copy_text_to_clipboard(text: String) -> Result<(), String>` (macOS manual Copy; no Accessibility requirement, key event, or clipboard restore)
- `open_whisper_setup_page() -> Result<(), String>`
- `install_dictation_model(model: String) -> Result<DictationModelSelection, String>`
- `delete_dictation_model(model: String) -> Result<DictationModelDeletion, String>`
- `start_native_dictation() -> Result<(), String>`
- `stop_native_dictation() -> Result<String, String>`
- `cancel_native_dictation() -> Result<(), String>`

Command payload notes:

- `MacosPermissions` contains `supported`, independent `microphone`, `accessibility`, and `input_monitoring` facts; microphone vocabulary is `not_determined | authorized | denied | restricted | unavailable`
- `PermissionPane` is the allowlisted `microphone | accessibility | input_monitoring`; arbitrary URLs are rejected
- see [`MACOS_PERMISSIONS.md`](MACOS_PERMISSIONS.md) for request/recheck semantics
- `DictationOnboardingPayload` includes `focused_field_insert_enabled: bool`
- `DictationOnboardingPayload` also includes `dictation_trigger_mode`, `dictation_trigger_status`, and `dictation_trigger_permission_hint`
- `DictationTriggerPayload` includes `trigger`, `default_trigger`, `trigger_mode`, `trigger_status`, and `trigger_permission_hint`
- `FocusedFieldInsertPayload` includes `enabled`, `permission_granted`, and `permission_status`

Event channels:

- frontend local fallback only: `dictation:hotkey-triggered` payload `{ pressed }`
- backend to frontend (and overlay): `dictation:state-changed` payload `{ state, error?, transcript?, session_id? }`
  - dictation session `state` vocabulary: `idle` | `listening` | `processing` | `error`
- backend to frontend/overlay: `dictation:audio-level` payload `{ session_id, peak_abs, rms, level, bars }` (always present; matches `DictationAudioLevelPayload`)
- backend/frontend to overlay: `dicktaint://pill-status` payload `{ message, state, visible }`
  - pill UI `state` vocabulary (distinct from dictation session states): `idle` | `working` | `live` | `ok` | `error`
- event name constants: SPA in `public/js/constants.js`; overlay duplicates strings in `public/pill.js`

Environment variables with contract impact:

- `HOST`, `PORT`
- `WHISPER_CLI_PATH`, `WHISPER_MODEL_PATH`
- `DICKTAINT_START_HIDDEN`

## Verification

Re-verify this file when these change:

1. route handling in `server.js`
2. `tauri::generate_handler!` registrations wired from `src-tauri/src/main.rs` (handlers in `src-tauri/src/commands.rs`)
3. event names or payload fields in `public/js/constants.js`, `public/js/events.js`, `public/js/ui.js`, and `public/pill.js`

## Related Docs

- [`WEB_SERVER_MODE.md`](WEB_SERVER_MODE.md)
- [`NATIVE_DESKTOP_DICTATION.md`](NATIVE_DESKTOP_DICTATION.md)
- [`CONFIG_AND_PATH_RESOLUTION.md`](CONFIG_AND_PATH_RESOLUTION.md)
