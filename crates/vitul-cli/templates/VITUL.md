# Vitul project

[简体中文](VITUL-zh_CN.md)

Vitul uses the system WebView on macOS, Windows, Android, and iOS. The AgentOS
target packages the same web build with its Rust OpenTool process and displays
it in an embedded WebView based on Chromium.

## Commands

- `pnpm install` installs the frontend, host API, and AgentOS TypeScript SDK dependencies. Install the Rust CLI separately; project scripts use its `vitul` executable from `PATH`.
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
  Set `bundle.targets` in `src-vitul/vitul.conf.json` to choose desktop formats.
- `vitul build --target agentos-arm64` builds the web assets and ARM64 Linux
  backend in Docker, then writes an AgentOS `.agt` archive to `dist/`.
- Installable AgentOS `.agt` packages use manifest v1 and support ARM64 and
  x64; the current Vitul packager builds ARM64 packages. OpenTool uses the
  separate `rpc.version: 2` binding.

Set `VITUL_AGENTOS_BUILD_IMAGE` to override the container image, or change
`agentos.buildImage` in `vitul.conf.json`.

Set `agentos.open.titleBar` in the project-root `vitul.conf.json` to `system`
or `overlay` to choose how the AgentOS App content is laid out relative to the
Shell title bar. The default is `system`. This preference is written to the
generated AgentOS `app.json`; the host reports the mode it actually applies.
Shell-managed windows keep Shell-owned controls above App content.
The AgentOS `bundleId` comes from `identifier` in
`src-vitul/vitul.conf.json`. Set `agentos.port` in the project-root config to
choose the backend's container-internal port from 1 to 65535; it defaults to
9627.

## Application APIs

Put business operations in the Rust backend and call them through
`src/opentool.ts`. The starter's `greet` example uses OpenTool JSON-RPC.
The generated `package.json` includes `agentos-sdk-ts` for apps that call
AgentOS Gateway APIs. The starter does not call the Gateway by default; import
the SDK only when your app needs those APIs.
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

Android and iOS use native IPC to call the OpenTool router embedded in the app's
native process. Requests keep the OpenTool JSON-RPC envelope, and `streamCall`
events travel over a native channel. This mobile transport does not start an HTTP
server. The backend is linked into the mobile app from `backend/`. Desktop apps
continue to call their separately running OpenTool backend over HTTP; the
desktop IPC handler only supplies its endpoint.

Keep business APIs in the OpenTool backend instead of adding one native command
per business operation.

## Sidecars

Declare sidecars through the compatible `bundle.externalBin` setting in
`src-vitul/vitul.conf.json`. Desktop builds keep the expected target-suffixed
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
