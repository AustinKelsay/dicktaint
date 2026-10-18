# CONTEXT.md

## Product

**Murmur** is a local-first, private dictation app: “Private dictation for your Mac.” Primary runtime is **macOS desktop** via Tauri + bundled `whisper-cli`. Web mode is a static SPA fallback (browser SpeechRecognition, which may use a remote browser service). iPhone/iOS is an explicit future track, not the current cleanup focus. The display brand is separate from the legacy `com.plebdev.dicktaint` identity, `.dicktaint` storage, internal executable, events, and environment variables; see [ADR 0002](docs/adr/0002-murmur-continuity.md).

## Glossary

| Term | Meaning |
| --- | --- |
| Native dictation | Desktop capture via Rust/cpal + local `whisper-cli` transcription |
| Web dictation | Browser `SpeechRecognition` path in the static SPA |
| Sidecar | Bundled `whisper-cli-*` binary under `src-tauri/binaries/` |
| Overlay pill | Native transparent macOS window showing dictation state |
| Focused-field insert | Paste finished transcript into the frontmost text field (Accessibility) |
| Required readiness | Local speech engine, selected model, and microphone access needed for capture |
| Optional Mac access | Accessibility for insertion/background Fn, or Input Monitoring as an alternative background Fn path; never required for button dictation |
| Onboarding | Guided required-readiness flow before first native dictation |
| Draft | Editable transcript in the current window; retained while moving between Dictation and Settings |
| Recent dictations | In-memory session history; not persisted across reloads or restarts |
| Staging tip | Integration branch for PRs; should track shipping `main` closely |

## Module map

| Module | Owns |
| --- | --- |
| `whisper_cli` | Path resolution, probes, sidecar candidates |
| `models` | Catalog, download, settings persistence, path resolution |
| `onboarding` | Assembles readiness DTO from models/hotkey/insert/audio/whisper |
| `permissions` | Observational macOS permission facts and explicit permission/settings actions |
| `audio` | Mic capture, resample, sanitize, WAV write |
| `transcribe` | Whisper invoke + transcript normalization |
| `hotkey_overlay` | Facade: dictation UI coordination + re-exports |
| `hotkey_overlay::macos_fn` | macOS Fn/Globe CGEventTap listener |
| `hotkey_overlay::trigger` | Dictation trigger normalize/resolve + hotkey registration |
| `hotkey_overlay::pill` | Overlay pill windows + status sync |
| `hotkey_overlay::tray` | macOS menu bar tray runtime |
| `hotkey_overlay::background_ui` | Background prefs, main window, pill/tray sync |
| `dictation_session` | Native start/stop/cancel/is_running session orchestration |
| `insert` | Clipboard / Accessibility insertion |
| `commands` | Tauri command handlers |
| Frontend `public/js/*` | Onboarding, settings, native session, history, web speech |

## Upgrade continuity

Murmur is the visible product name. The bundle identifier, internal executable, settings/model storage, existing settings schema, event namespace, and legacy environment variables keep their original names deliberately. See [ADR 0002](docs/adr/0002-murmur-continuity.md) before changing them. Permission observation must stay quiet and must not replace the Fn listener during capture or a pending start.

## Canonical docs

Implementation truth lives under `llm/`. Root `README.md` is the short human entrypoint. Repo-relative paths only in docs (never machine-absolute `/Users/...` anchors).
