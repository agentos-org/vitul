# WinTone

WinTone is an AgentOS desktop-user Native process app configured with the root `vitul.conf.json`. Its backend listens on the manifest port 28641 and remains available from the AgentOS tray after its window closes. On first launch, it prepares the shared Windows runtime in the background. Dropping an `.exe` or `.msi` installer then creates a per-installation prefix and opens the installer in an AgentOS Wayland session. After setup, WinTone discovers installed apps and offers launch and uninstall actions.

## Implemented

- Stores installers, installation tasks, the shared runtime, and per-installation Wine prefixes under `DATA_DIR`; uploads are checked by extension and file signature and receive a SHA-256 digest.
- The ARM64 `.agt` contains two runtime paths: x86/x64 apps use an x86-64 Ubuntu RootFS through FEX, and ARM64 PE files use the native ARM64 runtime. On first WinTone launch, it verifies and extracts the shared runtime in the background; each installation gets a private prefix.
- EXE imports are classified from their PE machine field and routed to the matching runtime. ARM64EC/ARM64X are not supported yet; MSI imports currently use the x86/x64 path.
- Uses AgentOS `native.external-wayland` sessions to start installers and apps. ARM64 Windows PE files launch directly through native ARM64 Wine; FEX is used only for x86/x64 Wine.
- Import can install a setup program or run a portable EXE directly. Portable apps remain runnable from their task after the first session exits; the checked-in GUI samples should use direct-run mode.
- Discovers launch and uninstall entries from the Wine prefix registry and tracks session state across WinTone restarts.
- Provides app launch and uninstall controls in the UI.
- The UI is built with React, TypeScript, Tailwind CSS, Vite, and pnpm. It supports Simplified Chinese and English, with system, light, and dark themes using a Windows blue accent. Preferences are stored in the local browser.
- Provides runtime preparation, task, app-list, and status APIs plus `runtime.status`, `installations.list`, and `apps.list` OpenTool methods.
- Registers the WinTone icon with AgentOS Shell through the process tray socket, so the Shell can reopen or exit WinTone from its tray menu while the window is closed.
- Migrates existing staged-installer records and displays task status in the management UI.
- Targets AgentOS ARM64 and x64; Vitul builds the Rust backend for the selected target with Docker.

The ARM64 package includes native ARM64 Wine and the existing x86/x64 Wine+FEX runtime. It can test PE architecture dispatch when AgentOS external Wayland sessions are available; application compatibility depends on Wine. ARM64EC/ARM64X and ARM64 MSI detection are not included yet. The x64 target does not yet ship a Wine runtime. The AgentOS session API uses scoped file grants: one shared read-only runtime, one writable prefix, and a read-only installer file during setup. GPU and clipboard permissions are configurable and default off.

## Layout

```text
backend/src/       Rust HTTP and OpenTool service, installation tasks, runtime status
frontend/src/      React, TypeScript, Tailwind CSS UI and bilingual strings
frontend/dist/     Vite output staged as public/ in the package
docs/              Module boundaries and runtime integration notes
vitul.conf.json    Unified Vitul project and AgentOS process configuration
```

## Build

Requirements: Vitul CLI, Docker Buildx, and network access to Ubuntu ports/archives and the FEX PPA. The ARM64 build assembles both Wine paths before Vitul compiles and packages WinTone.

```sh
cd frontend
pnpm install --ignore-workspace
cd ..
vitul agentos validate
vitul build --target agentos/arm64
# or vitul build --target agentos/x64
```

See the [architecture notes](docs/architecture.md) for module boundaries and the [runtime catalog example](docs/runtime-catalog.example.json).


## Integration verification / 集成验证

External Wayland is a window protocol, not a Wine compatibility guarantee. WinTone owns Wine/FEX and prefixes; AgentOS owns generic isolated sessions, authorization and presentation. Launchers require the platform-assigned private Wayland socket and clear DISPLAY. AgentOS explicitly does not provide XWayland. WinTone uses only Wine’s native Wayland backend. Windows programs requiring Wine’s X11 backend are unsupported; no X11 fallback or XWayland platform capability is requested. A running process or created window does not establish mapping, Flutter presentation, input/focus, scaling, multi-window, actual GPU presentation or cleanup. Each layer requires evidence; unknown results stay unverified.

See [shared review and platform dependencies](../docs/tone-platform-integration-review.md).
