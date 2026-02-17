# Troubleshooting

## Status Snapshot

- Date: 2026-02-17
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

- `/Users/plebdev/Desktop/code/dicktaint/public/app.js`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
- `/Users/plebdev/Desktop/code/dicktaint/scripts/smoke-whisper-sidecar.sh`

## Runbook

1) `whisper-cli` unavailable:

1. `bun run whisper:sidecar`
2. `bun run whisper:smoke`
3. refresh setup in app
4. set `WHISPER_CLI_PATH` if needed

2) no local model selected:

1. open setup
2. install model with `Download + Use`
3. confirm ready state

3) microphone/input errors:

1. verify input device in macOS sound settings
2. verify microphone permission
3. relaunch runtime after permission changes

4) hold-to-talk does not fire:

1. grant Input Monitoring + Accessibility
2. relaunch runtime
3. verify focused fallback key path

5) no transcript after stop:

1. verify mic levels and input route
2. retry longer utterance
3. run `bun run whisper:smoke`

6) web speech controls fail:

1. confirm browser speech API support
2. confirm mic permissions
3. use manual text fallback

7) iOS dev connectivity issues:

1. run with valid `APPLE_DEVELOPMENT_TEAM`
2. set `TAURI_DEV_HOST` to LAN IP
3. confirm dev host reachability from device

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
