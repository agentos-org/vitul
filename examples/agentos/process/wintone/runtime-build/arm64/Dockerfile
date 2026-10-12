# Wine and its x86-64 userspace run inside FEX's guest RootFS. FEX itself is
# installed for the ARM64 host that AgentOS's Ubuntu 24.04 process container
# provides.
FROM --platform=linux/arm64 ubuntu:24.04 AS wine-rootfs
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates debootstrap qemu-user-static \
    && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*.deb
RUN mkdir -p /runtime/fex/rootfs \
    && debootstrap --foreign --arch=amd64 --variant=minbase noble \
       /runtime/fex/rootfs http://archive.ubuntu.com/ubuntu \
    && install -m 0755 /usr/bin/qemu-x86_64-static /runtime/fex/rootfs/usr/bin/qemu-x86_64-static \
    && chroot /runtime/fex/rootfs /usr/bin/qemu-x86_64-static /bin/bash \
       /debootstrap/debootstrap --second-stage \
    && rm -f /runtime/fex/rootfs/etc/apt/sources.list.d/*.sources \
    && printf '%s\n' \
       'deb http://archive.ubuntu.com/ubuntu noble main universe' \
       'deb http://archive.ubuntu.com/ubuntu noble-updates main universe' \
       'deb http://security.ubuntu.com/ubuntu noble-security main universe' \
       > /runtime/fex/rootfs/etc/apt/sources.list
RUN chroot /runtime/fex/rootfs /usr/bin/qemu-x86_64-static /bin/bash -lc \
       'dpkg --add-architecture i386 && apt-get update && apt-get install -y --no-install-recommends \
          ca-certificates fonts-wine libwayland-client0:amd64 libwayland-cursor0:amd64 \
          libxkbcommon0:amd64 wine wine64 wine32:i386 \
        && find /usr/lib -type f -name winewayland.drv -print -quit | grep -q . \
        && find /usr/lib -type f -name winewayland.so -print -quit | grep -q . \
        && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*.deb' \
    && rm -f /runtime/fex/rootfs/usr/bin/qemu-x86_64-static \
    && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*.deb
RUN dpkg-query --admindir=/runtime/fex/rootfs/var/lib/dpkg -W \
      -f='${Version}\n' wine64:amd64 > /runtime/fex/rootfs/tmp/wine-version \
    && test -s /runtime/fex/rootfs/tmp/wine-version

# Ubuntu publishes a native ARM64 Wine build. This separate root filesystem
# supplies Wine's ARM64 Unix libraries without routing Windows ARM64 PE code
# through FEX's x86-64 Linux translator.
FROM --platform=linux/arm64 ubuntu:24.04 AS arm64-wine-rootfs
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates debootstrap \
    && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*.deb
RUN mkdir -p /runtime/native-arm64/rootfs \
    && debootstrap --foreign --arch=arm64 --variant=minbase noble \
       /runtime/native-arm64/rootfs http://ports.ubuntu.com/ubuntu-ports \
    && chroot /runtime/native-arm64/rootfs /bin/bash \
       /debootstrap/debootstrap --second-stage \
    && rm -f /runtime/native-arm64/rootfs/etc/apt/sources.list.d/*.sources \
    && printf '%s\n' \
       'deb http://ports.ubuntu.com/ubuntu-ports noble main universe' \
       'deb http://ports.ubuntu.com/ubuntu-ports noble-updates main universe' \
       'deb http://ports.ubuntu.com/ubuntu-ports noble-security main universe' \
       > /runtime/native-arm64/rootfs/etc/apt/sources.list \
    && chroot /runtime/native-arm64/rootfs /bin/bash -lc \
       'apt-get update && apt-get install -y --no-install-recommends \
          ca-certificates fonts-wine libwayland-client0 libwayland-cursor0 \
          libxkbcommon0 wine wine64 \
        && find /usr/lib -type f -name winewayland.drv -print -quit | grep -q . \
        && find /usr/lib -type f -name winewayland.so -print -quit | grep -q . \
        && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*.deb' \
    && dpkg-query --admindir=/runtime/native-arm64/rootfs/var/lib/dpkg -W \
       -f='${Version}\n' wine64:arm64 > /runtime/native-arm64/rootfs/tmp/wine-version \
    && test -s /runtime/native-arm64/rootfs/tmp/wine-version

FROM --platform=linux/arm64 ubuntu:24.04 AS fex-host
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates \
       software-properties-common \
    && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*.deb
RUN add-apt-repository -y ppa:fex-emu/fex \
    && apt-get update \
    && apt-get install -y --no-install-recommends fex-emu-armv8.0 \
    && test -x /usr/bin/FEX \
    && test -x /usr/bin/FEXBash \
    && dpkg-query -W -f='${Version}\n' fex-emu-armv8.0 > /tmp/fex-version \
    && mkdir -p /tmp/fex-docs \
    && for doc in /usr/share/doc/fex-emu*; do \
         [ ! -d "$doc" ] || cp -a "$doc" /tmp/fex-docs/; \
       done \
    && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*.deb

FROM --platform=linux/arm64 ubuntu:24.04 AS runtime
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends zstd \
    && rm -rf /var/lib/apt/lists/*
COPY --from=wine-rootfs /runtime/fex/rootfs/ /runtime/fex/rootfs/
COPY --from=arm64-wine-rootfs /runtime/native-arm64/rootfs/ /runtime/native-arm64/rootfs/
COPY --from=fex-host /usr/bin/FEX /runtime/fex/bin/FEX
COPY --from=fex-host /usr/bin/FEXBash /runtime/fex/bin/FEXBash
COPY --from=fex-host /usr/lib/aarch64-linux-gnu/fex-emu/ /runtime/fex/lib/fex-emu/
COPY --from=fex-host /usr/lib/aarch64-linux-gnu/ /runtime/fex/lib/
COPY --from=fex-host /usr/share/fex-emu/ /runtime/fex/share/fex-emu/
COPY --from=fex-host /tmp/fex-version /runtime/fex-version
COPY --from=fex-host /tmp/fex-docs/ /runtime/THIRD_PARTY/FEX/
COPY --from=wine-rootfs /runtime/fex/rootfs/tmp/wine-version /runtime/wine-version
COPY runtime-build/arm64/session-launch.sh /runtime/fex/bin/wintone-session-launch
COPY runtime-build/arm64/native-session-launch.sh /runtime/native-arm64/bin/wintone-native-session-launch
RUN chmod 0755 /runtime/fex/bin/wintone-session-launch \
    && chmod 0755 /runtime/native-arm64/bin/wintone-native-session-launch \
    && mkdir -p /out \
    && printf '{"fexVersion":"%s","wineVersion":"%s","nativeWineVersion":"%s"}\n' \
       "$(cat /runtime/fex-version)" "$(cat /runtime/wine-version)" \
       "$(cat /runtime/native-arm64/rootfs/tmp/wine-version)" \
       > /out/runtime-info.json \
    && tar --zstd -cf /out/wintone-runtime.tar.zst -C /runtime .

FROM scratch AS export
COPY --from=runtime /out/wintone-runtime.tar.zst /wintone-runtime.tar.zst
COPY --from=runtime /out/runtime-info.json /runtime-info.json
