#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_ARCH="${VITUL_TARGET_ARCH:-arm64}"

case "${TARGET_ARCH}" in
  arm64) DOCKER_PLATFORM="linux/arm64"; EXPECTED_ARCH="aarch64" ;;
  x64) DOCKER_PLATFORM="linux/amd64"; EXPECTED_ARCH="x86_64" ;;
  *) echo "Unsupported AgentOS target architecture: ${TARGET_ARCH}" >&2; exit 1 ;;
esac

HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"
if [ "${HOST_OS}" = "Linux" ] && [ "${HOST_ARCH}" = "${EXPECTED_ARCH}" ]; then
  cargo build --manifest-path "${APP_DIR}/backend/Cargo.toml" --release
else
  command -v docker >/dev/null 2>&1 || {
    echo "Building WinTone for AgentOS ${TARGET_ARCH} on this host requires Docker." >&2
    exit 1
  }
  docker run --rm --platform "${DOCKER_PLATFORM}" \
    -v "${APP_DIR}:/work" -w /work/backend \
    rust:1-bookworm cargo build --release
fi

mkdir -p "${APP_DIR}/bin"
cp "${APP_DIR}/backend/target/release/wintone-server" "${APP_DIR}/bin/wintone-server"
chmod 0755 "${APP_DIR}/bin/wintone-server"
