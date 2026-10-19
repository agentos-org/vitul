#!/bin/sh
set -eu

test_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
launcher_source=${WINTONE_LAUNCHER_SOURCE:-$test_dir/session-launch.sh}
tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

make_case() {
  case_dir="$tmp_dir/$1"
  runtime="$case_dir/runtime"
  prefix="$case_dir/prefix"
  mkdir -p "$runtime/fex/bin" "$runtime/fex/rootfs" "$prefix" "$case_dir/wayland"
  cp "$launcher_source" "$runtime/fex/bin/wintone-session-launch"
  chmod 0755 "$runtime/fex/bin/wintone-session-launch"
  printf 'preserve prefix data\n' > "$prefix/sentinel"
  cat > "$runtime/fex/bin/FEXBash" <<'FAKE'
#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$FEX_CALL_LOG"
case "$*" in
  *wineboot*)
    [ "$(readlink "$WINEPREFIX/dosdevices/z:")" = "$EXPECTED_ROOTFS" ]
    [ "$(readlink "$WINEPREFIX/dosdevices/y:")" = "$EXPECTED_GRANTS" ]
    [ -d "$WINEPREFIX/dosdevices/c:" ]
    [ -z "${TMPDIR+x}" ]
    [ "${FAIL_BOOT:-0}" = 0 ] || exit 53
    if [ "${OMIT_KERNEL:-0}" = 0 ]; then
      mkdir -p "$WINEPREFIX/dosdevices/c:/windows/system32"
      printf 'fake PE DLL\n' > "$WINEPREFIX/dosdevices/c:/windows/system32/kernel32.dll"
    fi
    ;;
  *reg.exe*) [ "${FAIL_REG:-0}" = 0 ] || exit 54 ;;
  *'wineserver -k'*) [ "${FAIL_SERVER:-0}" = 0 ] || exit 55; [ "${SERVER_ALREADY_EXITED:-0}" = 0 ] || exit 1 ;;
  *wineserver*) [ "${FAIL_SERVER:-0}" = 0 ] || exit 55 ;;
esac
FAKE
  chmod 0755 "$runtime/fex/bin/FEXBash"
  python3 - "$case_dir/wayland/wayland-test" <<'PY'
import socket,sys
s=socket.socket(socket.AF_UNIX); s.bind(sys.argv[1]); s.close()
PY
}

run_launcher() {
  FEX_CALL_LOG="$case_dir/calls" EXPECTED_ROOTFS="$runtime/fex/rootfs" \
    EXPECTED_GRANTS="$case_dir" XDG_RUNTIME_DIR="$case_dir/wayland" \
    WAYLAND_DISPLAY=wayland-test WINEPREFIX="$prefix" \
    FEX_ROOTFS="$runtime/fex/rootfs" TMPDIR="$case_dir/unavailable" \
    SERVER_ALREADY_EXITED="${SERVER_ALREADY_EXITED:-0}" FAIL_SERVER="${FAIL_SERVER:-0}" FAIL_BOOT="${FAIL_BOOT:-0}" FAIL_REG="${FAIL_REG:-0}" OMIT_KERNEL="${OMIT_KERNEL:-0}" \
    "$runtime/fex/bin/wintone-session-launch" /usr/lib/wine/wine64 '/usr/lib/wine/wine64 wineboot.exe -u' -- sample.exe
}

assert_ready() {
  [ "$(cat "$prefix/sentinel")" = 'preserve prefix data' ]
  [ -f "$prefix/.wintone-wayland-ready" ]
  [ -s "$prefix/dosdevices/c:/windows/system32/kernel32.dll" ]
}

for name in fresh legacy stale_marker; do
  make_case "$name"
  if [ "$name" != fresh ]; then
    mkdir -p "$prefix/dosdevices"
    ln -s / "$prefix/dosdevices/z:"
  fi
  [ "$name" != stale_marker ] || : > "$prefix/.wintone-wayland-ready"
  run_launcher
  assert_ready
  [ "$(wc -l < "$case_dir/calls" | tr -d ' ')" = 5 ]
  run_launcher
  [ "$(wc -l < "$case_dir/calls" | tr -d ' ')" = 6 ]
done

make_case cryptbase_repair
mkdir -p "$runtime/fex/compat/x86_64"
printf 'compat DLL\n' > "$runtime/fex/compat/x86_64/cryptbase.dll"
run_launcher
cmp "$runtime/fex/compat/x86_64/cryptbase.dll" "$prefix/drive_c/windows/system32/cryptbase.dll"
# Existing, already initialized prefixes receive the same repair.
rm "$prefix/drive_c/windows/system32/cryptbase.dll"
run_launcher
cmp "$runtime/fex/compat/x86_64/cryptbase.dll" "$prefix/drive_c/windows/system32/cryptbase.dll"
printf 'existing DLL\n' > "$prefix/drive_c/windows/system32/cryptbase.dll"
run_launcher
[ "$(cat "$prefix/drive_c/windows/system32/cryptbase.dll")" = 'existing DLL' ]

make_case stopped_server
SERVER_ALREADY_EXITED=1
run_launcher
assert_ready
SERVER_ALREADY_EXITED=0

for mapping in z y; do
  make_case "custom_$mapping"
  mkdir -p "$prefix/dosdevices"
  ln -s "$tmp_dir/custom" "$prefix/dosdevices/$mapping:"
  rc=0
  run_launcher 2>/dev/null || rc=$?
  [ "$rc" = 78 ]
  [ "$(readlink "$prefix/dosdevices/$mapping:")" = "$tmp_dir/custom" ]
  [ ! -f "$prefix/.wintone-wayland-ready" ]
  [ "$(cat "$prefix/sentinel")" = 'preserve prefix data' ]
done

for failure in boot registry missing_kernel server; do
  make_case "$failure"
  FAIL_BOOT=0; FAIL_REG=0; OMIT_KERNEL=0; FAIL_SERVER=0
  case "$failure" in boot) FAIL_BOOT=1; expected=53 ;; registry) FAIL_REG=1; expected=54 ;; missing_kernel) OMIT_KERNEL=1; expected=70 ;; server) FAIL_SERVER=1; expected=55 ;; esac
  rc=0
  run_launcher 2>/dev/null || rc=$?
  [ "$rc" = "$expected" ]
  [ ! -f "$prefix/.wintone-wayland-ready" ]
  FAIL_BOOT=0; FAIL_REG=0; OMIT_KERNEL=0; FAIL_SERVER=0
  run_launcher
  assert_ready
done

echo 'FEX drive mapping, legacy-prefix repair, and failed-bootstrap retry tests passed'
