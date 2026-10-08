#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PUBLIC_DIR="${PUBLIC_DIR:-${APP_DIR}/public}"

platform_dir=""
case "$(uname -s)-$(uname -m)" in
  Linux-aarch64|Linux-arm64) platform_dir="linux-arm64" ;;
  Linux-x86_64) platform_dir="linux-x86_64" ;;
  Darwin-arm64) platform_dir="macos-arm64-metal" ;;
  Darwin-x86_64) platform_dir="macos-x86_64" ;;
esac

if [ -z "${FFMPEG_BIN:-}" ]; then
  for candidate in \
    "${APP_DIR}/ffmpeg" \
    "${APP_DIR}/bin/ffmpeg" \
    "${ASSETS_DIR:-}/bin/whisper/${platform_dir}/ffmpeg" \
    "${AGENTOS_BIN_DIR:-}/whisper/${platform_dir}/ffmpeg"; do
    if [ -n "${candidate}" ] && [ -f "${candidate}" ]; then
      FFMPEG_BIN="${candidate}"
      break
    fi
  done
fi

if [ -z "${FFMPEG_BIN:-}" ] && command -v ffmpeg >/dev/null 2>&1; then
  FFMPEG_BIN="$(command -v ffmpeg)"
fi

export PUBLIC_DIR
export FFMPEG_BIN="${FFMPEG_BIN:-}"

if [ -x "${APP_DIR}/musicd" ]; then
  exec "${APP_DIR}/musicd"
fi

echo "musicd is missing; build the Rust service with scripts/build-musicd.sh" >&2
exit 1
