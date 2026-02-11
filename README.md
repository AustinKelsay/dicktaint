# dicktaint
A local ai dictation tool suitable for the most private chats and dirtiest language

## Quick start

1. Install [Bun](https://bun.sh/).
2. Install and run [Ollama](https://ollama.com/).
3. Pull at least one model, for example:
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
- In desktop mode, the frontend calls Rust Tauri commands for Ollama access.
- Ollama host defaults to `http://127.0.0.1:11434`.
- Override host for desktop runs with:
  ```bash
  OLLAMA_HOST=http://127.0.0.1:11434 bun run tauri:dev
  ```

Desktop build (currently configured for compile checks, bundling disabled):

```bash
bun run tauri:build
```

## What this starter does

- Lists your local Ollama models (with selector).
- Prefers `karanchopda333/whisper:latest` by default when available.
- Provides a basic dictation flow: start dictation, stop, edit transcript, clean with model.
- Returns cleaned dictation output.
- If live speech capture is unavailable in your runtime, you can still paste/type transcript text.

## Config

- `PORT` (default `3000`)
- `OLLAMA_HOST` (default `http://127.0.0.1:11434`)

Example:

```bash
PORT=3001 OLLAMA_HOST=http://127.0.0.1:11434 bun run start
```
