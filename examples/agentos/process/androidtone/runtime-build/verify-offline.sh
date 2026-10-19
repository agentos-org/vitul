#!/bin/sh
# Disposable Ubuntu ARM64 container with --network none. Never run on the host.
set -eu
export DEBIAN_FRONTEND=noninteractive
printf '#!/bin/sh\nexit 101\n' > /usr/sbin/policy-rc.d
chmod +x /usr/sbin/policy-rc.d
cp /payload/packages/*.deb /var/cache/apt/archives/
# Android images are a separate, system-authorized package. The generic
# dependency check has no Binder devices and must not run that package's
# Waydroid initialization maintainer script.
set --
for deb in /payload/packages/*.deb; do
  [ -f "$deb" ] || continue
  [ "$(dpkg-deb -f "$deb" Package)" = androidtone-waydroid-images ] && continue
  set -- "$@" "$deb"
done
apt-get install -y --no-install-recommends --no-download "$@"
apt-get check
waydroid --version
lxc-start --version
python3 -c 'import dbus, gi, gbinder; print("Python runtime imports OK")'
printf 'OFFLINE_INSTALL_VERIFIED\n'
