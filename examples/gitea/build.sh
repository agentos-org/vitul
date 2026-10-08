#!/usr/bin/env bash
# Build a standalone sandboxed Gitea .agt package for AgentOS.
#
# Usage:
#   GITEA_PLATFORM=linux/arm64 ./examples/gitea/build.sh
#   GITEA_PLATFORM=linux/amd64 ./examples/gitea/build.sh

set -euo pipefail
export COPYFILE_DISABLE="${COPYFILE_DISABLE:-1}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GITEA_VERSION="1.27.3"
GITEA_PLATFORM="${GITEA_PLATFORM:-linux/arm64}"
GITEA_BUILD_IMAGE="${GITEA_BUILD_IMAGE:-ubuntu:24.04}"
GITEA_CURL_DIRECT="${GITEA_CURL_DIRECT:-false}"

case "$GITEA_PLATFORM" in
  linux/arm64)
    APP_ARCH="arm64"
    TRAY_ARCH="arm64"
    UPSTREAM_ARCH="arm64"
    GITEA_SHA256="04c086d36dba793546e331484a9da34571763efdfa77dc526cc98e0f10917e7b"
    ;;
  linux/amd64)
    APP_ARCH="x64"
    TRAY_ARCH="amd64"
    UPSTREAM_ARCH="amd64"
    GITEA_SHA256="4da93c2c10b6980c359bcb86d5573ebfd7770e2e151756534edee24c8c12d971"
    ;;
  *)
    echo "ERROR: GITEA_PLATFORM must be linux/arm64 or linux/amd64" >&2
    exit 1
    ;;
esac

GITEA_CACHE_DIR="${GITEA_CACHE_DIR:-$SCRIPT_DIR/.cache}"
GITEA_BINARY="${GITEA_BINARY:-$GITEA_CACHE_DIR/gitea-$GITEA_VERSION-linux-$UPSTREAM_ARCH}"
GIT_RUNTIME_CACHE_DIR="${GIT_RUNTIME_CACHE_DIR:-$GITEA_CACHE_DIR/git-runtime-$APP_ARCH}"
GITEA_OUTPUT="${GITEA_OUTPUT:-$SCRIPT_DIR/dist/gitea-$APP_ARCH.agt}"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v docker >/dev/null 2>&1 || fail "docker is required to bundle Git for the AgentOS container runtime"
command -v go >/dev/null 2>&1 || fail "Go is required to build the AgentOS tray helper"
command -v tar >/dev/null 2>&1 || fail "tar is required"
if command -v sha256sum >/dev/null 2>&1; then
  SHA256_CHECK=sha256sum
elif command -v shasum >/dev/null 2>&1; then
  SHA256_CHECK=shasum
else
  fail "sha256sum or shasum is required"
fi
[[ -f "$SCRIPT_DIR/app.json" ]] || fail "missing package manifest: $SCRIPT_DIR/app.json"
[[ -f "$SCRIPT_DIR/start.sh" ]] || fail "missing app entrypoint: $SCRIPT_DIR/start.sh"
[[ -f "$SCRIPT_DIR/tray.go" ]] || fail "missing AgentOS tray helper source: $SCRIPT_DIR/tray.go"
[[ -f "$SCRIPT_DIR/icon.png" ]] || fail "missing package icon: $SCRIPT_DIR/icon.png"

mkdir -p "$GITEA_CACHE_DIR"
if [[ ! -s "$GITEA_BINARY" ]]; then
  DOWNLOAD="$GITEA_BINARY.download"
  rm -f "$DOWNLOAD"
  echo "==> Download official Gitea $GITEA_VERSION ($GITEA_PLATFORM)"
  CURL_ARGS=(curl -fL --retry 3 --retry-delay 2 --retry-all-errors --connect-timeout 30)
  if [[ "$GITEA_CURL_DIRECT" == "true" ]]; then
    CURL_ARGS+=(--noproxy '*')
  fi
  "${CURL_ARGS[@]}" \
    "https://dl.gitea.com/gitea/$GITEA_VERSION/gitea-$GITEA_VERSION-linux-$UPSTREAM_ARCH" \
    -o "$DOWNLOAD"
  mv "$DOWNLOAD" "$GITEA_BINARY"
fi

if [[ "$SHA256_CHECK" == "sha256sum" ]]; then
  printf '%s  %s\n' "$GITEA_SHA256" "$GITEA_BINARY" | sha256sum -c - || fail "Gitea binary checksum mismatch"
else
  printf '%s  %s\n' "$GITEA_SHA256" "$GITEA_BINARY" | shasum -a 256 -c - || fail "Gitea binary checksum mismatch"
fi

BUILD_DIR="$(mktemp -d "$SCRIPT_DIR/.build.XXXXXX")"
trap 'rm -rf "$BUILD_DIR"' EXIT
PACKAGE_DIR="$BUILD_DIR/package"
GIT_ROOT="$BUILD_DIR/git"
mkdir -p "$PACKAGE_DIR" "$GIT_ROOT"

echo "==> Bundle Git and its runtime libraries ($GITEA_PLATFORM)"
if [[ -x "$GIT_RUNTIME_CACHE_DIR/usr/bin/git" && -d "$GIT_RUNTIME_CACHE_DIR/usr/lib/git-core" ]]; then
  echo "    Reuse $GIT_RUNTIME_CACHE_DIR"
  cp -R "$GIT_RUNTIME_CACHE_DIR/." "$GIT_ROOT/"
else
  rm -rf "$GIT_RUNTIME_CACHE_DIR"
  MSYS_NO_PATHCONV=1 docker run --rm --platform "$GITEA_PLATFORM" \
    --volume "$GIT_ROOT:/out:rw" \
    --env DEBIAN_FRONTEND=noninteractive \
    "$GITEA_BUILD_IMAGE" bash -euo pipefail -c '
      apt-get update
      apt-get install -y --no-install-recommends git pax-utils file
      mkdir -p /out/usr/bin /out/usr/lib /out/usr/share/git-core /out/lib
      cp -a /usr/bin/git /out/usr/bin/
      cp -a /usr/lib/git-core /out/usr/lib/
      cp -a /usr/share/git-core/templates /out/usr/share/git-core/

      while IFS= read -r -d "" executable; do
        if file -b "$executable" | grep -q "^ELF "; then
          while IFS= read -r dependency; do
            case "$dependency" in
              /*)
                [[ "$dependency" == "$executable" ]] && continue
                name="$(basename "$dependency")"
                if [[ ! -e "/out/lib/$name" ]]; then
                  cp -L "$dependency" "/out/lib/$name"
                fi
                ;;
            esac
          done < <(lddtree -l "$executable")
        fi
      done < <(find /usr/bin/git /usr/lib/git-core -type f -print0)
      chmod -R a+rX /out
    '
  mkdir -p "$GIT_RUNTIME_CACHE_DIR"
  cp -R "$GIT_ROOT/." "$GIT_RUNTIME_CACHE_DIR/"
fi

echo "==> Assemble Gitea .agt contents"
sed "s/\"architecture\": \"arm64\"/\"architecture\": \"$APP_ARCH\"/" "$SCRIPT_DIR/app.json" >"$PACKAGE_DIR/app.json"
grep -F "\"architecture\": \"$APP_ARCH\"" "$PACKAGE_DIR/app.json" >/dev/null || fail "could not set package architecture"
cp "$GITEA_BINARY" "$PACKAGE_DIR/gitea"
chmod 0755 "$PACKAGE_DIR/gitea"
cp "$SCRIPT_DIR/start.sh" "$PACKAGE_DIR/start.sh"
chmod 0755 "$PACKAGE_DIR/start.sh"
echo "==> Build AgentOS tray helper ($TRAY_ARCH)"
GOOS=linux GOARCH="$TRAY_ARCH" CGO_ENABLED=0 \
  go build -trimpath -ldflags="-s -w" -o "$PACKAGE_DIR/tray" "$SCRIPT_DIR/tray.go"
chmod 0755 "$PACKAGE_DIR/tray"
cp "$SCRIPT_DIR/icon.png" "$PACKAGE_DIR/icon.png"
cp "$SCRIPT_DIR/LICENSE" "$PACKAGE_DIR/LICENSE"
cp -R "$GIT_ROOT/." "$PACKAGE_DIR/git/"

mkdir -p "$(dirname "$GITEA_OUTPUT")"
rm -f "$GITEA_OUTPUT"
echo "==> Create $GITEA_OUTPUT"
tar -czf "$GITEA_OUTPUT" -C "$PACKAGE_DIR" .
tar -tzf "$GITEA_OUTPUT" >/dev/null
tar -tzf "$GITEA_OUTPUT" | grep -Fxq "./app.json" || fail "package is missing root app.json"
tar -tzf "$GITEA_OUTPUT" | grep -Fxq "./gitea" || fail "package is missing the Gitea binary"
tar -tzf "$GITEA_OUTPUT" | grep -Fxq "./tray" || fail "package is missing the AgentOS tray helper"
tar -tzf "$GITEA_OUTPUT" | grep -Fxq "./git/usr/bin/git" || fail "package is missing Git"
tar -tzf "$GITEA_OUTPUT" | grep -Fxq "./git/usr/share/git-core/templates/description" || fail "package is missing Git templates"

echo "==> Gitea .agt package is ready"
echo "    $GITEA_OUTPUT"
