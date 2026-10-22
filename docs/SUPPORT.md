# Supported Platforms and Features

Vitul is a pre-1.0 project. This page describes what the current source tree
implements and what its automated checks cover. Vitul adopts familiar Tauri
commands and development conventions, but does not promise feature or plugin
compatibility or automatically import existing Tauri projects.

## Platform matrix

| Target          | UI and system APIs                                           | OpenTool transport                                                                                   | Build output                                                     | Current validation                                                                                                                                                                                   |
| --------------- | ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS desktop   | System WebView and Vitul host APIs                           | Local backend over HTTP/JSON-RPC                                                                     | `.app` and, by default, `.dmg`                                   | Inherited workflows exercise some desktop code; app signing and distribution remain app-owner tasks.                                                                                                |
| Windows desktop | WebView2 and Vitul host APIs                                 | Local backend over HTTP/JSON-RPC                                                                     | `.msi` and, by default, NSIS `.exe`                              | Inherited workflows exercise some desktop code; app signing and distribution remain app-owner tasks.                                                                                                |
| Android         | Android System WebView and Vitul host APIs                   | OpenTool router linked into the app process and called over native IPC; streams use a native channel | `.apk` or `.aab`                                                 | Existing mobile workflows exercise some runtime code. Validate app-specific OpenTool behavior on a device or emulator.                                                                              |
| iOS             | WKWebView and Vitul host APIs                                | OpenTool router linked into the app process and called over native IPC; streams use a native channel | `.ipa`                                                           | Existing mobile workflows exercise some runtime code. Validate app-specific OpenTool behavior on a device or simulator.                                                                             |
| AgentOS ARM64   | AgentOS Shell's embedded Chromium-based WebView and host IPC | App process over HTTP/JSON-RPC on a Shell-assigned loopback port                                     | `.agt` using manifest v1                                         | Vitul CI builds the OpenTool example into a real ARM64 `.agt` and validates its manifest and contents. Host window, permission, and installation behavior also requires an AgentOS Shell smoke test. |
| AgentOS x64     | Same AgentOS runtime contract                                | Same AgentOS runtime contract                                                                        | `.agt` using manifest v1                                         | CLI fixture tests cover x64 package manifests; the full application package smoke job runs on ARM64.                                                                                                             |

Linux desktop is not currently part of Vitul's documented target matrix. The
Tauri-derived source may have capabilities beyond the targets listed here;
those capabilities are not a Vitul support commitment.

## Choose capabilities by requirement

The platform matrix identifies targets; this table identifies the layers needed
for a feature. [AgentOS configuration](../crates/vitul-cli/src/agentos_config/README.md)
owns field details; [Architecture](../ARCHITECTURE.md) explains ownership.
Verify cross-platform requirements on each target.

| Requirement | Support conditions and dependencies | Evidence of completion |
| --- | --- | --- |
| Business logic and data | OpenTool backend; AgentOS persistence requires both declaration and persistent paths | Correct calls and expected data after restart |
| System interaction | Host API, target implementation, permissions; browser preview has partial fallbacks | Success on the target host plus denial/unavailable handling |
| Language and appearance | Initial host context, change subscriptions, translations, semantic styles | Correct initial state and live updates without losing business state |
| Window and process lifecycle | Execution mode, close policy, backend implementation, host management | Expected close, reopen, exit, and child cleanup |
| External programs or sidecars | Target binary, bundle declaration, permissions, spawn and stop code | Complete installed files and successful start/stop |
| Providers, deployment, networking | AgentOS protocol, declarations, methods, permissions, host services | Valid declarations plus real execution, failure handling, and recovery |
| Distribution | Platform tools, artifact format, signing, host version | Installation and user workflow on the target environment |

Process apps default to containers. Evaluate native profiles when host access is
required and check their restrictions. Remote-web packages have no local process
and cannot satisfy requirements needing a packaged backend. An inherited Tauri
module, a permission named `native.*`, or a working example does not establish
support across targets.

Configuration validation, package inspection, and host acceptance are distinct
evidence. The CI scope below covers only the checks explicitly listed, not all
application behavior.

## API boundary

Use the host API for operating-system actions such as window control, dialogs,
clipboard, notifications, and sidecars. Put application business operations in
an OpenTool backend. Desktop and AgentOS apps call that backend through
HTTP/JSON-RPC. Android and iOS apps call the in-process OpenTool router through
native IPC; this mobile transport does not start an HTTP server.

Vitul has its own supported configuration, Rust APIs, JavaScript APIs, and
desktop tooling. Check each API and plugin against Vitul's examples and tests;
shared source code or familiar package names do not promise Tauri app or plugin
compatibility.

## Build requirements

Use the operating system's native toolchain for desktop builds. Android builds
need Android Studio, the Android SDK/NDK, and the Rust Android target. iOS builds
need macOS, Xcode, and the Rust iOS target. The AgentOS ARM64 and x64 packager
uses Docker to build its Linux backend. See the root [README](../README.md) for
commands and package formats.

## What CI checks

The `Vitul AgentOS ARM64 package smoke` workflow builds the repository's OpenTool example
with the Vitul CLI, then checks that the output is an ARM64 process app with a
v1 manifest, container port, frontend entry, health check, and OpenTool RPC
configuration. It also generates a fresh project, installs its dependencies,
builds the frontend, and checks that its `vitul` script resolves the installed
Vitul CLI. Other inherited workflows exercise portions of the Tauri-derived
source; their status is not evidence that every Vitul target or plugin is
tested.
