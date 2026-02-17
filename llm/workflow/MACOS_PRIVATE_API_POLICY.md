# macOS Private API Policy

## Status Snapshot

- Date: 2026-02-17
- current config sets `macOSPrivateApi` to `true`

## Purpose

Define the release policy impact of macOS private API enablement.

## Scope

In scope:

- current policy decision
- release implications
- mandatory controls before publishing artifacts

Out of scope:

- implementation details unrelated to policy impact

## Source Anchors

- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/tauri.conf.json`
- `/Users/plebdev/Desktop/code/dicktaint/src-tauri/Cargo.toml`

## Runbook

Current decision:

- keep private API enabled for transparent overlay behavior

Implication:

- direct distribution is supported
- Mac App Store submission is not supported in current state

Mandatory controls:

1. confirm release channel per build
2. ensure channel policy matches `macOSPrivateApi` setting
3. include release-note disclosure of private API usage
4. smoke test overlay rendering + hold-to-talk behavior

Change control:

- do not flip `macOSPrivateApi` without explicit channel decision and implementation plan for overlay behavior replacement

## Verification

Policy remains valid when `src-tauri/tauri.conf.json` and release channel decisions remain aligned.

## Related Docs

- [`RELEASE_AND_DISTRIBUTION.md`](RELEASE_AND_DISTRIBUTION.md)
- [`../implementation/HOTKEY_AND_OVERLAY.md`](../implementation/HOTKEY_AND_OVERLAY.md)
