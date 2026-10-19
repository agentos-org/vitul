#!/bin/sh
set -eu

# AgentOS must inject the allocated private Wayland endpoint into this sandbox.
: "${WAYLAND_DISPLAY:?AgentOS private Wayland display is missing}"
: "${XDG_RUNTIME_DIR:?AgentOS private runtime directory is missing}"
case "$WAYLAND_DISPLAY" in */*|.|..) echo "Invalid private Wayland display" >&2; exit 64 ;; esac
[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] || { echo "Allocated Wayland socket is unavailable" >&2; exit 69; }
unset DISPLAY
# Wine's Ubuntu build mishandles TMPDIR when /run/user/$UID is unavailable
# inside an AgentOS session and aborts wineserver with "free(): invalid pointer".
unset TMPDIR

runtime=/grants/runtime/native-arm64/rootfs
wine_loader="$runtime/usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1"
export PATH="$runtime/usr/bin:$runtime/usr/sbin:$PATH"
export LD_LIBRARY_PATH="$runtime/lib:$runtime/lib/aarch64-linux-gnu:$runtime/usr/lib:$runtime/usr/lib/aarch64-linux-gnu:$runtime/usr/lib/aarch64-linux-gnu/wine:$runtime/usr/lib/wine${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export WINEDLLPATH="$runtime/usr/lib/aarch64-linux-gnu/wine:$runtime/usr/lib/wine"
export WINELOADER="$runtime/usr/lib/wine/wine64"
export WINESERVER="$runtime/usr/lib/wine/wineserver"
export FONTCONFIG_PATH="$runtime/etc/fonts"
export FONTCONFIG_FILE="$runtime/etc/fonts/fonts.conf"
export SSL_CERT_FILE="$runtime/etc/ssl/certs/ca-certificates.crt"

: "${WINEPREFIX:?Wine prefix grant is missing}"
wine_command=${1:?Wine command is missing}
wineboot_command=${2:?wineboot command is missing}
shift 2
if [ "${1:-}" != "--" ]; then
  echo "WinTone native ARM64 launch arguments are malformed." >&2
  exit 64
fi
shift

run_wine_command() {
  command_line=$1
  shift
  sh -c "exec \"$wine_loader\" --library-path \"\$LD_LIBRARY_PATH\" $command_line \"\$@\"" wintone "$@"
}

if [ ! -f "$WINEPREFIX/.wintone-wayland-ready" ]; then
  run_wine_command "$wineboot_command"
  run_wine_command "$wine_command" reg.exe add 'HKCU\Software\Wine\Drivers' /v Graphics /d wayland /f
  : > "$WINEPREFIX/.wintone-wayland-ready"
fi

run_wine_command "$wine_command" "$@"
