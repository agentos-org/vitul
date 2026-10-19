# Vitul project

[简体中文](VITUL-zh_CN.md)

Vitul uses the system WebView on macOS, Windows, Android, and iOS. The AgentOS
target packages the same web build with its Rust OpenTool process and displays
it in an embedded WebView based on Chromium.

## App icon

`app-icon.png` is the full-bleed square source for the default app icon. Replace
this file when designing your logo. Extend the background to all four edges;
avoid transparent outer padding, an extra frame, or pre-rounded corners, which
can make the icon appear smaller in the Dock. Keep the logo itself inside the
canvas so it remains visible when the platform applies its icon shape.

Regenerate the platform assets from the project root:

```sh
pnpm vitul icon app-icon.png --output src-vitul/icons
```

Rebuild the app to use the updated assets. Edit the source instead of individual
generated PNG, ICO, or ICNS files so all resolutions stay consistent.

## Language and appearance

The starter supports English/Chinese and light/dark themes. `index.html` initializes
`lang`, `data-theme`, and `color-scheme` before React starts. `src/host-context.ts`
reads AgentOS/WebShell launch parameters and subscribes to native `agentos:host`
context changes and parent `shell:theme` / `shell:locale` messages, with source and
origin validation. Host themes take priority over the local system theme. Browser
previews follow browser language and system appearance, including live changes.
Use `src/messages.ts` for UI translations and the semantic variables in
`src/styles.css` for colors. Subscribe through `useSyncExternalStore` so context
changes update the screen without resetting input or backend results. Raw backend
results and diagnostics retain their original text; the product title retains the
configured name.

## Commands

- `pnpm install` installs the frontend, AgentOS SDK, host API, and the local Vitul CLI.
- `pnpm vitul dev` builds and starts the OpenTool backend on an OS-assigned loopback
  port, then starts the Vitul desktop app.
- `pnpm vitul build` builds a desktop app through the compatible CLI.
- `pnpm vitul android dev` and `pnpm vitul ios dev` run the app on a mobile
  device or simulator through the compatible mobile CLI.
- `pnpm vitul android build --apk` builds an Android APK; use `--aab` for an
  Android App Bundle. `pnpm vitul ios build` builds an iOS IPA.
- `vitul dev` and `vitul build` are also available when the Rust CLI is on PATH.
- Desktop builds produce native packages: by default macOS builds an `.app`
  bundle and `.dmg` image; Windows builds `.msi` and NSIS `.exe` installers.
  Set `bundle.targets` in the root `vitul.conf.json` to choose desktop formats.
- `vitul build --target agentos/arm64` builds the web assets and ARM64 Linux
  backend in Docker, then writes an AgentOS `.agt` archive to `dist/`.
- `vitul build --target agentos/x64` builds the x64 Linux backend and package.
- Installable AgentOS `.agt` packages use manifest v1 and support ARM64 and
  x64. OpenTool uses the
  separate `rpc.version: 2` binding.

The preset uses Tailwind CSS 4 through the official Vite plugin. Generated iOS
apps target iOS 26.0, which provides a modern WebView baseline for the starter.

Set `VITUL_AGENTOS_BUILD_IMAGE` to override the container image, or change
`agentos.buildImage` in the root `vitul.conf.json`.

Set `agentos.open.titleBar` in the project-root `vitul.conf.json` to `system`
or `overlay` to choose how the AgentOS App content is laid out relative to the
Shell title bar. The default is `system`. This preference is written to the
generated AgentOS `app.json`; the host reports the mode it actually applies.
Shell-managed windows keep Shell-owned controls above App content.
The AgentOS `bundleId` comes from `identifier` in
the root `vitul.conf.json`. Set `agentos.port` in the same file to
choose the backend's container-internal port from 1 to 65535; it defaults to 9627.
Set `bundle.icon` in the root `vitul.conf.json` to a PNG or SVG asset to
include it in the AgentOS package as `icon.png` or `icon.svg`.
Running `vitul icon app-icon.png` also generates the macOS ICNS. Use
`--macos-mode passthrough` for an image that already has its macOS layout.

## Application APIs

Put business operations in `backend/src/tools/` and call them through
`src/opentool.ts`. The starter's `greet` example uses OpenTool JSON-RPC;
`backend/src/lib.rs` creates the OpenTool server.
The project includes `agentos-sdk-ts`. When your app needs AgentOS Gateway
APIs, import `createAgentOSClient()` from `src/lib/agentos.ts`. It creates the
client only when called. Copy `.env.example` to `.env` and set
`VITE_AGENTOS_GATEWAY_URL` to override the Gateway address. This value is
bundled into the frontend, so never put keys or other secrets in `VITE_*`
variables. Keep app business operations in the OpenTool backend.
`src/host.ts` defines the system API boundary; `src/desktop-host.ts` maps it to
the compatible native runtime on desktop and mobile, while the AgentOS provider
talks to the host's `window.agentos.host` bridge. The starter demonstrates
minimizing over host IPC and requests only `native.window-control` for AgentOS. Add other
`native.*` permissions in `vitul.conf.json` only when your app needs them.
AgentOS starts the packaged backend with `HOST=0.0.0.0` and `PORT` set to
`agentos.port`, then maps an assigned loopback host port to that container port.
The frontend is served at `/`; OpenTool RPC uses `/opentool` on the same origin.
Windows for one installed App share its backend and host port. When AgentOS
WebShell hosts the App at `/apps/{appId}/`, the generated client uses that proxy
prefix to reach the same backend. AgentOS App OpenTool requests are limited to
1 MiB; the generated Rust backend applies that limit in AgentOS and keeps the
OpenTool SDK's 16 MiB default on other hosts.

The root `vitul.conf.json` is the project configuration. Vitul creates and
ignores a runtime projection in `src-vitul/` and refreshes it when desktop or
mobile commands run.
The generated `build.rs` prepares the configuration filename expected by the
installed Rust desktop crates; do not edit generated files under `src-vitul/`.

Android and iOS use native IPC to call the OpenTool router embedded in the app's
native process. Requests keep the OpenTool JSON-RPC envelope, and `streamCall`
events travel over a native channel. This mobile transport does not start an HTTP
server. The backend is linked into the mobile app from `backend/`. Desktop apps
continue to call their separately running OpenTool backend over HTTP; the
desktop IPC handler only supplies its endpoint.

Keep business APIs in the OpenTool backend instead of adding one native command
per business operation.

## Sidecars

Declare sidecars through the compatible `bundle.externalBin` setting in the
root `vitul.conf.json`. Desktop builds keep the expected target-suffixed
sidecar files. The AgentOS ARM64 builder collects the matching ARM64 Linux
binaries into the `.agt` archive's `sidecars/` directory. AgentOS sidecar names
come from each configured path's basename and must match
`[a-z][a-z0-9-]{0,31}`; for example, `binaries/worker` is exposed as `worker`.

To use sidecars through the shared host API, call
`host.sidecar.spawn("binaries/worker", ["--version"])` and later call the
returned handle's `kill()` method. On AgentOS, add `native.sidecars` to
`agentos.permissions` in the project-root `vitul.conf.json`; packaging fails
when sidecars are declared without this permission request. AgentOS still asks
the user to grant the requested host permission. Browser previews do not
support sidecars.
