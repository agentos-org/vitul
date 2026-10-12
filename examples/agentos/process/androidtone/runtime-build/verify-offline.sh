#!/bin/sh
# Disposable Ubuntu ARM64 container with --network none. Never run on the host.
set -eu
export DEBIAN_FRONTEND=noninteractive
printf '#!/bin/sh\nexit 101\n' > /usr/sbin/policy-rc.d
chmod +x /usr/sbin/policy-rc.d
cp /payload/packages/*.deb /var/cache/apt/archives/
apt-get install -y --no-install-recommends --no-download /payload/packages/*.deb
apt-get check
waydroid --version
lxc-start --version
python3 -c 'import dbus, gi, gbinder; print("Python runtime imports OK")'
printf 'OFFLINE_INSTALL_VERIFIED\n'
