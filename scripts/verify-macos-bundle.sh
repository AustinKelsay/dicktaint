#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="${1:-$ROOT_DIR/src-tauri/target/release/bundle/macos/Murmur.app}"
fail() { echo "FAIL: $*" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" ]] || fail "Bundle verification requires macOS."
[[ $# -le 1 ]] || fail "Usage: $0 [path/to/Murmur.app]"
[[ -d "$APP_PATH" ]] || fail "Missing app bundle: $APP_PATH"
PLIST="$APP_PATH/Contents/Info.plist"
[[ -f "$PLIST" ]] || fail "Missing Info.plist."
plist_value() { /usr/libexec/PlistBuddy -c "Print :$1" "$PLIST" 2>/dev/null || fail "Missing Info.plist key: $1"; }
require_value() {
  local actual
  actual="$(plist_value "$1")"
  [[ "$actual" == "$2" ]] || fail "$1 must be '$2' (found '$actual')."
}

require_value CFBundleIdentifier com.plebdev.dicktaint
require_value CFBundleExecutable dicktaint
require_value CFBundleName Murmur
require_value CFBundleDisplayName Murmur
require_value CFBundlePackageType APPL

# Read the source versions independently of the built artifact.
SOURCE_VERSION="$(cd "$ROOT_DIR" && bun -e '
  const fs = require("node:fs");
  const pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));
  const tauri = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
  const cargo = Bun.TOML.parse(fs.readFileSync("src-tauri/Cargo.toml", "utf8"));
  if (!pkg.version || pkg.version !== tauri.version || pkg.version !== cargo.package.version) {
    console.error("FAIL: Source versions differ across package.json, Cargo.toml, and tauri.conf.json.");
    process.exit(1);
  }
  console.log(pkg.version);
')" || fail "Could not establish source version parity."
require_value CFBundleShortVersionString "$SOURCE_VERSION"
require_value CFBundleVersion "$SOURCE_VERSION"
MIC_DESCRIPTION="$(plist_value NSMicrophoneUsageDescription)"
[[ -n "${MIC_DESCRIPTION//[[:space:]]/}" ]] || fail "Microphone usage description is empty."
echo "Metadata: Murmur $SOURCE_VERSION; com.plebdev.dicktaint; executable dicktaint."

ARCH="$(uname -m)"
case "$ARCH" in arm64|x86_64) ;; *) fail "Unsupported Mac architecture: $ARCH" ;; esac
for NAME in dicktaint whisper-cli; do
  BINARY="$APP_PATH/Contents/MacOS/$NAME"
  [[ -x "$BINARY" && -f "$BINARY" ]] || fail "Missing executable $NAME in Contents/MacOS."
  /usr/bin/file -b "$BINARY" | /usr/bin/grep -q 'Mach-O' || fail "$NAME is not a Mach-O binary (a placeholder script cannot ship)."
  /usr/bin/lipo "$BINARY" -verify_arch "$ARCH" >/dev/null 2>&1 || fail "$NAME does not support this Mac's $ARCH architecture."
  echo "Binary: $NAME supports $ARCH."
done

ENTITLEMENTS="$(/usr/bin/codesign -d --entitlements :- "$APP_PATH" 2>/dev/null)" || fail "Cannot read signed app entitlements."
AUDIO_INPUT="$(printf '%s' "$ENTITLEMENTS" | /usr/bin/plutil -extract 'com\.apple\.security\.device\.audio-input' raw -o - - 2>/dev/null)" || fail "Signed app is missing the audio-input entitlement."
[[ "$AUDIO_INPUT" == "true" ]] || fail "Signed app audio-input entitlement is not enabled."
/usr/bin/codesign --verify --deep --strict "$APP_PATH" || fail "Bundle signature verification failed."
SIGNATURE="$(/usr/bin/codesign -dv --verbose=4 "$APP_PATH" 2>&1)"
if [[ "$SIGNATURE" == *"Signature=adhoc"* ]]; then
  echo "LOCAL ONLY: ad hoc signature verified; Developer ID signing, notarization, stapling, and clean-machine checks remain release gates."
else
  printf '%s\n' "$SIGNATURE" | /usr/bin/grep -E '^(Authority|TeamIdentifier|Signature)=' || true
  echo "Signature verified. This check does not establish Developer ID eligibility, signing-identity continuity, notarization, or clean-machine behavior."
fi
echo "Bundle verification passed: $APP_PATH"
