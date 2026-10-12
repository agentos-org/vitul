#!/bin/sh
# Runs only inside the disposable Docker builder, never on the host.
set -eu
export DEBIAN_FRONTEND=noninteractive
sed -i 's/Types: deb$/Types: deb deb-src/; s|http://ports.ubuntu.com|https://ports.ubuntu.com|' /etc/apt/sources.list.d/ubuntu.sources
# The base image has no CA bundle yet; bootstrap from the signed Ubuntu HTTP repository.
sed -i 's|https://ports.ubuntu.com|http://ports.ubuntu.com|' /etc/apt/sources.list.d/ubuntu.sources
apt-get update -qq
apt-get install -y --no-install-recommends ca-certificates curl gnupg python3
sed -i 's|http://ports.ubuntu.com|https://ports.ubuntu.com|' /etc/apt/sources.list.d/ubuntu.sources
curl --fail --location --retry 3 --proto '=https' https://repo.waydro.id/waydroid.gpg -o /usr/share/keyrings/waydroid.gpg
printf 'deb [signed-by=/usr/share/keyrings/waydroid.gpg] https://repo.waydro.id/ noble main\ndeb-src [signed-by=/usr/share/keyrings/waydroid.gpg] https://repo.waydro.id/ noble main\n' > /etc/apt/sources.list.d/waydroid.list
apt-get update -qq
mkdir -p /out/packages/partial /out/sources /out/provenance
cp /etc/apt/sources.list.d/ubuntu.sources /etc/apt/sources.list.d/waydroid.list /out/provenance/
cp /usr/share/keyrings/waydroid.gpg /usr/share/keyrings/ubuntu-archive-keyring.gpg /out/provenance/
gpg --show-keys --with-colons /usr/share/keyrings/waydroid.gpg > /out/provenance/waydroid-key.txt
# Empty status is essential: download dependencies even when the builder already has them.
: > /tmp/empty-status
python3 /build/download-debs.py
python3 /build/catalog-debs.py /out
rm -rf /out/packages/partial /out/packages/lock
