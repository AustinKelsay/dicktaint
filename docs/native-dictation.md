# Native Desktop Dictation (Tauri)

This document explains the desktop dictation path end-to-end.

Important: this native Whisper CLI path is desktop-only. Mobile runtime currently uses the HTTP `/api/*` flow and does not invoke these desktop-native commands.

## Architecture

Desktop mode uses a native speech path:

1. Rust captures microphone audio with `cpal`.
2. Rust writes a temporary mono 16 kHz WAV.
3. Rust runs `whisper-cli` (from `whisper.cpp`) to transcribe audio.
4. Transcript is returned to the frontend.
5. Ollama is used after that to refine/clean text.

Important: Ollama is not doing speech-to-text in this desktop flow.

Model onboarding path:

1. App checks whether `whisper-cli` is available on this machine.
2. App profiles local hardware (RAM + CPU basics).
3. App shows the full Whisper model list with fit labels and one best-fit recommendation.
4. User downloads one model locally for this device.
5. Selected model is persisted in `$HOME/.dicktaint/dictation-settings.json`.
6. Start Dictation stays blocked until both `whisper-cli` and a local model are ready.

## Requirements

- Desktop OS with microphone access enabled for the app process.
- `whisper-cli` installed and executable (for `tauri:dev`), or bundled as a sidecar in packaged builds.
- Ollama running for refinement (`/api/tags` and `/api/generate`).

## Bundled CLI Strategy

Packaged desktop builds are configured to ship `whisper-cli` as an external sidecar binary.

- Config: `src-tauri/tauri.conf.json` `bundle.externalBin`
- Sidecar placement: `src-tauri/binaries/` (see `src-tauri/binaries/README.md`)

Runtime path resolution order:

1. `WHISPER_CLI_PATH` override (if set)
2. Bundled sidecar path
3. `whisper-cli` from system `PATH`

## Primary Shipping Flow

For packaged desktop users, first-run should look like this:

1. App starts with bundled `whisper-cli` already present.
2. Onboarding profiles device hardware and shows the full model list.
3. App marks one recommended model for the machine.
4. User downloads that model locally and starts dictating.

## Install whisper-cli (`tauri:dev` only)

Homebrew:

```bash
brew install whisper-cpp
which whisper-cli
```

Expected result:

```text
/opt/homebrew/bin/whisper-cli
```

## Choose and Download a Model

In desktop mode, use onboarding in the app:

1. Open the `Dictation Setup (whisper.cpp)` panel.
2. Review the device profile and fit notes.
3. If `whisper-cli` is missing, click `Get whisper-cli (dev)` and then `Retry Check`.
4. Click `Download + Use` on the recommended model (or another model you want).
5. Wait for the model to finish downloading locally.

In packaged desktop builds, `whisper-cli` should already be bundled as a sidecar. In `tauri:dev`, it comes from your local install or `WHISPER_CLI_PATH`.

## Run Desktop Dictation

```bash
bun run tauri:dev
```

If `whisper-cli` is not on `PATH`:

```bash
WHISPER_CLI_PATH="/absolute/path/to/whisper-cli" bun run tauri:dev
```

Optional hard override (bypasses onboarding selection):

```bash
WHISPER_MODEL_PATH="/absolute/path/to/ggml-base.en.bin" bun run tauri:dev
```

## First Smoke Test

1. Open desktop app in `tauri:dev`.
2. Click `Start Dictation`.
3. Speak for 3-5 seconds.
4. Click `Stop`.
5. Verify transcript appears in `Transcript`.
6. Click `Polish with Ollama` to run Ollama cleanup.

## Fast Functional Test Model

Homebrew `whisper-cpp` includes a tiny test model:

```text
/opt/homebrew/opt/whisper-cpp/share/whisper-cpp/for-tests-ggml-tiny.bin
```

You can use this to validate pipeline wiring quickly. Accuracy is low.

## Troubleshooting

`No local dictation model selected yet`
- Complete onboarding and download/select a model in the app.

`Start Dictation` stays disabled
- Verify `whisper-cli` availability in onboarding.
- In `tauri:dev`, install `whisper-cpp` or set `WHISPER_CLI_PATH`.
- Download/select a local model in onboarding.

`Could not download whisper model ...`
- Check internet access and retry.
- Verify the destination folder is writable.

`Could not execute 'whisper-cli'`
- Install `whisper-cpp` or set `WHISPER_CLI_PATH`.

`No microphone input device found`
- Check `System Settings > Sound > Input` and pick a valid input device.

`Failed to start microphone stream`
- Mic permissions likely blocked, or input device is unavailable.

`No audio captured` or `No speech detected`
- Speak longer/louder, check selected input device and input level.

`whisper-cli transcription failed: ...`
- Verify model file is valid, command path is correct, and CLI can run manually.

## Manual CLI Verification

If you need to verify CLI independently:

```bash
whisper-cli --help
```

And with a known sample WAV:

```bash
whisper-cli \
  -m "$HOME/.dicktaint/whisper-models/ggml-base.en.bin" \
  -f /opt/homebrew/opt/whisper-cpp/share/whisper-cpp/jfk.wav \
  -l en \
  -otxt \
  -nt
```

## Notes on Language

Current implementation forces English transcription (`-l en` in Rust command invocation).

- Use `.en` models for best results with this setup.
- Multilingual transcription support would require code changes.
