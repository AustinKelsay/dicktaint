# Bundled whisper-cli sidecars

Tauri `externalBin` is configured as `binaries/whisper-cli`.

Provide platform-specific binaries named:

| File | Status in this repo |
| --- | --- |
| `whisper-cli-aarch64-apple-darwin` | **Real** arm64 macOS binary for local/dev and Apple Silicon releases |
| `whisper-cli-x86_64-apple-darwin` | **Placeholder** — replace with a real Intel macOS binary before shipping Intel builds |
| `whisper-cli-x86_64-pc-windows-msvc.exe` | **Placeholder** — Windows not an MVP ship target |
| `whisper-cli-x86_64-unknown-linux-gnu` | **Placeholder** — Linux not an MVP ship target |

Placeholders exit non-zero and print a replace-me message. Do not treat them as working transcription binaries.

Rebuild the host macOS sidecar:

```bash
bun run whisper:sidecar
```

Smoke test sidecar transcription:

```bash
bun run whisper:smoke
```
