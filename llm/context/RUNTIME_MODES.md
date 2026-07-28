# Runtime Modes

## Status Snapshot

- Date: 2026-02-17
- Runtime split is determined by Tauri bridge presence, UA platform classification, and browser speech API support

## Purpose

Define runtime-specific behavior so debugging and feature work start from the correct execution mode.

## Scope

In scope:

- web mode behavior
- native desktop behavior (mac vs non-mac)
- iOS mode behavior

Out of scope:

- future runtime support promises

## Source Anchors

- `public/app.js` (ESM entry)
- `public/js/`
- `server.js`
- `package.json`

## Current State

Mode detection primitives:

- `isNativeDesktopMode()`
- `isFocusedMacDesktopMode()`
- `SpeechRecognitionApi` availability

Mode behavior:

1. Web mode (`bun run start`): static server + browser speech path if available.
2. Native desktop macOS: Tauri invoke/event bridge, onboarding gate, native dictation commands.
3. Native desktop non-mac: explicit unsupported desktop messaging for MVP native path.
4. iOS mode: mobile runtime scripts exist, desktop-native Whisper path is not active.

Rule of thumb:

- never assume a dictation bug without first identifying runtime mode.

## Verification

Re-verify runtime mode docs when these change:

1. mode helper functions in `public/app.js`
2. Tauri scripts in `package.json`
3. server behavior in `server.js`

## Related Docs

- [`CURRENT_STATE.md`](CURRENT_STATE.md)
- [`USER_FLOWS.md`](USER_FLOWS.md)
- [`../implementation/FRONTEND_RUNTIME_BEHAVIOR.md`](../implementation/FRONTEND_RUNTIME_BEHAVIOR.md)
