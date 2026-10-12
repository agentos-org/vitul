#!/usr/bin/env bash
set -euo pipefail
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${VITUL_TARGET_ARCH:-arm64}" != arm64 ]]; then
  echo 'The bundled runtime currently supports Ubuntu 24.04 ARM64 only.' >&2
  exit 1
fi
exec python3 "$APP_DIR/runtime-build/build-runtime.py"
