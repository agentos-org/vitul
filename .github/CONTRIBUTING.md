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
- `crates/create-vitul-app` contains the frontend template engine called by
  `vitul init`.
- `crates/tauri-*` and `packages/api` provide the Tauri-compatible desktop
  runtime and JavaScript API. Their public names and protocol identifiers are
  compatibility interfaces; preserve them when changing the implementation.
- `examples/starter/vitul-demo` demonstrates a native Vitul app, with host IPC for
  system actions and OpenTool for application business calls.
- `examples/agentos/process` and `examples/agentos/url` cover AgentOS packaging for existing
  processes and hosted websites.

See [Architecture](../ARCHITECTURE.md) for the runtime and package boundaries.

## Pull requests

Keep each change focused and describe its user-visible effect in the pull
request. Include the relevant build or test commands you ran and their results.
Read the [support matrix](../docs/SUPPORT.md) before relying on a platform, and
the [maintenance policy](../docs/MAINTENANCE.md) for review and release rules.

The compatibility runtime includes code derived from other projects. Keep its
copyright headers and license files intact, and do not attribute inherited code
to Vitul authors.

## Documentation changes

Follow [documentation ownership](../docs/MAINTENANCE.md#documentation-ownership-and-feature-completeness).
Review a capability as a complete user outcome across configuration, code,
artifacts, startup, authorization, data, cleanup, and verification. Update the
owning reference, English/Chinese entry points, and generated project instructions
when affected. Reuse existing documentation structure; a reported example should
lead to checking the general failure mode across related capabilities.
