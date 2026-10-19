#!/bin/sh
# Inspect package contents inside the disposable builder; never install it.
set -eu
DEB=$(find /payload/packages -maxdepth 1 -name 'androidtone-waydroid-images_*.deb' -print -quit)
test -n "$DEB"
test "$(dpkg-deb --field "$DEB" Package)" = androidtone-waydroid-images
test "$(dpkg-deb --field "$DEB" Version)" = 0.1.8-1
dpkg-deb --contents "$DEB" > /tmp/image-package-files
grep -q './usr/share/waydroid-extra/images/system.img' /tmp/image-package-files
grep -q './usr/share/waydroid-extra/images/vendor.img' /tmp/image-package-files
grep -q './usr/share/waydroid-extra/images/androidtone-image-info.json' /tmp/image-package-files
dpkg-deb --extract "$DEB" /tmp/image-package-root
SYSTEM=/tmp/image-package-root/usr/share/waydroid-extra/images/system.img
VENDOR=/tmp/image-package-root/usr/share/waydroid-extra/images/vendor.img
cmp /payload/images/system.img "$SYSTEM"
cmp /payload/images/vendor.img "$VENDOR"
cmp /payload/provenance/image-info.json /tmp/image-package-root/usr/share/waydroid-extra/images/androidtone-image-info.json
grep -q '"imageArchitecture": "arm64_only"' /tmp/image-package-root/usr/share/waydroid-extra/images/androidtone-image-info.json
grep -q '"aarch32El0Required": false' /tmp/image-package-root/usr/share/waydroid-extra/images/androidtone-image-info.json
# Check the actual official image payload, not just the archive label. The
# vendor service retains its reboot_on_failure self-test, but the 32-bit
# binary is absent and both images explicitly advertise an empty 32-bit ABI.
for image_path in /bin/boringssl_self_test32 /bin/hw/boringssl_self_test32; do
  if debugfs -R "stat $image_path" "$VENDOR" 2>&1 | grep -q '^Inode:'; then
    echo "ARM64-only vendor image unexpectedly contains $image_path" >&2
    exit 1
  fi
done
debugfs -R 'stat /bin/boringssl_self_test64' "$VENDOR" 2>&1 | grep -q '^Inode:'
debugfs -R 'cat /build.prop' "$VENDOR" 2>/dev/null | grep -Fx 'ro.vendor.product.cpu.abilist32='
debugfs -R 'cat /system/build.prop' "$SYSTEM" 2>/dev/null | grep -Fx 'ro.system.product.cpu.abilist32='
debugfs -R 'cat /etc/init/boringssl_self_test.rc' "$VENDOR" 2>/dev/null | grep -Fq 'exec_start boringssl_self_test32_vendor'
debugfs -R 'cat /etc/init/boringssl_self_test.rc' "$VENDOR" 2>/dev/null | grep -Fq 'reboot_on_failure reboot,boringssl-self-check-failed'
dpkg-deb --control "$DEB" /tmp/image-package-control
test -x /tmp/image-package-control/postinst
grep -q '/usr/bin/waydroid init' /tmp/image-package-control/postinst
printf 'WAYDROID_IMAGE_PACKAGE_VERIFIED\n'
