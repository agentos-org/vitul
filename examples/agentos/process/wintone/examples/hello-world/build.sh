#!/bin/sh
set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

zig cc -target x86_64-windows-gnu -O2 \
  "$SCRIPT_DIR/hello.c" \
  -o "$SCRIPT_DIR/hello-windows-x64.exe" \
  -luser32 -Wl,--subsystem,windows

zig cc -target aarch64-windows-gnu -O2 \
  "$SCRIPT_DIR/hello.c" \
  -o "$SCRIPT_DIR/hello-windows-arm64.exe" \
  -luser32 -Wl,--subsystem,windows
