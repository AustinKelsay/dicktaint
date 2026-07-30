# Platform Support

## Status Snapshot

- Date: 2026-07-30
- Priority order: macOS desktop -> iPhone iOS -> web fallback

## Purpose

Define support boundaries and distribution implications by platform.

## Scope

In scope:

- current supported behavior by platform
- private API release policy implications

Out of scope:

- target-date commitments for deferred platforms

## Source Anchors

- `public/app.js` (ESM entry)
- `public/js/platform.js`
- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/commands.rs`
- `src-tauri/src/hotkey_overlay.rs`
- `src-tauri/src/state.rs`
- `src-tauri/tauri.conf.json`
- `package.json`

## Current State

macOS desktop (primary):

- native dictation fully integrated with local Whisper CLI
- model onboarding and persistence active
- hold-to-talk and overlay windows active
- private API enabled for overlay transparency path

iOS (secondary):

- Tauri iOS init/dev/run/build scripts available
- no native desktop Whisper CLI command path

non-mac desktop:

- intentionally de-prioritized for current native dictation UX

Android:

- deferred

## Verification

Re-verify this document when any of these change:

1. platform guards in `public/js/platform.js`
2. macOS-specific blocks in `src-tauri/src/hotkey_overlay.rs` / `src-tauri/src/commands.rs`
3. private API config in `src-tauri/tauri.conf.json`

## Related Docs

- [`RUNTIME_MODES.md`](RUNTIME_MODES.md)
- [`../workflow/RELEASE_AND_DISTRIBUTION.md`](../workflow/RELEASE_AND_DISTRIBUTION.md)
- [`../workflow/MACOS_PRIVATE_API_POLICY.md`](../workflow/MACOS_PRIVATE_API_POLICY.md)
