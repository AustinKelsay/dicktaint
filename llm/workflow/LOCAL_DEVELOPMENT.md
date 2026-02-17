# Local Development Workflows

## Status Snapshot

- Date: 2026-02-17
- this runbook covers day-to-day dev flow for web, desktop, and iOS commands

## Purpose

Provide a repeatable local workflow that keeps runtime behavior and docs quality in sync.

## Scope

In scope:

- install/start/test commands
- desktop sidecar helper flow
- iOS development commands
- docs verification gate

Out of scope:

- production release notarization specifics

## Source Anchors

- `/Users/plebdev/Desktop/code/dicktaint/package.json`
- `/Users/plebdev/Desktop/code/dicktaint/scripts/build-whisper-sidecar.sh`
- `/Users/plebdev/Desktop/code/dicktaint/scripts/smoke-whisper-sidecar.sh`
- `/Users/plebdev/Desktop/code/dicktaint/scripts/verify-docs.sh`

## Runbook

Bootstrap:

1. `bun install`

Web mode:

1. `bun run start`
2. optional watch mode: `bun run dev`

Desktop mode:

1. optional sidecar rebuild: `bun run whisper:sidecar`
2. optional sidecar smoke: `bun run whisper:smoke`
3. run desktop dev: `bun run tauri:dev`

Tests:

1. `bun run test`
2. `bun run test:rust`
3. `bun run test:all`

Docs quality gate:

1. `bun run docs:verify`

iOS commands:

1. init: `bun run tauri:ios:init`
2. dev: `APPLE_DEVELOPMENT_TEAM=<team-id> TAURI_DEV_HOST=<lan-ip> bun run tauri:ios:dev`
3. run/build: `bun run tauri:ios:run` and `bun run tauri:ios:build`

Recommended daily sequence (desktop-heavy work):

1. `bun run tauri:dev`
2. perform manual smoke checks
3. `bun run test:all`
4. `bun run docs:verify`

## Verification

This runbook is valid when all listed scripts exist in `/Users/plebdev/Desktop/code/dicktaint/package.json` and execute in the current environment.

## Related Docs

- [`SMOKE_TESTS.md`](SMOKE_TESTS.md)
- [`DOCS_VERIFICATION.md`](DOCS_VERIFICATION.md)
- [`../implementation/API_SURFACE.md`](../implementation/API_SURFACE.md)
