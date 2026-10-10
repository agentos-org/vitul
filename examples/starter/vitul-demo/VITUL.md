# Vitul Demo

This is the recommended starter example for Vitul. It demonstrates two separate
routes from the web UI:

- The Minimize window button uses system IPC through a host provider. Desktop
  builds use the desktop runtime; AgentOS calls its host bridge.
- The greet form sends business data to the Rust backend through OpenTool
  JSON-RPC at `/opentool/call`. When AgentOS WebShell hosts the App under
  `/apps/{appId}/`, the client uses that proxy prefix to reach the same backend.
- Android and iOS carry the same OpenTool request format through Tauri IPC and
  call the embedded router in-process. `streamCall` events use a Tauri Channel;
  mobile apps do not start an HTTP server.

macOS and Windows use their system WebView. AgentOS displays the same frontend
in its embedded Chromium-based WebView.

## Run from this repository

At the repository root, install dependencies and build the local API package
and Vitul CLI:

    pnpm install
    pnpm build:api
    pnpm build:cli:debug

Then, from this directory:

    pnpm vitul dev
    pnpm vitul build

For mobile development and builds, run:

    pnpm vitul android dev
    pnpm vitul android build --apk
    pnpm vitul android build --aab
    pnpm vitul ios dev
    pnpm vitul ios build

Android builds can produce `.apk` or `.aab` packages; iOS builds produce an
`.ipa` package. Android builds require the Android SDK toolchain. iOS builds
require macOS and Xcode.

Vitul dev starts the Rust OpenTool backend on an operating-system-assigned
loopback port before opening the desktop app. The Vite development server
proxies /opentool to that backend.

Installable AgentOS `.agt` packages use manifest v1 and support ARM64 and x64.
OpenTool uses the separate `rpc.version: 2` binding. The project-root
`vitul.conf.json` contains desktop, mobile, and AgentOS settings. Build both
declared architectures with Docker:

    pnpm vitul build --target agentos

Use `--target agentos/arm64` or `--target agentos/x64` to build one architecture.

The package builder reads the port, native permissions, and title bar settings
from the project-root `vitul.conf.json`. This example requests `overlay`, the
default container port `9627`, and only the `native.window-control` permission.
AgentOS maps an assigned loopback host port to the container port; windows of
the same installed App share one backend. The manifest `bundleId` comes from
the root `identifier`. The `.agt` package is written to `dist/`.

## Code map

- src/host.ts selects a host and handles AgentOS window context updates.
- src/desktop-host.ts maps window operations to the compatible native runtime.
- src/main.ts implements the language/theme selectors and window-control demo.
- src/opentool.ts creates and validates an OpenTool JSON-RPC request and
  response.
- backend/src/lib.rs implements the greet business tool and creates its
  OpenTool router.
- backend/src/main.rs hosts the router over HTTP for desktop development and
  AgentOS packaging.
- src-vitul/src/lib.rs selects Tauri IPC on mobile and exposes the dynamic
  backend endpoint on desktop.
