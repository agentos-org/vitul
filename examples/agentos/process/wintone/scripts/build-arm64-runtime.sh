#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_ARCH="${VITUL_TARGET_ARCH:-arm64}"
if [[ "$TARGET_ARCH" != "arm64" ]]; then
  echo "No bundled Wine/FEX runtime is configured for $TARGET_ARCH yet; skipping it."
  exit 0
fi

FINAL_RUNTIME_DIR="$PROJECT_ROOT/agentos-build/wintone-runtime"
mkdir -p "$PROJECT_ROOT/agentos-build"
BUILD_TEMP="$(mktemp -d "$PROJECT_ROOT/agentos-build/.wintone-runtime.XXXXXX")"
OUTPUT_DIR="$BUILD_TEMP/export"
RUNTIME_DIR="$BUILD_TEMP/wintone-runtime"
BACKUP_DIR="$BUILD_TEMP/previous-runtime"

cleanup() {
  if [[ -e "$BACKUP_DIR" && ! -e "$FINAL_RUNTIME_DIR" ]]; then
    mv "$BACKUP_DIR" "$FINAL_RUNTIME_DIR"
  fi
  rm -rf "$BUILD_TEMP"
}
trap cleanup EXIT

build_proxy_args=()
for lower_name in http_proxy https_proxy no_proxy; do
  case "$lower_name" in
    http_proxy) upper_name=HTTP_PROXY ;;
    https_proxy) upper_name=HTTPS_PROXY ;;
    no_proxy) upper_name=NO_PROXY ;;
  esac
  value="${!lower_name:-}"
  if [[ -z "$value" ]]; then
    value="${!upper_name:-}"
  fi
  if [[ -z "$value" ]]; then
    continue
  fi
  if [[ "$lower_name" != no_proxy ]]; then
    value="${value//127.0.0.1/host.docker.internal}"
    value="${value//localhost/host.docker.internal}"
  fi
  build_proxy_args+=(--build-arg "$lower_name=$value")
  build_proxy_args+=(--build-arg "$upper_name=$value")
done

docker buildx build \
  "${build_proxy_args[@]}" \
  --platform linux/arm64 \
  --file "$PROJECT_ROOT/runtime-build/arm64/Dockerfile" \
  --output "type=local,dest=$OUTPUT_DIR" \
  "$PROJECT_ROOT"

mkdir -p "$RUNTIME_DIR"
cp "$OUTPUT_DIR/wintone-runtime.tar.zst" "$RUNTIME_DIR/runtime-bundle.tar.zst"

python3 - "$OUTPUT_DIR/runtime-info.json" "$RUNTIME_DIR/runtime-catalog.json" "$RUNTIME_DIR/runtime-bundle.tar.zst" <<'PY'
import hashlib
import json
import pathlib
import re
import sys

info_path, catalog_path, bundle_path = map(pathlib.Path, sys.argv[1:])
info = json.loads(info_path.read_text())
bundle = bundle_path.read_bytes()
digest = hashlib.sha256(bundle).hexdigest()

def version_component(value):
    return re.sub(r"[^A-Za-z0-9._-]+", "-", value).strip(".-") or "unknown"

fex_version = info["fexVersion"]
wine_version = info["wineVersion"]
native_wine_version = info["nativeWineVersion"]
runtime_version = (
    f"arm64-fex-{version_component(fex_version)}"
    f"-w{version_component(wine_version)}"
    f"-n{version_component(native_wine_version)}-{digest[:12]}"
)
if len(runtime_version) > 80:
    raise ValueError(f"runtime version identifier is too long: {runtime_version}")
catalog = {
    "schemaVersion": 1,
    "version": runtime_version,
    "hostArchitecture": "aarch64",
    "wineVersion": wine_version,
    "nativeWineVersion": native_wine_version,
    "fexVersion": fex_version,
    "artifact": {
        "bundlePath": "runtime-bundle.tar.zst",
        "format": "tar.zst",
        "sha256": digest,
        "maxBytes": len(bundle),
    },
    "wineCommand": ["usr/lib/wine/wine64"],
    "winebootCommand": ["usr/lib/wine/wine64", "wineboot.exe", "-u"],
    "nativeWineCommand": ["native-arm64/rootfs/usr/lib/wine/wine64"],
    "nativeWinebootCommand": [
        "native-arm64/rootfs/usr/lib/wine/wine64",
        "wineboot.exe",
        "-u",
    ],
    "nativeSessionLauncher": ["native-arm64/bin/wintone-native-session-launch"],
    "fexCommand": ["fex/bin/wintone-session-launch"],
    "fexRootfs": "fex/rootfs",
}
catalog_path.write_text(json.dumps(catalog, indent=2) + "\n")
print(
    f"Prepared bundled runtime: x86-64 Wine {wine_version}; "
    f"native ARM64 Wine {native_wine_version}; FEX {fex_version}; {len(bundle)} bytes"
)
PY

if [[ -e "$FINAL_RUNTIME_DIR" ]]; then
  mv "$FINAL_RUNTIME_DIR" "$BACKUP_DIR"
fi
if ! mv "$RUNTIME_DIR" "$FINAL_RUNTIME_DIR"; then
  echo "Could not publish the rebuilt WinTone runtime bundle." >&2
  exit 1
fi
rm -rf "$BACKUP_DIR"
