#!/usr/bin/env bash
# Cross-compile channelcast-ffi for Android ABIs into app/src/main/jniLibs.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/27.0.12077973}"
if [[ ! -d "$ANDROID_NDK_HOME" ]]; then
  echo "NDK not found at $ANDROID_NDK_HOME" >&2
  exit 1
fi
OUT="$ROOT/android/app/src/main/jniLibs"
mkdir -p "$OUT"
cargo ndk --platform 26 -t arm64-v8a -t armeabi-v7a -o "$OUT" \
  build -p channelcast-ffi --release
ls -lh "$OUT"/*/libchannelcast_ffi.so
