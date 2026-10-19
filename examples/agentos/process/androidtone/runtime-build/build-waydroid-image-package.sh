#!/bin/sh
# Build an offline Debian package in the disposable ARM64 builder only.
set -eu
ROOT=/tmp/androidtone-waydroid-images
IMAGE_PACKAGE_VERSION=0.1.8-1
rm -rf "$ROOT"
mkdir -p "$ROOT/DEBIAN" "$ROOT/usr/share/waydroid-extra/images"
cp /payload/images/system.img "$ROOT/usr/share/waydroid-extra/images/system.img"
cp /payload/images/vendor.img "$ROOT/usr/share/waydroid-extra/images/vendor.img"
cp /payload/provenance/image-info.json "$ROOT/usr/share/waydroid-extra/images/androidtone-image-info.json"
cat > "$ROOT/DEBIAN/control" <<'EOF'
Package: androidtone-waydroid-images
Version: 0.1.8-1
Architecture: all
Depends: waydroid (= 1.6.2)
Section: misc
Priority: optional
Maintainer: AndroidTone <androidtone@agentos.invalid>
Description: AndroidTone offline Waydroid Android 13 images
 Contains the digest-pinned Waydroid system and vendor images. During the
 first authorized installation, the package initializes Waydroid through its
 supported command-line interface. Existing initialized data is preserved.
EOF
cat > "$ROOT/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -eu
if [ "${1:-}" = configure ] && [ ! -f /var/lib/waydroid/waydroid.cfg ]; then
  /usr/bin/waydroid init
fi
exit 0
EOF
chmod 0755 "$ROOT/DEBIAN/postinst"
export SOURCE_DATE_EPOCH=1704067200
find "$ROOT" -exec touch -h -d "@$SOURCE_DATE_EPOCH" {} +
dpkg-deb --build --root-owner-group -Zzstd -z19 "$ROOT" "/payload/packages/androidtone-waydroid-images_${IMAGE_PACKAGE_VERSION}_all.deb"
