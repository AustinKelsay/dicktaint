# Release And Distribution

## Status Snapshot

- Date: 2026-10-18
- desktop release posture is direct macOS distribution while private API mode is enabled

## Purpose

Define current release gates and distribution constraints.

## Scope

In scope:

- pre-release validation gates
- sidecar packaging requirements
- private API channel implications
- signing/notarization expectations

Out of scope:

- CI implementation specifics

## Source Anchors

- `src-tauri/tauri.conf.json`
- `src-tauri/binaries/README.md`
- `package.json`
- `scripts/verify-macos-bundle.sh`
- `src-tauri/dicktaint.entitlements`
- `src-tauri/Info.plist`

## Runbook

Pre-release gates:

1. sync release version across `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`
2. update top release notes entry in `CHANGELOG.md`
3. `bun run test:all`
4. `bun run docs:verify`
5. run manual smoke checklist from `SMOKE_TESTS.md`
6. verify sidecar binaries required for target distribution are present
7. verify private API setting matches release channel policy
8. run `bun run verify:bundle` against each built app on its matching Mac architecture; verify metadata, source/bundle version parity, real Mach-O main executable and sidecar, microphone usage description, signed audio-input entitlement, and strict deep signature
9. record actual command results and distinguish automated tests, browser DTO simulations, native facts, and manual dictation evidence

Local review build (keeps the installed app untouched):

```bash
bun run tauri:build -- --bundles app
# Without a configured signing identity, bind local metadata and entitlements explicitly.
codesign --force --sign - src-tauri/target/release/bundle/macos/Murmur.app/Contents/MacOS/whisper-cli
codesign --force --sign - --entitlements src-tauri/dicktaint.entitlements src-tauri/target/release/bundle/macos/Murmur.app
bun run verify:bundle
# Or inspect a supplied artifact:
bun run verify:bundle /path/to/Murmur.app
```

The default artifact is the `Murmur.app` under the release bundle directory. Verification reads the app without launching, installing, changing privacy settings, or touching model/settings data. The sidecar check establishes a native executable for this Mac; use the fixture smoke to establish actual transcription. An ad hoc signature is accepted for local verification and clearly reports `LOCAL ONLY`. An unsigned or linker-only artifact without bound audio-input entitlement fails. Any signature mode still requires the distribution gates below; the verifier does not establish notarization or identity continuity. Rebuilds can invalidate a local signature, so sign and verify again after the final build.

Keep source version `0.3.12` until a deliberate release bump. Display name is Murmur; bundle id `com.plebdev.dicktaint`, internal executable `dicktaint`, storage, settings schema, and event names remain stable. Preserve the established signing identity for a distributable upgrade: bundle-id continuity alone does not preserve macOS TCC trust.

Tag-and-publish path:

1. release from `main`
2. push `v*` tag after version/docs are merged
3. confirm GitHub Actions signing/notarization secrets are populated before tagging
4. if those secrets are environment-scoped, confirm the release job targets that environment before tagging
5. confirm the `Release macOS App` workflow publishes both architectures
6. confirm the workflow validates the built app bundle with `codesign --verify --deep --strict`
7. review generated GitHub release notes and replace them with the curated changelog summary if needed
8. run a clean-machine launch + dictation smoke when the signed artifacts are available

Packaging requirement:

- `bundle.externalBin` is configured for `binaries/whisper-cli`
- target platform sidecar binaries must be available for intended targets
- release automation must fail if signing/notarization credentials are missing rather than publishing ad hoc app bundles
- current CI path publishes a notarized Apple Silicon `.dmg` and notarized `.app.tar.gz` archives for both architectures; Intel DMG generation is temporarily skipped in CI because that bundler path is unstable on the GitHub Intel runner
- hardened-runtime macOS releases must include the audio-input entitlement or TCC will deny microphone access before the app can appear in the Microphone settings pane

Direct macOS distribution requirements:

1. sign artifacts with Developer ID
2. notarize artifacts
3. staple notarization ticket
4. run clean-machine launch + dictation smoke

If pursuing App Store channel:

1. remove private API dependency path
2. set `macOSPrivateApi` to `false`
3. re-run full verification matrix

## Verification

Release readiness requires all runbook gates to pass and policy alignment to be documented.

## Related Docs

- [`MACOS_PRIVATE_API_POLICY.md`](MACOS_PRIVATE_API_POLICY.md)
- [`SMOKE_TESTS.md`](SMOKE_TESTS.md)
- [`DOCS_VERIFICATION.md`](DOCS_VERIFICATION.md)
