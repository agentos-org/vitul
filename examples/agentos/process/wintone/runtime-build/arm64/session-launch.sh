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

launcher_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
runtime=$(CDPATH= cd "$launcher_dir/../.." && pwd)
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

# Wine traverses DOS drive paths using directory-relative syscalls. FEX's
# absolute-path RootFS fallback does not apply to those traversals: Z: must
# name the actual mounted x86 RootFS, while Y: exposes the separate file grants.
# Precreating dosdevices also bypasses Wine's default C: creation, so prepare it.
prefix_drives="$WINEPREFIX/dosdevices"
setup_needed=0
[ -f "$WINEPREFIX/.wintone-wayland-ready" ] || setup_needed=1
mkdir -p "$prefix_drives" "$WINEPREFIX/drive_c"

prefix_z="$prefix_drives/z:"
if [ -L "$prefix_z" ]; then
  z_target=$(readlink "$prefix_z")
  case "$z_target" in
    "$FEX_ROOTFS") ;;
    /) ln -sfn "$FEX_ROOTFS" "$prefix_z"; setup_needed=1 ;;
    *) echo "WinTone cannot replace the custom Wine Z: mapping: $z_target" >&2; exit 78 ;;
  esac
elif [ -e "$prefix_z" ]; then
  echo 'WinTone requires a Wine Z: symlink to the FEX RootFS.' >&2
  exit 78
else
  ln -s "$FEX_ROOTFS" "$prefix_z"
  setup_needed=1
fi

prefix_c="$prefix_drives/c:"
if [ ! -e "$prefix_c" ] && [ ! -L "$prefix_c" ]; then
  ln -s ../drive_c "$prefix_c"
  setup_needed=1
fi

grants_root=$(CDPATH= cd "$WINEPREFIX/.." && pwd)
prefix_y="$prefix_drives/y:"
if [ -L "$prefix_y" ]; then
  if [ "$(readlink "$prefix_y")" != "$grants_root" ]; then
    echo 'WinTone cannot replace the custom Wine Y: mapping; Y: is required for session file grants.' >&2
    exit 78
  fi
elif [ -e "$prefix_y" ]; then
  echo 'WinTone requires a Wine Y: symlink to session file grants.' >&2
  exit 78
else
  ln -s "$grants_root" "$prefix_y"
  setup_needed=1
fi

if [ "$setup_needed" = 1 ]; then
  # A previous failed bootstrap may have left registry files but no C: drive.
  # Keep that prefix and its applications; wineboot repairs it in place.
  rm -f "$WINEPREFIX/.wintone-wayland-ready"
  "$fexbash" -c "$wineboot_command"
  if [ ! -s "$prefix_c/windows/system32/kernel32.dll" ]; then
    echo 'WinTone Wine setup did not create kernel32.dll; the prefix is not ready.' >&2
    exit 70
  fi
  "$fexbash" -c "exec $wine_command reg.exe add 'HKCU\\Software\\Wine\\Drivers' /v Graphics /d wayland /f"
  # Wine's desktop processes cache the driver chosen during wineboot. Stop
  # this prefix's server before launching the app so the Wayland registry
  # setting takes effect on the very first run.
  # wineserver -k returns 1 when the prefix server already exited normally.
  "$fexbash" -c 'exec /usr/lib/wine/wineserver -k' || {
    server_status=$?
    [ "$server_status" = 1 ] || exit "$server_status"
  }
  "$fexbash" -c 'exec /usr/lib/wine/wineserver -w'
  : > "$WINEPREFIX/.wintone-wayland-ready"
fi

# Wine 9 lacks cryptbase.dll, which Chromium's random initialization requires.
# Repair existing prefixes too, without replacing a DLL supplied by Wine or an app.
cryptbase_compat="$runtime/fex/compat/x86_64/cryptbase.dll"
cryptbase_target="$prefix_c/windows/system32/cryptbase.dll"
if [ -f "$cryptbase_compat" ] && [ ! -e "$cryptbase_target" ] && [ ! -L "$cryptbase_target" ]; then
  cp "$cryptbase_compat" "$cryptbase_target"
fi

exec "$fexbash" -c "exec $wine_command \"\$@\"" wintone "$@"
