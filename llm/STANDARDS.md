# Documentation Standards

Status snapshot: 2026-02-17.

This file defines the required format and verification bar for docs under `/Users/plebdev/Desktop/code/dicktaint/llm`.

## Required Sections

Every document in `llm/` must include these sections in this order:

1. `Status Snapshot`
2. `Purpose`
3. `Scope`
4. `Source Anchors`
5. Main body section:
  - Context docs: `Current State`
  - Implementation docs: `Contract`
  - Workflow docs: `Runbook`
6. `Verification`
7. `Related Docs`

## Authoring Rules

- Write only the current implementation state. Do not describe aspirational behavior as shipped behavior.
- Prefer explicit command names and file paths.
- Keep platform boundaries explicit.
- Keep runtime mode differences explicit.
- If behavior changes in code, update docs in the same PR.

## Verification Rules

Before finalizing doc changes, run:

1. markdown link/path integrity check across `README.md`, `llm/**/*.md`, `docs/**/*.md`
2. command reference check against `package.json` scripts
3. existence check for referenced local source files in docs

Optional but recommended for behavior drift checks:

- `bun run test:all`

## Canonical Entry

- [`README.md`](README.md)
