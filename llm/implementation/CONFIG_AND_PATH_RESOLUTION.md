# Config And Path Resolution

## Status Snapshot

- Date: 2026-02-20
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

- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/src/main.rs`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/tauri.conf.json`
- `/Users/plebdev/Desktop/code/dicktaint/package.json`

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

CLI validation requirements:

- candidate exists
- candidate file/executable characteristics are valid
- `--help` output resembles real whisper-cli and rejects placeholder behavior

Model resolution order:

1. `WHISPER_MODEL_PATH` override when set and valid
2. persisted selected model path from local settings

Local persistence:

- settings path: `$HOME/.dicktaint/dictation-settings.json`
- model directory: `$HOME/.dicktaint/whisper-models/`
- settings include model selection, dictation trigger config, and `focused_field_insert_enabled`

## Verification

Re-verify when configuration behavior changes:

1. run desktop startup with and without overrides
2. check onboarding returns expected detected CLI path
3. confirm start dictation fails with clear message on invalid override paths

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`NATIVE_DESKTOP_DICTATION.md`](NATIVE_DESKTOP_DICTATION.md)
- [`../workflow/LOCAL_DEVELOPMENT.md`](../workflow/LOCAL_DEVELOPMENT.md)
