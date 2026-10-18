#!/usr/bin/env bash
set -euo pipefail

# Run after replacing the full-bleed source. Pass a built Vitul CLI as $1.
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
icon_cli="${1:-$repo_root/target/debug/vitul}"
icon_source="$repo_root/crates/vitul-cli/assets/vitul-logo.png"
template_root="$repo_root/crates/create-vitul-app/templates/_base_"
icon_output="$(mktemp -d)"
trap 'rm -rf "$icon_output"' EXIT

"$icon_cli" icon "$icon_source" --output "$icon_output"
cp "$icon_source" "$template_root/app-icon.png"
for icon_path in "$template_root"/src-tauri/icons/*; do
  cp "$icon_output/$(basename "$icon_path")" "$icon_path"
done
