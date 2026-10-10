#!/usr/bin/env bash
# Build a self-contained ONLYOFFICE Docs AgentOS container-sandbox package.
#
# Usage:
#   ONLYOFFICE_ARCH=arm64 ./examples/agentos/process/only-office/build.sh
#   ONLYOFFICE_ARCH=x64 ./examples/agentos/process/only-office/build.sh

set -euo pipefail
export COPYFILE_DISABLE="${COPYFILE_DISABLE:-1}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ONLYOFFICE_VERSION="${ONLYOFFICE_VERSION:-9.4.0}"
ONLYOFFICE_IMAGE="${ONLYOFFICE_IMAGE:-onlyoffice/documentserver:${ONLYOFFICE_VERSION}}"
ONLYOFFICE_ARCH="${ONLYOFFICE_ARCH:-}"
ONLYOFFICE_PLATFORM="${ONLYOFFICE_PLATFORM:-}"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

if [[ -z "$ONLYOFFICE_ARCH" ]]; then
  case "$ONLYOFFICE_PLATFORM" in
    linux/arm64) ONLYOFFICE_ARCH=arm64 ;;
    linux/amd64) ONLYOFFICE_ARCH=x64 ;;
    "") ONLYOFFICE_ARCH=arm64 ;;
    *) fail "ONLYOFFICE_PLATFORM must be linux/arm64 or linux/amd64" ;;
  esac
fi

case "$ONLYOFFICE_ARCH" in
  arm64)
    APP_ARCH=arm64
    IMAGE_ARCH=arm64
    PLATFORM=linux/arm64
    ;;
  x64)
    APP_ARCH=x64
    IMAGE_ARCH=amd64
    PLATFORM=linux/amd64
    ;;
  *) fail "ONLYOFFICE_ARCH must be arm64 or x64" ;;
esac

if [[ -n "$ONLYOFFICE_PLATFORM" && "$ONLYOFFICE_PLATFORM" != "$PLATFORM" ]]; then
  fail "ONLYOFFICE_ARCH=$ONLYOFFICE_ARCH conflicts with ONLYOFFICE_PLATFORM=$ONLYOFFICE_PLATFORM"
fi

command -v docker >/dev/null 2>&1 || fail "docker is required"
command -v python3 >/dev/null 2>&1 || fail "python3 is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"
command -v gzip >/dev/null 2>&1 || fail "gzip is required"

for file in app.json start.sh server.cjs prepare-runtime.py extract-rootfs.py icon.png LICENSES.md README.md README-zh_CN.md; do
  [[ -f "$SCRIPT_DIR/$file" ]] || fail "missing package material: $SCRIPT_DIR/$file"
done

if ! docker image inspect "$ONLYOFFICE_IMAGE" >/dev/null 2>&1; then
  CACHE_ARCHIVE="$SCRIPT_DIR/.cache/documentserver-${ONLYOFFICE_VERSION}-${APP_ARCH}.tar"
  if [[ -f "$CACHE_ARCHIVE" ]]; then
    echo "==> Load cached official image archive $CACHE_ARCHIVE"
    docker load -i "$CACHE_ARCHIVE"
  else
    echo "==> Pull official ONLYOFFICE Docs Community image $ONLYOFFICE_IMAGE ($PLATFORM)"
    docker pull --platform "$PLATFORM" "$ONLYOFFICE_IMAGE"
  fi
fi

IMAGE_PLATFORM="$(docker image inspect --format '{{.Os}}/{{.Architecture}}' "$ONLYOFFICE_IMAGE")"
[[ "$IMAGE_PLATFORM" == "$PLATFORM" ]] || fail "image architecture is $IMAGE_PLATFORM, expected $PLATFORM"

BUILD_DIR="$(mktemp -d "$SCRIPT_DIR/.build.XXXXXX")"
PACKAGE_DIR="$BUILD_DIR/package"
CONTAINER_ID=""
OUTPUT="${ONLYOFFICE_OUTPUT:-$SCRIPT_DIR/dist/onlyoffice-docs-$APP_ARCH.agt}"
OUTPUT_TMP="$OUTPUT.tmp.$$"
APP_VERSION="$ONLYOFFICE_VERSION-agentos.15"
MAX_UPLOAD_BYTES="${ONLYOFFICE_MAX_UPLOAD_BYTES:-2147483648}"
UPLOAD_MARGIN_BYTES="${ONLYOFFICE_UPLOAD_MARGIN_BYTES:-16777216}"

cleanup() {
  if [[ -n "$CONTAINER_ID" ]]; then
    docker rm -f "$CONTAINER_ID" >/dev/null 2>&1 || true
  fi
  if [[ -d "$BUILD_DIR" ]]; then
    find "$BUILD_DIR" -type d -exec chmod u+rwx {} + >/dev/null 2>&1 || true
    rm -rf "$BUILD_DIR" >/dev/null 2>&1 || true
  fi
  rm -f "$OUTPUT_TMP" >/dev/null 2>&1 || true
}
trap cleanup EXIT

mkdir -p "$PACKAGE_DIR/deps" "$(dirname "$OUTPUT")"

echo "==> Generate the runtime font tables and editor assets"
CONTAINER_ID="$(docker create --platform "$PLATFORM" --entrypoint /usr/bin/documentserver-generate-allfonts.sh "$ONLYOFFICE_IMAGE" true)"
docker start -a "$CONTAINER_ID"
GENERATOR_STATUS="$(docker wait "$CONTAINER_ID")"
[[ "$GENERATOR_STATUS" == "0" ]] || fail "ONLYOFFICE asset generation exited with status $GENERATOR_STATUS"

echo "==> Export $ONLYOFFICE_IMAGE ($PLATFORM) into the AgentOS package"
docker export "$CONTAINER_ID" | python3 "$SCRIPT_DIR/extract-rootfs.py" - "$PACKAGE_DIR/deps"
[[ -s "$PACKAGE_DIR/deps/var/www/onlyoffice/documentserver/sdkjs/common/AllFonts.js" ]] || fail "generated editor asset sdkjs/common/AllFonts.js is missing"
[[ -s "$PACKAGE_DIR/deps/var/www/onlyoffice/documentserver/server/FileConverter/bin/AllFonts.js" ]] || fail "generated converter asset server/FileConverter/bin/AllFonts.js is missing"
[[ -s "$PACKAGE_DIR/deps/var/www/onlyoffice/documentserver/server/FileConverter/bin/font_selection.bin" ]] || fail "generated converter asset font_selection.bin is missing"
python3 - "$PACKAGE_DIR/deps/var/www/onlyoffice/documentserver/server/FileConverter/bin/font_selection.bin" "$PACKAGE_DIR/deps" <<'PY'
import struct
import sys
from pathlib import Path

path = Path(sys.argv[1])
deps_root = Path(sys.argv[2])
data = path.read_bytes()
replacements = (
    (
        b"/usr/share/fonts",
        b"/opt/agentos/app/deps/usr/share/fonts",
    ),
    (
        b"/var/www/onlyoffice/documentserver/core-fonts",
        b"/opt/agentos/app/deps/var/www/onlyoffice/documentserver/core-fonts",
    ),
)

def find_font_paths(blob, prefix):
    found = []
    cursor = 0
    while True:
        cursor = blob.find(prefix, cursor)
        if cursor < 0:
            return found
        if cursor >= 4:
            length = struct.unpack_from("<I", blob, cursor - 4)[0]
            candidate = blob[cursor : cursor + length]
            if candidate.startswith(prefix) and b"\0" not in candidate:
                found.append((cursor, length, candidate))
        cursor += len(prefix)

patched = bytearray(data)
patches = []
for old_prefix, new_prefix in replacements:
    matches = find_font_paths(data, old_prefix)
    if not matches:
        raise SystemExit(f"font_selection.bin has no paths under {old_prefix.decode()}")
    for offset, old_length, old_path in matches:
        suffix = old_path[len(old_prefix) :]
        patches.append((offset, old_length, new_prefix + suffix))

for offset, old_length, new_path in sorted(patches, reverse=True):
    if struct.unpack_from("<I", patched, offset - 4)[0] != old_length:
        raise SystemExit(f"font_selection.bin path length is inconsistent at byte {offset}")
    patched[offset - 4 : offset] = struct.pack("<I", len(new_path))
    patched[offset : offset + old_length] = new_path

for old_prefix, new_prefix in replacements:
    if find_font_paths(patched, old_prefix):
        raise SystemExit(f"font_selection.bin still contains runtime paths under {old_prefix.decode()}")
    if not find_font_paths(patched, new_prefix):
        raise SystemExit(f"font_selection.bin is missing rewritten paths under {new_prefix.decode()}")

runtime_root = b"/opt/agentos/app/deps/"
font_paths = [
    font_path
    for _, _, font_path in find_font_paths(
        patched, runtime_root
    )
]
missing = [
    font_path.decode()
    for font_path in font_paths
    if not (deps_root / font_path[len(runtime_root) :].decode()).is_file()
]
if missing:
    raise SystemExit(
        "font_selection.bin references font files missing from the app bundle: "
        + ", ".join(missing[:10])
    )

path.write_bytes(patched)
print(f"rewrote {len(patches)} font paths for the AgentOS app bundle")
print(f"validated {len(font_paths)} font_selection.bin font files in the app bundle")
PY
python3 - "$PACKAGE_DIR/deps/var/www/onlyoffice/documentserver/server/FileConverter/bin/AllFonts.js" "$PACKAGE_DIR/deps" <<'PY'
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
deps_root = Path(sys.argv[2])
data = path.read_bytes()
replacements = (
    (
        b"/usr/share/fonts",
        b"/opt/agentos/app/deps/usr/share/fonts",
    ),
    (
        b"/var/www/onlyoffice/documentserver/core-fonts",
        b"/opt/agentos/app/deps/var/www/onlyoffice/documentserver/core-fonts",
    ),
)

total = 0
for old_prefix, new_prefix in replacements:
    count = data.count(old_prefix)
    if not count:
        raise SystemExit(f"AllFonts.js has no paths under {old_prefix.decode()}")
    data = data.replace(old_prefix, new_prefix)
    if b'"' + old_prefix in data:
        raise SystemExit(f"AllFonts.js still contains paths under {old_prefix.decode()}")
    if b'"' + new_prefix not in data:
        raise SystemExit(f"AllFonts.js is missing rewritten paths under {new_prefix.decode()}")
    total += count

runtime_root = b"/opt/agentos/app/deps/"
font_paths = re.findall(
    rb'"(/opt/agentos/app/deps/[^\"]+\.(?:ttf|ttc|otf))"', data, flags=re.IGNORECASE
)
missing = [
    font_path.decode()
    for font_path in font_paths
    if not (deps_root / font_path[len(runtime_root) :].decode()).is_file()
]
if missing:
    raise SystemExit(
        "AllFonts.js references font files missing from the app bundle: "
        + ", ".join(missing[:10])
    )

path.write_bytes(data)
print(f"rewrote {total} converter font manifest paths for the AgentOS app bundle")
print(f"validated {len(font_paths)} converter font files in the AgentOS app bundle")
PY
# The official image ships these templates as root:root 0640. AgentOS runs
# container apps as an unprivileged user, so make the immutable templates
# readable before prepare-runtime.py copies them into the app's writable data.
chmod a+r "$PACKAGE_DIR/deps/etc/onlyoffice/documentserver/"*.json

python3 - "$SCRIPT_DIR/app.json" "$PACKAGE_DIR/app.json" "$APP_ARCH" "$APP_VERSION" <<'PY'
import json
import sys
from pathlib import Path

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
architecture = sys.argv[3]
version = sys.argv[4]
manifest = json.loads(source.read_text(encoding="utf-8"))
manifest["architecture"] = architecture
manifest["version"] = version
manifest["description"] = "A complete ONLYOFFICE Docs Community service running inside the AgentOS app container sandbox."
manifest["description_zh"] = "完整的 ONLYOFFICE Docs Community 服务，运行于 AgentOS 应用容器沙盒内。"
manifest["permissions"] = []
manifest["execution"] = {"mode": "container"}
destination.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
PY

for file in start.sh server.cjs prepare-runtime.py icon.png LICENSES.md README.md README-zh_CN.md; do
  cp "$SCRIPT_DIR/$file" "$PACKAGE_DIR/$file"
done
chmod 0755 "$PACKAGE_DIR/start.sh"
chmod 0644 "$PACKAGE_DIR/server.cjs" "$PACKAGE_DIR/prepare-runtime.py" "$PACKAGE_DIR/app.json"

grep -F '"mode": "container"' "$PACKAGE_DIR/app.json" >/dev/null || fail "package must use the AgentOS container sandbox"
grep -F '"permissions": []' "$PACKAGE_DIR/app.json" >/dev/null || fail "sandbox package must request no native permissions"

echo "==> Create compressed .agt package"
tar -cf - -C "$PACKAGE_DIR" . | gzip -1 >"$OUTPUT_TMP"
tar -tzf "$OUTPUT_TMP" >/dev/null || fail "built .agt archive is invalid"
python3 - "$OUTPUT_TMP" <<'PY'
import posixpath
import sys
import tarfile

archive_path = sys.argv[1]
with tarfile.open(archive_path, "r:gz") as archive:
    members = list(archive)

def normalized(name):
    while name.startswith("./"):
        name = name[2:]
    return name.rstrip("/")

names = {normalized(member.name) for member in members}
dangling = []
for member in members:
    if not member.issym():
        continue
    name = normalized(member.name)
    if member.linkname.startswith("/"):
        target = posixpath.normpath(member.linkname.lstrip("/"))
    else:
        target = posixpath.normpath(posixpath.join(posixpath.dirname(name), member.linkname))
    if target not in names:
        dangling.append((name, member.linkname, target))

if dangling:
    for name, link, target in dangling[:20]:
        print(f"dangling symlink: {name} -> {link} (resolves to {target})", file=sys.stderr)
    raise SystemExit(f"package contains {len(dangling)} dangling symbolic link(s)")
print(f"validated {sum(member.issym() for member in members)} package symlink(s)")
PY
tar -tzf "$OUTPUT_TMP" | grep -Fxq "./app.json" || fail "package is missing root app.json"
tar -tzf "$OUTPUT_TMP" | grep -Fxq "./start.sh" || fail "package is missing the sandbox launcher"
tar -tzf "$OUTPUT_TMP" | grep -Fxq "./deps/var/www/onlyoffice/documentserver/server/DocService/docservice" || fail "package is missing ONLYOFFICE Docs"
tar -tzf "$OUTPUT_TMP" | grep -Fxq "./deps/var/www/onlyoffice/documentserver/server/FileConverter/converter" || fail "package is missing the converter"
tar -tzf "$OUTPUT_TMP" | grep -Fxq "./deps/usr/sbin/nginx" || fail "package is missing Nginx"

PACKAGE_SIZE="$(python3 - "$OUTPUT_TMP" <<'PY'
import os
import sys
print(os.path.getsize(sys.argv[1]))
PY
)"
if (( PACKAGE_SIZE + UPLOAD_MARGIN_BYTES >= MAX_UPLOAD_BYTES )); then
  fail ".agt is $PACKAGE_SIZE bytes; it must stay at least $UPLOAD_MARGIN_BYTES bytes below the $MAX_UPLOAD_BYTES-byte upload limit"
fi

mv -f "$OUTPUT_TMP" "$OUTPUT"
echo "==> Self-contained ONLYOFFICE sandbox .agt is ready"
echo "    $OUTPUT"
echo "    size: $PACKAGE_SIZE bytes"
echo "    upload limit: $MAX_UPLOAD_BYTES bytes (including $UPLOAD_MARGIN_BYTES bytes of headroom)"
