#!/usr/bin/env bash
set -euo pipefail
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pnpm --dir "$APP_DIR/frontend" install --frozen-lockfile
pnpm --dir "$APP_DIR/frontend" build
