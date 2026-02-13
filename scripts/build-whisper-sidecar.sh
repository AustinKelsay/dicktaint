#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT_DIR/src-tauri/binaries"
SRC_DIR="${WHISPER_SRC_DIR:-/tmp/whisper.cpp-src}"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

if [[ "$OS" != "darwin" ]]; then
  echo "This helper currently supports macOS only (detected: $OS)." >&2
  exit 1
fi

case "$ARCH" in
  arm64)
    TARGET_NAME="whisper-cli-aarch64-apple-darwin"
    ;;
  x86_64)
    TARGET_NAME="whisper-cli-x86_64-apple-darwin"
    ;;
  *)
    echo "Unsupported macOS architecture: $ARCH" >&2
    exit 1
    ;;
esac

if ! command -v git >/dev/null 2>&1; then
  echo "git is required" >&2
  exit 1
fi
if ! command -v cmake >/dev/null 2>&1; then
  echo "cmake is required (brew install cmake)" >&2
  exit 1
fi

if [[ ! -d "$SRC_DIR" ]]; then
  git clone --depth 1 https://github.com/ggml-org/whisper.cpp "$SRC_DIR"
else
  git -C "$SRC_DIR" fetch --depth 1 origin
  git -C "$SRC_DIR" reset --hard origin/master
fi

BUILD_DIR="$SRC_DIR/build-static-$ARCH"
cmake -S "$SRC_DIR" -B "$BUILD_DIR" \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=OFF \
  -DWHISPER_BUILD_EXAMPLES=ON
cmake --build "$BUILD_DIR" -j"$(sysctl -n hw.ncpu)"

mkdir -p "$BIN_DIR"
cp "$BUILD_DIR/bin/whisper-cli" "$BIN_DIR/$TARGET_NAME"
chmod +x "$BIN_DIR/$TARGET_NAME"

echo "Installed sidecar: $BIN_DIR/$TARGET_NAME"
file "$BIN_DIR/$TARGET_NAME"
"$BIN_DIR/$TARGET_NAME" --help >/dev/null 2>&1
echo "whisper-cli sidecar is executable."
