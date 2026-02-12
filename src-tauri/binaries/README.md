Bundled `whisper-cli` sidecar binaries go in this folder.

Tauri `externalBin` is configured as:

- `binaries/whisper-cli`

Provide platform-specific binaries named like:

- `whisper-cli-aarch64-apple-darwin`
- `whisper-cli-x86_64-apple-darwin`
- `whisper-cli-x86_64-pc-windows-msvc.exe`
- `whisper-cli-x86_64-unknown-linux-gnu`

These are consumed at bundle time for multiplatform delivery.

This repo currently includes placeholder files for the listed targets so
build/test can run with `externalBin` configured. Replace placeholders with real
`whisper-cli` binaries before shipping builds.
