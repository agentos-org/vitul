#!/usr/bin/env bash
# Build a standalone Dockge .agt package for a Linux AgentOS target.
#
# Usage:
#   DOCKGE_PLATFORM=linux/arm64 ./examples/agentos/process/dockge/build.sh
#   DOCKGE_PLATFORM=linux/amd64 ./examples/agentos/process/dockge/build.sh
#
# The unmodified Dockge 1.5.0 source is vendored under examples/agentos/process/dockge/vendor/dockge
# and copied to a temporary directory before dependency installation and
# frontend compilation. No upstream source files are patched or rewritten.

set -euo pipefail
export COPYFILE_DISABLE="${COPYFILE_DISABLE:-1}"

SCRIPT_DIR="$(cd "$(dirname "$BASH_SOURCE")" && pwd)"

DOCKGE_PLATFORM="${DOCKGE_PLATFORM:-linux/arm64}"
DOCKGE_BUILD_MODE="${DOCKGE_BUILD_MODE:-container}"
DOCKGE_NODE_IMAGE="${DOCKGE_NODE_IMAGE:-node:22.23.1-bookworm}"
SOURCE_DIR="$SCRIPT_DIR/vendor/dockge"

case "$DOCKGE_PLATFORM" in
  linux/arm64)
    APP_ARCH="arm64"
    ;;
  linux/amd64)
    APP_ARCH="x64"
    ;;
  *)
    echo "ERROR: DOCKGE_PLATFORM must be linux/arm64 or linux/amd64" >&2
    exit 1
    ;;
esac

DOCKGE_NPM_CACHE_DIR="${DOCKGE_NPM_CACHE_DIR:-$SCRIPT_DIR/.cache/npm-$APP_ARCH}"
DOCKGE_OUTPUT="${DOCKGE_OUTPUT:-$SCRIPT_DIR/dist/dockge-$APP_ARCH.agt}"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

[[ -f "$SOURCE_DIR/package.json" ]] || fail "missing vendored Dockge source: $SOURCE_DIR"
[[ -f "$SOURCE_DIR/package-lock.json" ]] || fail "vendored Dockge source has no package-lock.json"
[[ -f "$SOURCE_DIR/backend/index.ts" ]] || fail "vendored Dockge source has no backend/index.ts"
[[ -f "$SCRIPT_DIR/app.json" ]] || fail "missing package manifest: $SCRIPT_DIR/app.json"
[[ -f "$SCRIPT_DIR/start.sh" ]] || fail "missing app entrypoint: $SCRIPT_DIR/start.sh"
[[ -f "$SCRIPT_DIR/missing-docker-guide.cjs" ]] || fail "missing Docker prerequisite guide"

case "$DOCKGE_BUILD_MODE" in
  container)
    command -v docker >/dev/null 2>&1 || fail "docker is required for container builds"
    ;;
  host)
    [[ "$(uname -s)" == "Linux" ]] || fail "host builds require a Linux host matching DOCKGE_PLATFORM"
    case "$(uname -m)" in
      aarch64|arm64) HOST_PLATFORM="linux/arm64" ;;
      x86_64|amd64) HOST_PLATFORM="linux/amd64" ;;
      *) fail "unsupported host architecture: $(uname -m)" ;;
    esac
    [[ "$HOST_PLATFORM" == "$DOCKGE_PLATFORM" ]] || fail "host architecture $HOST_PLATFORM does not match $DOCKGE_PLATFORM"
    command -v npm >/dev/null 2>&1 || fail "npm is required for host builds"
    ;;
  *)
    fail "DOCKGE_BUILD_MODE must be container or host"
    ;;
esac

command -v tar >/dev/null 2>&1 || fail "tar is required"
mkdir -p "$SCRIPT_DIR" "$DOCKGE_NPM_CACHE_DIR"
BUILD_DIR="$(mktemp -d "$SCRIPT_DIR/.build.XXXXXX")"
trap 'rm -rf "$BUILD_DIR"' EXIT

WORK_SOURCE="$BUILD_DIR/source"
PACKAGE_DIR="$BUILD_DIR/package"
mkdir -p "$WORK_SOURCE" "$PACKAGE_DIR"
echo "==> Copy unmodified Dockge 1.5.0 source to temporary build directory"
tar -cf - -C "$SOURCE_DIR" . | tar -xf - -C "$WORK_SOURCE"

echo "==> Install production dependencies and build frontend ($DOCKGE_PLATFORM)"
if [[ "$DOCKGE_BUILD_MODE" == "container" ]]; then
  DOCKER_ARGS=(docker run --rm --platform "$DOCKGE_PLATFORM")
  if [[ -n "${DOCKGE_DOCKER_PROXY:-}" ]]; then
    DOCKER_ARGS+=(
      --env "HTTP_PROXY=$DOCKGE_DOCKER_PROXY"
      --env "HTTPS_PROXY=$DOCKGE_DOCKER_PROXY"
      --env "ALL_PROXY=$DOCKGE_DOCKER_PROXY"
      --env "http_proxy=$DOCKGE_DOCKER_PROXY"
      --env "https_proxy=$DOCKGE_DOCKER_PROXY"
      --env "all_proxy=$DOCKGE_DOCKER_PROXY"
      --env "NO_PROXY=${DOCKGE_DOCKER_NO_PROXY:-localhost,127.0.0.1}"
      --env "no_proxy=${DOCKGE_DOCKER_NO_PROXY:-localhost,127.0.0.1}"
    )
  fi
  MSYS_NO_PATHCONV=1 "${DOCKER_ARGS[@]}" \
    --user "$(id -u):$(id -g)" \
    --env HOME=/tmp/dockge-home \
    --env NPM_CONFIG_CACHE=/tmp/dockge-npm-cache \
    --volume "$WORK_SOURCE:/work:rw" \
    --volume "$DOCKGE_NPM_CACHE_DIR:/tmp/dockge-npm-cache:rw" \
    --workdir /work \
    "$DOCKGE_NODE_IMAGE" \
    bash -c '
      set -euo pipefail
      npm ci --no-audit --no-fund
      npm run build:frontend
      npm prune --omit=dev --no-audit --no-fund
    '
else
  (
    cd "$WORK_SOURCE"
    export NPM_CONFIG_CACHE="$DOCKGE_NPM_CACHE_DIR"
    npm ci --no-audit --no-fund
    npm run build:frontend
    npm prune --omit=dev --no-audit --no-fund
  )
fi

[[ -s "$WORK_SOURCE/frontend-dist/index.html" ]] || fail "frontend build did not produce frontend-dist/index.html"
[[ -f "$WORK_SOURCE/node_modules/tsx/dist/cli.mjs" ]] || fail "production dependencies did not include tsx"
[[ -d "$WORK_SOURCE/common" ]] || fail "upstream source has no common runtime directory"

sed "s/\"architecture\": \"arm64\"/\"architecture\": \"$APP_ARCH\"/" "$SCRIPT_DIR/app.json" >"$PACKAGE_DIR/app.json"
grep -F "\"architecture\": \"$APP_ARCH\"" "$PACKAGE_DIR/app.json" >/dev/null || fail "could not set package architecture"
cp "$SCRIPT_DIR/start.sh" "$PACKAGE_DIR/start.sh"
chmod 0755 "$PACKAGE_DIR/start.sh"
cp "$SCRIPT_DIR/missing-docker-guide.cjs" "$PACKAGE_DIR/missing-docker-guide.cjs"
cp "$SCRIPT_DIR/icon.png" "$PACKAGE_DIR/icon.png"
cp "$WORK_SOURCE/package.json" "$PACKAGE_DIR/package.json"
cp "$WORK_SOURCE/package-lock.json" "$PACKAGE_DIR/package-lock.json"
cp "$WORK_SOURCE/LICENSE" "$PACKAGE_DIR/LICENSE"
cp -R "$WORK_SOURCE/backend" "$PACKAGE_DIR/backend"
cp -R "$WORK_SOURCE/common" "$PACKAGE_DIR/common"
cp -R "$WORK_SOURCE/frontend-dist" "$PACKAGE_DIR/frontend-dist"
# Dereference .bin symlinks so the resulting package is portable across hosts.
cp -R -L "$WORK_SOURCE/node_modules" "$PACKAGE_DIR/node_modules"

mkdir -p "$(dirname "$DOCKGE_OUTPUT")"
rm -f "$DOCKGE_OUTPUT"
echo "==> Create $DOCKGE_OUTPUT"
tar -czf "$DOCKGE_OUTPUT" -C "$PACKAGE_DIR" .
tar -tzf "$DOCKGE_OUTPUT" >/dev/null
tar -tzf "$DOCKGE_OUTPUT" | grep -Fxq "./app.json" || fail "package is missing root app.json"
tar -tzf "$DOCKGE_OUTPUT" | grep -Fq "frontend-dist/index.html" || fail "package is missing Dockge frontend"

echo "==> Dockge .agt package is ready"
echo "    $DOCKGE_OUTPUT"
