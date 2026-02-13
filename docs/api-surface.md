# API Surface (Current MVP)

Status snapshot: implemented as of 2026-02-13.

This project exposes two runtime interfaces:

1. A minimal HTTP server used for web mode (`server.js`)
2. A Tauri invoke/event bridge used in native desktop mode (`src-tauri/src/main.rs`)

## 1) HTTP server (`server.js`)

Base URL defaults to `http://127.0.0.1:3000` in web mode.

### `GET /api/*`

- Always disabled in this dictation-only MVP.
- Response:
  - status: `404`
  - content-type: `application/json`
  - body:

```json
{
  "ok": false,
  "error": "No API routes are enabled in dictation-only mode."
}
```

### Static asset serving

- Source directory: `public/`
- `/` serves `public/index.html`
- Content types are inferred from file extension (`.html`, `.css`, `.js`, `.json`, `.svg`, `.png`, `.jpg/.jpeg`)

### SPA fallback behavior

- If a non-root file lookup fails:
  - request is treated as navigation when `Accept` includes `text/html`, or
  - pathname has no file extension
- Then server returns `public/index.html` with status `200`.
- Missing asset requests with file extensions return `404 Not Found`.

### Path traversal handling

- Request paths are URL-decoded and normalized.
- Paths that resolve outside `public/` are rejected with `400 Bad Request`.

## 2) Tauri invoke bridge (`src-tauri/src/main.rs`)

These commands are registered in `tauri::generate_handler!`.

## Command reference

### `get_dictation_onboarding() -> DictationOnboardingPayload`

Returns onboarding/setup state for desktop dictation.

Returned payload fields:

- `onboarding_required: bool`
- `selected_model_id: string | null`
- `selected_model_path: string | null`
- `selected_model_exists: bool`
- `whisper_cli_available: bool`
- `whisper_cli_path: string`
- `models_dir: string`
- `device`:
  - `total_memory_gb: number`
  - `logical_cpu_cores: number`
  - `architecture: string`
  - `os: string`
- `models: DictationModelOption[]`

`DictationModelOption` fields:

- `id`, `display_name`, `whisper_ref`, `file_name`, `path`
- `installed`, `likely_runnable`, `recommended`
- `approx_size_gb`, `min_ram_gb`, `recommended_ram_gb`
- `speed_note`, `quality_note`

### `install_dictation_model({ model: string }) -> DictationModelSelection`

Behavior:

- Validates `model` id against internal catalog.
- Verifies `whisper-cli` is executable before attempting install.
- Downloads model from:
  - `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/<file_name>`
- Persists selection in local settings.

Return payload:

- `selected_model_id: string`
- `selected_model_path: string`
- `installed: boolean` (currently always `true` on success)

### `delete_dictation_model({ model: string }) -> DictationModelDeletion`

Behavior:

- Deletes the requested model file if present.
- If deleted model was selected, auto-selects best remaining installed model by local fit/size rank.
- Clears selection if no fallback model is installed.

Return payload:

- `deleted_model_id: string`
- `selected_model_id: string | null` (post-delete active selection)
- `selected_model_path: string | null`

### `start_native_dictation() -> ()`

Behavior:

- Fails fast unless:
  - active model path resolves and file exists
  - `whisper-cli` is executable/valid
  - no active recording is already in progress
- Opens microphone stream via `cpal` on a background thread.

### `stop_native_dictation() -> string`

Behavior:

- Stops active recording thread.
- Reads captured samples and resolves active model path.
- Runs `whisper-cli` with:
  - `-m <model>`
  - `-f <temp wav>`
  - `-l en`
  - `-otxt -nt -of <temp prefix>`
- Reads generated transcript `.txt`, strips known artifact tokens, and returns cleaned text.

### `cancel_native_dictation() -> ()`

- Stops and drops any active recording session without transcription.
- Safe no-op when nothing is recording.

### `open_whisper_setup_page() -> ()`

- Opens `https://github.com/ggml-org/whisper.cpp#quick-start` via OS shell opener (`open`/`xdg-open`/`cmd /C start`).

## Event channels

### `dicktaint://fn-state` (backend -> frontend)

- Emitted on macOS global `fn` modifier transitions.
- Payload:

```json
{
  "pressed": true
}
```

### `dicktaint://pill-status` (frontend -> overlay windows)

- Emitted by frontend to update native overlay pill UI.
- Payload:

```json
{
  "message": "Hold fn to dictate",
  "state": "idle",
  "visible": true
}
```

Allowed `state` values: `idle`, `working`, `live`, `ok`, `error`.

## Runtime-only environment variables

- `HOST` (web server bind host; default `127.0.0.1`)
- `PORT` (web server port; default `3000`)
- `WHISPER_CLI_PATH` (desktop CLI override)
- `WHISPER_MODEL_PATH` (desktop model override; bypasses onboarding selection)
- `DICKTAINT_START_HIDDEN` (`1/true/on` to start desktop app hidden)
