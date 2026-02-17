# Release And Distribution

## Status Snapshot

- Date: 2026-02-17
- desktop release posture is direct macOS distribution while private API mode is enabled

## Purpose

Define current release gates and distribution constraints.

## Scope

In scope:

- pre-release validation gates
- sidecar packaging requirements
- private API channel implications
- signing/notarization expectations

Out of scope:

- CI implementation specifics

## Source Anchors

- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/tauri.conf.json`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/binaries/README.md`
- `/Users/plebdev/Desktop/code/dicktaint/package.json`

## Runbook

Pre-release gates:

1. `bun run test:all`
2. `bun run docs:verify`
3. run manual smoke checklist from `SMOKE_TESTS.md`
4. verify sidecar binaries required for target distribution are present
5. verify private API setting matches release channel policy

Packaging requirement:

- `bundle.externalBin` is configured for `binaries/whisper-cli`
- target platform sidecar binaries must be available for intended targets

Direct macOS distribution requirements:

1. sign artifacts with Developer ID
2. notarize artifacts
3. staple notarization ticket
4. run clean-machine launch + dictation smoke

If pursuing App Store channel:

1. remove private API dependency path
2. set `macOSPrivateApi` to `false`
3. re-run full verification matrix

## Verification

Release readiness requires all runbook gates to pass and policy alignment to be documented.

## Related Docs

- [`MACOS_PRIVATE_API_POLICY.md`](MACOS_PRIVATE_API_POLICY.md)
- [`SMOKE_TESTS.md`](SMOKE_TESTS.md)
- [`DOCS_VERIFICATION.md`](DOCS_VERIFICATION.md)
