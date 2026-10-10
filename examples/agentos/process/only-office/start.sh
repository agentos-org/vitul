#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_DIR="$APP_DIR/deps"
DATA_DIR="${DATA_DIR:?DATA_DIR is required}"
PORT="${PORT:-3000}"
HOST="${HOST:-0.0.0.0}"
NODE="${NODE_BIN:-node}"
PYTHON="$RUNTIME_DIR/usr/bin/python3"
export PORT HOST
STATE_DIR="$DATA_DIR/onlyoffice"
DOCS_URL="http://127.0.0.1:8080"
DOCS_DIR="$RUNTIME_DIR/var/www/onlyoffice/documentserver"
DOCSERVICE="$DOCS_DIR/server/DocService/docservice"
CONVERTER="$DOCS_DIR/server/FileConverter/converter"
NGINX="$RUNTIME_DIR/usr/sbin/nginx"
CHILD_PIDS=()

fail() {
  echo "ONLYOFFICE: $*" >&2
  exit 1
}

[[ -x "$DOCSERVICE" ]] || fail "bundled DocService binary is missing: $DOCSERVICE"
[[ -x "$CONVERTER" ]] || fail "bundled converter binary is missing: $CONVERTER"
[[ -x "$NGINX" ]] || fail "bundled Nginx binary is missing: $NGINX"
[[ -x "$PYTHON" ]] || fail "bundled Linux Python runtime is missing: $PYTHON"
[[ -f "$RUNTIME_DIR/etc/onlyoffice/documentserver/default.json" ]] || fail "ONLYOFFICE runtime files are missing"

if [[ "$NODE" == */* ]]; then
  [[ -x "$NODE" ]] || fail "Node.js executable is not available: $NODE"
else
  command -v "$NODE" >/dev/null 2>&1 || fail "Node.js is not available"
fi

mkdir -p "$DATA_DIR/documents" "$STATE_DIR"

case "$(uname -m)" in
  aarch64|arm64) LIBARCH="aarch64-linux-gnu" ;;
  x86_64|amd64) LIBARCH="x86_64-linux-gnu" ;;
  *) fail "unsupported container architecture: $(uname -m)" ;;
esac

export LD_LIBRARY_PATH="$DOCS_DIR/server/FileConverter/bin:$RUNTIME_DIR/usr/lib/$LIBARCH:$RUNTIME_DIR/lib/$LIBARCH:$RUNTIME_DIR/usr/lib:$RUNTIME_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PYTHONHOME="$RUNTIME_DIR/usr"
export PYTHONNOUSERSITE=1
"$PYTHON" "$APP_DIR/prepare-runtime.py" "$RUNTIME_DIR" "$DATA_DIR" "$APP_DIR"

export HOME="$STATE_DIR/home"
export TMPDIR="$STATE_DIR/tmp"
export XDG_CACHE_HOME="$STATE_DIR/tmp/cache"
export FONTCONFIG_FILE="$STATE_DIR/fonts.conf"
export FONTCONFIG_PATH="$RUNTIME_DIR/etc/fonts"
export NODE_ENV=production-linux
export NODE_DISABLE_COLORS=1
export NODE_CONFIG_DIR="$STATE_DIR/config/onlyoffice"
export APPLICATION_NAME=onlyoffice
export PKG_NATIVE_CACHE_PATH="$STATE_DIR/tmp/.cache"
export ONLYOFFICE_DOCS_URL="$DOCS_URL"
export ONLYOFFICE_JWT_SECRET="$(cat "$STATE_DIR/jwt-secret")"

mkdir -p "$HOME" "$TMPDIR" "$XDG_CACHE_HOME" "$PKG_NATIVE_CACHE_PATH"

start_child() {
  local name="$1"
  local directory="$2"
  local logfile="$3"
  shift 3
  (
    cd "$directory"
    exec "$@"
  ) >>"$logfile" 2>&1 &
  CHILD_PIDS+=("$!")
  echo "ONLYOFFICE: started $name (pid $!)"
}

stop_children() {
  local status=$?
  trap - EXIT TERM INT
  for pid in "${CHILD_PIDS[@]}"; do
    kill -TERM "$pid" 2>/dev/null || true
  done
  for pid in "${CHILD_PIDS[@]}"; do
    wait "$pid" 2>/dev/null || true
  done
  exit "$status"
}
trap stop_children EXIT TERM INT

"$NGINX" -p "$STATE_DIR/nginx/" -c "$STATE_DIR/nginx/nginx.conf" -t

start_child docservice "$DOCS_DIR/server/DocService" "$STATE_DIR/logs/docservice.log" "$DOCSERVICE"
start_child converter "$DOCS_DIR/server/FileConverter" "$STATE_DIR/logs/converter.log" "$CONVERTER"
start_child nginx "$STATE_DIR" "$STATE_DIR/logs/nginx.log" "$NGINX" -p "$STATE_DIR/nginx/" -c "$STATE_DIR/nginx/nginx.conf" -g "daemon off;"

echo "ONLYOFFICE: waiting for Docs health check"
deadline=$((SECONDS + 240))
until "$NODE" -e 'fetch(process.argv[1]).then(r => process.exit(r.ok ? 0 : 1)).catch(() => process.exit(1))' "$DOCS_URL/healthcheck" >/dev/null 2>&1; do
  for index in "${!CHILD_PIDS[@]}"; do
    pid="${CHILD_PIDS[$index]}"
    if ! kill -0 "$pid" 2>/dev/null; then
      name=(docservice converter nginx)
      logfile=("$STATE_DIR/logs/docservice.log" "$STATE_DIR/logs/converter.log" "$STATE_DIR/logs/nginx.log")
      wait "$pid" || true
      echo "ONLYOFFICE: ${name[$index]} exited during startup; recent log:" >&2
      tail -n 60 "${logfile[$index]}" >&2 || true
      exit 1
    fi
  done
  if (( SECONDS >= deadline )); then
    echo "ONLYOFFICE: Docs did not become healthy within 240 seconds" >&2
    tail -n 60 "$STATE_DIR/logs/docservice.log" "$STATE_DIR/logs/converter.log" "$STATE_DIR/logs/nginx.log" >&2 || true
    exit 1
  fi
  sleep 2
done

echo "ONLYOFFICE: Docs is ready; starting the AgentOS workspace"
start_child workspace "$APP_DIR" "$STATE_DIR/logs/workspace.log" "$NODE" "$APP_DIR/server.cjs"

while :; do
  for index in "${!CHILD_PIDS[@]}"; do
    pid="${CHILD_PIDS[$index]}"
    if ! kill -0 "$pid" 2>/dev/null; then
      name=(docservice converter nginx workspace)
      logfile=("$STATE_DIR/logs/docservice.log" "$STATE_DIR/logs/converter.log" "$STATE_DIR/logs/nginx.log" "$STATE_DIR/logs/workspace.log")
      wait "$pid" || true
      echo "ONLYOFFICE: ${name[$index]} exited; recent log:" >&2
      tail -n 60 "${logfile[$index]}" >&2 || true
      exit 1
    fi
  done
  sleep 2
done
