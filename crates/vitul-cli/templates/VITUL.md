# Vitul project

[简体中文](VITUL-zh_CN.md)

Vitul uses the system WebView on macOS, Windows, Android, and iOS. The AgentOS
target packages the same web build with its Rust OpenTool process and displays
it in an embedded WebView based on Chromium.

## Commands

- `pnpm install` installs the frontend, host API, Vitul CLI, and AgentOS TypeScript SDK dependencies.
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
- AgentOS v2 `.agt` manifests support ARM64 and x64. The current Vitul
  packager builds ARM64 packages.

Set `VITUL_AGENTOS_BUILD_IMAGE` to override the container image, or change
`agentos.buildImage` in `vitul.conf.json`.

## Application APIs

Put business operations in the Rust backend and call them through
`src/opentool.ts`. The starter's `greet` example uses OpenTool JSON-RPC.
The generated `package.json` includes `agentos-sdk-ts` for apps that call
AgentOS Gateway APIs. The starter does not call the Gateway by default; import
the SDK only when your app needs those APIs.
`src/host.ts` defines the system API boundary; `src/desktop-host.ts` maps it to
the compatible native runtime on desktop and mobile, while the AgentOS provider
talks to the host's `window.agentos.host` bridge. The starter demonstrates window minimization over
host IPC and requests only `native.window-control` for AgentOS. Add other
`native.*` permissions in `vitul.conf.json` only when your app needs them.
AgentOS starts the packaged backend on a per-instance dynamic loopback port and
serves the frontend at `/`; OpenTool RPC uses `/opentool` on that same origin.
When AgentOS WebShell hosts the App at `/apps/{appId}/`, the generated client
uses that proxy prefix to reach the same App backend.

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
