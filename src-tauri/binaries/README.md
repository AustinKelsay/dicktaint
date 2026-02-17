Bundled `whisper-cli` sidecar binaries go in this folder.

Tauri `externalBin` is configured as:

- `binaries/whisper-cli`

Provide platform-specific binaries named like:

- `whisper-cli-aarch64-apple-darwin`
- `whisper-cli-x86_64-apple-darwin`
- `whisper-cli-x86_64-pc-windows-msvc.exe`
- `whisper-cli-x86_64-unknown-linux-gnu`

These are consumed at bundle time for multiplatform delivery.

Current repo state:

- `whisper-cli-aarch64-apple-darwin` is a real built binary (for local macOS arm64 testing).
- Other target files may still be placeholders.

To rebuild the host macOS sidecar:

```bash
bun run whisper:sidecar
```

To smoke test sidecar transcription end-to-end:

```bash
bun run whisper:smoke
```
