# User Flows

## Status Snapshot

- Date: 2026-02-17
- Flow descriptions match current app behavior in web and desktop paths

## Purpose

Document end-user flow sequences that the implementation currently supports.

## Scope

In scope:

- first-run setup
- button dictation flow
- hold-to-talk flow
- model management flow
- web fallback flow

Out of scope:

- not-yet-built UX variations

## Source Anchors

- `/Users/plebdev/Desktop/code/dicktaint/public/index.html`
- `/Users/plebdev/Desktop/code/dicktaint/public/app.js`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`

## Current State

Desktop first-run (macOS):

1. app opens setup screen
2. onboarding checks CLI, hardware profile, model state
3. user installs/selects local model
4. setup unlocks dictation

Desktop dictation (button):

1. click start
2. speak
3. click stop
4. transcript returns and appends

Desktop dictation (hold-to-talk):

1. hold `fn`
2. app starts capture
3. release `fn`
4. app stops/transcribes
5. transcript appends

Race control detail:

- if key release occurs during startup, frontend defers stop to avoid lifecycle drop.

Desktop model maintenance:

- user can download/use installed model or delete model with confirmation.

Web flow:

- browser speech recognition path when available, manual text entry fallback always available.

## Verification

Re-verify when changing:

1. onboarding and dictation handlers in `/Users/plebdev/Desktop/code/dicktaint/public/app.js`
2. command implementations in `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
3. setup UI structure in `/Users/plebdev/Desktop/code/dicktaint/public/index.html`

## Related Docs

- [`../implementation/FRONTEND_RUNTIME_BEHAVIOR.md`](../implementation/FRONTEND_RUNTIME_BEHAVIOR.md)
- [`../implementation/API_SURFACE.md`](../implementation/API_SURFACE.md)
- [`../workflow/SMOKE_TESTS.md`](../workflow/SMOKE_TESTS.md)
