# Vitul Architecture

[简体中文](ARCHITECTURE-zh_CN.md)

Vitul separates the app's business backend from the operating-system host.
Business calls use the OpenTool message format; the transport depends on the
runtime. System operations use the host's IPC bridge, where permissions and
platform behavior can be enforced.

## Runtime targets

### macOS and Windows

The desktop target keeps the Tauri-compatible runtime and the operating
system's WebView. The Vitul host provider maps window, dialog, clipboard,
notification, and URL-opening calls to the desktop APIs. `vitul dev` starts the
OpenTool backend on a loopback port and exposes its endpoint to the frontend
through the desktop IPC layer.

### Android and iOS

The mobile target uses the platform WebView and Tauri IPC. The native app links
the OpenTool backend library into its own process and dispatches the existing
OpenTool router in-process. Unary calls carry the same JSON-RPC request and
response messages over `invoke`; `streamCall` forwards OpenTool events through
a Tauri Channel. No HTTP listener is started on mobile.

### AgentOS

AgentOS hosts the Vitul frontend in an embedded WebView based on Chromium. The
host provides system operations through IPC and checks the App's declared
`native.*` permissions. AgentOS starts the App backend on an instance-specific
dynamic loopback port, waits for its health check, and loads the manifest's
frontend entry in the WebView.

The AgentOS App backend exposes OpenTool on the same service origin as the frontend.
The manifest declares `rpc.protocol: "opentool"`, binding version `2`, and the
service path, normally `/opentool`. RPC uses OpenTool's HTTP and JSON-RPC
messages; host IPC is a separate channel for system actions.

## Toolchain

- `crates/vitul-cli/` implements `vitul init`, `vitul dev`, and Vitul's
  AgentOS `.agt` packaging target.
- `vitul` and `cargo vitul` are the only user-facing shell entry points.
- Tauri-compatible tooling is called internally by Vitul where needed. New
  projects use Vitul filenames, origins, packages, and command names while
  preserving compatible config schemas and API behavior.
- `vendor/create-vitul-app/` supplies the upstream-based scaffold that the
  Vitul generator customizes into an OpenTool-first project.
- `crates/tauri-*` contain the compatible desktop runtime, CLI, build support,
  and bundler. Vitul keeps their established names and interfaces for existing
  projects and plugins.
- `packages/` contains compatible frontend APIs and CLI packages.

## AgentOS package contract

An installable `.agt` package uses `manifestVersion: 2` and an explicit
`architecture` (`arm64` or `x64`). Process Apps use port `0`, Shell-managed
startup, a frontend entry, and an OpenTool RPC declaration. The package also
declares the supported `native.*` host capabilities it may request. The
manifest supports both AgentOS architectures; the current Vitul packager emits
ARM64 packages.

## Compatibility and attribution

Vitul changes the product-facing brand while preserving Tauri-compatible
crate names, macros, JavaScript package names, configuration files, and IPC
identifiers. These names are part of the compatibility surface. Inherited
source remains subject to its original copyright and license notices; see the
repository's license files.
