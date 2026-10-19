#!/bin/sh
set -eu

# Ubuntu's Wine wrapper hard-codes /usr/lib/wine, which is outside the
# read-only runtime grant when Wine is launched without chroot. Resolve the
# helper beside this wrapper so the bundled rootfs remains relocatable.
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ -x "$script_dir/wineserver64" ]; then
  wineserver="$script_dir/wineserver64"
elif [ -x "$script_dir/wineserver32" ]; then
  wineserver="$script_dir/wineserver32"
else
  echo "error: unable to find bundled wineserver executable." >&2
  exit 1
fi

exec "$wineserver" -p0 "$@"
