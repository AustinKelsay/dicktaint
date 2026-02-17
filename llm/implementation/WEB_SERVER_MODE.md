# Web Server Mode

## Status Snapshot

- Date: 2026-02-17
- Server mode: static + SPA fallback + disabled API routes

## Purpose

Document exact web server behavior and guarantees.

## Scope

In scope:

- path safety and routing behavior
- content type mapping
- fallback behavior
- test coverage expectations

Out of scope:

- browser-side dictation state machine

## Source Anchors

- `/Users/plebdev/Desktop/code/dicktaint/server.js`
- `/Users/plebdev/Desktop/code/dicktaint/tests/server.test.js`

## Contract

Core behavior:

1. reject missing URL with `400 Bad Request`
2. return disabled API response for `/api/*`
3. serve `public/index.html` at `/`
4. resolve non-root paths via `safePublicPath()`
5. reject invalid traversal path as `400`
6. serve file if found
7. on miss:
  - SPA fallback for navigation-like request
  - plain `404 Not Found` for missing asset request

Content type mappings:

- `.html`, `.css`, `.js`, `.json`, `.svg`, `.png`, `.jpg`, `.jpeg`
- default fallback `text/plain; charset=utf-8`

Invariants:

- `/api/*` remains disabled until explicitly changed
- traversal outside `public/` is blocked

## Verification

Run after server behavior edits:

1. `bun run test`
2. manual checks:
  - `/api/health` returns disabled payload
  - `/some/fake/path` returns SPA shell
  - `/missing.js` returns `404`

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`../workflow/SMOKE_TESTS.md`](../workflow/SMOKE_TESTS.md)
