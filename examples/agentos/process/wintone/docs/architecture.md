# WinTone architecture and implementation status

## Current structure

```text
public/                  installer import, task status, installed-app launch and uninstall
backend/src/api.rs       HTTP API, installer import, app launch and uninstall routes
backend/src/opentool.rs  read-only methods discoverable by AgentOS Chat
backend/src/storage.rs   tasks, session links, and app records under DATA_DIR
backend/src/runtime.rs   shared runtime catalog, download, integrity check, and atomic extraction
backend/src/prefixes.rs  private Wine prefix per installation
backend/src/external_wayland.rs per-App Unix socket client for AgentOS sessions
backend/src/shortcuts.rs Wine uninstall registry and app-entry discovery
backend/src/installation.rs runtime setup, session lifecycle, and app management
scripts/build-server.sh  Linux ARM64/x64 server build
```

The importer accepts `.exe` and `.msi`, limits each file to 2 GiB, checks the file signature, and records SHA-256. Binary files use random UUID filenames while the UI keeps the original display name. Each import creates a persistent task, prepares the shared runtime and a per-installation prefix, and starts an interactive installer in an AgentOS Wayland session when those services are available.

The HTTP endpoints and OpenTool functions share the same installation-task manifest. OpenTool currently exposes `runtime.status`, `installations.list`, and `apps.list`. Large installer uploads use HTTP multipart rather than OpenTool JSON messages.

After import, WinTone prepares a shared Wine/FEX runtime from a per-target `runtime-catalog.json`, verifies its SHA-256, and allocates a separate prefix directory for the installation. The catalog pins architecture, HTTPS bundle URL, archive format, digest, and Wine/FEX launch commands. The repository does not include a published runtime catalog or bundle yet.

Once runtime and prefix preparation finish, WinTone calls the AgentOS API at `AGENTOS_EXTERNAL_WAYLAND_SOCKET`. WinTone supplies the Wine/FEX command, environment, prefix path, and `/data` read-write grant; AgentOS owns the compositor, window presentation, input, and session lifecycle. WinTone polls `/v1/sessions` to recover state and detect installer completion, then scans Wine uninstall registry entries for launch and uninstall commands.

## AgentOS constraints and validation still needed

AgentOS v1 can only mount the full WinTone persistent data directory as `/data`, so each Windows app session can read and write every prefix, installer, and runtime. Prefixes are logically separate, but this API does not enforce per-installation filesystem isolation. AgentOS currently presents Wayland `wl_shm` surfaces without XWayland; GPU zero-copy, client cursor images, IME/text-input, cross-session clipboard, multiple outputs, and fractional scale remain unavailable or limited.

WinTone has not yet been validated with a published runtime bundle or a Windows installer on an AgentOS device. Runtime bundles must provide Wine's Wayland driver and required libraries; ARM64 bundles also need FEX and an x86-64 RootFS. Registry discovery is heuristic and may not find installers that omit standard uninstall metadata.

## Planned module boundaries

| Module                   | Responsibility                                                                                              |
| ------------------------ | ----------------------------------------------------------------------------------------------------------- |
| `runtime`                | Runtime manifests, integrity checks, install, upgrade, and rollback; Wine/FEX versions are shared on-device |
| `prefixes`               | One prefix per Windows installation by default, with its data path and runtime reference                    |
| `installers`             | Automatically advance imported tasks, launch setup sessions, observe completion, and discover uninstall data |
| `apps`                   | Manage launch entries, icons, and uninstall actions; one install environment may contain several entries    |
| `sessions`               | Prepare Wine/FEX commands for installation and app sessions and track their lifecycle through AgentOS.        |
| `external_wayland`       | Connect to the per-App Unix socket to query capabilities, create sessions, and recover session state.         |
| `shortcuts`              | Discover launch and supported uninstall entries from Wine registry files.                                     |

For ARM64, a Linux FEX command also needs an x86-64 RootFS in the bundle, as described in the [FEX RootFS documentation](https://wiki.fex-emu.com/index.php/Development:Setting_up_RootFS). The catalog format is shown in [runtime-catalog.example.json](runtime-catalog.example.json). Runtime distribution source, license, package signing, and update policy still need to be set before publishing bundles.
