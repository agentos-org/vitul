#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${DATA_DIR:?DATA_DIR is required}"
PORT="${PORT:-3000}"

# AgentOS starts sandboxed apps with the host account's numeric UID:GID. That
# UID may resolve to a different name inside the container (for example,
# "ubuntu"), and Gitea requires RUN_USER to match the current account.
APP_RUN_USER="$(id -un 2>/dev/null || true)"
if [[ -z "$APP_RUN_USER" ]]; then
  # Preserve Gitea's USER fallback for runtimes whose UID has no passwd entry.
  APP_RUN_USER="${USER:-agentos-app}"
fi
export USER="$APP_RUN_USER"
export USERNAME="$USER"
export HOME="$DATA_DIR/home"
export TMPDIR="$DATA_DIR/tmp"
export XDG_CACHE_HOME="$DATA_DIR/cache"
export GITEA_WORK_DIR="$DATA_DIR"
export GITEA_CUSTOM="$DATA_DIR/custom"
export GIT_EXEC_PATH="$APP_DIR/git/usr/lib/git-core"
export GIT_TEMPLATE_DIR="$APP_DIR/git/usr/share/git-core/templates"
export GIT_CONFIG_NOSYSTEM=1
export PATH="$APP_DIR/git/usr/bin:$PATH"
export LD_LIBRARY_PATH="$APP_DIR/git/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

mkdir -p \
  "$HOME" \
  "$TMPDIR" \
  "$XDG_CACHE_HOME" \
  "$DATA_DIR/data" \
  "$DATA_DIR/repositories" \
  "$DATA_DIR/log" \
  "$DATA_DIR/sessions" \
  "$GITEA_CUSTOM/conf"

APP_INI="$GITEA_CUSTOM/conf/app.ini"
if [[ ! -f "$APP_INI" ]]; then
  GITEA_BIN="$APP_DIR/gitea"
  SECRET_KEY="$("$GITEA_BIN" generate secret SECRET_KEY)"
  INTERNAL_TOKEN="$("$GITEA_BIN" generate secret INTERNAL_TOKEN)"
  JWT_SECRET="$("$GITEA_BIN" generate secret JWT_SECRET)"
  PUBLIC_URL="${AGENTOS_APP_PUBLIC_URL:-http://127.0.0.1:$PORT}"
  PUBLIC_URL="${PUBLIC_URL%/}/"

  TEMP_INI="$APP_INI.tmp.$$"
  cat >"$TEMP_INI" <<EOF
APP_NAME = Gitea: Git with a cup of tea
RUN_USER = $USER
RUN_MODE = prod

[database]
DB_TYPE = sqlite3
PATH = $DATA_DIR/data/gitea.db
SQLITE_TIMEOUT = 20000
SQLITE_JOURNAL_MODE = WAL

[repository]
ROOT = $DATA_DIR/repositories
DEFAULT_BRANCH = main

[server]
APP_DATA_PATH = $DATA_DIR/data
DOMAIN = localhost
HTTP_ADDR = 0.0.0.0
HTTP_PORT = $PORT
ROOT_URL = $PUBLIC_URL
LOCAL_ROOT_URL = http://127.0.0.1:$PORT/
PUBLIC_URL_DETECTION = auto
DISABLE_SSH = true
START_SSH_SERVER = false
LFS_START_SERVER = true

[security]
INSTALL_LOCK = true
SECRET_KEY = $SECRET_KEY
INTERNAL_TOKEN = $INTERNAL_TOKEN

[oauth2]
JWT_SECRET = $JWT_SECRET

[service]
DISABLE_REGISTRATION = false

[session]
PROVIDER = file
PROVIDER_CONFIG = $DATA_DIR/sessions

[picture]
AVATAR_UPLOAD_PATH = $DATA_DIR/data/avatars
REPOSITORY_AVATAR_UPLOAD_PATH = $DATA_DIR/data/repo-avatars

[attachment]
PATH = $DATA_DIR/data/attachments

[lfs]
PATH = $DATA_DIR/data/lfs

[log]
MODE = console
ROOT_PATH = $DATA_DIR/log
LEVEL = Info

[git]
PATH = $APP_DIR/git/usr/bin/git
HOME_PATH = $HOME

[actions]
ENABLED = false
EOF
  chmod 0600 "$TEMP_INI"
  mv "$TEMP_INI" "$APP_INI"
fi

# DATA_DIR persists across restarts and package upgrades, so also repair the
# setting written by earlier builds without replacing the rest of app.ini.
TEMP_INI="$APP_INI.tmp.$$"
if ! awk -v run_user="$APP_RUN_USER" '
  BEGIN { updated = 0 }
  /^[[:space:]]*RUN_USER[[:space:]]*=/ {
    if (!updated) print "RUN_USER = " run_user
    updated = 1
    next
  }
  /^[[:space:]]*\[/ && !updated {
    print "RUN_USER = " run_user
    updated = 1
  }
  { print }
  END {
    if (!updated) print "RUN_USER = " run_user
  }
' "$APP_INI" >"$TEMP_INI"; then
  rm -f "$TEMP_INI"
  echo "ERROR: could not update RUN_USER in $APP_INI" >&2
  exit 1
fi
chmod 0600 "$TEMP_INI"
mv "$TEMP_INI" "$APP_INI"

TRAY_PID=""
GITEA_PID=""

stop_children() {
  trap - EXIT TERM INT
  if [[ -n "$GITEA_PID" ]] && kill -0 "$GITEA_PID" 2>/dev/null; then
    kill -TERM "$GITEA_PID" 2>/dev/null || true
  fi
  if [[ -n "$TRAY_PID" ]] && kill -0 "$TRAY_PID" 2>/dev/null; then
    kill -TERM "$TRAY_PID" 2>/dev/null || true
  fi
  if [[ -n "$GITEA_PID" ]]; then
    wait "$GITEA_PID" 2>/dev/null || true
  fi
  if [[ -n "$TRAY_PID" ]]; then
    wait "$TRAY_PID" 2>/dev/null || true
  fi
}

trap stop_children EXIT
trap 'exit 143' TERM
trap 'exit 130' INT

if [[ -n "${AGENTOS_TRAY_SOCKET:-}" ]]; then
  if [[ -x "$APP_DIR/tray" ]]; then
    "$APP_DIR/tray" &
    TRAY_PID=$!
  else
    echo "WARNING: AgentOS tray helper is missing; tray integration is disabled" >&2
  fi
fi

"$APP_DIR/gitea" web \
  --work-path "$DATA_DIR" \
  --custom-path "$GITEA_CUSTOM" \
  --config "$APP_INI" \
  --port "$PORT" &
GITEA_PID=$!

wait "$GITEA_PID"
