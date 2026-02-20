# Current State

## Status Snapshot

- Date: 2026-02-17
- Product focus: private local-first dictation
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

- `/Users/plebdev/Desktop/code/dicktaint/public/app.js`
- `/Users/plebdev/Desktop/code/dicktaint/server.js`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/tauri.conf.json`

## Current State

Implemented now:

- static web server with SPA fallback and disabled `/api/*`
- native desktop dictation path using Rust capture + local `whisper-cli` transcription
- local model onboarding with recommendation/install/delete/switch behavior
- background desktop behavior with hide-on-close and `fn` hold-to-talk (macOS)
- native overlay pill windows for dictation feedback on macOS

Explicitly out of scope now:

- enabled backend API routes
- cloud transcription mode
- Android support
- non-mac desktop parity for hold-to-talk overlay UX

Key policy:

- `macOSPrivateApi` is enabled to support transparent overlay behavior

## Verification

Validate this document when any of these change:

1. app runtime mode detection in `/Users/plebdev/Desktop/code/dicktaint/public/app.js`
2. command/event registrations in `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
3. desktop bundle/runtime config in `/Users/plebdev/Desktop/code/dicktaint/src-tauri/tauri.conf.json`

## Related Docs

- [`RUNTIME_MODES.md`](RUNTIME_MODES.md)
- [`PLATFORM_SUPPORT.md`](PLATFORM_SUPPORT.md)
- [`../implementation/API_SURFACE.md`](../implementation/API_SURFACE.md)
