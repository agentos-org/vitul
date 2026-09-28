# Vitul

[简体中文](README-zh_CN.md)

Vitul is an OpenTool-first app development toolchain. Apps on macOS, Windows,
Android, and iOS use their system WebView; AgentOS apps use an embedded WebView
based on Chromium.

## Runtime targets

| Target | UI runtime | System APIs | App backend |
| --- | --- | --- | --- |
| macOS and Windows | System WebView | Host-specific IPC | OpenTool over HTTP/JSON-RPC |
| Android and iOS | System WebView | Tauri IPC | OpenTool JSON-RPC over Tauri IPC; `streamCall` uses a Tauri Channel |
| AgentOS | Embedded Chromium-based WebView | Host IPC bridge | OpenTool over HTTP/JSON-RPC |

Business operations belong in the OpenTool backend. Window controls, dialogs,
clipboard, notifications, and other operating-system actions go through the
host API bridge.

## Coming from Tauri?

Vitul is an OpenTool-first toolchain evolved from Tauri 2. It reuses the
compatible desktop foundation while adding its own project workflow and AgentOS
target, and carries OpenTool business calls over Tauri IPC on mobile:

| Area | Tauri | Vitul |
| --- | --- | --- |
| Foundation | Desktop app framework | OpenTool-first toolchain based on Tauri 2's compatible runtime, APIs, and configuration model |
| Desktop UI | System WebView | Same system WebView on macOS and Windows |
| CLI | `tauri`, `cargo tauri` | `vitul`, `cargo vitul` |
| Desktop config path | `src-tauri/tauri.conf.json` | `src-vitul/vitul.conf.json` |
| Frontend calls | IPC commands commonly handle host and app operations | Host IPC handles operating-system actions; OpenTool JSON-RPC handles business operations |
| Build targets | Linux, macOS, and Windows desktop apps; Android and iOS mobile apps | macOS/Windows desktop apps, Android/iOS mobile apps, and AgentOS `.agt` apps (manifest architectures: ARM64 and x64) |
| API and plugins | Tauri APIs and its plugin ecosystem | Compatible Rust/JavaScript APIs and supported compatible plugins; check support for the features you use |

Your Rust, WebView, window, permission, and packaging experience carries over.
For a new app, start with `vitul init`; Vitul uses its own commands and project
files, so a Tauri project's `src-tauri` directory is not automatically treated
as a Vitul project. Android and iOS use native WebViews and Tauri IPC; Vitul
routes the OpenTool request format through that IPC, with stream events carried
by a Tauri Channel. The mobile backend is linked into the app and runs in its
native process. Mobile builds need the platform toolchains; iOS builds require
macOS and Xcode. See [Tauri's mobile prerequisites](https://v2.tauri.app/start/prerequisites/).

## Create and run an app

Install the Vitul CLI from this repository, then create an OpenTool-first app:

```sh
cargo install --path crates/vitul-cli
vitul init my-app
cd my-app
pnpm install
```

Choose the row for the platform where the app will run:

| Target | Commands | Runtime and build output | Requirements |
| --- | --- | --- | --- |
| AgentOS (ARM64 / x64) | `pnpm vitul build --target agentos-arm64` | AgentOS embedded Chromium-based WebView; v2 `.agt` manifests support ARM64 and x64, and the Shell assigns the backend a dynamic loopback port at launch. | The current Vitul packager builds ARM64 on an ARM64 Mac with Docker. |
| Android | Dev: `pnpm vitul android dev`<br>APK: `pnpm vitul android build --apk`<br>Play Store: `pnpm vitul android build --aab` | System Android WebView; produces an `.apk` or `.aab`. The OpenTool backend runs in the app process and uses Tauri IPC. | Android Studio, Android SDK/NDK, and the Android Rust target. |
| iOS | Dev: `pnpm vitul ios dev`<br>Build: `pnpm vitul ios build` | System WKWebView; produces an `.ipa`. The OpenTool backend runs in the app process and uses Tauri IPC. | macOS, Xcode, and the iOS Rust target. |
| Windows | Dev: `pnpm vitul dev`<br>Build: `pnpm vitul build` | System WebView2; `.msi` and NSIS `.exe` installers by default. | Build on Windows with Rust MSVC, Node.js/pnpm, Visual Studio C++ Build Tools (“Desktop development with C++”), and WebView2 Runtime. |
| macOS | Dev: `pnpm vitul dev`<br>Build: `pnpm vitul build` | System WebView; `.app` bundle and `.dmg` disk image by default. | Build on macOS with Rust, Node.js/pnpm, and Xcode Command Line Tools (`xcode-select --install`). |

Desktop package formats can be selected with `bundle.targets` in
`src-vitul/vitul.conf.json`. AgentOS v2 app manifests support ARM64 and x64
installations. The current Vitul packager produces ARM64 packages. These
commands do not build the Ubuntu 24 system image or the AgentOS Desktop host
application; build those from their AgentOS projects.

## AgentOS SDKs

AgentOS SDKs are clients for apps that call AgentOS Gateway APIs. Use them as
language-specific package dependencies instead of copying their source into
Vitul. Projects created by `vitul init` include the TypeScript SDK in
`package.json`; the starter does not call it unless you add that integration:

| Language | Package | Useful for |
| --- | --- | --- |
| TypeScript | [`agentos-sdk-ts`](https://github.com/agentos-org/agentos-sdk-ts) | Vitul web frontends that call an AgentOS Gateway, locally or remotely |
| Dart | [`agentos_sdk`](https://github.com/agentos-org/agentos-sdk-dart) | Dart-based AgentOS clients, including the AgentOS Desktop project |
| Python | [`agentos-sdk-python`](https://gitlab.litevar.com/litevar/agentos/agentos-sdk-python) | Python services and automation that call AgentOS APIs |

For a Vitul frontend, import the TypeScript SDK only if the app needs AgentOS
Gateway APIs; the default starter uses OpenTool for its business backend. The
SDKs do not build AgentOS system images or package `.agt` apps.

## Repository layout

- `crates/vitul-cli/` contains the Vitul project generator, development runner,
  and AgentOS packager.
- `crates/tauri-*` contains the compatible desktop engine. Its Cargo package and
  Rust crate names remain compatible with the existing runtime and plugin ecosystem.
- `packages/` contains the compatible JavaScript API (`@tauri-apps/api`) and
  the Vitul CLI package (`@vitul/cli`).
- `vendor/create-vitul-app/` contains the upstream-based project scaffold used
  by `vitul init`.

Vitul keeps compatible configuration schemas, API behavior, and desktop tooling
internally. New projects use Vitul filenames, origins, and commands; the only
user-facing CLI names are `vitul` and `cargo vitul`. The compatible JavaScript
API retains its `@tauri-apps/api` import path, while the CLI package is named
`@vitul/cli`. Vitul-native features live under `crates/vitul-*`; the existing
`crates/tauri-*` directories and Rust crate identities identify the compatibility
engine. Copyright and license notices for inherited code remain in place.
