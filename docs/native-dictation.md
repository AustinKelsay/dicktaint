# Native Desktop Dictation (Tauri)

This document explains the desktop dictation path end-to-end.

Important: this native Whisper CLI path is desktop-only. Mobile runtime does not invoke these desktop-native commands.
Current MVP focus: macOS desktop + iPhone (iOS).

Companion reference for exact route/command payloads:
- [`docs/api-surface.md`](api-surface.md)

## Architecture

Desktop mode uses a native speech path:

1. Rust captures microphone audio with `cpal`.
2. Rust writes a temporary mono 16 kHz WAV.
3. Rust runs `whisper-cli` (from `whisper.cpp`) to transcribe audio.
4. Transcript is returned to the frontend.

Model onboarding path:

1. App checks whether `whisper-cli` is available on this machine.
2. App profiles local hardware (RAM + CPU basics).
3. App shows the full Whisper model list with fit labels and one best-fit recommendation.
4. User downloads one model locally for this device.
5. Selected model is persisted in `$HOME/.dicktaint/dictation-settings.json`.
6. Start Dictation stays blocked until both `whisper-cli` and a local model are ready.

## Native command contract (desktop invoke bridge)

Current frontend flow in `public/app.js` depends on this command sequence:

1. `get_dictation_onboarding`
2. `install_dictation_model` / `delete_dictation_model` (as needed)
3. `start_native_dictation`
4. `stop_native_dictation` (or `cancel_native_dictation`)

Command summary:

- `get_dictation_onboarding`:
  - returns `whisper_cli_available`, `whisper_cli_path`, `selected_model_*`, `device`, and full model list with `recommended` and `likely_runnable` flags.
- `install_dictation_model`:
  - validates model id against the internal catalog, verifies CLI health, downloads from Hugging Face if missing, then persists selection.
- `delete_dictation_model`:
  - removes the model file and switches selection to the best remaining installed model when available.
- `start_native_dictation`:
  - validates active model + CLI readiness and starts audio capture thread.
- `stop_native_dictation`:
  - stops capture, transcribes with `whisper-cli`, returns cleaned transcript text.
- `cancel_native_dictation`:
  - force-stops recording without transcription.

Related event channels used during desktop runtime:

- backend -> frontend: `dicktaint://fn-state` (`{ pressed: boolean }`)
- frontend -> overlay windows: `dicktaint://pill-status` (`{ message, state, visible }`)

## Model catalog and recommendation behavior

The built-in model catalog currently contains 12 entries:

- tiny-en
- tiny
- base-en
- base
- small-en
- small
- medium-en
- medium
- large-v1
- large-v2
- large-v3
- turbo

Recommendation logic is memory-driven:

- A model is marked `likely_runnable` when machine RAM is at or above its `min_ram_gb`.
- A model is marked `recommended` when it is the highest ranked runnable model for that machine.
- Ranking prefers:
  - better fit level (`recommended_ram_gb` threshold first, then `min_ram_gb`)
  - higher `recommended_ram_gb`
  - larger model size as final tie-break

This intentionally biases toward the strongest model likely to run on the current machine, not the smallest model.

## Requirements

- macOS desktop with microphone access enabled for the app process.
- `whisper-cli` installed and executable (for `tauri:dev`), or bundled as a sidecar in packaged builds.

Note: non-macOS desktop targets are currently de-prioritized in this MVP.

## Bundled CLI Strategy

Packaged desktop builds are configured to ship `whisper-cli` as an external sidecar binary.

- Config: `src-tauri/tauri.conf.json` `bundle.externalBin`
- Sidecar placement: `src-tauri/binaries/` (see `src-tauri/binaries/README.md`)

Runtime path resolution order:

1. `WHISPER_CLI_PATH` override (if set)
2. Bundled sidecar path
3. `whisper-cli` from system `PATH`
4. Local `src-tauri/binaries` sidecar candidates in `tauri:dev`

Additional candidate probing (platform-specific) is built in for common install paths, for example:

- macOS: `/opt/homebrew/bin/whisper-cli`, `/usr/local/bin/whisper-cli`
- Linux: `/usr/local/bin/whisper-cli`, `/usr/bin/whisper-cli`
- Windows: `C:\Program Files\whisper.cpp\whisper-cli.exe`

## Primary Shipping Flow

For packaged desktop users, first-run should look like this:

1. App starts with bundled `whisper-cli` already present.
2. Onboarding profiles device hardware and shows the full model list.
3. App marks one recommended model for the machine.
4. User downloads that model locally and starts dictating.

## Prepare whisper-cli (`tauri:dev` only)

Preferred dev path (build/update local sidecar):

```bash
bun run whisper:sidecar
```

Optional sidecar smoke test:

```bash
bun run whisper:smoke
```

Fallback option (system install):

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

1. Open the `Speech-to-Text Setup (Whisper)` panel.
2. Review the device profile and fit notes.
3. If `whisper-cli` is missing, click `Open CLI Setup (dev)` and then `Refresh Setup`.
4. Click `Download + Use` on the recommended model (or another model you want).
5. Wait for the model to finish downloading locally.
6. Use `Delete Local Model` whenever you want to remove a local model file.

In packaged desktop builds, `whisper-cli` should already be bundled as a sidecar. In `tauri:dev`, it comes from your local install or `WHISPER_CLI_PATH`.

## Local persistence layout

Desktop onboarding/model state is local-first and persisted under app data:

- settings file:
  - `$HOME/.dicktaint/dictation-settings.json`
- downloaded models:
  - `$HOME/.dicktaint/whisper-models/`

Selected model state stores:

- `selected_model_id`
- `selected_model_path`

Settings writes are atomic (temp file + rename) to reduce corruption risk on interruption.

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
- In `tauri:dev`, run `bun run whisper:sidecar` (preferred), or install `whisper-cpp`, or set `WHISPER_CLI_PATH`.
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

`Model download completed but file is still missing ...`
- Destination path was not persisted after download.
- Check local filesystem permissions and available disk space.

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
