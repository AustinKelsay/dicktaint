# Changelog

## v0.3.5 - 2026-03-27

### Highlights

- fixed a macOS desktop regression where native hotkey dictation could still foreground the main app window on start even when microphone permission was already granted
- restored the close-to-background pill sync so hiding the main window keeps the idle overlay visible for the next hotkey interaction
- clarified hotkey/runtime docs so `Fn` fallback and explicit hotkey disable behavior are easier to diagnose

### Release Notes

- this patch targets the background dictation flow after `v0.3.4`, specifically the cases where `Fn` could steal focus away from another app field and the floating pill could disappear after closing the main window
- macOS desktop remains the required validation path for this fix because the behavior depends on native Tauri windowing, Input Monitoring, and focused-field insertion timing

## v0.3.4 - 2026-03-26

### Highlights

- fixed a frontend regression where the native macOS desktop dictation flow still called browser `getUserMedia()` before invoking the Tauri backend
- removed the web-only microphone preflight from focused native macOS so packaged desktop builds now rely solely on the native permission and capture path
- corrected the startup path behind the “request is not allowed by the user agent or the platform” error that could appear even after macOS microphone permission was granted

### Release Notes

- this patch targets the desktop startup bug discovered after `v0.3.3`: the app bundle was entering the web/browser capture flow instead of staying on the native Tauri dictation path
- packaged `dicktaint.app` remains the correct validation path for this fix

## v0.3.3 - 2026-03-26

### Highlights

- moved the macOS microphone authorization request onto the Tauri main thread and foregrounded the main app window before requesting access
- fixed a permission-prompt failure mode where `dicktaint` could report microphone denial without surfacing a fresh macOS TCC prompt after reset/reinstall
- kept the native zero-audio startup guards from `v0.3.2` while tightening the prompt path for first-run and post-reset permission recovery

### Release Notes

- this patch specifically targets the remaining macOS permission regression seen in `v0.3.2`, where the installed app bundle was valid but microphone authorization could still fail without presenting the expected system prompt
- packaged `dicktaint.app` remains the required validation path; test by launching from `/Applications` and starting dictation in the foreground

## v0.3.2 - 2026-03-26

### Highlights

- fixed the macOS microphone permission path so native dictation now checks AVFoundation authorization before capture starts instead of falling through into silent zeroed recordings
- tightened startup validation again so native capture now fails immediately when a stream never delivers any audio frames after opening
- improved zero-audio error handling for built-in microphones as well as Bluetooth routes by separating permission failures from stale-input routing failures

### Release Notes

- this patch corrects a gap in `v0.3.1`, where the changelog referenced microphone preflight behavior but the packaged app still did not perform an explicit AVFoundation mic authorization request before capture
- packaged `dicktaint.app` remains the preferred validation path on macOS because TCC permission state is app-bundle specific

## v0.3.1 - 2026-03-26

### Highlights

- fixed a macOS native capture regression where some microphones, especially Bluetooth/AirPods routes, could open successfully but deliver only silent zeroed frames
- added an input-stream startup probe so dictation rejects stale or muted routes earlier instead of recording silence and failing only at transcription time
- stopped collapsing native capture candidates by display name so same-named CoreAudio input routes can still be tried individually

### Release Notes

- this patch specifically targets the native desktop microphone path introduced in `v0.3.0`
- packaged `dicktaint.app` remains the preferred macOS validation path for microphone capture because it has its own TCC identity instead of inheriting Terminal permissions from `tauri:dev`

## v0.3.0 - 2026-03-26

### Highlights

- added explicit native microphone selection so desktop dictation can target the intended macOS input device instead of relying on implicit backend device ordering
- added a microphone permission preflight before native desktop capture starts so packaged builds can prompt earlier and fail less opaquely
- refreshed the application icon set and bundled release assets for the next macOS release

### Release Notes

- packaged `dicktaint.app` is now the preferred macOS validation path for microphone capture because it has its own TCC identity instead of inheriting Terminal permissions from `tauri:dev`
- macOS desktop remains the primary release target for this build
- `macOSPrivateApi` remains enabled to support the transparent overlay behavior, so this release is intended for direct distribution rather than Mac App Store submission

## v0.2.0 - 2026-03-21

### Highlights

- added live mic-level feedback to native desktop dictation so the in-app waveform and floating pill now react to real captured audio instead of placeholder loops
- hardened low-signal handling so near-silent recordings fail with a clear microphone/input-level error instead of hallucinated one-word transcripts like `"you"`
- simplified the setup flow by removing the extra back control and turning the shared footer action into a clearer `Done` path while in settings

### Release Notes

- macOS desktop remains the primary release target for this build
- Apple Silicon publishes both `.dmg` and `.app.tar.gz`, while Intel currently publishes a notarized `.app.tar.gz`
- `macOSPrivateApi` remains enabled to support the transparent overlay behavior, so this release is intended for direct distribution rather than Mac App Store submission

## v0.1.13 - 2026-03-21

### Highlights

- kept the new icon-only compact dictation pill from `v0.1.12`
- unblocked signed macOS releases by skipping the flaky Intel DMG packaging path while still publishing the notarized Intel app archive
- preserved the notarized Apple Silicon DMG path for the primary macOS release install flow

### Release Notes

- macOS desktop remains the primary release target for this build
- Apple Silicon publishes both `.dmg` and `.app.tar.gz`, while Intel currently publishes a notarized `.app.tar.gz`
- `macOSPrivateApi` remains enabled to support the transparent overlay behavior, so this release is intended for direct distribution rather than Mac App Store submission

## v0.1.12 - 2026-03-21

### Highlights

- tightened the floating macOS dictation pill again so it sits lighter at the bottom of the screen
- removed all text from the pill so the overlay is just a mic glyph plus live recording animation
- kept the signed macOS release pipeline aligned with the GitHub `prod` environment while retrying notarized distribution

### Release Notes

- macOS desktop remains the primary release target for this build
- `macOSPrivateApi` remains enabled to support the transparent overlay behavior, so this release is intended for direct distribution rather than Mac App Store submission

## v0.1.11 - 2026-03-21

### Highlights

- connected the macOS release job to the GitHub `prod` environment so environment-scoped Apple signing secrets are actually available during release builds
- kept the strict signing gate in place so releases still fail fast if the certificate or notarization credentials are missing

### Release Notes

- macOS desktop remains the primary release target for this build
- `macOSPrivateApi` remains enabled to support the transparent overlay behavior, so this release is intended for direct distribution rather than Mac App Store submission

## v0.1.10 - 2026-03-19

### Highlights

- fixed the macOS release workflow so Apple signing and notarization credentials are forwarded into the Tauri build
- added release-time validation that fails the workflow if the built `.app` bundle does not pass `codesign --verify --deep --strict`
- tightened the release runbook so unsigned ad hoc desktop artifacts stop shipping as if they were production-ready

### Release Notes

- macOS desktop remains the primary release target for this build
- `macOSPrivateApi` remains enabled to support the transparent overlay behavior, so this release is intended for direct distribution rather than Mac App Store submission

## v0.1.9 - 2026-03-19

### Highlights

- hardened the macOS `Fn` hold-to-talk listener so it can recover when the system disables the event tap
- fixed a focused-field insertion crash by replacing the fragile pasteboard restore path with a safer text-only restore
- corrected hotkey runtime reporting so onboarding and settings reflect the real `Fn` mode more accurately

### Release Notes

- macOS desktop remains the primary release target for this build
- `macOSPrivateApi` remains enabled to support the transparent overlay behavior, so this release is intended for direct distribution rather than Mac App Store submission

## v0.1.8 - 2026-03-19

### Highlights

- refined the macOS floating dictation pill so it keeps the overlay feel with a smaller, lighter footprint
- reorganized the settings screen into clearer sections for model management, hotkeys, focused-field insertion, and permissions
- polished status and helper surfaces so setup remains powerful without feeling as overwhelming

### Release Notes

- macOS desktop remains the primary release target for this build
- `macOSPrivateApi` remains enabled to support the transparent overlay behavior, so this release is intended for direct distribution rather than Mac App Store submission
