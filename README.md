# dicktaint
A local AI dictation tool suitable for the most private chats and dirtiest language.

Current MVP focus: macOS desktop + iPhone (iOS) mobile.

## Documentation map

- Canonical docs system: [`llm/README.md`](llm/README.md)
- Context and product/runtime scope: [`llm/context/`](llm/context/)
- Implementation contracts and internals: [`llm/implementation/`](llm/implementation/)
- Dev/release/troubleshooting workflows: [`llm/workflow/`](llm/workflow/)

Legacy compatibility redirects (kept for old links):
- [`docs/api-surface.md`](docs/api-surface.md)
- [`docs/native-dictation.md`](docs/native-dictation.md)
- [`docs/background-hotkey-mvp.md`](docs/background-hotkey-mvp.md)
- [`docs/macos-private-api-checklist.md`](docs/macos-private-api-checklist.md)

## Quick start (web mode)

1. Install [Bun](https://bun.sh/).
2. Install JS dependencies:
   ```bash
   bun install
   ```
3. Start web mode:
   ```bash
   bun run start
   ```
4. Open [http://localhost:3000](http://localhost:3000)

## Optional dev mode

```bash
bun run dev
```

## Desktop quick start (local-first)

Desktop MVP currently targets macOS.
Rust toolchain requirement for desktop (`src-tauri`): `rustc >= 1.77.2`.

1. Build/update the local sidecar binary (dev helper):
   ```bash
   bun run whisper:sidecar
   ```
2. Run desktop dev mode:
   ```bash
   bun run tauri:dev
   ```
3. In the setup screen, wait for local checks to finish.
4. Choose a model and click `Download + Use`.
5. Click `Start Dictation` once status shows ready.

Optional smoke test for sidecar + model pipeline:

```bash
bun run whisper:smoke
```

## Testing

```bash
bun run test
bun run test:rust
bun run test:all
```

Coverage snapshot:
- `bun run test`: web/static server behavior (`/api/*` disabled contract, SPA fallback, content-type/path safety helpers).
- `bun run test:rust`: native audio + transcript normalization helpers used by Tauri commands.

## Desktop (Tauri)

This repo now includes a Tauri v2 boilerplate in `/src-tauri`.

Run desktop dev mode:

```bash
bun run tauri:dev
```

Notes:
- Tauri dev launches your web server automatically on `http://localhost:43210` and opens a native desktop window.
- In desktop mode, the frontend calls Rust Tauri commands for local dictation capture/transcription and model management.
- Desktop dictation capture/transcription is native (Rust): microphone audio is captured with `cpal` and transcribed locally by invoking `whisper-cli`.
- MVP background behavior: closing the desktop window now hides it instead of quitting, so dictation state can stay running in the background process.
- App launches visible by default. Optional: set `DICKTAINT_START_HIDDEN=1` if you want hidden startup behavior.
- MVP hold-to-talk hotkey (macOS): hold `fn` (or fallback `F19`) to record, then release to stop and transcribe, even while the app window is hidden.
- Global hotkeys are now executed in the Rust backend instead of relying on the main window webview, so hidden/background dictation keeps working even when the renderer is suspended.
- The onboarding/settings flow now shows per-machine setup + permission guidance (microphone, Input Monitoring, Accessibility) so Intel and Apple silicon Macs surface the same runtime expectations.
- The settings screen groups models, hotkeys, insertion, and permissions into separate sections so the same options are easier to scan and maintain.
- A small bottom-center hotkey pill now renders as a native macOS transparent overlay window (outside the main app window), with an extra-compact icon-only footprint and quick dictation state feedback that follows the active hotkey mode (`Fn` hold vs custom toggle shortcut).
- The macOS `Fn` listener now recovers more gracefully if the system temporarily disables its event tap, which reduces the “Fn stopped working until relaunch” failure mode.
- If hold-to-talk hotkey capture does not fire, allow Input Monitoring/Accessibility for the app (or Terminal during `tauri:dev`) and relaunch.
- Desktop onboarding is local-first and model-first: verify `whisper-cli`, inspect hardware, then download/select one local Whisper model per device.
- Packaged desktop builds are expected to provide `whisper-cli` as a bundled sidecar.
- `tauri:dev` resolves `whisper-cli` from sidecar candidates, `WHISPER_CLI_PATH`, or system `PATH`.
- Onboarding marks one best-fit recommended model for the current machine (and still shows the full model list).
- Dictation start is blocked until both prerequisites are met on that device: `whisper-cli` present and a local model selected.
- Selected dictation model state and optional dictation hotkey are saved at `$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/dictation-settings.json`, and model files are stored under `$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/whisper-models/`.
- The saved dictation hotkey is registered as a desktop global shortcut (system-wide while app is running). On macOS, `Fn` uses a native global listener when permitted; if blocked by permissions it falls back to in-app handling.
- Dictation state events now include a backend session id so a completed older transcript cannot incorrectly clear a newer live recording in the UI.
- Focused-field insertion now restores prior clipboard text through a safer path to avoid pasteboard crashes after a successful paste.
- Desktop bundle config uses a `whisper-cli` sidecar (`src-tauri/tauri.conf.json` `externalBin`) so packaged app users do not need a separate CLI install.
- In setup UI, use `Refresh Setup` to re-run checks and `Delete Local Model` to remove a downloaded model file.
- If `WHISPER_MODEL_PATH` is set, it overrides onboarding selection for desktop dictation.
- Full setup and troubleshooting guide: [`llm/README.md`](llm/README.md), [`llm/workflow/TROUBLESHOOTING.md`](llm/workflow/TROUBLESHOOTING.md).
- Background + `fn` hotkey MVP implementation notes: [`llm/implementation/HOTKEY_AND_OVERLAY.md`](llm/implementation/HOTKEY_AND_OVERLAY.md).
- Optional legacy override with an explicit model path:
  ```bash
  WHISPER_MODEL_PATH=/absolute/path/to/ggml-base.en.bin bun run tauri:dev
  ```
  If `whisper-cli` is not on your `PATH` in `tauri:dev`, also set:
  ```bash
  WHISPER_CLI_PATH=/absolute/path/to/whisper-cli bun run tauri:dev
  ```

Hotkey setup (desktop):
- Open `Settings` in the dictation screen.
- In `Dictation Hotkey`, choose a preset, click `Record`, or type your own combo, then click `Save Hotkey`.
- Optional: enable `Dictate Into Focused Field` to paste completed transcripts into the currently focused field in the frontmost app (macOS desktop).
- Focused-field insertion runs when dictation finalizes while another app is focused; the transcript box inside dicktaint is still updated.
- If paste fails, allow Accessibility for the app (or Terminal during `tauri:dev`) and retry. dicktaint now opens the Accessibility settings page automatically when that permission is missing.
- The saved combo is registered as a global hotkey while the desktop app is running. On macOS, `Fn` is global when Input Monitoring permissions allow it, and otherwise the UI now calls out that it has fallen back to focused-window behavior until that permission is granted.
- `Reset Default` sets `Fn` on macOS and `CmdOrCtrl+Shift+D` on other desktop platforms.
- `Disable Hotkey` removes the shortcut.
- Hotkey config is per-device and persisted in `$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/dictation-settings.json`.

Desktop build (local/manual):

```bash
bun run tauri:build
```

To ship desktop builds, place platform sidecar binaries in:

`src-tauri/binaries/` (see `src-tauri/binaries/README.md`).

## Simple GitHub release process (macOS)

This repo now includes a GitHub Actions workflow at `.github/workflows/release-macos.yml`.

What it does:
- runs on tag push (`v*`)
- builds notarized Tauri macOS bundles for Apple Silicon and Intel, with `.dmg` kept on Apple Silicon and `.app.tar.gz` published for both architectures
- passes Apple signing/notarization credentials through to the Tauri build
- validates the built `.app` signature before publishing artifacts
- creates a GitHub Release and uploads artifacts + `SHA256SUMS.txt`

How to release:

1. Make sure `main` is green and pushed.
2. Make sure GitHub Actions has the Apple signing secrets configured:
   - if they are stored as environment secrets, the release job must target that environment
   - `APPLE_CERTIFICATE`
   - `APPLE_CERTIFICATE_PASSWORD`
   - either `APPLE_ID` + `APPLE_PASSWORD` + `APPLE_TEAM_ID`, or `APPLE_API_KEY` + `APPLE_API_ISSUER` + `APPLE_API_KEY_CONTENT`
3. Bump the version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
4. Add or update the top changelog entry in `CHANGELOG.md`, including the private API disclosure if overlay behavior still depends on it.
5. Create and push a version tag:
   ```bash
   git checkout main
   git pull
   git tag v0.1.12
   git push origin v0.1.12
   ```
6. Wait for the `Release macOS App` workflow to finish.
7. Open GitHub Releases, verify the generated notes/artifacts, and share the uploaded `.dmg` with users.

Important:
- the workflow now fails instead of publishing ad hoc macOS apps when signing/notarization inputs are missing or the built bundle fails `codesign --verify --deep --strict`
- Intel release automation currently publishes a notarized `.app.tar.gz` instead of a `.dmg` because the GitHub Intel runner DMG bundling path is still unstable

User install flow:
- download `.dmg` from Releases
- open and drag `dicktaint.app` to Applications
- launch the app

Current limitation:
- this automated release is macOS-first (Apple Silicon + Intel). Non-macOS sidecar binaries in this repo are still placeholders.

## Mobile (Tauri iOS / iPhone)

Current mobile MVP targets iPhone (iOS).

Available mobile scripts:

```bash
bun run tauri:ios:init
bun run tauri:ios:dev
bun run tauri:ios:run
bun run tauri:ios:build
```

### Prerequisites (iOS)

- Install full Xcode app (not just CLT), open it once, accept license.
- Install CocoaPods + xcodegen (`brew install cocoapods xcodegen`).
- Have an Apple development team ID (`APPLE_DEVELOPMENT_TEAM`) for device signing.

### iPhone Smoke Test (physical device)

1. Initialize iOS project once:
   ```bash
   bun run tauri:ios:init
   ```
2. Set your Apple team ID and LAN IP:
   ```bash
   APPLE_DEVELOPMENT_TEAM=<your-team-id> TAURI_DEV_HOST=<your-lan-ip> bun run tauri:ios:dev
   ```
3. Smoke test on-device:
   - Type/paste transcript text manually (or use browser speech if available in your runtime).
   - Confirm transcript capture starts/stops correctly.
   - Confirm text appears in the transcript box.

Notes:
- Mobile dev binds the local server to `0.0.0.0` so phones can access the dev URL.
- Mobile runtime does not use desktop-only native Whisper commands.
- If you only want a production smoke test build, use `bun run tauri:ios:run`.
- If `ios init` says Xcode/xcodegen missing, install prerequisites above first.
- Android support is deferred for now while MVP stays macOS + iPhone focused.

## What this starter does

- Provides a local model management flow: pull, select, and delete Whisper models per device.
- Provides a basic dictation flow: start dictation, stop, and edit transcript.
- In desktop mode, allows configuring a dictation hotkey (default `Fn` on macOS, `CmdOrCtrl+Shift+D` elsewhere) to toggle start/stop dictation.
- Web mode dictation uses browser speech recognition when available.
- Desktop mode dictation uses native Rust audio capture + `whisper-cli`, with onboarding that recommends and installs local Whisper models per device.
- Mobile mode currently does not use native Whisper CLI dictation; it uses manual text input or runtime speech API support.
- Current platform focus is macOS desktop + iPhone (iOS). Non-macOS desktop targets are intentionally de-prioritized in this MVP.
- If live speech capture is unavailable in your runtime, you can still paste/type transcript text.

## HTTP server contract (web mode)

`server.js` is intentionally static + SPA-only for this MVP:

- `GET /api/*` always returns `404` JSON:
  - `{ "ok": false, "error": "No API routes are enabled in dictation-only mode." }`
- Static assets are served from `public/`.
- Unknown navigation-like routes (`Accept: text/html` or extensionless path) fall back to `public/index.html`.
- Missing asset paths with file extensions return plain `404 Not Found`.

## Config

- `PORT` (default `3000`)
- `HOST` (default `127.0.0.1`; use `0.0.0.0` for iPhone dev on physical devices)
- `WHISPER_CLI_PATH` (desktop dictation only; optional override for `whisper-cli` executable path)
- `WHISPER_MODEL_PATH` (desktop dictation only; optional hard override path that bypasses onboarding selection)

Desktop path resolution order:
- `WHISPER_CLI_PATH` override
- bundled sidecar binary
- `whisper-cli` in system `PATH`
- local dev sidecar candidates under `src-tauri/binaries/`

Local storage paths (desktop):
- settings: `$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/dictation-settings.json`
- models: `$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/whisper-models/`

Example:

```bash
HOST=127.0.0.1 PORT=3001 bun run start
```
