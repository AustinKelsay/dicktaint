# Troubleshooting

## Status Snapshot

- Date: 2026-10-18
- issue patterns mapped to current runtime behavior and scripts

## Purpose

Provide action-first diagnosis steps for common failures.

## Scope

In scope:

- CLI/model setup errors
- microphone/capture failures
- hotkey path failures
- web speech fallback issues
- iOS dev connectivity issues

Out of scope:

- long-form platform installation tutorials

## Source Anchors

- `public/app.js` (ESM entry)
- `public/js/`
- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/commands.rs`
- `src-tauri/src/hotkey_overlay/mod.rs`
- `src-tauri/src/state.rs`
- `scripts/smoke-whisper-sidecar.sh`

## Runbook

1) `whisper-cli` unavailable:

1. `bun run whisper:sidecar`
2. `bun run whisper:smoke`
3. refresh setup in app
4. set `WHISPER_CLI_PATH` if needed

2) no local model selected:

1. open setup
2. install the recommended model or choose an installed model
3. confirm ready state

3) microphone/input errors:

1. inspect Microphone in Murmur setup/Settings; `not_determined` offers an explicit Allow action, `denied` offers Open Microphone settings, and `restricted` indicates policy-controlled access
2. for an unavailable check, choose Recheck; do not treat unknown status as a grant
3. after changing a grant, return to Murmur or choose Recheck; active capture and the draft are preserved
4. if capture still fails, verify the selected input device and macOS Sound settings; authorization does not prove the device opens successfully
5. if trust appears stale, quit and reopen that same app build, then inspect its privacy row again; a local ad hoc build may have a different trust identity from the installed app
6. for a built app that never appears in Microphone settings, run `bun run verify:bundle` to check the usage description, signed audio-input entitlement, and signature

4) hold-to-talk does not fire:

1. inspect the hotkey runtime mode in Settings separately from each permission row
2. Accessibility or Input Monitoring may enable background Fn; either is optional, and both grants are independent
3. explicitly open the desired privacy pane, enable the app, return to Murmur, and Recheck while idle
4. verify the focused-window Fn/F19 fallback and button dictation; an optional hotkey failure must not block required-ready button capture
5. quit/reopen the same build if macOS trust remains stale; avoid resetting the privacy database or overwriting the installed working app as routine recovery

4a) `Fn` fires but brings Murmur to the front:

1. open Settings and inspect the runtime hotkey mode
2. if it says `focused-window-hold`, use an explicit Accessibility or Input Monitoring settings action, return, and Recheck while idle
3. if it says `global-hold`, treat it as a regression in the start/focus path

5) no transcript after stop:

1. verify mic levels and input route
2. retry longer utterance
3. run `bun run whisper:smoke`

6) web speech controls fail:

1. confirm browser speech API support
2. confirm mic permissions
3. the browser provider may require network/remote processing; native local Whisper privacy does not describe this runtime
4. use manual text fallback

7) iOS dev connectivity issues:

1. run with valid `APPLE_DEVELOPMENT_TEAM`
2. set `TAURI_DEV_HOST` to LAN IP
3. confirm dev host reachability from device

8) focused-field insertion does not paste:

1. enable `Dictate Into Focused Field` in app settings
2. ensure target app text field is focused when dictation completes
3. explicitly open Accessibility settings from Murmur, grant the actual app/runtime, and return or Recheck; Input Monitoring does not grant insertion access
4. retry with a plain text field to rule out app-specific input restrictions

9) Copy does not work:

1. on macOS desktop, Copy uses the native clipboard command and does not require Accessibility
2. in the browser, clipboard access depends on browser support and permission; try a secure origin or select the draft and use the Edit menu
3. copy a draft/history entry and paste manually into a plain text field to check the result independently of automatic focused-field insertion

Escalation payload to capture:

- runtime mode
- exact command used
- full visible error text
- `bun run test:all` result
- `bun run docs:verify` result

## Verification

Run this triage order against the matching symptom group before escalating to deeper code-level debugging.

## Related Docs

- [`SMOKE_TESTS.md`](SMOKE_TESTS.md)
- [`DOCS_VERIFICATION.md`](DOCS_VERIFICATION.md)
- [`../context/RUNTIME_MODES.md`](../context/RUNTIME_MODES.md)
