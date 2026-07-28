# CONTEXT.md

## Product

**dicktaint** is a local-first, private dictation app. Primary runtime is **macOS desktop** via Tauri + bundled `whisper-cli`. Web mode is a static SPA fallback (browser SpeechRecognition). iPhone/iOS is an explicit future track, not the current cleanup focus.

## Glossary

| Term | Meaning |
| --- | --- |
| Native dictation | Desktop capture via Rust/cpal + local `whisper-cli` transcription |
| Web dictation | Browser `SpeechRecognition` path in the static SPA |
| Sidecar | Bundled `whisper-cli-*` binary under `src-tauri/binaries/` |
| Overlay pill | Native transparent macOS window showing dictation state |
| Focused-field insert | Paste finished transcript into the frontmost text field (Accessibility) |
| Onboarding | Local model + whisper-cli readiness flow before dictation |
| Staging tip | Integration branch for PRs; should track shipping `main` closely |

## Module map

| Module | Owns |
| --- | --- |
| `whisper_cli` | Path resolution, probes, sidecar candidates |
| `models` | Catalog, download, settings persistence, onboarding payload |
| `audio` | Mic capture, resample, sanitize, WAV write |
| `transcribe` | Whisper invoke + transcript normalization |
| `hotkey_overlay` | Global hotkeys, Fn hold, pill windows, tray sync |
| `insert` | Clipboard / Accessibility insertion |
| `commands` | Tauri command handlers |
| Frontend `public/js/*` | Onboarding, settings, native session, history, web speech |

## Canonical docs

Implementation truth lives under `llm/`. Root `README.md` is the short human entrypoint. Repo-relative paths only in docs (never machine-absolute `/Users/...` anchors).
