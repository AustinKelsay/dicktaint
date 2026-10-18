# Hotkey And Overlay

## Status Snapshot

- Date: 2026-10-18
- hold-to-talk and overlay behavior are macOS-focused MVP features
- overlay document is `public/pill.html`; live runtime is `public/pill.js`

## Purpose

Define background `fn` hold-to-talk behavior and native overlay window contract.

## Scope

In scope:

- global fn monitor behavior
- frontend hold-state behavior
- overlay window creation and status updates
- hide-on-close / reopen behavior coupling

Out of scope:

- tray/menu UX controls
- non-mac parity path

## Source Anchors

- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/commands.rs`
- `src-tauri/src/hotkey_overlay/mod.rs`
- `src-tauri/src/state.rs`
- `public/app.js` (ESM entry)
- `public/js/events.js`
- `public/js/native-dictation.js`
- `public/js/ui.js`
- `public/js/constants.js`
- `public/pill.html`
- `public/pill.js`
- `src-tauri/tauri.conf.json`

## Contract

Backend behavior:

- registers global monitor for macOS `flagsChanged`
- executes global hotkey actions in Rust so start/stop does not depend on the main window webview being awake
- emits `dictation:state-changed` with backend `session_id` values so late transcripts cannot clear a newer live recording
- creates native transparent overlay windows per monitor (up to 6)
- emits `dicktaint://pill-status` directly for listening / processing / idle / error transitions so the overlay stays accurate while the main window is hidden
- close request on main window hides app instead of quitting
- macOS reopen event re-shows and focuses main window

Frontend behavior:

- fallback focused listeners for `Fn` / `F19`
- uses focused-window `Fn` / `F19` fallback when actual global hotkey delivery is unavailable; either Accessibility or Input Monitoring can enable the global Fn path
- listens for `dictation:state-changed` and treats `session_id` as the authoritative native session identity
- release-during-start race handled by deferred stop flag
- status may still emit `dicktaint://pill-status` for setup/onboarding text, but backend owns dictation lifecycle pill updates
- pill copy reflects the saved hotkey and its mode (`global-hold`, `focused-window-hold`, `global-toggle`)
- onboarding/settings surface hotkey runtime state plus permission guidance
- finalized transcript appends locally and can optionally paste into the focused field when setting is enabled and another app is focused

Overlay window (`public/pill.html` + `public/pill.js`):

- classic script (not SPA ESM); event name strings mirror `public/js/constants.js`
- listens for `dicktaint://pill-status`, `dictation:state-changed`, `dictation:audio-level`, and `dictation:hotkey-triggered`
- renders icon / audio-level waveform / spinner states (not text-label pill UI)

Permission expectations:

- Accessibility enables focused-field paste and can enable background Fn; Input Monitoring is an alternative background Fn route
- each grant is observed separately from the actual hotkey capability; neither optional grant blocks button dictation
- startup and status observation stay quiet; only explicit permission/settings actions may prompt or open System Settings
- returning to the app can re-arm the listener while idle; active recording or a pending start keeps its current listener
- focused-field paste uses native pasteboard + key event posting instead of `System Events`
- see [`MACOS_PERMISSIONS.md`](MACOS_PERMISSIONS.md) for DTO and recovery details

Dependency constraint:

- overlay transparency path depends on `macOSPrivateApi` enablement

## Verification

Manual verification after hotkey/overlay edits:

1. run desktop app and complete setup
2. close main window
3. hold/release `fn` and speak
4. reopen app and verify transcript append
5. enable focused-field insertion, focus another app text field, repeat hold/release flow, verify paste
6. verify overlay status transitions across lifecycle (including audio-level waveform while listening)

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`../workflow/SMOKE_TESTS.md`](../workflow/SMOKE_TESTS.md)
- [`../workflow/MACOS_PRIVATE_API_POLICY.md`](../workflow/MACOS_PRIVATE_API_POLICY.md)
