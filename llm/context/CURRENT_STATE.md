# Current State

## Status Snapshot

- Date: 2026-10-18
- Product: Murmur — private dictation for your Mac
- Product focus: native local transcription with a separate browser speech fallback
- MVP platform priority: macOS desktop, then iPhone iOS, then web fallback

## Purpose

Define what the product currently does and does not do.

## Scope

In scope:

- shipped features in this repository
- explicit MVP boundaries
- references to canonical behavior docs

Out of scope:

- roadmap commitments
- speculative architecture

## Source Anchors

- `public/app.js` (ESM entry)
- `public/js/platform.js`
- `public/js/events.js`
- `public/pill.html`
- `public/pill.js`
- `server.js`
- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/commands.rs`
- `src-tauri/src/hotkey_overlay/mod.rs`
- `src-tauri/src/state.rs`
- `src-tauri/tauri.conf.json`

## Current State

Implemented now:

- static web server with SPA fallback and disabled `/api/*`
- native desktop dictation path using Rust capture + local `whisper-cli` transcription
- local model onboarding with recommendation/install/delete/switch behavior
- background desktop behavior with hide-on-close and `fn` hold-to-talk (macOS)
- native overlay pill windows for dictation feedback on macOS, using an extra-compact floating presentation with icon-only status feedback
- compact Dictation workspace with editable/copyable draft and temporary recent dictations
- required setup for engine, selected model, and microphone; ready users open into Dictation
- separate Settings for local models, optional Mac access, and advanced preferences
- explicit permission requests/settings actions and recording-safe rechecks; observation never prompts
- system fonts and responsive light/dark UI with keyboard focus and reduced-motion support
- optional macOS setting to paste finalized transcript into the currently focused external text field

Explicitly out of scope now:

- enabled backend API routes
- cloud transcription mode
- Android support
- non-mac desktop parity for hold-to-talk overlay UX

Key policy:

- `macOSPrivateApi` is enabled to support transparent overlay behavior
- native captured audio is transcribed locally; browser SpeechRecognition may use a remote provider
- visible branding is Murmur; legacy bundle identifier, executable, storage, and event names remain stable (ADR 0002)

## Verification

Validate this document when any of these change:

1. app runtime mode detection in `public/js/platform.js` (bootstrapped from `public/app.js`)
2. command/event registrations across `src-tauri/src/commands.rs` and `src-tauri/src/hotkey_overlay/mod.rs`
3. desktop bundle/runtime config in `src-tauri/tauri.conf.json`
4. overlay runtime in `public/pill.html` / `public/pill.js`

## Related Docs

- [`RUNTIME_MODES.md`](RUNTIME_MODES.md)
- [`PLATFORM_SUPPORT.md`](PLATFORM_SUPPORT.md)
- [`../implementation/API_SURFACE.md`](../implementation/API_SURFACE.md)
