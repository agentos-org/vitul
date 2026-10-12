# Vitul

[简体中文](README-zh_CN.md)

[GitHub repository](https://github.com/agentos-org/vitul) · [Supported platforms](docs/SUPPORT.md) · [Maintenance and releases](docs/MAINTENANCE.md)

Vitul is an OpenTool-first app development toolchain. Apps on macOS, Windows,
Android, and iOS use their system WebView; AgentOS apps use an embedded WebView
based on Chromium.

## Runtime targets

| Target            | UI runtime                      | System APIs       | App backend                                                         |
| ----------------- | ------------------------------- | ----------------- | ------------------------------------------------------------------- |
| macOS and Windows | System WebView                  | Host-specific IPC | OpenTool over HTTP/JSON-RPC                                         |
| Android and iOS   | System WebView                  | Tauri IPC         | OpenTool JSON-RPC over Tauri IPC; `streamCall` uses a Tauri Channel |
| AgentOS           | Embedded Chromium-based WebView | Host IPC bridge   | OpenTool over HTTP/JSON-RPC                                         |

Business operations belong in the OpenTool backend. Window controls, dialogs,
clipboard, notifications, and other operating-system actions go through the
host API bridge.

## Coming from Tauri?

Vitul is an independently maintained, OpenTool-first toolchain derived from
the [Tauri 2 source repository](https://github.com/tauri-apps/tauri), with
inherited copyright and license notices preserved. It follows familiar Tauri
command and project conventions, while defining its own project format,
supported APIs, plugins, and AgentOS workflow. It is not an official Tauri
project, and existing Tauri apps are not automatically imported:

| Area                | Tauri                                                               | Vitul                                                                                                                |
| ------------------- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Foundation          | Desktop app framework                                               | OpenTool-first toolchain derived from Tauri 2 source                                                                 |
| Desktop UI          | System WebView                                                      | Same system WebView on macOS and Windows                                                                             |
| CLI                 | `tauri`, `cargo tauri`                                              | `vitul`, `cargo vitul`                                                                                               |
| Project config path | `src-tauri/tauri.conf.json`                                         | `vitul.conf.json` at the project root                                                                                |
| Frontend calls      | IPC commands commonly handle host and app operations                | Host IPC handles operating-system actions; OpenTool JSON-RPC handles business operations                             |
| Build targets       | Linux, macOS, and Windows desktop apps; Android and iOS mobile apps | macOS/Windows desktop apps, Android/iOS mobile apps, and AgentOS `.agt` apps (manifest architectures: ARM64 and x64) |
| API and plugins     | Tauri APIs and its plugin ecosystem                                 | Vitul's supported APIs and plugins; shared source does not promise Tauri feature or plugin compatibility             |

Your Rust, WebView, window, permission, packaging, and CLI habits carry over.
Create a new app with `vitul create` or `pnpm create vitul-app`; Vitul uses its
own project format and does not import or automatically convert an existing
Tauri project. Android and iOS use native WebViews and Tauri IPC; Vitul
routes the OpenTool request format through that IPC, with stream events carried
by a Tauri Channel. The mobile backend is linked into the app and runs in its
native process. Mobile builds need the platform toolchains; iOS builds require
macOS and Xcode. See [Tauri's mobile prerequisites](https://v2.tauri.app/start/prerequisites/).

## Create and run an app

Vitul-created apps use a preset stack of React, TypeScript, Tailwind CSS 4,
Vite, and pnpm. Generated iOS apps target iOS 26.0. Tailwind 4 requires Safari
16.4 or newer, and is integrated through its official Vite plugin. The npm
entry points are `vitul` and `create-vitul-app`. Once their npm release is
available, create an OpenTool-first app with:

```sh
pnpm create vitul-app@latest my-app
cd my-app
pnpm install
pnpm vitul dev
```

Generated projects require Node.js 22.13 or newer and use pnpm 12.4.2.

You can also install the CLI globally and create in the current empty
directory. The package name comes from the directory, and the display name is
derived from it:

```sh
npm install --global @litevar/vitul-cli
mkdir time-memo && cd time-memo
vitul create . --identifier com.example.timeMemo --yes
pnpm install
pnpm build
pnpm vitul dev
```

Use `--product-name "Time Memo"` to set the app display name and
`--window-title "Time Memo"` to override the desktop window title. The default
package name comes from the project directory; the default display name and
window title use its title-cased form. The `--identifier` value is preserved.

To run CLI changes from this repository before they are published, build it from source:

```sh
git clone https://github.com/agentos-org/vitul.git
cd vitul
cargo install --path crates/vitul-cli --locked
mkdir ../my-app && cd ../my-app
vitul create . --identifier com.example.myApp --yes
pnpm pkg delete devDependencies.@litevar/vitul-cli
pnpm install
pnpm build
pnpm vitul dev
```

The generated package includes the published CLI by default. Remove that
dependency as shown to make package scripts use the `vitul` executable
installed from this checkout.

Choose the row for the platform where the app will run:

| Target                | Commands                                                                                                               | Runtime and build output                                                                                                                                                      | Requirements                                                                                                                         |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| AgentOS (ARM64 / x64) | `pnpm vitul build --target agentos`                                                                                    | AgentOS embedded Chromium-based WebView; installable v1 `.agt` manifests support ARM64 and x64. AgentOS maps an assigned loopback host port to the configured container port. | Docker builds the Rust backend for each declared Linux architecture.                                                                 |
| Android               | Dev: `pnpm vitul android dev`<br>APK: `pnpm vitul android build --apk`<br>Play Store: `pnpm vitul android build --aab` | System Android WebView; produces an `.apk` or `.aab`. The OpenTool backend runs in the app process and uses Tauri IPC.                                                        | Android Studio, Android SDK/NDK, and the Android Rust target.                                                                        |
| iOS                   | Dev: `pnpm vitul ios dev`<br>Build: `pnpm vitul ios build`                                                             | System WKWebView; produces an `.ipa`. The OpenTool backend runs in the app process and uses Tauri IPC.                                                                        | macOS, Xcode, and the iOS Rust target.                                                                                               |
| Windows               | Dev: `pnpm vitul dev`<br>Build: `pnpm vitul build`                                                                     | System WebView2; `.msi` and NSIS `.exe` installers by default.                                                                                                                | Build on Windows with Rust MSVC, Node.js/pnpm, Visual Studio C++ Build Tools (“Desktop development with C++”), and WebView2 Runtime. |
| macOS                 | Dev: `pnpm vitul dev`<br>Build: `pnpm vitul build`                                                                     | System WebView; `.app` bundle and `.dmg` disk image by default.                                                                                                               | Build on macOS with Rust, Node.js/pnpm, and Xcode Command Line Tools (`xcode-select --install`).                                     |

New projects keep desktop, mobile, and AgentOS settings in the root
`vitul.conf.json`. Set desktop package formats with `bundle.targets` and AgentOS
architectures with `agentos.targets`. Vitul generates ignored runtime config
files under `src-vitul/` when desktop or mobile tooling runs; edit the root
config. Existing projects can continue using their runtime config. The standalone `Vitulfile`
flow below packages existing process apps for Linux ARM64 and x64 and remote
websites as architecture-independent AgentOS URL apps. OpenTool's
`rpc.version: 2` is an independent API binding version.
These commands do not build the Ubuntu 24 system image or the AgentOS Desktop host
application; build those from their AgentOS projects.

### Package an existing web app for AgentOS

`Vitulfile` is for standalone web projects that already have their own frontend
and backend, or for websites hosted elsewhere. It does not require
the project to use Rust or Tauri and does not add a Rust proxy. A local app
provides its own HTTP service; a remote-web profile creates an AgentOS URL
package that loads an HTTPS site in the embedded CEF WebApp window.

```sh
cd path/to/existing-project
vitul agentos init
# Edit Vitulfile for the project's build, artifacts, and run command
vitul agentos inspect
vitul agentos validate
vitul build --target agentos
```

Declare `targets: [agentos/arm64, agentos/x64]` in `Vitulfile` to build both
process architectures. Use `--target agentos/arm64` or `--target agentos/x64`
to build only one declared target. For a hosted website, run:

```sh
vitul agentos init --type remote-web --url https://example.com
```

Process packages are written as
`dist/<app>-<version>-agentos-arm64.agt` and/or
`dist/<app>-<version>-agentos-x64.agt`; URL packages use
`dist/<app>-<version>-agentos-any.agt`. See the
[Vitulfile guide](crates/vitul-cli/src/vitulfile/README.md) or
[简体中文说明](crates/vitul-cli/src/vitulfile/README-zh_CN.md) for config
examples, build behavior, and limitations. Both Vitulfile `app.icon` and the
legacy AgentOS `bundle.icon` setting accept PNG and SVG assets for `.agt`
packages.

## AgentOS SDKs

AgentOS SDKs are clients for apps that call AgentOS Gateway APIs. Use them as
language-specific package dependencies instead of copying their source into
Vitul. Projects created by `vitul create` include the TypeScript SDK in
`package.json`; the starter does not call it unless you add that integration:

| Language   | Package                                                           | Useful for                                                            |
| ---------- | ----------------------------------------------------------------- | --------------------------------------------------------------------- |
| TypeScript | [`agentos-sdk-ts`](https://github.com/agentos-org/agentos-sdk-ts) | Vitul web frontends that call an AgentOS Gateway, locally or remotely |
| Dart       | [`agentos_sdk`](https://github.com/agentos-org/agentos-sdk-dart)  | Dart-based AgentOS clients, including the AgentOS Desktop project     |

For a Vitul frontend, import the TypeScript SDK only if the app needs AgentOS
Gateway APIs; the default starter uses OpenTool for its business backend. The
SDKs do not build AgentOS system images or package `.agt` apps. Only SDKs with
public GitHub repositories are listed here.

## Repository layout

- `crates/vitul-cli/` contains the Vitul project generator, development runner,
  and AgentOS packagers. The standalone `Vitulfile` workflow is documented in
  `crates/vitul-cli/src/vitulfile/`.
- `crates/tauri-*` contains desktop runtime code derived from Tauri. Its Cargo
  package and Rust crate names are retained for the current engine.
- `packages/` contains the JavaScript API package (`@tauri-apps/api`), the
  Vitul CLI package (`@litevar/vitul-cli`), and the `vitul` / `create-vitul-app` npm
  command packages.
- `crates/create-vitul-app/` contains the internal scaffold crate used by
  `vitul create`, derived from the upstream `create-tauri-app` project.

Vitul reuses parts of Tauri's source and tooling internally. New projects use
Vitul filenames and commands; the user-facing CLI entry points are `vitul`,
`cargo vitul`, and `create-vitul-app`. The JavaScript API package currently
uses the `@tauri-apps/api` name for inherited runtime modules; that package name
does not promise that existing Tauri apps or plugins work unchanged. Vitul-native
features live under `crates/vitul-*`; inherited source keeps its original
copyright and license notices.

## Project status

Vitul is a pre-1.0 project. See the [support matrix](docs/SUPPORT.md) for
platform coverage and the [maintenance policy](docs/MAINTENANCE.md) for version
and release commitments. External bug reports and pull requests belong in the
[GitHub repository](https://github.com/agentos-org/vitul).
