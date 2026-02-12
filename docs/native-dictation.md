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

## Requirements

- macOS with microphone access enabled for the app process.
- `whisper-cli` installed and executable.
- A local Whisper GGML `.bin` model file.
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

## Choose a Whisper Model

Good defaults for English dictation:

- `ggml-base.en.bin` (recommended start)
- `ggml-small.en.bin` (better accuracy, slower)
- `ggml-tiny.en.bin` (fastest, least accurate)

Model sources:

- https://huggingface.co/ggerganov/whisper.cpp/tree/main
- https://ggml.ggerganov.com/

## Download a Model

Example (`ggml-base.en.bin`):

```bash
mkdir -p "$HOME/.local/share/whisper-models"

curl -L --fail \
  -o "$HOME/.local/share/whisper-models/ggml-base.en.bin" \
  "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin"

shasum -a 256 "$HOME/.local/share/whisper-models/ggml-base.en.bin"
```

Expected SHA256:

```text
a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002
```

## Run Desktop Dictation

```bash
WHISPER_MODEL_PATH="$HOME/.local/share/whisper-models/ggml-base.en.bin" bun run tauri:dev
```

If `whisper-cli` is not on `PATH`:

```bash
WHISPER_CLI_PATH="/absolute/path/to/whisper-cli" \
WHISPER_MODEL_PATH="$HOME/.local/share/whisper-models/ggml-base.en.bin" \
bun run tauri:dev
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

`Could not start dictation: WHISPER_MODEL_PATH is not set`
- Set `WHISPER_MODEL_PATH` to a real `.bin` model file.

`WHISPER_MODEL_PATH file not found`
- Path is wrong or file does not exist.

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
