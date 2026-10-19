# Smoke Tests

## Status Snapshot

- Date: 2026-10-18
- smoke coverage targets runtime-critical user paths

## Purpose

Provide manual high-signal checks to catch regressions quickly.

## Scope

In scope:

- web server path
- desktop setup + dictation
- hold-to-talk background path
- sidecar transcription pipeline
- automated baseline checks

Out of scope:

- exhaustive QA matrix

## Source Anchors

- `server.js`
- `public/app.js` (ESM entry)
- `public/js/`
- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/commands.rs`
- `src-tauri/src/hotkey_overlay/mod.rs`
- `src-tauri/src/state.rs`
- `scripts/smoke-whisper-sidecar.sh`
- `scripts/verify-macos-bundle.sh`
- `tests/macos-bundle.test.js`

## Runbook

A) Web mode smoke:

1. run `bun run start`
2. open `http://127.0.0.1:3000`
3. validate `/api/health` disabled response
4. validate SPA fallback route
5. validate missing asset `404`
6. verify browser speech privacy copy says the provider may process audio remotely
7. edit/copy a draft, reuse a recent dictation, navigate to Settings and back, and check draft retention
8. inspect desktop and small-window layouts, keyboard focus, light/dark appearance, and reduced-motion behavior; verify no external font request is needed

B) Desktop setup smoke (macOS):

1. run `bun run tauri:dev` or use the separate built app without replacing the installed working app
2. verify a ready returning Mac opens Dictation, while a first/incomplete setup opens the required checklist
3. inspect engine, selected model, and microphone separately; install/select a model and explicitly allow microphone access when unrequested
4. verify both optional grants can remain off while required-ready button dictation is enabled
5. use Settings to switch/install/delete models, including confirmation and valid fallback after active-model deletion; verify saved custom model files and model overrides remain usable
6. verify permission observation does not prompt, open System Settings, or steal focus
7. check `not_determined`, `authorized`, `denied`, `restricted`, unavailable/error states using frontend DTO tests/simulation; record separately which states were observed on real macOS
8. use explicit Open settings and Recheck actions, return from Settings, and confirm updated independent Accessibility/Input Monitoring facts and actual Fn runtime mode
9. start capture, type a draft, return focus/recheck, and verify capture/draft/navigation survive; stop while Settings is open
10. verify Copy draft and Copy recent entry by manually pasting into a plain text field, without Accessibility access; clear history and confirm it is temporary across restart

C) Desktop dictation smoke (button path):

1. start dictation
2. speak 3-5 seconds
3. stop dictation
4. verify transcript append, editable draft, and recent-item reuse/copy

D) Background hold-to-talk smoke:

1. inspect actual global Fn capability; Accessibility or Input Monitoring can provide it
2. close main window, hold `fn`, speak, release, and confirm Murmur does not take focus
3. reopen app
4. verify transcript append and no stuck recording state
5. verify focused-window Fn/F19 fallback when global delivery is unavailable
6. verify overlay listening/processing/ready/error transitions and microphone indicator teardown

E) Focused-field insertion smoke (macOS desktop):

1. open app settings and enable focused-field insertion
2. focus a text input in another app (Notes, browser textarea, etc.)
3. trigger dictation by button path or hold-to-talk path
4. verify transcript pastes into the external focused field
5. disable setting and verify paste no longer occurs

F) Sidecar smoke:

1. run `bun run whisper:smoke`
2. verify transcript file generated and content check passes
3. for a release-bundle check, run the built app’s `Contents/MacOS/whisper-cli` against the same fixture/model too; this is separate from merely inspecting architecture/signature

G) Automated baseline:

1. run `bun run test:all`
2. run `bun run docs:verify`

H) Local bundle verification:

1. follow the build/sign sequence in [`RELEASE_AND_DISTRIBUTION.md`](RELEASE_AND_DISTRIBUTION.md)
2. run `bun run verify:bundle` (or supply an app path); keep the original app installed/running untouched
3. record app name, identifier, executable, versions, matching architecture, real sidecar, signed audio-input entitlement, and strict deep signature result
4. label ad hoc output as local verification only; public Developer ID, notarization, stapling, and clean-machine checks remain separate gates

## Verification

Record the runtime, artifact/commit, command results, browser simulation coverage, and native observations. This checklist is complete only when sections A-H pass in their target runtimes. A DTO simulation establishes rendered behavior, not an OS permission grant; a signature check establishes bundle integrity, not successful recording or transcription.

## Related Docs

- [`LOCAL_DEVELOPMENT.md`](LOCAL_DEVELOPMENT.md)
- [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md)
- [`../implementation/HOTKEY_AND_OVERLAY.md`](../implementation/HOTKEY_AND_OVERLAY.md)
