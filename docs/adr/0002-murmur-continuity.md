# ADR 0002: Reframe the product while preserving its macOS identity

Murmur replaces the visible dicktaint brand and rebuilds its setup and everyday workspace. It keeps the proven local Whisper capture/transcription engine and the established Tauri + ES module boundaries from ADR 0001. The former requirement to keep all UI flows identical is superseded by the guided setup and workspace described in the rebuild spec.

Keep `com.plebdev.dicktaint`, the `.dicktaint` settings/model directory, existing settings schema, internal event names, Rust executable name, and legacy environment variables. A cosmetic rename must not orphan downloaded models, preferences, or permissions tied to the app identity. Preserve the signing identity when producing a distributable upgrade; retaining a bundle identifier alone cannot guarantee that macOS trusts a newly signed binary.

Permission inspection is observational. Microphone is required for capture; Accessibility enables insertion and can enable background Fn; Input Monitoring is an alternative Fn path. Optional permission failures never prevent dictation inside Murmur. Only an explicit user action may request a permission or open System Settings. Returning from settings rechecks capability without interrupting an active recording.
