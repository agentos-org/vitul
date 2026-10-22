# Vitul Architecture

[简体中文](ARCHITECTURE-zh_CN.md)

Vitul separates the app's business backend from the operating-system host.
Business calls use the OpenTool message format; the transport depends on the
runtime. System operations use the host's IPC bridge, where permissions and
platform behavior can be enforced.

## Ownership from requirement to implementation

| Requested behavior | Main implementation | Check together |
| --- | --- | --- |
| Screens, interactions, language, appearance | `src/` and host context adapter | Initial state, host changes, translations, style tokens |
| Business operations, data, long-running tasks | `backend/src/tools/` through OpenTool | Data paths, task lifecycle, errors, progress |
| Windows, dialogs, clipboard, notifications, external links | `src/host.ts` and the host implementation | Platform support, permissions, denial, unavailable behavior |
| AgentOS Gateway access | `src/lib/agentos.ts` and SDK | Gateway address and authentication; separate from app OpenTool calls |
| External processes and system integration | Backend or host facility for that protocol | Executables, architecture, permissions, startup, cleanup |

Configuration declares intent, app code implements business logic and protocol
integration, the packager collects artifacts, and the host authorizes and executes
platform behavior. These responsibilities do not substitute for one another.
If a feature is absent from `HostProvider`, check the platform protocol and existing
implementation before inventing an API call. The same feature name does not
promise the same implementation across desktop, mobile, and AgentOS.

Edit root `vitul.conf.json`; runtime configuration under `src-vitul/` and packaged
`app.json` are projections or build output. Trace failures through root fields,
generated fields/files, startup entry points, and host effects, then edit the
source responsible for the failing step.

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
- `crates/create-vitul-app/` is the internal scaffold crate, derived from the
  upstream `create-tauri-app` project and customized into an OpenTool-first
  project by the Vitul generator.
- `crates/tauri-*` contain the compatible desktop runtime, CLI, build support,
  and bundler. Vitul keeps their established names; shared source does not imply existing
  Tauri projects or plugins work unchanged.
- `packages/` contains compatible frontend APIs and CLI packages.

## AgentOS package contract

The root `vitul.conf.json` uses `schemaVersion: 2`; generated `.agt` packages
use `manifestVersion: 1`. Process packages declare `architecture` as `arm64`
or `x64`, and the packager supports both. Remote-web packages use `any`.
These versions are independent of the OpenTool binding's `rpc.version: 2`.

For process apps, `agentos.run.port` (default `9627`) becomes manifest `run.port`
and must match the backend listener. In container mode, Shell maps an assigned
loopback host port to the container port. Packages declare a frontend entry,
optional OpenTool RPC binding, and requested `native.*` capabilities. Developers
edit the root config; packaged `app.json` is generated output.


## Compatibility and attribution

Vitul changes the product-facing brand while preserving Tauri-compatible
crate names, macros, JavaScript package names, configuration files, and IPC
identifiers. These names are part of the compatibility surface. Inherited
source remains subject to its original copyright and license notices; see the
repository's license files.
