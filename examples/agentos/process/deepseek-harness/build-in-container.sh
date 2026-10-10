#!/usr/bin/env bash
set -euo pipefail

PLATFORM="${DSH_PLATFORM:?DSH_PLATFORM is required}"
case "$PLATFORM" in
  linux/arm64) NODE_ARCH=arm64; APP_ARCH=arm64 ;;
  linux/amd64) NODE_ARCH=x64; APP_ARCH=x64 ;;
  *) echo "Unsupported DSH target: $PLATFORM" >&2; exit 1 ;;
esac

BUILD_ROOT="$(mktemp -d /tmp/dsh-agentos-build.XXXXXX)"
trap 'rm -rf "$BUILD_ROOT"' EXIT
SOURCE="$BUILD_ROOT/source"
mkdir -p "$SOURCE"

# Build from a disposable copy so package output never modifies the neighboring checkout.
tar -C /upstream \
  --exclude='./.git' \
  --exclude='./.env' \
  --exclude='./.env.*' \
  --exclude='./node_modules' \
  --exclude='node_modules' \
  --exclude='./.pnpm-store' \
  --exclude='./.dsh-build' \
  --exclude='./dist-exe' \
  --exclude='./.release' \
  --exclude='./python/sdk-runtime/src/deepseek_harness_runtime/runtime' \
  -cf - . | tar -C "$SOURCE" -xf -
cp -R /upstream/.git "$SOURCE/.git"

APT_PROXY_ARGS=()
if [[ -n "${http_proxy:-}" ]]; then APT_PROXY_ARGS+=(-o "Acquire::http::Proxy=$http_proxy"); fi
if [[ -n "${https_proxy:-}" ]]; then APT_PROXY_ARGS+=(-o "Acquire::https::Proxy=$https_proxy"); fi
APT_CACHE_ARGS=(-o Dir::State::Lists=/dsh-cache/apt/lists)
mkdir -p /dsh-cache/apt/lists/partial /dsh-cache/apt/archives/partial
apt-get "${APT_PROXY_ARGS[@]}" "${APT_CACHE_ARGS[@]}" update
apt-get "${APT_PROXY_ARGS[@]}" "${APT_CACHE_ARGS[@]}" \
  -o Dir::Cache::archives=/dsh-cache/apt/archives \
  install -y --no-install-recommends build-essential ca-certificates curl git musl-tools python3 tar

# The adjacent checkout can contain ignored TypeScript build output from a different
# revision. Keep the disposable build copy to tracked source before compiling.
git -C "$SOURCE" clean -xfdq

# Apply the small AgentOS-specific runtime fixes in the disposable source copy:
# allow settings on the authenticated WebShell surface and use physical icon
# sidecars when package metadata resolves an image omitted from the executable snapshot.
git -C "$SOURCE" apply /package/agentos-runtime.patch

cd "$SOURCE"
corepack enable
corepack prepare pnpm@11.7.0 --activate
pnpm config set store-dir /dsh-cache/pnpm --location project
pnpm install --frozen-lockfile
pnpm exec tsx scripts/build-exe-for-python-sdk.ts --targets="node24-linux-$NODE_ARCH"

PRODUCT="$SOURCE/dist-exe/deepseek-harness-sdk-runtime-linux-$NODE_ARCH"
[[ -x "$PRODUCT" ]] || { echo "DSH executable was not produced: $PRODUCT" >&2; exit 1; }

VERSION="$(node -p "require('./package.json').version")"
PACKAGE_DIR="$BUILD_ROOT/package"
mkdir -p "$PACKAGE_DIR"
cp /package/app.json /package/start.sh /package/proxy.mjs /package/icon.svg /package/icon.png \
  /package/LICENSE /package/THIRD_PARTY_NOTICES.md "$PACKAGE_DIR/"
cp "$PRODUCT" "$PACKAGE_DIR/"
cp "$SOURCE/dist-exe/deepseek-harness-sdk-runtime-linux-$NODE_ARCH-rg" "$PACKAGE_DIR/"
cp -R "$SOURCE/dist-exe/deepseek-harness-sdk-runtime-linux-$NODE_ARCH-office" "$PACKAGE_DIR/"
cp -R "$SOURCE/dist-exe/linux-$NODE_ARCH" "$PACKAGE_DIR/"
for pair in \
  'dsh-experimental-agent-team-profile:agent-team-profile' \
  'dsh-experimental-auto-review:auto-review' \
  'dsh-experimental-inspector-profile:inspector-profile' \
  'dsh-experimental-voice-input-bundle:voice-input-bundle'
do
  package_name="${pair%%:*}"
  source_directory="${pair#*:}"
  package_directory="$PACKAGE_DIR/plugin-icons/@deepseek-ai/$package_name"
  mkdir -p "$package_directory"
  cp "$SOURCE/packages/experimental/$source_directory/icon.svg" "$package_directory/icon.svg"
  cp "$SOURCE/packages/experimental/$source_directory/package.json" "$package_directory/package.json"
done
cat >"$PACKAGE_DIR/build-info.json" <<EOF
{
  "dshVersion": "$VERSION",
  "dshCommit": "${DSH_SOURCE_COMMIT:?DSH_SOURCE_COMMIT is required}",
  "dshDirty": ${DSH_SOURCE_DIRTY:?DSH_SOURCE_DIRTY is required},
  "target": "$PLATFORM",
  "nodeTarget": "node24-linux-$NODE_ARCH"
}
EOF

node - "$PACKAGE_DIR/app.json" "$APP_ARCH" "$VERSION" <<'NODE'
const fs = require('node:fs')
const [manifestPath, architecture, version] = process.argv.slice(2)
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'))
const agentosSuffix = typeof manifest.version === 'string'
  ? manifest.version.match(/-agentos\.\d+$/)?.[0]
  : undefined
if (!agentosSuffix) throw new Error('app.json version must end in -agentos.<number>')
manifest.architecture = architecture
manifest.version = `${version}${agentosSuffix}`
fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
NODE

AGT="/out/deepseek-harness-$APP_ARCH.agt"
tar -czf "$AGT" -C "$PACKAGE_DIR" .
ARCHIVE_LIST="$BUILD_ROOT/archive-files.txt"
tar -tzf "$AGT" >"$ARCHIVE_LIST"
require_archive_entry() {
  local expected="$1"
  if ! grep -Fxq "$expected" "$ARCHIVE_LIST"; then
    echo "package is missing $expected" >&2
    echo "Package root files:" >&2
    ls -1 "$PACKAGE_DIR" >&2
    echo "Archive entries near the root:" >&2
    sed -n '1,40p' "$ARCHIVE_LIST" >&2
    exit 1
  fi
}
require_archive_entry './app.json'
require_archive_entry './start.sh'
require_archive_entry './icon.png'
require_archive_entry "./deepseek-harness-sdk-runtime-linux-$NODE_ARCH"
require_archive_entry "./linux-$NODE_ARCH/primary-runtime/dependencies/python/bin/python3"
require_archive_entry "./linux-$NODE_ARCH/primary-runtime/dependencies/node/bin/node"

sha256sum "$PRODUCT" | awk '{print "DSH executable SHA-256: " $1}'
du -h "$AGT"
