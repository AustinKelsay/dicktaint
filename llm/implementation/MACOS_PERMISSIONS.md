# macOS Permissions

## Status Snapshot

- Date: 2026-10-18
- Microphone is required for native capture; Accessibility and Input Monitoring are independent optional capabilities

## Purpose

Define permission observation, explicit actions, and recovery without interrupting dictation.

## Scope

In scope: macOS permission facts, setup readiness, privacy settings actions, and safe Fn rechecks.

Out of scope: changing macOS privacy settings automatically or guaranteeing trust across signing identities.

## Source Anchors

- `src-tauri/src/permissions.rs`
- `src-tauri/src/audio.rs`
- `src-tauri/src/hotkey_overlay/macos_fn.rs`
- `public/js/permissions.js`
- `public/js/readiness.js`
- `public/js/settings/hotkey-ui.js`
- `src-tauri/Info.plist`
- `src-tauri/dicktaint.entitlements`

## Contract

`get_macos_permissions` returns `supported`, `microphone`, `accessibility`, and `input_monitoring`. A snapshot quietly observes each fact; it does not request trust, open System Settings, or replace the Fn listener. Accessibility and Input Monitoring are booleans reported independently. Actual background Fn delivery is reported through the trigger runtime payload, rather than inferred from either grant.

Microphone states:

| State | Meaning and next action |
| --- | --- |
| `not_determined` | macOS has not recorded a decision; the user can choose Allow microphone |
| `authorized` | Required microphone readiness is satisfied; capture still checks the actual input device |
| `denied` | Open Microphone settings explicitly and enable Murmur |
| `restricted` | Access is controlled by system policy; contact the device administrator and recheck after policy changes |
| `unavailable` | Status is unverified; retry rather than treating it as granted or denied |

`request_microphone_permission` requests only from `not_determined`, on a blocking worker, then returns current facts. A timeout remains unconfirmed. `open_macos_permission_settings` accepts only `microphone`, `accessibility`, or `input_monitoring`; opening Input Monitoring explicitly also asks macOS to register/request listen-event access. The app never automatically opens settings or requests Input Monitoring on startup.

Required setup is the local speech engine, a usable selected model, and authorized microphone access. Optional Accessibility enables focused-field insertion and can enable background Fn. Optional Input Monitoring offers another background Fn path. Both optional grants may remain off while button dictation works. Ordinary Copy writes the clipboard without Accessibility access.

`recheck_macos_permissions` returns facts and attempts to refresh the optional Fn listener only while idle. Native code guards active recording and pending starts; focus/visibility return also guards frontend start/stop work. Rechecks preserve recording, draft, and navigation. Initial routing waits for both onboarding and permission checks to finish, including unknown/error results, then happens once: ready users open Dictation; incomplete users see setup. Manual navigation while checks are pending claims the route and is preserved. Overall readiness copy includes microphone access; model-ready copy describes only the model. Later changes update readiness controls without forcing a page switch.

Murmur preserves `com.plebdev.dicktaint`, but macOS trust also depends on the signed executable identity. Ad hoc local builds can have different trust from an installed Developer ID build. Offer explicit settings/recheck/relaunch recovery for stale grants; keep the installed working app and user data intact. Avoid using a privacy database reset as routine recovery.

## Verification

1. Run `bun run test:all` for permission DTO, allowlist, quiet observation, required-vs-optional gating, and recording-safe rechecks.
2. Use the permission matrix in [`../workflow/SMOKE_TESTS.md`](../workflow/SMOKE_TESTS.md) on an actual Mac; browser DTO simulations do not establish native grants.
3. Run `bun run verify:bundle` on the built app to verify a nonempty microphone usage description and signed audio-input entitlement.

## Related Docs

- [`API_SURFACE.md`](API_SURFACE.md)
- [`HOTKEY_AND_OVERLAY.md`](HOTKEY_AND_OVERLAY.md)
- [`../workflow/TROUBLESHOOTING.md`](../workflow/TROUBLESHOOTING.md)
- [`../../docs/adr/0002-murmur-continuity.md`](../../docs/adr/0002-murmur-continuity.md)
