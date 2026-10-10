# WinTone

WinTone is an AgentOS process app packaged with Vitulfile. Dropping an `.exe` or `.msi` installer prepares Wine/FEX and a per-installation prefix, then opens the installer in an AgentOS Wayland session. After setup, WinTone reads Wine uninstall registry entries and offers launch and uninstall actions for discovered apps.

## Implemented

- Stores installers, installation tasks, the shared runtime, and per-installation Wine prefixes under `DATA_DIR`; uploads are checked by extension and file signature and receive a SHA-256 digest.
- On first import, reads a runtime catalog, downloads the Wine/FEX bundle, verifies SHA-256, and creates a separate prefix for that installation.
- Uses AgentOS `native.external-wayland` sessions to start installers and apps; ARM64 invokes Wine through FEX, while x64 invokes Wine directly.
- Discovers launch and uninstall entries from the Wine prefix registry and tracks session state across WinTone restarts.
- Provides app launch and uninstall controls in the UI.
- Provides runtime preparation, task, app-list, and status APIs plus `runtime.status`, `installations.list`, and `apps.list` OpenTool methods.
- Migrates existing staged-installer records and displays task status in the management UI.
- Targets AgentOS ARM64 and x64; Docker is used for cross-architecture builds.

This checkout does not include a published Wine/FEX runtime catalog, so it cannot yet run a Windows installer. The AgentOS session API bridge is implemented; publishing and validating Wine/FEX bundles is the remaining runtime dependency. AgentOS v1 currently mounts WinTone's full data directory as writable `/data` for every Windows app session. See the [architecture notes](docs/architecture.md) for this access boundary and other API limitations.

## Layout

```text
backend/src/       Rust HTTP and OpenTool service, installation tasks, runtime status
public/            WinTone management UI
scripts/           AgentOS architecture build script
docs/              Module boundaries and runtime integration notes
Vitulfile          AgentOS process packaging config
```

## Build

Requirements: Vitul CLI, stable Rust and Cargo; Docker is required for cross builds.

```sh
vitul agentos validate
vitul build --target agentos/arm64
# or vitul build --target agentos/x64
```

See the [architecture notes](docs/architecture.md) for module boundaries and the [runtime catalog example](docs/runtime-catalog.example.json).
