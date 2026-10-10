#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${DATA_DIR:?DATA_DIR is required}"
PORT="${PORT:-3080}"
UPSTREAM_PORT="${DSH_UPSTREAM_PORT:-$((PORT + 1))}"
case "$(uname -m)" in
  aarch64|arm64) DSH_ARCH=arm64 ;;
  x86_64|amd64) DSH_ARCH=x64 ;;
  *) echo "DeepSeek Harness: unsupported container architecture: $(uname -m)" >&2; exit 1 ;;
esac
DSH_BINARY="$APP_DIR/deepseek-harness-sdk-runtime-linux-$DSH_ARCH"
BUNDLED_NODE="$APP_DIR/linux-$DSH_ARCH/primary-runtime/dependencies/node/bin/node"
TOKEN_FILE="$DATA_DIR/launch-token"

[[ -x "$DSH_BINARY" ]] || { echo "DeepSeek Harness: packaged runtime is missing: $DSH_BINARY" >&2; exit 1; }
[[ -x "$BUNDLED_NODE" ]] || { echo "DeepSeek Harness: bundled Node.js is missing: $BUNDLED_NODE" >&2; exit 1; }
export PATH="$(dirname "$BUNDLED_NODE"):$PATH"

export DSH_HOME="$DATA_DIR/home"
export HOME="$DATA_DIR/home"
export TMPDIR="$DATA_DIR/tmp"
export XDG_CACHE_HOME="$DATA_DIR/cache"
export DSH_UPSTREAM_PORT="$UPSTREAM_PORT"
export DSH_LAUNCH_TOKEN_FILE="$TOKEN_FILE"
export DSH_BUNDLED_PRIMARY_RUNTIME="$APP_DIR/linux-$DSH_ARCH/primary-runtime"
export DSH_AGENTOS_PLUGIN_ICON_DIR="$APP_DIR/plugin-icons"

mkdir -p "$DSH_HOME" "$DATA_DIR/workspace" "$TMPDIR" "$XDG_CACHE_HOME"
chmod 0700 "$DSH_HOME"
cd "$DATA_DIR/workspace"
rm -f "$TOKEN_FILE" "$TOKEN_FILE.tmp"

DSH_PID=""
PROXY_PID=""

stop_children() {
  trap - EXIT TERM INT
  if [[ -n "$DSH_PID" ]] && kill -0 "$DSH_PID" 2>/dev/null; then
    kill -TERM "$DSH_PID" 2>/dev/null || true
  fi
  if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
    kill -TERM "$PROXY_PID" 2>/dev/null || true
  fi
  [[ -z "$DSH_PID" ]] || wait "$DSH_PID" 2>/dev/null || true
  [[ -z "$PROXY_PID" ]] || wait "$PROXY_PID" 2>/dev/null || true
}
trap stop_children EXIT
trap 'exit 143' TERM
trap 'exit 130' INT

"$BUNDLED_NODE" "$APP_DIR/proxy.mjs" &
PROXY_PID=$!

PUBLIC_URL="${AGENTOS_APP_PUBLIC_URL:-http://127.0.0.1:$PORT}"
PUBLIC_URL="${PUBLIC_URL%/}/"

"$DSH_BINARY" web \
  --host 127.0.0.1 \
  --port "$UPSTREAM_PORT" \
  --public-url "$PUBLIC_URL" \
  --trusted-host 127.0.0.1 \
  --no-open 2>&1 | while IFS= read -r line; do
  case "$line" in
    'dsh web: '*\?token=*)
      token="${line#*?token=}"
      token="${token%%\ *}"
      token="${token%%&*}"
      if [[ "$token" =~ ^[A-Za-z0-9_-]+$ ]]; then
        printf '%s\n' "$token" >"$TOKEN_FILE.tmp"
        chmod 0600 "$TOKEN_FILE.tmp"
        mv "$TOKEN_FILE.tmp" "$TOKEN_FILE"
        echo "DeepSeek Harness is ready; browser login token stored privately."
      else
        echo "DeepSeek Harness emitted a startup URL; token output was redacted."
      fi
      ;;
    *) printf '%s\n' "$line" ;;
  esac
done &
DSH_PID=$!

wait "$DSH_PID"
