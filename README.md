# Murmur

Private dictation for your Mac.

*A little voice. A lot less typing.*

Current MVP focus: **macOS desktop**. iPhone/iOS is a future track.

## Documentation map

Canonical docs live under [`llm/`](llm/README.md):

- Context: [`llm/context/`](llm/context/)
- Implementation contracts: [`llm/implementation/`](llm/implementation/)
- Dev / release / troubleshooting: [`llm/workflow/`](llm/workflow/)
- Glossary + ADRs: [`CONTEXT.md`](CONTEXT.md), [`docs/adr/`](docs/adr/)
- Murmur rebuild evidence and remaining release gates: [`docs/murmur-verification.md`](docs/murmur-verification.md)

## Quick start (web mode)

```bash
bun install
bun run start
```

Open [http://localhost:3000](http://localhost:3000). Use `bun run dev` for watch mode.

## Desktop quick start (macOS)

Rust toolchain: `rustc >= 1.77.2`.

```bash
bun run whisper:sidecar
bun run tauri:dev
```

Complete setup by checking the local speech engine, installing or selecting a model, and allowing microphone access. A ready Mac opens directly into Dictation. Accessibility (paste into other apps) and Input Monitoring (an alternative background Fn path) are optional; button dictation works without them.

Optional pipeline smoke test:

```bash
bun run whisper:smoke
```

Closing the main window hides to tray by default (dictation can keep running). Hold `Fn` (or fallback `F19`) for hold-to-talk. Background delivery can use Accessibility or Input Monitoring; the app reports the actual hotkey capability separately from each permission. Details: [`llm/implementation/HOTKEY_AND_OVERLAY.md`](llm/implementation/HOTKEY_AND_OVERLAY.md).

## Testing

```bash
bun run test
bun run test:rust
bun run test:all
bun run docs:verify
bun run verify:bundle # after building/signing a local app
```

## Release (macOS)

Tag-driven GitHub Actions build/notarize lives in `.github/workflows/release-macos.yml`. Full steps: [`llm/workflow/RELEASE_AND_DISTRIBUTION.md`](llm/workflow/RELEASE_AND_DISTRIBUTION.md).

For a local verification build, follow the build/sign/inspect commands in the release runbook; ad hoc signing is local verification only. Public distribution still requires Developer ID signing, notarization, stapling, and clean-machine checks.

Bump versions together in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`, update `CHANGELOG.md`, then push a `v*` tag from `main`.

## Mobile (iPhone / iOS)

```bash
bun run tauri:ios:init
APPLE_DEVELOPMENT_TEAM=<team-id> TAURI_DEV_HOST=<lan-ip> bun run tauri:ios:dev
```

Requires Xcode. Android is deferred. See [`llm/context/PLATFORM_SUPPORT.md`](llm/context/PLATFORM_SUPPORT.md).

## What it does

- Local Whisper model download / select / delete per device
- Native desktop dictation via Rust capture + bundled `whisper-cli`
- Configurable global hotkey, overlay pill, focused-field insert (macOS)
- Editable draft, copy actions, and recent dictations kept in memory for the current session
- Web fallback via browser speech recognition when available; the browser provider may process audio remotely

## HTTP server contract (web mode)

`server.js` is static + SPA-only:

- `GET /api/*` → `404` JSON (`No API routes are enabled in dictation-only mode.`)
- Assets from `public/`; HTML navigations fall back to `index.html`

## Config

| Variable | Default / notes |
| --- | --- |
| `PORT` | `3000` |
| `HOST` | `127.0.0.1` (`0.0.0.0` for device LAN access) |
| `WHISPER_CLI_PATH` | Optional desktop CLI override |
| `WHISPER_MODEL_PATH` | Optional model path override (bypasses onboarding selection) |
| `DICKTAINT_START_HIDDEN` | `1` to start hidden |

Desktop CLI resolution: env override → bundled sidecar → `PATH` → local `src-tauri/binaries/` candidates.

**Sidecar policy:** which `whisper-cli-*` binaries are real vs placeholder is documented in [`src-tauri/binaries/README.md`](src-tauri/binaries/README.md). Placeholders must not ship as working transcription binaries.

**Vendored `cpal`:** macOS CoreAudio retain-cycle / idle-mic leak fix — path-patched via `src-tauri/Cargo.toml` → [`vendor/cpal`](vendor/cpal). Rationale: [`vendor/cpal/VENDOR.md`](vendor/cpal/VENDOR.md).

Settings / models on macOS live under:

`$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/`

Murmur keeps the original bundle identifier, internal executable name, settings format, storage path, and environment variables so existing models and preferences continue to work. See [ADR 0002](docs/adr/0002-murmur-continuity.md) for the upgrade continuity contract.
