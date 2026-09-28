# Vitul OpenTool example

This is the recommended starter example for Vitul. It demonstrates two separate
routes from the web UI:

- The Minimize window button uses system IPC through a host provider. Desktop
  builds use the desktop runtime; AgentOS calls its host bridge.
- The greet form sends business data to the Rust backend through OpenTool
  JSON-RPC at /opentool/call.
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

Desktop builds use native package formats: macOS produces an `.app` bundle and
`.dmg` image by default; Windows produces `.msi` and NSIS `.exe` installers.
The `bundle.targets` setting in `src-vitul/vitul.conf.json` selects the desktop
formats.

Vitul dev starts the Rust OpenTool backend on an operating-system-assigned
loopback port before opening the desktop app. The Vite development server
proxies /opentool to that backend.

AgentOS v2 `.agt` manifests support ARM64 and x64. The command below builds an
ARM64 package with Docker:

    pnpm vitul build --target agentos-arm64

The package builder reads the native permissions from vitul.conf.json. This
example requests only native.window-control. It writes the .agt package to
dist/.

## Code map

- src/host.ts selects the desktop or AgentOS host provider.
- src/desktop-host.ts maps window.minimize to the compatible native runtime.
- src/opentool.ts creates and validates an OpenTool JSON-RPC request and
  response.
- backend/src/lib.rs implements the greet business tool and creates its
  OpenTool router.
- backend/src/main.rs hosts the router over HTTP for desktop development and
  AgentOS packaging.
- src-vitul/src/lib.rs selects Tauri IPC on mobile and exposes the dynamic
  backend endpoint on desktop.
