# ADR 0001: Domain modules for native desktop + ES modules for SPA

## Status

Accepted

## Context

`src-tauri/src/main.rs` (~5k LOC) and `public/app.js` (~3k LOC) absorbed every desktop feature. Changes have poor locality. Cleanup P1 needs durable seams without changing product behavior.

## Decision

1. Split Rust into domain modules under `src-tauri/src/` (`whisper_cli`, `models`, `audio`, `transcribe`, `hotkey_overlay`, `insert`, `state`, `commands`, `dictation_session`, `onboarding`) with a thin `main.rs` entry. Prefer `pub(crate)` and keep Tauri command names stable.
2. Split the SPA into ES modules under `public/js/` loaded via `type="module"` from `public/app.js`. No bundler in this pass.
3. Keep behavior identical: same Tauri commands, events, settings schema, and UI flows.

## Consequences

- Higher navigability and test locality.
- First PR may be large but should be mechanical moves + import wiring.
- Follow-up health loops can deepen interfaces further without another god-file tax.
