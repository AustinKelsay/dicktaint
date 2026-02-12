# dicktaint
A local AI dictation tool suitable for the most private chats and dirtiest language.

## Quick start (web mode)

1. Install [Bun](https://bun.sh/).
2. Install and run [Ollama](https://ollama.com/) for cleanup/refine.
3. Pull at least one cleanup model, for example:
   ```bash
   ollama pull llama3.2:3b
   ```
4. Install JS dependencies:
   ```bash
   bun install
   ```
5. Start web mode:
   ```bash
   bun run start
   ```
6. Open [http://localhost:3000](http://localhost:3000)

## Optional dev mode

```bash
bun run dev
```

## Desktop quick start (local-first)

1. Run desktop dev mode:
   ```bash
   bun run tauri:dev
   ```
2. In the app, open `Dictation Setup (whisper.cpp)`.
3. Confirm `whisper-cli` availability and download one local Whisper model for this device.
4. Click `Start Dictation` once setup is marked ready.

If `whisper-cli` is missing in `tauri:dev`, install it:

```bash
brew install whisper-cpp
which whisper-cli
```

## Testing

```bash
bun run test
bun run test:rust
bun run test:all
```

## Desktop (Tauri)

This repo now includes a Tauri v2 boilerplate in `/src-tauri`.

Run desktop dev mode:

```bash
bun run tauri:dev
```

Notes:
- Tauri dev launches your web server automatically on `http://localhost:43210` and opens a native desktop window.
- In desktop mode, the frontend calls Rust Tauri commands for both dictation capture/transcription and Ollama refinement.
- Ollama host defaults to `http://127.0.0.1:11434`.
- Desktop dictation capture/transcription is native (Rust): microphone audio is captured with `cpal` and transcribed locally by invoking `whisper-cli`.
- Desktop onboarding is local-first and model-first: verify `whisper-cli`, inspect hardware, then download/select one local Whisper model per device.
- Packaged desktop builds are expected to provide `whisper-cli` as a bundled sidecar.
- `tauri:dev` resolves `whisper-cli` from `WHISPER_CLI_PATH` or system `PATH`.
- Onboarding marks one best-fit recommended model for the current machine (and still shows the full model list).
- Dictation start is blocked until both prerequisites are met on that device: `whisper-cli` present and a local model selected.
- Selected dictation model state is saved at `$HOME/.dicktaint/dictation-settings.json`, and model files are stored under `$HOME/.dicktaint/whisper-models/`.
- Desktop bundle config uses a `whisper-cli` sidecar (`src-tauri/tauri.conf.json` `externalBin`) so packaged app users do not need a separate CLI install.
- If `WHISPER_MODEL_PATH` is set, it overrides onboarding selection for desktop dictation.
- Full setup and troubleshooting guide: [`docs/native-dictation.md`](docs/native-dictation.md).
- Override host for desktop runs with:
  ```bash
  OLLAMA_HOST=http://127.0.0.1:11434 bun run tauri:dev
  ```
  Optional legacy override with an explicit model path:
  ```bash
  WHISPER_MODEL_PATH=/absolute/path/to/ggml-base.en.bin bun run tauri:dev
  ```
  If `whisper-cli` is not on your `PATH` in `tauri:dev`, also set:
  ```bash
  WHISPER_CLI_PATH=/absolute/path/to/whisper-cli bun run tauri:dev
  ```

Desktop build (currently configured for compile checks, bundling disabled):

```bash
bun run tauri:build
```

To actually ship sidecar binaries, place platform builds in:

`src-tauri/binaries/` (see `src-tauri/binaries/README.md`).

## Mobile (Tauri iOS + Android)

This repo now includes mobile scripts:

```bash
bun run tauri:android:init
bun run tauri:android:dev
bun run tauri:android:run
bun run tauri:android:build

bun run tauri:ios:init
bun run tauri:ios:dev
bun run tauri:ios:run
bun run tauri:ios:build
```

### Prerequisites

Android:
- Install Android Studio with SDK + NDK.
- Set `ANDROID_HOME` and `NDK_HOME`.
- Make sure `adb` is on your `PATH`.

iOS (macOS only):
- Install full Xcode app (not just CLT), open it once, accept license.
- Install CocoaPods + xcodegen (`brew install cocoapods xcodegen`).
- Have an Apple development team ID (`APPLE_DEVELOPMENT_TEAM`) for device signing.

Helpful env setup example (zsh):

```bash
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/<your-ndk-version>"
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
```

### Android Smoke Test (physical device)

1. Enable USB debugging on your Android device.
2. Confirm device is visible:
   ```bash
   adb devices
   ```
3. Initialize Android project once:
   ```bash
   bun run tauri:android:init
   ```
4. Find your Mac LAN IP:
   ```bash
   ipconfig getifaddr en0
   ```
5. Run dev on device:
   ```bash
   TAURI_DEV_HOST=<your-lan-ip> bun run tauri:android:dev
   ```
6. Smoke test on-device:
   - Type/paste transcript text manually (or use browser speech if available in your runtime).
   - Click `Polish with Ollama`.
   - Confirm the cleanup call succeeds.

### iOS Smoke Test (physical device)

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
   - Click `Polish with Ollama`.
   - Confirm the cleanup call succeeds.

Notes:
- Mobile dev binds the local server to `0.0.0.0` so phones can access the dev URL.
- Mobile runtime intentionally uses the HTTP `/api/*` path (not desktop-only Tauri native Whisper commands).
- If you only want a production smoke test build, use `bun run tauri:android:run` or `bun run tauri:ios:run`.
- If `android init` says SDK/NDK not found or `ios init` says Xcode/xcodegen missing, install prerequisites above first.

## What this starter does

- Lists your local Ollama models (with selector).
- Prefers `llama3.2:3b` by default when available for text refinement.
- Provides a basic dictation flow: start dictation, stop, edit transcript, and polish with Ollama.
- Returns cleaned dictation output.
- Web mode dictation uses browser speech recognition when available.
- Desktop mode dictation uses native Rust audio capture + `whisper-cli`, with onboarding that recommends and installs local Whisper models per device.
- Mobile mode currently does not use native Whisper CLI dictation; it uses manual text input or runtime speech API support.
- If live speech capture is unavailable in your runtime, you can still paste/type transcript text.

## Config

- `PORT` (default `3000`)
- `HOST` (default `127.0.0.1`; use `0.0.0.0` for mobile dev on physical devices)
- `OLLAMA_HOST` (default `http://127.0.0.1:11434`)
- `WHISPER_CLI_PATH` (desktop dictation only; optional override for `whisper-cli` executable path)
- `WHISPER_MODEL_PATH` (desktop dictation only; optional hard override path that bypasses onboarding selection)

Example:

```bash
HOST=127.0.0.1 PORT=3001 OLLAMA_HOST=http://127.0.0.1:11434 bun run start
```
