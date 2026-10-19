# Config And Path Resolution

## Status Snapshot

- Date: 2026-10-18
- runtime config and path probing behavior are implemented in Rust backend + package scripts

## Purpose

Define environment variables and executable/model resolution order.

## Scope

In scope:

- env vars with runtime behavior impact
- whisper-cli candidate path detection and validation
- active model path resolution
- local persistence location contract

Out of scope:

- OS-specific install tutorials beyond command-level guidance

## Source Anchors

- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/commands.rs`
- `src-tauri/src/hotkey_overlay/mod.rs`
- `src-tauri/src/state.rs`
- `src-tauri/tauri.conf.json`
- `package.json`

## Contract

Environment variables:

- `HOST`, `PORT` for web/tauri dev server binding
- `WHISPER_CLI_PATH` explicit CLI override
- `WHISPER_MODEL_PATH` explicit model override
- `DICKTAINT_START_HIDDEN` startup visibility control

CLI resolution order:

1. explicit `WHISPER_CLI_PATH` if provided
2. bundled sidecar candidate path if present
3. default `whisper-cli` command
4. candidate probing through local sidecar and common OS install paths

Bundled sidecar policy (real vs placeholder per target triple):

- see [`../../src-tauri/binaries/README.md`](../../src-tauri/binaries/README.md)
- validation rejects placeholder binaries (`--help` / probe behavior)

CLI validation requirements:

- candidate exists
- candidate file/executable characteristics are valid
- `--help` output resembles real whisper-cli and rejects placeholder behavior

Model resolution order:

1. `WHISPER_MODEL_PATH` override when set and valid
2. persisted selected model path from local settings

Setup and capture use the same model resolver: known catalog entries must pass the file-size heuristic, while existing saved custom model files remain compatible. An override is trimmed, must be a file, and leaves the persisted selection intact. See [`MODEL_MANAGEMENT.md`](MODEL_MANAGEMENT.md).

Local persistence:

- settings path: `$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/dictation-settings.json`
- model directory: `$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/whisper-models/`
- settings include model selection, dictation trigger config, microphone selection, background UI preferences, and `focused_field_insert_enabled`
- the Murmur display rename retains the legacy bundle identifier/executable/storage/env/event contracts; signing continuity is a separate distribution concern (ADR 0002)

## Verification

Re-verify when configuration behavior changes:

1. run desktop startup with and without overrides
2. check onboarding returns expected detected CLI path
3. confirm start dictation fails with clear message on invalid override paths

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`NATIVE_DESKTOP_DICTATION.md`](NATIVE_DESKTOP_DICTATION.md)
- [`../workflow/LOCAL_DEVELOPMENT.md`](../workflow/LOCAL_DEVELOPMENT.md)
