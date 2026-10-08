#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$BASH_SOURCE")" && pwd)"
NODE="${NODE_BIN:-node}"

if [[ "$NODE" == */* ]]; then
  [[ -x "$NODE" ]] || { echo "Dockge: Node.js executable is not available: $NODE" >&2; exit 1; }
else
  command -v "$NODE" >/dev/null 2>&1 || { echo "Dockge: Node.js executable is not available: $NODE" >&2; exit 1; }
fi

# Dockge starts Docker and Compose as child processes; keep AgentOS's Node path
# in PATH while inheriting the host's Docker CLI and environment.
export PATH="$(dirname "$NODE"):$PATH"

if ! command -v docker >/dev/null 2>&1; then
  export DOCKGE_GUIDE_REASON="当前主机没有找到 Docker CLI。"
  exec "$NODE" "$SCRIPT_DIR/missing-docker-guide.cjs"
fi

if ! docker compose version >/dev/null 2>&1; then
  export DOCKGE_GUIDE_REASON="已找到 Docker，但 Docker Compose V2 不可用。"
  exec "$NODE" "$SCRIPT_DIR/missing-docker-guide.cjs"
fi

export NODE_ENV="${NODE_ENV:-production}"
export DOCKGE_PORT="${DOCKGE_PORT:-${PORT:?PORT is required}}"
export DOCKGE_HOSTNAME="${DOCKGE_HOSTNAME:-${HOST:-127.0.0.1}}"
export DOCKGE_DATA_DIR="${DOCKGE_DATA_DIR:-${DATA_DIR:?DATA_DIR is required}}"
export DOCKGE_STACKS_DIR="${DOCKGE_STACKS_DIR:-${DATA_DIR:?DATA_DIR is required}/stacks}"

mkdir -p "$DOCKGE_DATA_DIR" "$DOCKGE_STACKS_DIR"

TSX="$SCRIPT_DIR/node_modules/tsx/dist/cli.mjs"
[[ -f "$TSX" ]] || { echo "Dockge: bundled tsx runtime is missing: $TSX" >&2; exit 1; }

exec "$NODE" "$TSX" "$SCRIPT_DIR/backend/index.ts"
