# WinTone Windows Hello World

This directory contains a tiny native Win32 GUI program and two prebuilt PE files:

- `hello-windows-x64.exe` is an x86-64 Windows application. It is the relevant sample for the current AgentOS ARM64 + FEX + x86-64 Wine runtime path.
- `hello-windows-arm64.exe` is an ARM64 Windows application. It is included for testing a future native ARM64 Wine runtime or Windows ARM64; the current FEX + x86-64 Wine path does not run ARM64 Windows PE files.

Both programs show a message box and then exit. They are application samples, not installers, so importing them through WinTone's installer flow will not create an installed-app entry. Use them as direct launch/session smoke tests once the Wine/FEX runtime and AgentOS Wayland session are available.

The binaries are built from `hello.c` with Zig's Windows GNU targets. Rebuild both with:

```sh
./build.sh
```

The checked-in executables were built with Zig 0.16.0 and verified as Windows GUI PE32+ files for x86-64 and AArch64 respectively.
