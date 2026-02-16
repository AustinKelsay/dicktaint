# macOS Private API Checklist

Use this checklist for any desktop release while `src-tauri/tauri.conf.json` has `"macOSPrivateApi": true`.

## Decision and scope

- [ ] Confirm release channel before cutting artifacts.
- [ ] If channel is direct distribution (outside Mac App Store), keep `"macOSPrivateApi": true`.
- [ ] If channel is Mac App Store, change `"macOSPrivateApi"` to `false` and remove transparency behavior that depends on private APIs before release.

## Release notes

- [ ] Include a release-note line that this build uses macOS private APIs for transparent overlay windows.
- [ ] Note that Mac App Store submission is not supported while private APIs are enabled.

## CI and build gates

- [ ] Add/keep a CI check that validates `src-tauri/tauri.conf.json` has the expected `macOSPrivateApi` value for the selected release channel.
- [ ] Fail CI if release channel and `macOSPrivateApi` value do not match policy.

## Signing and notarization (direct distribution)

- [ ] Sign app bundle with Apple Developer ID credentials.
- [ ] Notarize the signed artifact with Apple notary service.
- [ ] Staple the notarization ticket to shipped artifacts.
- [ ] Smoke-test on a clean macOS machine: app launch, hold-to-talk hotkey, and transparent overlay pill rendering.
