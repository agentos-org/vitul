#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="${DSH_SOURCE_DIR:-$SCRIPT_DIR/../../../../../GitHub/deepseek-harness}"
BUILD_IMAGE="${DSH_BUILD_IMAGE:-node:24-bookworm-slim}"
PLATFORM="${DSH_PLATFORM:-linux/arm64}"
CACHE_DIR="${DSH_BUILD_CACHE_DIR:-$SCRIPT_DIR/.cache}"

case "$PLATFORM" in
  linux/arm64) APP_ARCH=arm64 ;;
  linux/amd64) APP_ARCH=x64 ;;
  *) echo "ERROR: DSH_PLATFORM must be linux/arm64 or linux/amd64" >&2; exit 1 ;;
esac

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

command -v docker >/dev/null 2>&1 || fail "Docker is required to build the Linux runtime"
command -v node >/dev/null 2>&1 || fail "Node.js is required to write the package manifest"
command -v tar >/dev/null 2>&1 || fail "tar is required"
[[ -d "$SOURCE_DIR" ]] || fail "DeepSeek Harness source directory not found: $SOURCE_DIR (set DSH_SOURCE_DIR to its checkout)"
[[ -f "$SOURCE_DIR/package.json" && -f "$SOURCE_DIR/pnpm-lock.yaml" ]] || fail "DSH_SOURCE_DIR is not a DeepSeek Harness checkout"
[[ -f "$SCRIPT_DIR/build-in-container.sh" ]] || fail "missing Linux build entrypoint"
[[ -s "$SCRIPT_DIR/agentos-runtime.patch" ]] || fail "missing AgentOS runtime patch"
[[ -s "$SCRIPT_DIR/icon.png" ]] || fail "missing package icon: $SCRIPT_DIR/icon.png"

SOURCE_DIR="$(cd "$SOURCE_DIR" && pwd)"
SOURCE_COMMIT="$(git -C "$SOURCE_DIR" rev-parse HEAD 2>/dev/null || true)"
[[ -n "$SOURCE_COMMIT" ]] || fail "DSH_SOURCE_DIR must be a Git checkout so the build can be identified"
if [[ -n "$(git -C "$SOURCE_DIR" status --porcelain --untracked-files=all)" ]]; then
  SOURCE_DIRTY=true
else
  SOURCE_DIRTY=false
fi

BUILD_DIR="$(mktemp -d "$SCRIPT_DIR/.build.XXXXXX")"
trap 'rm -rf "$BUILD_DIR"' EXIT
mkdir -p "$BUILD_DIR/output" "$CACHE_DIR/pkg-home"
OUTPUT="${DSH_OUTPUT:-$SCRIPT_DIR/dist/deepseek-harness-$APP_ARCH.agt}"
[[ "$OUTPUT" == *.agt ]] || fail "DSH_OUTPUT must end in .agt"

PROXY_ENV_FILE="$BUILD_DIR/docker-proxy.env"
node - "$PROXY_ENV_FILE" <<'NODE'
const fs = require('node:fs')
const [filename] = process.argv.slice(2)
const values = {}
for (const name of ['HTTP_PROXY', 'HTTPS_PROXY']) {
  const value = process.env[name] ?? process.env[name.toLowerCase()]
  if (value === undefined || value === '') continue
  const proxy = new URL(value)
  if (['localhost', '127.0.0.1', '::1'].includes(proxy.hostname)) proxy.hostname = 'host.docker.internal'
  values[name] = proxy.toString()
  values[name.toLowerCase()] = proxy.toString()
}
const noProxy = process.env.NO_PROXY ?? process.env.no_proxy
if (noProxy !== undefined) {
  values.NO_PROXY = noProxy
  values.no_proxy = noProxy
}
for (const value of Object.values(values)) {
  if (/[\r\n]/.test(value)) throw new Error('proxy environment values must be single-line')
}
fs.writeFileSync(filename, `${Object.entries(values).map(([name, value]) => `${name}=${value}`).join('\n')}\n`, { mode: 0o600 })
NODE

echo "==> Build DeepSeek Harness $SOURCE_COMMIT for $PLATFORM in a temporary Linux workspace"
docker run --rm --platform "$PLATFORM" \
  --mount "type=bind,source=$SOURCE_DIR,target=/upstream,readonly" \
  --mount "type=bind,source=$SCRIPT_DIR,target=/package,readonly" \
  --mount "type=bind,source=$BUILD_DIR/output,target=/out" \
  --mount "type=bind,source=$CACHE_DIR,target=/dsh-cache" \
  --mount "type=bind,source=$CACHE_DIR/pkg-home,target=/pkg-home" \
  --env-file "$PROXY_ENV_FILE" \
  --env "DSH_PLATFORM=$PLATFORM" \
  --env "DSH_SOURCE_COMMIT=$SOURCE_COMMIT" \
  --env "DSH_SOURCE_DIRTY=$SOURCE_DIRTY" \
  --env COREPACK_HOME=/dsh-cache/corepack \
  --env HOME=/pkg-home \
  --env PKG_CACHE_PATH=/pkg-home/.pkg-cache \
  --env NODE_USE_ENV_PROXY=1 \
  "$BUILD_IMAGE" bash /package/build-in-container.sh

PACKAGE="$BUILD_DIR/output/deepseek-harness-$APP_ARCH.agt"
[[ -s "$PACKAGE" ]] || fail "Linux build did not produce the .agt package"

mkdir -p "$(dirname "$OUTPUT")"
rm -f "$OUTPUT"
echo "==> Copy $PACKAGE to $OUTPUT"
cp "$PACKAGE" "$OUTPUT"
ARCHIVE_LIST="$BUILD_DIR/output/archive-files.txt"
tar -tzf "$OUTPUT" >"$ARCHIVE_LIST" || fail "the generated .agt archive is invalid"
grep -Fxq './app.json' "$ARCHIVE_LIST" || fail "package is missing app.json"
grep -Fxq './start.sh' "$ARCHIVE_LIST" || fail "package is missing start.sh"
grep -Fxq './proxy.mjs' "$ARCHIVE_LIST" || fail "package is missing the login proxy"
grep -Fxq './icon.png' "$ARCHIVE_LIST" || fail "package is missing the AgentOS app icon"
for package_name in \
  dsh-experimental-agent-team-profile \
  dsh-experimental-auto-review \
  dsh-experimental-inspector-profile \
  dsh-experimental-voice-input-bundle
do
  grep -Fxq "./plugin-icons/@deepseek-ai/$package_name/icon.svg" "$ARCHIVE_LIST" || fail "package is missing the $package_name plugin icon"
done
grep -Fxq "./deepseek-harness-sdk-runtime-linux-$APP_ARCH" "$ARCHIVE_LIST" || fail "package is missing the DSH executable"
grep -Fxq "./linux-$APP_ARCH/primary-runtime/dependencies/python/bin/python3" "$ARCHIVE_LIST" || fail "package is missing the bundled Python runtime"
grep -Fxq "./linux-$APP_ARCH/primary-runtime/dependencies/node/bin/node" "$ARCHIVE_LIST" || fail "package is missing bundled Node.js"
tar -xOzf "$OUTPUT" ./app.json | node -e 'const fs=require("node:fs"); const app=JSON.parse(fs.readFileSync(0,"utf8")); if (app.architecture !== process.argv[1]) process.exit(1)' "$APP_ARCH" || fail "package manifest has the wrong architecture"
echo "==> DeepSeek Harness AgentOS package is ready"
echo "    $OUTPUT"
du -h "$OUTPUT"
