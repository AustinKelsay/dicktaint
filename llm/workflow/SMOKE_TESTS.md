# Smoke Tests

## Status Snapshot

- Date: 2026-02-17
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

- `/Users/plebdev/Desktop/code/dicktaint/server.js`
- `/Users/plebdev/Desktop/code/dicktaint/public/app.js`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
- `/Users/plebdev/Desktop/code/dicktaint/scripts/smoke-whisper-sidecar.sh`

## Runbook

A) Web mode smoke:

1. run `bun run start`
2. open `http://127.0.0.1:3000`
3. validate `/api/health` disabled response
4. validate SPA fallback route
5. validate missing asset `404`

B) Desktop setup smoke (macOS):

1. run `bun run tauri:dev`
2. verify setup checks
3. install/select one model
4. verify setup ready indicators

C) Desktop dictation smoke (button path):

1. start dictation
2. speak 3-5 seconds
3. stop dictation
4. verify transcript append
5. verify transcribe status includes non-zero capture duration

D) Background hold-to-talk smoke:

1. close main window
2. hold `fn`, speak, release
3. reopen app
4. verify transcript append and no stuck recording state

E) Long-utterance quality smoke (desktop):

1. hold dictation for ~10 seconds with a full sentence
2. stop and verify transcript is not collapsed into repeated short tokens
3. if quality warning appears, retry closer to mic in lower-noise environment and compare
4. if repeated collapse persists, log model id + runtime mode + sample phrase

F) Sidecar smoke:

1. run `bun run whisper:smoke`
2. verify transcript file generated and content check passes

G) Automated baseline:

1. run `bun run test:all`
2. run `bun run docs:verify`

## Verification

This checklist is complete only when sections A-G all pass in the target runtime under test.

## Related Docs

- [`LOCAL_DEVELOPMENT.md`](LOCAL_DEVELOPMENT.md)
- [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md)
- [`../implementation/HOTKEY_AND_OVERLAY.md`](../implementation/HOTKEY_AND_OVERLAY.md)
