# Vitul CLI

[简体中文](README-zh_CN.md)

The Vitul CLI exposes `vitul` as its user-facing command. It runs Vitul app
lifecycle commands and uses desktop tools derived from Tauri where needed.

## npm packages

This package supplies the native JavaScript binding used by the public `vitul`
and `create-vitul-app` command packages. New projects also install `@litevar/vitul-cli`
as a development dependency, which provides the `vitul` script command.

The native binding and its target-specific `@litevar/vitul-cli-*` packages are
published through the GitHub Actions release workflow. The `vitul` and
`create-vitul-app` command wrappers use a separate manual workflow and may be
published independently. For local development, install the Rust CLI from a
source checkout with `cargo install --path crates/vitul-cli --locked`.

Use `vitul` for project commands, for example `pnpm vitul dev` and
`pnpm vitul build`. New Vitul projects expose no `tauri` command alias.

This JavaScript package wraps the Vitul CLI for npm distribution.
See the [Vitul architecture](../../ARCHITECTURE.md) for the runtime boundaries.

## Licenses

Inherited code retains its original copyright and license notices.
