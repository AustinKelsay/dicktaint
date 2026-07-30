# Vendored `cpal` (dicktaint)

This tree is a local path dependency, not an upstream submodule checkout for convenience.

## Why it exists

`src-tauri/Cargo.toml` patches crates.io `cpal` to `../vendor/cpal`:

```toml
[patch.crates-io]
cpal = { path = "../vendor/cpal" }
```

macOS CoreAudio input teardown in stock `cpal` 0.15 could retain the input stream (retain cycle), so the system mic indicator stayed on after dicktaint reported idle. The local patch fixes that idle-mic leak for packaged builds.

Do **not** remove this vendor tree or the `[patch.crates-io]` entry without verifying CoreAudio stop/teardown on a non-default input device.

## Upstream

Based on `cpal` 0.15. See the stock `README.md` / `CHANGELOG.md` in this directory for library docs. App-facing release notes: root `CHANGELOG.md` (`v0.3.9`).
