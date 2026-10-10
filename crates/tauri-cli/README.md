# Vitul Desktop CLI

This crate packages the desktop and mobile command implementation inherited from Tauri 2 under Vitul's Rust package name, `vitul-desktop-cli`.

Vitul users should invoke desktop commands through `vitul` (or `cargo vitul`). The Vitul CLI controls the user-facing command name, project layout, configuration filenames, and schema URL while using the compatible desktop runtime implementation. Some lower-level runtime APIs and executable names retain Tauri naming for compatibility with the existing runtime and ecosystem.

For user documentation, see the [Vitul README](../../README.md) and [architecture overview](../../ARCHITECTURE.md). Historical upstream behavior and APIs can be compared with the upstream [Tauri CLI documentation](https://v2.tauri.app/reference/cli/).
