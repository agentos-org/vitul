# Vitul Desktop CLI Compatibility Layer

This crate retains the upstream `tauri-cli` package and command implementation as an internal compatibility layer for Vitul's desktop runtime.

Vitul users should invoke desktop commands through `vitul` (or `cargo vitul`). The Vitul CLI controls the user-facing command name, project layout, configuration filenames, and schema URL while delegating compatible desktop operations to this crate. The crate name and inherited implementation remain unchanged to preserve Rust and plugin compatibility.

For user documentation, see the [Vitul README](../../README.md) and [architecture overview](../../ARCHITECTURE.md). Historical upstream behavior and APIs can be compared with the upstream [Tauri CLI documentation](https://v2.tauri.app/reference/cli/).
