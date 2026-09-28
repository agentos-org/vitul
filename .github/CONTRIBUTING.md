# Vitul Contribution Guide

Vitul is maintained in the [Vitul GitLab project](https://gitlab.litevar.com:90/litevar/agentos/vitul).
Use its issue tracker for bugs and proposals, and submit changes as merge requests.
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

## Changes and merge requests

Keep each change focused and describe its user-visible effect in the merge
request. Include the relevant build or test commands you ran and their results.
Vitul's release process is not configured yet; see the [release notes](./RELEASING.md).

The compatibility runtime includes code derived from other projects. Keep its
copyright headers and license files intact, and do not attribute inherited code
to Vitul authors.
