#!/usr/bin/env bash
set -euo pipefail
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${VITUL_TARGET_ARCH:-arm64}" != arm64 ]]; then
  echo 'The bundled runtime currently supports Ubuntu 24.04 ARM64 only.' >&2
  exit 1
fi
if [[ "${ANDROIDTONE_RUNTIME_ALREADY_BUILT:-0}" == 1 ]]; then
  exec python3 "$APP_DIR/scripts/sync-runtime-deployment.py"
fi
python3 "$APP_DIR/runtime-build/build-runtime.py"
exec python3 "$APP_DIR/scripts/sync-runtime-deployment.py"
