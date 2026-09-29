# Supported Platforms and Features

Vitul is a pre-1.0 project. This page describes what the current source tree
implements and what its automated checks cover. Compatibility with Tauri APIs
does not mean every Tauri plugin or platform feature is supported by Vitul.

## Platform matrix

| Target          | UI and system APIs                                           | OpenTool transport                                                                                   | Build output                                                     | Current validation                                                                                                                                                                                   |
| --------------- | ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS desktop   | System WebView and the compatible native host API            | Local backend over HTTP/JSON-RPC                                                                     | `.app` and, by default, `.dmg`                                   | Inherited workflows exercise parts of the compatible runtime; app signing and distribution remain app-owner tasks.                                                                                   |
| Windows desktop | WebView2 and the compatible native host API                  | Local backend over HTTP/JSON-RPC                                                                     | `.msi` and, by default, NSIS `.exe`                              | Inherited workflows exercise parts of the compatible runtime; app signing and distribution remain app-owner tasks.                                                                                   |
| Android         | Android System WebView and the compatible native host API    | OpenTool router linked into the app process and called over native IPC; streams use a native channel | `.apk` or `.aab`                                                 | Existing mobile API workflows exercise the compatible runtime. Validate app-specific OpenTool behavior on a device or emulator.                                                                      |
| iOS             | WKWebView and the compatible native host API                 | OpenTool router linked into the app process and called over native IPC; streams use a native channel | `.ipa`                                                           | Existing mobile API workflows exercise the compatible runtime. Validate app-specific OpenTool behavior on a device or simulator.                                                                     |
| AgentOS ARM64   | AgentOS Shell's embedded Chromium-based WebView and host IPC | App process over HTTP/JSON-RPC on a Shell-assigned loopback port                                     | `.agt` using manifest v2                                         | Vitul CI builds the OpenTool example into a real ARM64 `.agt` and validates its manifest and contents. Host window, permission, and installation behavior also requires an AgentOS Shell smoke test. |
| AgentOS x64     | Same AgentOS runtime contract                                | Same AgentOS runtime contract                                                                        | No x64 `.agt` build target is provided by the current Vitul CLI. | The manifest format can describe x64; that does not mean the Vitul packager emits an x64 artifact.                                                                                                   |

Linux desktop is not currently part of Vitul's documented target matrix. The
underlying compatible runtime may have capabilities beyond the targets listed
here; those capabilities are not a Vitul support commitment.

## API boundary

Use the host API for operating-system actions such as window control, dialogs,
clipboard, notifications, and sidecars. Put application business operations in
an OpenTool backend. Desktop and AgentOS apps call that backend through
HTTP/JSON-RPC. Android and iOS apps call the in-process OpenTool router through
native IPC; this mobile transport does not start an HTTP server.

Vitul uses Tauri-compatible configuration, Rust APIs, JavaScript APIs, and
desktop tooling internally. Check the specific plugin and API against Vitul's
examples and tests before depending on it; compatibility is not a blanket
promise for the complete Tauri ecosystem.

## Build requirements

Use the operating system's native toolchain for desktop builds. Android builds
need Android Studio, the Android SDK/NDK, and the Rust Android target. iOS builds
need macOS, Xcode, and the Rust iOS target. The current AgentOS ARM64 packager
uses Docker to build its Linux backend. See the root [README](../README.md) for
commands and package formats.

## What CI checks

The `Vitul AgentOS ARM64 package smoke` workflow builds the repository's OpenTool example
with the Vitul CLI, then checks that the output is an ARM64 process app with a
v2 manifest, dynamic port, frontend entry, health check, and OpenTool RPC
configuration. It also generates a fresh project, installs its dependencies,
builds the frontend, and checks that its `vitul` script resolves the installed
Rust CLI. Other inherited workflows continue to exercise portions of the
compatible Tauri runtime; their status is not evidence that every Vitul target
or plugin is tested.
