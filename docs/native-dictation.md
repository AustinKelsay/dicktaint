# Native Desktop Dictation (Tauri)

This document explains the desktop dictation path end-to-end.

Important: this native Whisper CLI path is desktop-only. Mobile runtime currently uses the HTTP `/api/*` flow and does not invoke these desktop-native commands.

## Architecture

Desktop mode (`bun run tauri:dev`) uses a native speech path:

1. Rust captures microphone audio with `cpal`.
2. Rust writes a temporary mono 16 kHz WAV.
3. Rust runs `whisper-cli` (from `whisper.cpp`) to transcribe audio.
4. Transcript is returned to the frontend.
5. Ollama is used after that to refine/clean text.

Important: Ollama is not doing speech-to-text in this desktop flow.

Model onboarding path:

1. App profiles local hardware (RAM + CPU basics).
2. App shows the primary Wispr model options with likely fit labels.
3. User picks a model and clicks download in onboarding.
4. App runs Wispr CLI pull commands and stores a local model file for that device.
5. Selected model is persisted in `$HOME/.dicktaint/dictation-settings.json`.

## Requirements

- macOS with microphone access enabled for the app process.
- `whisper-cli` installed and executable.
- `wispr` CLI installed for model onboarding pulls.
- Ollama running for refinement (`/api/tags` and `/api/generate`).

## Install whisper-cli

Homebrew:

```bash
brew install whisper-cpp
which whisper-cli
```

Expected result:

```text
/opt/homebrew/bin/whisper-cli
```

## Install Wispr CLI

Install Wispr CLI using your preferred install method, then verify:

```bash
wispr --help
```

If it is not on your `PATH`, set:

```bash
WISPR_CLI_PATH=/absolute/path/to/wispr bun run tauri:dev
```

## Choose and Download a Model

In desktop mode, use onboarding in the app:

1. Open the `Dictation Model (Wispr CLI)` panel.
2. Review the device profile and fit notes.
3. Click `Download + Use` on a selected model.
4. Wait for the model to finish downloading locally.

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
6. Click `Polish With Model` to run Ollama cleanup.

## Fast Functional Test Model

Homebrew `whisper-cpp` includes a tiny test model:

```text
/opt/homebrew/opt/whisper-cpp/share/whisper-cpp/for-tests-ggml-tiny.bin
```

You can use this to validate pipeline wiring quickly. Accuracy is low.

## Troubleshooting

`No local dictation model selected yet`
- Complete onboarding and download/select a model in the app.

`Could not pull model via Wispr CLI`
- Confirm `wispr --help` works.
- If needed set `WISPR_CLI_PATH` and retry onboarding pull.

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
  -m "$HOME/.local/share/whisper-models/ggml-base.en.bin" \
  -f /opt/homebrew/opt/whisper-cpp/share/whisper-cpp/jfk.wav \
  -l en \
  -otxt \
  -nt
```

## Notes on Language

Current implementation forces English transcription (`-l en` in Rust command invocation).

- Use `.en` models for best results with this setup.
- Multilingual transcription support would require code changes.
