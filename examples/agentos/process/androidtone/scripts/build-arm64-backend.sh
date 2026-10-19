#!/usr/bin/env bash
set -euo pipefail
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="$APP_DIR/agentos-build/bin"
TARGET_DIR="$APP_DIR/agentos-build/rust-target-arm64"
CARGO_HOME_DIR="$APP_DIR/agentos-build/cargo-home"
mkdir -p "$OUTPUT_DIR" "$TARGET_DIR" "$CARGO_HOME_DIR"

docker run --rm --platform linux/arm64 \
  --mount "type=bind,src=$APP_DIR/backend,dst=/src,readonly" \
  --mount "type=bind,src=$APP_DIR/vitul.conf.json,dst=/vitul.conf.json,readonly" \
  --mount "type=bind,src=$OUTPUT_DIR,dst=/out" \
  --mount "type=bind,src=$TARGET_DIR,dst=/target" \
  --mount "type=bind,src=$CARGO_HOME_DIR,dst=/cargo-home" \
  --env CARGO_HOME=/cargo-home \
  --env CARGO_TARGET_DIR=/target \
  rust:1.90-bookworm sh -ec '
    cargo build --release --locked --manifest-path /src/Cargo.toml
    install -m 0755 /target/release/androidtone-server /out/androidtone-server
  '

file "$OUTPUT_DIR/androidtone-server"
python3 - "$OUTPUT_DIR/androidtone-server" <<'PY'
import pathlib,struct,sys
p=pathlib.Path(sys.argv[1])
header=p.read_bytes()[:20]
if header[:4] != b"\x7fELF" or header[4] != 2 or header[5] != 1 or struct.unpack_from("<H",header,18)[0] != 183:
    raise SystemExit("Docker output is not an ELF64 ARM64 backend")
print(f"ARM64_BACKEND_BUILT {p.stat().st_size} bytes")
PY
