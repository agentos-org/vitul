#!/bin/sh
# Install the complete offline payload in a disposable ARM64 container and
# exercise the image package's exact first-initialization maintainer script.
set -eu
export DEBIAN_FRONTEND=noninteractive
export PATH="/usr/sbin:/usr/bin:/sbin:/bin:$PATH"
printf '#!/bin/sh\nexit 101\n' > /usr/sbin/policy-rc.d
chmod 0755 /usr/sbin/policy-rc.d
cp /payload/packages/*.deb /var/cache/apt/archives/
# Simulate the Binder device nodes provided by the preceding AgentOS actions.
# These regular files are disposable test fixtures, not runtime payload files.
for device in binder hwbinder vndbinder; do
  : > "/dev/$device"
done
# Waydroid probes the optional legacy ashmem device during init. An executable
# is present in production hosts through kmod; this test stub reports that the
# optional module is unavailable while memfd remains provided by the kernel.
cat > /usr/sbin/modprobe <<'EOF'
#!/bin/sh
exit 1
EOF
chmod 0755 /usr/sbin/modprobe
apt-get install -y --no-install-recommends --no-download /payload/packages/*.deb
apt-get check
test -s /var/lib/waydroid/waydroid.cfg
grep -Eq '^images_path[[:space:]]*=[[:space:]]*/usr/share/waydroid-extra/images$' /var/lib/waydroid/waydroid.cfg
test -s /usr/share/waydroid-extra/images/system.img
test -s /usr/share/waydroid-extra/images/vendor.img
grep -q '"packageVersion": "0.1.8-1"' /usr/share/waydroid-extra/images/androidtone-image-info.json
grep -q '"imageArchitecture": "arm64_only"' /usr/share/waydroid-extra/images/androidtone-image-info.json
printf 'WAYDROID_INIT_VERIFIED\n'
