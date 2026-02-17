# dicktaint LLM Docs

## Status Snapshot

- Date: 2026-02-17
- Canonical docs root: `/Users/plebdev/Desktop/code/dicktaint/llm`
- Canonical scope: current shipped behavior for this repository

## Purpose

Provide implementation-aligned documentation for product context, runtime contracts, and operating runbooks.

## Scope

In scope:

- context: product/runtime/platform boundaries
- implementation: web server, frontend runtime branching, native desktop dictation, model lifecycle, hotkey/overlay, config resolution
- workflow: local development, smoke tests, release policy, troubleshooting

Out of scope:

- speculative features not present in code
- historical docs versions

## Source Anchors

Primary source files:

- `/Users/plebdev/Desktop/code/dicktaint/server.js`
- `/Users/plebdev/Desktop/code/dicktaint/public/app.js`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/tauri.conf.json`
- `/Users/plebdev/Desktop/code/dicktaint/scripts/build-whisper-sidecar.sh`
- `/Users/plebdev/Desktop/code/dicktaint/scripts/smoke-whisper-sidecar.sh`
- `/Users/plebdev/Desktop/code/dicktaint/package.json`

## Current State

Folder map:

- `context/`: high-level product and runtime truth
- `implementation/`: strict behavior contracts from code
- `workflow/`: operational runbooks and release controls
- `STANDARDS.md`: authoring and verification quality bar

## Verification

Minimum checks after doc changes:

1. Link/path check for all markdown docs.
2. Command mention check for `bun run <script>` references against `package.json` scripts.
3. File existence check for local absolute-path source anchors.

## Related Docs

Start here:

1. [`context/CURRENT_STATE.md`](context/CURRENT_STATE.md)
2. [`context/RUNTIME_MODES.md`](context/RUNTIME_MODES.md)
3. [`implementation/API_SURFACE.md`](implementation/API_SURFACE.md)
4. [`workflow/LOCAL_DEVELOPMENT.md`](workflow/LOCAL_DEVELOPMENT.md)
5. [`STANDARDS.md`](STANDARDS.md)
