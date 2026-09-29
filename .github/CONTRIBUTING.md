# Vitul Contribution Guide

Vitul's designated public project home, issue tracker, and contribution entry point is
[GitHub](https://github.com/agentos-org/vitul). Report bugs and proposals there
and submit changes as pull requests.
Before contributing, read the [Code of Conduct](./CODE_OF_CONDUCT.md).

## Development setup

Install the repository's JavaScript dependencies with `pnpm install`. Rust
packages use the workspace manifests in the repository root. The workspace
requires Rust 1.90 or newer; the JavaScript workspace requires the version of
Node.js declared by the package you are working on.

## Project areas

- `crates/vitul-cli` implements the Vitul command, project initialization,
  OpenTool integration, and AgentOS packaging.
- `vendor/create-vitul-app` contains the frontend template engine called by
  `vitul init`.
- `crates/tauri-*` and `packages/api` provide the Tauri-compatible desktop
  runtime and JavaScript API. Their public names and protocol identifiers are
  compatibility interfaces; preserve them when changing the implementation.
- `examples/vitul-opentool` demonstrates host IPC for system actions and
  OpenTool for application business calls.

See [Architecture](../ARCHITECTURE.md) for the runtime and package boundaries.

## Pull requests

Keep each change focused and describe its user-visible effect in the pull
request. Include the relevant build or test commands you ran and their results.
Read the [support matrix](../docs/SUPPORT.md) before relying on a platform, and
the [maintenance policy](../docs/MAINTENANCE.md) for review and release rules.

The compatibility runtime includes code derived from other projects. Keep its
copyright headers and license files intact, and do not attribute inherited code
to Vitul authors.
