# Hotkey And Overlay

## Status Snapshot

- Date: 2026-02-20
- hold-to-talk and overlay behavior are macOS-focused MVP features

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

- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
- `/Users/plebdev/Desktop/code/dicktaint/public/app.js`
- `/Users/plebdev/Desktop/code/dicktaint/public/pill.js`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/tauri.conf.json`

## Contract

Backend behavior:

- registers global monitor for macOS `flagsChanged`
- emits `dictation:hotkey-triggered` with `{ pressed }` so `Fn` hold can start on press and stop on release even when the main window is hidden
- creates native transparent overlay windows per monitor (up to 6)
- close request on main window hides app instead of quitting
- macOS reopen event re-shows and focuses main window

Frontend behavior:

- listens for `dictation:hotkey-triggered`
- fallback focused listeners for `Fn` / `F19`
- on `Fn` press: start native dictation
- on `Fn` release: stop native dictation and transcribe
- on non-`Fn` shortcut press: toggle start/stop
- release-during-start race handled by deferred stop flag
- status emits `dicktaint://pill-status` updates for overlay UI
- pill copy reflects the saved hotkey and its mode (`global-hold`, `focused-window-hold`, `global-toggle`)
- onboarding/settings surface hotkey runtime state plus permission guidance
- finalized transcript appends locally and can optionally paste into the focused field when setting is enabled and another app is focused

Permission expectations:

- Input Monitoring and Accessibility may be required for global key monitoring
- Accessibility is required for focused-field paste; the app now uses native pasteboard + key event posting instead of `System Events`

Dependency constraint:

- overlay transparency path depends on `macOSPrivateApi` enablement

## Verification

Manual verification after hotkey/overlay edits:

1. run desktop app and complete setup
2. close main window
3. hold/release `fn` and speak
4. reopen app and verify transcript append
5. enable focused-field insertion, focus another app text field, repeat hold/release flow, verify paste
6. verify overlay status transitions across lifecycle

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`../workflow/SMOKE_TESTS.md`](../workflow/SMOKE_TESTS.md)
- [`../workflow/MACOS_PRIVATE_API_POLICY.md`](../workflow/MACOS_PRIVATE_API_POLICY.md)
