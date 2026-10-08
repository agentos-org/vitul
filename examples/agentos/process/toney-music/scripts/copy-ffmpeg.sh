#!/usr/bin/env bash
set -euo pipefail

: "${FFMPEG_BIN:?Set FFMPEG_BIN to a Linux ARM64 FFmpeg executable before building}"
if [ ! -x "${FFMPEG_BIN}" ]; then
  echo "FFMPEG_BIN must point to an executable Linux ARM64 FFmpeg binary" >&2
  exit 1
fi

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "${APP_DIR}/bin"
SOURCE_PATH="$(cd "$(dirname "${FFMPEG_BIN}")" && pwd)/$(basename "${FFMPEG_BIN}")"
TARGET_PATH="${APP_DIR}/bin/ffmpeg"
if [ "${SOURCE_PATH}" = "${TARGET_PATH}" ]; then
  chmod 755 "${TARGET_PATH}"
  exit 0
fi
install -m 755 "${FFMPEG_BIN}" "${APP_DIR}/bin/ffmpeg"
