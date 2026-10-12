#!/bin/sh
set -eu

# AgentOS must inject the allocated private Wayland endpoint into this sandbox.
: "${WAYLAND_DISPLAY:?AgentOS private Wayland display is missing}"
: "${XDG_RUNTIME_DIR:?AgentOS private runtime directory is missing}"
case "$WAYLAND_DISPLAY" in */*|.|..) echo "Invalid private Wayland display" >&2; exit 64 ;; esac
[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] || { echo "Allocated Wayland socket is unavailable" >&2; exit 69; }
unset DISPLAY

runtime=/grants/runtime
fex="$runtime/fex/bin/FEX"
fexbash="$runtime/fex/bin/FEXBash"
export PATH="$runtime/fex/bin:$PATH"
export LD_LIBRARY_PATH="$runtime/fex/lib:$runtime/fex/lib/fex-emu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export FEX_THUNKHOSTLIBS="$runtime/fex/lib/fex-emu/HostThunks"
export FEX_THUNKGUESTLIBS="$runtime/fex/share/fex-emu/GuestThunks"
export FEX_THUNKCONFIG="$runtime/fex/share/fex-emu/ThunksDB.json"

: "${WINEPREFIX:?Wine prefix grant is missing}"
: "${FEX_ROOTFS:?FEX RootFS grant is missing}"
export FEX_APP_CONFIG_LOCATION="$WINEPREFIX/.fex-emu"
mkdir -p "$FEX_APP_CONFIG_LOCATION"

wine_command=${1:?Wine command is missing}
wineboot_command=${2:?Wineboot command is missing}
shift 2
if [ "${1:-}" != "--" ]; then
  echo "WinTone runtime launch arguments are malformed." >&2
  exit 64
fi
shift

if [ ! -f "$WINEPREFIX/.wintone-wayland-ready" ]; then
  "$fexbash" -c "$wineboot_command"
  "$fexbash" -c "exec $wine_command reg.exe add 'HKCU\\Software\\Wine\\Drivers' /v Graphics /d wayland /f"
  : > "$WINEPREFIX/.wintone-wayland-ready"
fi

exec "$fexbash" -c "exec $wine_command \"\$@\"" wintone "$@"
