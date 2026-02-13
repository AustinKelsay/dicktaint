# dicktaint
A local AI dictation tool suitable for the most private chats and dirtiest language.

Current MVP focus: macOS desktop + iPhone (iOS) mobile.

## Documentation map

- Runtime/API contract: [`docs/api-surface.md`](docs/api-surface.md)
- Native desktop speech pipeline: [`docs/native-dictation.md`](docs/native-dictation.md)
- Background runtime + `fn` hold-to-talk MVP: [`docs/background-hotkey-mvp.md`](docs/background-hotkey-mvp.md)

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

1. Build/update the local sidecar binary (dev helper):
   ```bash
   bun run whisper:sidecar
   ```
2. Run desktop dev mode:
   ```bash
   bun run tauri:dev
   ```
3. In `Speech-to-Text Setup (Whisper)`, wait for setup checks to finish.
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
- A small bottom-center hotkey pill now renders as a native macOS transparent overlay window (outside the main app window), with rounded edges and quick dictation state feedback (ready/listening/transcribing/error) on active screens.
- If hold-to-talk hotkey capture does not fire, allow Input Monitoring/Accessibility for the app (or Terminal during `tauri:dev`) and relaunch.
- Desktop onboarding is local-first and model-first: verify `whisper-cli`, inspect hardware, then download/select one local Whisper model per device.
- Packaged desktop builds are expected to provide `whisper-cli` as a bundled sidecar.
- `tauri:dev` resolves `whisper-cli` from sidecar candidates, `WHISPER_CLI_PATH`, or system `PATH`.
- Onboarding marks one best-fit recommended model for the current machine (and still shows the full model list).
- Dictation start is blocked until both prerequisites are met on that device: `whisper-cli` present and a local model selected.
- Selected dictation model state is saved at `$HOME/.dicktaint/dictation-settings.json`, and model files are stored under `$HOME/.dicktaint/whisper-models/`.
- Desktop bundle config uses a `whisper-cli` sidecar (`src-tauri/tauri.conf.json` `externalBin`) so packaged app users do not need a separate CLI install.
- In setup UI, use `Refresh Setup` to re-run checks and `Delete Local Model` to remove a downloaded model file.
- If `WHISPER_MODEL_PATH` is set, it overrides onboarding selection for desktop dictation.
- Full setup and troubleshooting guide: [`docs/native-dictation.md`](docs/native-dictation.md).
- Background + `fn` hotkey MVP implementation notes: [`docs/background-hotkey-mvp.md`](docs/background-hotkey-mvp.md).
- Optional legacy override with an explicit model path:
  ```bash
  WHISPER_MODEL_PATH=/absolute/path/to/ggml-base.en.bin bun run tauri:dev
  ```
  If `whisper-cli` is not on your `PATH` in `tauri:dev`, also set:
  ```bash
  WHISPER_CLI_PATH=/absolute/path/to/whisper-cli bun run tauri:dev
  ```

### Desktop command/events contract

Frontend desktop mode invokes these Tauri commands:

- `get_dictation_onboarding`: returns machine profile, model catalog, selected model state, and `whisper-cli` availability.
- `install_dictation_model`: downloads/selects a model (`{ model: string }`) and persists selection.
- `delete_dictation_model`: deletes a local model (`{ model: string }`) and auto-falls back to another installed model when possible.
- `start_native_dictation`: starts microphone capture (blocked unless model + `whisper-cli` are ready).
- `stop_native_dictation`: stops capture and returns transcript text.
- `cancel_native_dictation`: aborts active capture without transcription.
- `open_whisper_setup_page`: opens the upstream `whisper.cpp` quick-start page.

Desktop runtime events:

- `dicktaint://fn-state`: backend -> frontend event for global `fn` key pressed state (`{ pressed: boolean }`).
- `dicktaint://pill-status`: frontend -> native overlay event for hotkey status pill (`{ message, state, visible }`).

Desktop build (currently configured for compile checks, bundling disabled):

```bash
bun run tauri:build
```

To actually ship sidecar binaries, place platform builds in:

`src-tauri/binaries/` (see `src-tauri/binaries/README.md`).

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
- settings: `$HOME/.dicktaint/dictation-settings.json`
- models: `$HOME/.dicktaint/whisper-models/`

Example:

```bash
HOST=127.0.0.1 PORT=3001 bun run start
```
