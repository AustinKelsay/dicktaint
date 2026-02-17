# Hotkey And Overlay

## Status Snapshot

- Date: 2026-02-17
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
- emits `dicktaint://fn-state` only on edge transitions
- creates native transparent overlay windows per monitor (up to 6)
- close request on main window hides app instead of quitting
- macOS reopen event re-shows and focuses main window

Frontend behavior:

- listens for `dicktaint://fn-state`
- fallback focused listeners for `Fn` / `F19`
- on press: start native dictation
- on release: stop native dictation and transcribe
- release-during-start race handled by deferred stop flag
- status emits `dicktaint://pill-status` updates for overlay UI

Permission expectations:

- Input Monitoring and Accessibility may be required for global key monitoring

Dependency constraint:

- overlay transparency path depends on `macOSPrivateApi` enablement

## Verification

Manual verification after hotkey/overlay edits:

1. run desktop app and complete setup
2. close main window
3. hold/release `fn` and speak
4. reopen app and verify transcript append
5. verify overlay status transitions across lifecycle

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`../workflow/SMOKE_TESTS.md`](../workflow/SMOKE_TESTS.md)
- [`../workflow/MACOS_PRIVATE_API_POLICY.md`](../workflow/MACOS_PRIVATE_API_POLICY.md)
