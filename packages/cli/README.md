# Vitul CLI

[简体中文](README-zh_CN.md)

The Vitul CLI exposes `vitul` as its user-facing command. It runs Vitul app
lifecycle commands and calls Tauri-compatible tooling internally where the
desktop runtime requires it.

## This module

Install this package in a Vitul project and use the product command:

```sh
pnpm add -D @vitul/cli
pnpm vitul dev
pnpm vitul build
```

Use `vitul` for project commands, for example `pnpm vitul dev` and
`pnpm vitul build`. New Vitul projects expose no `tauri` command alias.

The JavaScript package wraps the Rust CLI and is compatible with npm, pnpm,
yarn, and bun. See the [Vitul architecture](../../ARCHITECTURE.md) for the
runtime boundaries.

## Licenses

Inherited code retains its original copyright and license notices.
