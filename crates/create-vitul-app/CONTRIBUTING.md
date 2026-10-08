# Contributing to the Vitul App Generator

`create-vitul-app` is an internal template engine maintained in the Vitul repository. User-facing project creation and release work belongs to the main Vitul workspace.

## Development

Install Rust and Cargo, then run the workspace's formatting and validation commands from the repository root. The entry point for the generator is `crates/vitul-cli`; changes to templates should be checked through `vitul init` and the generated project structure.

## Adding a template

Templates live under `templates/template-<name>` and include a `.manifest` file describing frontend commands, the development URL, and assets copied from `templates/_assets_`. Update the template list in `src/template.rs` and package-manager mappings in `src/package_manager.rs` when adding a supported template. New user-facing templates should include Vitul naming and configuration conventions.
