#!/usr/bin/env bash
set -euo pipefail
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$APP_DIR/frontend"
export CI=true
if [[ "${ANDROIDTONE_SKIP_PNPM_INSTALL:-0}" != 1 ]]; then
  pnpm --config.manage-package-manager-versions=false --config.confirmModulesPurge=false install --frozen-lockfile
fi
pnpm --config.manage-package-manager-versions=false build
