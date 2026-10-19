# Murmur rebuild

## Problem Statement

The existing private dictation app works exceptionally well once configured. Its name, busy interface, scattered settings, and incomplete macOS permission guidance make opening and setting it up harder than using it. Rebuild its product surface without losing the hard-won reliability of local transcription, background Fn hold-to-talk, focused-field insertion, or existing saved configuration.

## Solution

Call the app **Murmur**. Give it a calm, compact Mac workspace, a guided setup that describes real readiness, and specific recovery actions. Existing users retain models and preferences. The app opens directly into dictation when ready. First-time users see the small set of required steps, with optional capabilities clearly separated. The orchestrator resolves product and implementation choices from the user's request and documented intent; the user explicitly requested no interview or intermediate approval loop.

## User Stories

1. As an existing user, I want my models and settings retained so I can keep dictating after the rename.
2. As a user, I want a simple friendly app name and recognizable icon so the app feels comfortable anywhere on my Mac.
3. As a returning user, I want a ready app to open into dictation so setup does not slow me down.
4. As a new user, I want a short setup checklist so I know the next required action.
5. As a user, I want local engine and model checks explained in ordinary language so I understand readiness without knowing Whisper CLI.
6. As a user, I want a recommended local model so I can choose without comparing every technical option.
7. As a user, I want to install, select, and remove models safely so I control storage and quality.
8. As a user, I want to grant microphone access before capture so I avoid surprise failures.
9. As a user, I want denied, restricted, unknown, and unrequested microphone access distinguished so the suggested action matches macOS.
10. As a user, I want direct buttons to the relevant macOS privacy pane so I do not hunt through System Settings.
11. As a user, I want returning from System Settings to recheck permissions so I see whether the fix worked.
12. As a user, I want permission checks to avoid prompts and window switching so opening the app is calm.
13. As a user, I want permission refresh to preserve a recording already underway so no words are lost.
14. As a user, I want background Fn status explained as a capability so the interface does not claim that a particular permission is granted incorrectly.
15. As a user, I want Accessibility described for insertion and background Fn so I understand why it is requested.
16. As a user, I want Input Monitoring treated as an optional alternative Fn route so I can use the app with fewer permissions.
17. As a user, I want button dictation to remain available without optional grants so setup does not trap me.
18. As a user, I want a clear listening/processing/ready/error indicator so I know what the app is doing.
19. As a user, I want easy start/stop and an editable transcript so everyday dictation takes little effort.
20. As a user, I want to copy a transcript and reuse recent dictations so I can recover text quickly.
21. As a user, I want the existing Fn hold-to-talk, overlay, menu bar, microphone selection, and focused-field insertion retained so my established workflow survives.
22. As a user, I want advanced preferences tucked away so the everyday screen stays uncluttered.
23. As a user, I want helpful permission recovery instructions when macOS trust is stale so I can recover without destructive reset commands.
24. As a user, I want the interface to work with keyboard navigation, reduced motion, light/dark appearance, and smaller windows so it feels at home on my Mac.
25. As a browser user, I want an accurate privacy explanation so browser speech recognition is not represented as guaranteed on-device transcription.
26. As a user, I want no external font dependency so the desktop interface remains complete offline.
27. As a maintainer, I want a reproducible build and truthful verification record so I can distinguish a tested local build from a signed/notarized public release.

## Implementation Decisions

- Preserve the capture, resampling, local Whisper, session ordering, clipboard serialization, and background engine. Rebuild the product interface rather than replace working engine behavior without evidence.
- Separate display branding from stable application identity, storage keys, executable name, settings schema, and internal event contracts, as recorded in ADR 0002.
- Add a focused native permissions boundary with typed permission states, observational snapshot, explicit microphone request, and allowlisted privacy settings actions. Reuse existing AVFoundation authorization logic without blocking the main thread.
- Report microphone, Accessibility, and Input Monitoring independently. Report background hotkey delivery from actual listener/runtime capability. Treat not-yet-requested microphone as a setup action, denied/restricted access as actionable recovery, and unknown checks as unverified.
- Eliminate automatic Input Monitoring prompts/settings launches during startup or observational checks. Re-arm Fn after permission changes only while idle; keep focused-window fallback and both proven native delivery paths.
- Keep required readiness separate from optional convenience permissions. Model + engine + granted microphone complete first-time setup. If permission inspection is unavailable, show that honestly and retain explicit retry/request recovery. Browser/manual mode remains usable.
- Build a compact navigation shell, quiet dictation workspace with clear primary controls, editable/copyable text, recent dictations, visible runtime/privacy context, guided setup, and progressively disclosed advanced settings.
- Preserve desktop command compatibility; new permission commands are additive. Keep settings persistence and model selection behavior compatible. Do not introduce a frontend framework or cloud service.
- Use native system fonts, a small coherent vector mark, responsive layout, accessible focus states, adequate contrast, and reduced-motion handling.

## Testing Decisions

- Test observable behavior at the existing frontend bootstrap/action seam with the Tauri bridge mocked as the OS boundary. Cover required vs optional setup gates, real permission statuses, explicit request/settings actions, focus recheck, ready-user routing, transcript actions, and unchanged native session races.
- Exercise the native permission DTO/allowlisted action boundary and recording-safe Fn refresh policy with Rust tests, alongside existing audio/hotkey/session/insertion tests. Use literal scenarios, never assertions that mirror the implementation.
- Verify bundle metadata retains its identifier/entitlements and the new display name. Keep all existing regression tests green.
- Use the collaborative browser to check actual rendered desktop and small-window layouts and behavior. Any native DTO simulation is explicitly a simulation, separate from native permission checks.
- Build a local macOS application bundle, inspect metadata/signing/entitlements and bundled sidecar, and run a real local Whisper fixture transcription. Inspect the native app without overwriting or resetting the currently installed working app.
- Run independent Standards and Spec reviews against the pinned starting commit `f4a1b02` and fix material findings before delivery.

## Out of Scope

- Cloud transcription, accounts, analytics, or new paid services.
- iOS/Android feature expansion, App Store migration, removal of transparent overlay support.
- Renaming the GitHub repository, deleting existing app data, changing user privacy settings automatically, or replacing the installed working app.
- Public release/tag/notarization claims without the required signing credentials and release gates.

## Authorized local deployment follow-up

After the initial isolated validation, the user explicitly requested rebuilding, renaming, and redeploying on this machine. That follow-up authorizes replacing the installed app after a backup, retaining the stable identity and byte-identical settings/models, and recovering stale macOS trust through the normal System Settings UI when necessary. It supersedes the initial installed-app replacement exclusion above; it does not authorize deleting existing data or bypassing macOS permission controls. The local deployment and its verification limits are recorded in [`../murmur-verification.md`](../murmur-verification.md).

The cancellation follow-up must also release a frontend start delivered after Stop/Clear found no native reservation. Cleanup is scoped to the canceled request, and later local starts wait for both cleanup and Clear acknowledgement. Native Fn ownership remains independent.

## Further Notes

Baseline: 42 Bun tests and 44 Rust tests pass. Canonical current implementation docs remain under `llm/`. The continuity/surface audits established that microphone state was missing, Fn status reads rebuilt listeners, Accessibility and Input Monitoring were conflated, and history is temporary session memory. Durable decisions live in ADR 0002 and this spec. The throwaway readiness prototype is captured on branch `prototype/murmur-readiness` at `09fd14b`; its walkthrough verdict is recorded on issue #35. Pipeline: design/domain modeling, spec, dependency-linked tracer-bullet tickets, worktree implementation with TDD, integration, dual-axis review, complete build verification.
