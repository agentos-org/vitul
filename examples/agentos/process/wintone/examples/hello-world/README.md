# WinTone Windows Hello World

This directory contains a tiny native Win32 GUI program and two prebuilt PE files:

- `hello-windows-x64.exe` is an x86-64 Windows application. It runs through the bundled compatibility runtime on an AgentOS ARM64 device.
- `hello-windows-arm64.exe` is an ARM64 Windows application. It runs through the bundled native ARM64 Windows runtime on an AgentOS ARM64 device.

Both programs show a message box and then exit. They are application samples, not installers. In WinTone, select **Run EXE directly** before importing them; each appears as a runnable task and can be launched again after it exits. Importing them as installers will not create an installed-app entry.

The binaries are built from `hello.c` with Zig's Windows GNU targets. Rebuild both with:

```sh
./build.sh
```

The checked-in executables were built with Zig 0.16.0 and verified as Windows GUI PE32+ files for x86-64 and AArch64 respectively.
