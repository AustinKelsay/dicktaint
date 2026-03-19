# Changelog

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
