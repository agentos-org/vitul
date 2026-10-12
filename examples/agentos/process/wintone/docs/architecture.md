# WinTone architecture and implementation status

## Current structure

```text
vitul.conf.json         unified Vitul project and AgentOS process configuration
public/                  installer import, task status, installed-app launch and uninstall
backend/src/api.rs       HTTP API, installer import, app launch and uninstall routes
backend/src/opentool.rs  read-only methods discoverable by AgentOS Chat
backend/src/storage.rs   tasks, session links, and app records under DATA_DIR
backend/src/pe_arch.rs   PE machine parsing and x86/x64 versus ARM64 routing
backend/src/runtime.rs   shared runtime catalog, bundled/HTTPS integrity check, and atomic extraction
backend/src/prefixes.rs  private Wine prefix per installation
backend/src/external_wayland.rs per-App Unix socket client for AgentOS sessions
backend/src/shortcuts.rs Wine uninstall registry and app-entry discovery
backend/src/installation.rs runtime setup, session lifecycle, and app management
```

`vitul build --target agentos/arm64` or `vitul build --target agentos/x64` reads the root config, cross-builds the Rust backend for the selected target, and stages the declared process artifacts.

The importer accepts `.exe` and `.msi`, limits each file to 2 GiB, checks the file signature, and records SHA-256. Binary files use random UUID filenames while the UI keeps the original display name. Each import creates a persistent task, prepares the shared runtime and a per-installation prefix, and starts an interactive installer in an AgentOS Wayland session when those services are available.

The HTTP endpoints and OpenTool functions share the same installation-task manifest. OpenTool currently exposes `runtime.status`, `installations.list`, and `apps.list`. Large installer uploads use HTTP multipart rather than OpenTool JSON messages.

For `.exe` imports, WinTone reads the PE machine field. i386 and x86-64 PE files use x86/x64 Wine; ARM64 PE files use native ARM64 Wine. ARM64EC/ARM64X are not supported yet, and `.msi` imports currently default to the x86/x64 path. Each installation gets a private prefix tagged with its Wine architecture.

The ARM64 `.agt` contains a SHA-256-pinned `tar.zst` bundle and adjacent `runtime-catalog.json`, with the x86/x64 compatibility path and a separate native ARM64 path. When WinTone starts for the first time, it verifies and extracts the bundle into `DATA_DIR` in a background task so the UI and service can become available immediately. The catalog also supports HTTPS bundles for later independent updates. Both paths select the Wayland driver.

Once runtime and prefix preparation finish, WinTone calls the AgentOS API at `AGENTOS_EXTERNAL_WAYLAND_SOCKET`. Each session receives the shared Wine/FEX runtime read-only and its own prefix read-write. Installer sessions also receive the imported installer as a read-only file at `/grants/installer/content`. Runtime, prefix, and installer paths use `/grants/runtime`, `/grants/prefix`, and `/grants/installer/content`; WinTone does not grant the full `/data` directory. AgentOS owns the compositor, window presentation, input, and session lifecycle. WinTone polls `/v1/sessions` to recover state and detect installer completion, then scans Wine uninstall registry entries for launch and uninstall commands.

## AgentOS constraints and validation still needed

WinTone checks `fileGrants.available` before creating a session and waits with the reported reason if scoped grants are unavailable; it never falls back to whole-data access. GPU acceleration and independent clipboard read/write permissions are configurable for future sessions and default off. AgentOS currently supports the relevant Wayland window, cursor, text-input, clipboard, and output APIs; GPU/DMA-BUF uses an opt-in copy into Flutter's texture rather than a zero-copy path. Visible desktop, physical GPU, and target-app behavior still need validation.

The ARM64 bundle is assembled from Ubuntu 24.04 AMD64 Wine packages, Ubuntu ARM64 ports, and the official FEX ARM64 PPA. The build checks that both Wine Wayland drivers exist. PE parser and session-routing tests run locally, but the native ARM64 Wine Wayland session and Windows apps still need testing on the target AgentOS device. Registry discovery is heuristic and may not find installers that omit standard uninstall metadata. The x64 AgentOS target does not include a Wine runtime yet.

## Planned module boundaries

| Module                   | Responsibility                                                                                              |
| ------------------------ | ----------------------------------------------------------------------------------------------------------- |
| `runtime`                | Runtime manifests, integrity checks, install, upgrade, and rollback; Wine/FEX versions are shared on-device |
| `pe_arch`                | Parse PE machine values and reject unsupported ARM64EC/ARM64X executables                                 |
| `prefixes`               | One prefix per Windows installation by default, with its data path and runtime reference                    |
| `installers`             | Automatically advance imported tasks, launch setup sessions, observe completion, and discover uninstall data |
| `apps`                   | Manage launch entries, icons, and uninstall actions; one install environment may contain several entries    |
| `sessions`               | Prepare Wine/FEX commands for installation and app sessions and track their lifecycle through AgentOS.        |
| `external_wayland`       | Connect to the per-App Unix socket to query capabilities, create sessions, and recover session state.         |
| `shortcuts`              | Discover launch and supported uninstall entries from Wine registry files.                                     |

On ARM64, only the x86/x64 Wine path uses FEX and its x86-64 RootFS; ARM64 PE files use native ARM64 Wine. The catalog format is shown in [runtime-catalog.example.json](runtime-catalog.example.json). The bundle retains upstream copyright files; a public release still needs a reviewed notices and update policy.


## Integration verification / 集成验证

External Wayland is a window protocol, not a Wine compatibility guarantee. WinTone owns Wine/FEX and prefixes; AgentOS owns generic isolated sessions, authorization and presentation. Launchers require the platform-assigned private Wayland socket and clear DISPLAY. AgentOS explicitly does not provide XWayland. WinTone uses only Wine’s native Wayland backend. Windows programs requiring Wine’s X11 backend are unsupported; no X11 fallback or XWayland platform capability is requested. A running process or created window does not establish mapping, Flutter presentation, input/focus, scaling, multi-window, actual GPU presentation or cleanup. Each layer requires evidence; unknown results stay unverified.

See [shared review and platform dependencies](../../docs/tone-platform-integration-review.md).
