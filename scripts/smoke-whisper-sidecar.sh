#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT_DIR/src-tauri/binaries"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

if [[ "$OS" != "darwin" ]]; then
  echo "This smoke test currently supports macOS only (detected: $OS)." >&2
  exit 1
fi

case "$ARCH" in
  arm64)
    SIDE_CAR="$BIN_DIR/whisper-cli-aarch64-apple-darwin"
    ;;
  x86_64)
    SIDE_CAR="$BIN_DIR/whisper-cli-x86_64-apple-darwin"
    ;;
  *)
    echo "Unsupported macOS architecture: $ARCH" >&2
    exit 1
    ;;
esac

if [[ ! -x "$SIDE_CAR" ]]; then
  echo "Missing executable sidecar at $SIDE_CAR" >&2
  exit 1
fi

MODEL_PATH="${SMOKE_MODEL_PATH:-/tmp/ggml-tiny.en.bin}"
AUDIO_PATH="${SMOKE_AUDIO_PATH:-/tmp/jfk.wav}"
OUT_DIR="/tmp/dicktaint-sidecar-smoke"

if [[ ! -f "$MODEL_PATH" ]]; then
  curl -L --fail -o "$MODEL_PATH" \
    https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin
fi

if [[ ! -f "$AUDIO_PATH" ]]; then
  if [[ -f "/opt/homebrew/opt/whisper-cpp/share/whisper-cpp/jfk.wav" ]]; then
    cp "/opt/homebrew/opt/whisper-cpp/share/whisper-cpp/jfk.wav" "$AUDIO_PATH"
  else
    curl -L --fail -o "$AUDIO_PATH" \
      https://raw.githubusercontent.com/ggml-org/whisper.cpp/master/samples/jfk.wav
  fi
fi

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

"$SIDE_CAR" \
  -m "$MODEL_PATH" \
  -f "$AUDIO_PATH" \
  -ng \
  -l en \
  -otxt \
  -nt \
  -of "$OUT_DIR/out" >/tmp/dicktaint-sidecar-smoke.log 2>&1

if [[ ! -f "$OUT_DIR/out.txt" ]]; then
  echo "Smoke test failed: transcript file missing" >&2
  sed -n '1,80p' /tmp/dicktaint-sidecar-smoke.log >&2
  exit 1
fi

echo "--- transcript ---"
cat "$OUT_DIR/out.txt"

if ! grep -qi "country" "$OUT_DIR/out.txt"; then
  echo "Smoke test failed: expected transcript content not found" >&2
  sed -n '1,80p' /tmp/dicktaint-sidecar-smoke.log >&2
  exit 1
fi

echo "Smoke test passed: sidecar transcribed audio successfully."
