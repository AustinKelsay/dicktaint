# Docs Verification

## Status Snapshot

- Date: 2026-02-17
- docs verification is automated via `scripts/verify-docs.sh`

## Purpose

Define the reproducible verification process for documentation quality and integrity.

## Scope

In scope:

- required section checks for `llm/` docs
- markdown link/path integrity checks
- `bun run` command reference validation against `package.json`
- absolute source-anchor path existence checks

Out of scope:

- semantic correctness checks that require human review

## Source Anchors

- `/Users/plebdev/Desktop/code/dicktaint/scripts/verify-docs.sh`
- `/Users/plebdev/Desktop/code/dicktaint/package.json`
- `/Users/plebdev/Desktop/code/dicktaint/llm/STANDARDS.md`

## Runbook

Run:

```bash
bun run docs:verify
```

Pass criteria:

- script exits `0`
- output includes `Documentation verification passed.`

Failure handling:

1. fix all reported failures
2. rerun `bun run docs:verify`
3. only proceed when script passes cleanly

## Verification

This runbook is valid when `docs:verify` exists in `package.json` and maps to `./scripts/verify-docs.sh`.

## Related Docs

- [`LOCAL_DEVELOPMENT.md`](LOCAL_DEVELOPMENT.md)
- [`../STANDARDS.md`](../STANDARDS.md)
