# Vitul CLI

[简体中文](README-zh_CN.md)

The Vitul CLI exposes `vitul` as its user-facing command. It runs Vitul app
lifecycle commands and calls Tauri-compatible tooling internally where the
desktop runtime requires it.

## Installation status

The `@vitul/cli` npm package is not published yet. For now, install the Rust
CLI from a Vitul source checkout with `cargo install --path crates/vitul-cli
--locked`. Generated projects use that `vitul` executable from `PATH` and do
not depend on this JavaScript package.

Use `vitul` for project commands, for example `pnpm vitul dev` and
`pnpm vitul build`. New Vitul projects expose no `tauri` command alias.

This JavaScript package wraps the Rust CLI and is being prepared as an optional
distribution channel. See the [Vitul architecture](../../ARCHITECTURE.md) for
the runtime boundaries.

## Licenses

Inherited code retains its original copyright and license notices.
