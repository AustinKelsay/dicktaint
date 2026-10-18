# Murmur rebuild verification

Verified on 2026-10-18 on an Apple Silicon Mac. Application source: integration commit `ba6dbd59e0353ad239e3a4780f938d76428851d0`; baseline: `f4a1b024b913f7d99e65abd8f2487712b8206b9a`. The subsequent delivery-record commit changes documentation only.

## Delivery and continuity

- [Spec #35](https://github.com/AustinKelsay/dicktaint/issues/35) and [PR #40](https://github.com/AustinKelsay/dicktaint/pull/40) track the complete rebuild. Tickets #36, #37, #38, #39, and #41 cover identity, permissions, workspace, verification, and model-readiness parity.
- The Matt Pocock pipeline covered design/domain modeling, document-led grilling, spec, dependency-linked tickets, a throwaway readiness prototype, worktree implementation with TDD, merger integration, and independent Standards and Spec reviews. The [prototype verdict](https://github.com/AustinKelsay/dicktaint/issues/35#issuecomment-6254338209) records the walkthrough; branch `prototype/murmur-readiness` retains the experiment outside production code.
- Murmur changes display branding and the product surface. It retains `com.plebdev.dicktaint`, the `dicktaint` executable, `.dicktaint` data, existing settings, models, environment variables, and native events. See [ADR 0002](adr/0002-murmur-continuity.md).
- The installed `/Applications/dicktaint.app` was not replaced. Its privacy grants and application data were not changed. A local ad hoc signature does not establish TCC continuity with an installed build.

## Automated verification

| Check | Baseline | Final result |
| --- | --- | --- |
| `bun test` | 42 tests | 69 passed, 0 failed, 254 assertions |
| `bun run test:rust` | 44 tests | 54 passed, 0 failed |
| `bun run docs:verify` | Passing | Passing |
| `git diff --check` | Clean | Clean |
| Local arm64 application build | Existing app | Murmur.app built successfully |
| `bun run verify:bundle` | New verifier | Passing on the signed local artifact |
| Bundled Whisper fixture | Existing engine | Real audio transcribed successfully |

New regressions exercise required versus optional permissions, unrequested/denied/restricted/unknown microphone access, explicit recovery actions, focus rechecks, preservation of a recording during checks, model readiness matching capture resolution, draft/history actions, native clipboard command routing, and initial-route timing. Cancellation tests cover pending Stop/Clear, duplicate Fn delivery, and late promises/listening/idle events from a canceled start while a newer start is pending. Bundle-verifier fixtures reject missing or placeholder sidecars, version mismatch, missing microphone entitlement, and signature tampering.

Clipboard tests mock the OS boundary. They do not replace the user's real clipboard. Audio, hotkey, and insertion regression tests do not establish physical device delivery.

## Independent reviews

Standards and Spec were assigned separately to independent Sol and Astra reviewers against the pinned baseline, followed by focused reviews of fixes.

| Axis | Findings and disposition | Final verdict |
| --- | --- | --- |
| Standards | No hard documented-standard violations. A nonblocking duplicated microphone recovery policy was moved into a shared leaf helper. The final cancellation event correlation also resolved the reviewer's event-order audit question. | PASS at `c4782ad`; no remaining actionable findings. |
| Spec | Pending native starts could survive Stop/Clear; an early focus check could choose the initial screen before model readiness arrived. Both were reproduced and fixed. A subsequent delayed cancellation-idle reproduction exposed a newer-start race; request correlation now covers that event too. | PASS at `c4782ad`; original and follow-up reproductions pass, plus five focused regression tests / 20 assertions. |

The controller also corrected a misleading green overall-readiness footer when microphone access was still unrequested. Native session intent is reserved before worker dispatch; cancellation and publication share a short lock, while authorization and device opening occur outside it. A canceled device-open result is stopped and joined before it can publish. Cancellation does not forcibly dismiss an OS permission dialog or interrupt a device initializer already executing.

## Rendered browser checks

The collaborative browser rendered the production frontend with an external validation adapter at the Tauri boundary. The adapter is not shipped. Its macOS permission/model responses were simulated and are distinct from the native observations below.

Observed scenarios: ready, first setup, microphone denied, restricted, permission service unavailable, engine unavailable, and model missing. Ready opens Dictation; incomplete required checks open setup and disable capture. Optional Accessibility/Input Monitoring do not gate button dictation. Startup performed observational checks without permission requests, privacy-pane launches, or listener-rearm commands. Explicit Allow/Open Settings actions invoked their corresponding commands.

The start → navigate to settings → recheck/focus → stop flow preserved the simulated recording, produced one draft/history entry despite both result and event delivery, and allowed editing, Copy, Clear, and history reuse. Copy used adapter memory. Light/dark appearance, 900×720, the 740×560 native minimum, and a 375px browser viewport were inspected without horizontal overflow or console errors. Browser-only manual drafting and provider-dependent speech privacy copy were also checked.

## Native macOS observations

A separate compile-time configuration used identifier `com.plebdev.murmur.validation` and product name `Murmur Validation`, so Tauri's application-data path and macOS privacy identity were isolated from the installed app. Changing only Info.plist would not isolate Tauri's compiled application-data lookup.

The validation build at `3119a04` launched through CuaDriver without foreground activation. Its task-owned settings selected the real tiny English fixture model and disabled background trigger, insertion, tray, and overlay features. The actual native app detected the bundled engine and custom model, reported microphone access as unrequested, displayed the explicit Allow action, and disabled capture. No permission prompt or System Settings window appeared during startup. Dictation/settings navigation and manual typing in the draft were inspected in the actual WebKit window.

Native inspection exposed the misleading readiness footer corrected in the reviewed final source. It also showed a stale sidebar color transition in a background WebKit window; navigation now updates its selection without that transition. Final frontend behavior was rechecked in the collaborative browser; the final package was rebuilt, signed, inspected, and fixture-tested separately. The native validation window was closed after inspection.

## Reproduce the final local package

From the repository root, after dependency installation and creation of a real host-architecture sidecar:

```bash
./node_modules/.bin/tauri build --bundles app
codesign --force --sign - 'src-tauri/target/release/bundle/macos/Murmur.app/Contents/MacOS/whisper-cli'
codesign --force --sign - --entitlements src-tauri/dicktaint.entitlements 'src-tauri/target/release/bundle/macos/Murmur.app'
bun run verify:bundle
```

The unsigned local Tauri output initially had linker ad hoc signing without the required audio-input entitlement; explicit local signing above produced the verified artifact. The verifier checked display name/version, stable identifier/executable, microphone description, arm64 Mach-O application and sidecar, audio-input entitlement, and strict deep signature validity. Only the arm64 sidecar was validated; placeholder sidecars for other targets must not be distributed.

`bun run whisper:smoke` downloaded the tiny English model and JFK audio fixture. The final **bundled** `whisper-cli` was then invoked against those real files and produced the expected country/you passage. This exercises local transcription, not live capture.

Local artifacts remain under the ignored `src-tauri/target/release/bundle/macos/` directory:

- `Murmur.app`
- `Murmur-0.3.12-local-arm64.zip`
- ZIP SHA-256: `be2bc6603d156a6cbbda149236dccef3b8fcbb0c39254a0b85ecc298f0325b91`

## Remaining physical and distribution gates

No microphone grant, live recording, physical Fn delivery, focused-field paste into another app, or real clipboard mutation was performed. Those paths retain regression coverage, but a complete physical-device walkthrough remains a release gate. Existing signing identity and trust must be checked when upgrading the working installation.

There is no valid Developer ID signing identity available in this environment. The delivered local artifact is ad hoc signed; it is not notarized or stapled and is not a public distribution build. Developer ID signing, notarization/stapling, clean-machine Gatekeeper checks, and the physical walkthrough remain in the [release runbook](../llm/workflow/RELEASE_AND_DISTRIBUTION.md) and [smoke checklist](../llm/workflow/SMOKE_TESTS.md). No release tag was created and no production installation was changed.
