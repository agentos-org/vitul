use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

use vitul_desktop_cli::{MacOSIconOptions, MacOSKind, MacOSMode};

const SIDECAR: &str = "icon.icns.vitul.json";
const GENERATED_ICON: &str = "src-vitul/icons/macos/icon.icns";

/// Adds a Vitul-managed macOS ICNS to the generated desktop projection.
/// The root project config remains the source of truth; `bundle.macosIcon` is
/// Vitul-only metadata and is removed before Tauri reads the projected config.
pub(crate) fn prepare_projection(root: &Path, config: &mut Value) -> Result<()> {
  let Some(bundle) = config.get_mut("bundle").and_then(Value::as_object_mut) else {
    return Ok(());
  };
  let custom = bundle.remove("macosIcon");
  let settings = custom.as_ref().and_then(Value::as_object);
  let icon_paths = read_icon_paths(bundle.get("icon"));
  if custom.is_none() && icon_paths.is_empty() {
    return Ok(());
  }
  let custom_icns = icon_paths.iter().any(|path| has_extension(path, "icns"));

  let tracked = icon_paths
    .iter()
    .filter(|path| has_extension(path, "icns"))
    .map(|path| root.join(path))
    .find_map(|path| {
      let sidecar = path.parent()?.join(SIDECAR);
      let metadata = read_metadata(&sidecar).ok()?;
      (metadata.get("generator").and_then(Value::as_str) == Some("vitul-macos-icon-v1"))
        .then_some((path, sidecar, metadata))
    });

  let explicit_mode = settings
    .and_then(|settings| settings.get("mode"))
    .and_then(Value::as_str);
  let tracked_mode = tracked
    .as_ref()
    .and_then(|(_, _, metadata)| metadata.get("mode"))
    .and_then(Value::as_str);
  let mode_name = explicit_mode.or(tracked_mode).unwrap_or("auto");
  let mode = parse_mode(mode_name)?;
  let force_auto = explicit_mode == Some("auto");

  // An untracked .icns is treated as a user-authored asset. Merely adding a
  // PNG beside it must not silently replace it.
  if custom_icns && tracked.is_none() && !force_auto {
    return Ok(());
  }
  if mode == MacOSMode::Passthrough && custom_icns {
    return Ok(());
  }

  let source = settings
    .and_then(|settings| settings.get("source"))
    .and_then(Value::as_str)
    .map(|path| root.join(path))
    .or_else(|| {
      tracked
        .as_ref()
        .and_then(|(_, _, metadata)| metadata.get("source"))
        .and_then(Value::as_str)
        .map(|path| root.join(path))
        .filter(|path| path.is_file())
    })
    .or_else(|| {
      icon_paths
        .iter()
        .find(|path| has_extension(path, "png") || has_extension(path, "svg"))
        .map(|path| root.join(path))
    });
  let Some(source) = source else {
    if custom_icns {
      return Ok(());
    }
    bail!(
      "macOS icon adaptation needs a PNG or SVG source in bundle.icon or bundle.macosIcon.source"
    );
  };
  if !source.is_file() {
    bail!("macOS icon source does not exist: {}", source.display());
  }

  let logo_scale_value = settings
    .and_then(|settings| settings.get("logoScale"))
    .or_else(|| {
      tracked
        .as_ref()
        .and_then(|(_, _, metadata)| metadata.get("logoScale"))
    });
  let logo_scale = match logo_scale_value {
    Some(value) => value
      .as_u64()
      .ok_or_else(|| anyhow::anyhow!("bundle.macosIcon.logoScale must be an integer"))?,
    None => 72,
  };
  if !(1..=100).contains(&logo_scale) {
    bail!("bundle.macosIcon.logoScale must be between 1 and 100");
  }

  let options = MacOSIconOptions {
    mode,
    kind: settings
      .and_then(|settings| settings.get("kind"))
      .and_then(Value::as_str)
      .map(parse_kind)
      .transpose()?
      .or_else(|| {
        tracked
          .as_ref()
          .and_then(|(_, _, metadata)| metadata.get("kind"))
          .and_then(Value::as_str)
          .and_then(|value| parse_kind(value).ok())
      })
      .unwrap_or_default(),
    background: settings
      .and_then(|settings| settings.get("background"))
      .and_then(Value::as_str)
      .map(str::to_owned)
      .or_else(|| {
        tracked
          .as_ref()
          .and_then(|(_, _, metadata)| metadata.get("background"))
          .and_then(Value::as_str)
          .map(str::to_owned)
      })
      .unwrap_or_else(|| "#ffffff".to_owned()),
    logo_scale: logo_scale as u32,
  };

  let output_dir = tracked
    .as_ref()
    .and_then(|(path, _, _)| path.parent())
    .map(Path::to_path_buf)
    .unwrap_or_else(|| root.join(GENERATED_ICON).parent().unwrap().to_path_buf());
  let icns_path = output_dir.join("icon.icns");
  let sidecar = output_dir.join(SIDECAR);
  if !is_up_to_date(&source, &sidecar, &icns_path, &options)? {
    vitul_desktop_cli::generate_macos_icon(&source, &output_dir, &options)
      .map_err(|error| anyhow::anyhow!("could not generate macOS icon: {error}"))?;
  }

  let mut projected_icons: Vec<String> = icon_paths
    .into_iter()
    .filter(|path| !has_extension(path, "icns") || !force_auto)
    .collect();
  let projected_icns = relative_to_project(root, &icns_path)?;
  if !projected_icons.iter().any(|path| path == &projected_icns) {
    projected_icons.push(projected_icns);
  }
  bundle.insert("icon".to_owned(), json!(projected_icons));
  Ok(())
}

fn is_up_to_date(
  source: &Path,
  sidecar: &Path,
  icns: &Path,
  options: &MacOSIconOptions,
) -> Result<bool> {
  if !sidecar.is_file() || !icns.is_file() {
    return Ok(false);
  }
  let metadata = read_metadata(sidecar)?;
  let source_path = source
    .canonicalize()
    .unwrap_or_else(|_| source.to_path_buf());
  let bytes = fs::read(&source_path)
    .with_context(|| format!("could not read macOS icon source {}", source_path.display()))?;
  let hash = format!("{:x}", Sha256::digest(bytes));
  Ok(
    metadata.get("generator").and_then(Value::as_str) == Some("vitul-macos-icon-v1")
      && metadata.get("source").and_then(Value::as_str) == source_path.to_str()
      && metadata.get("sourceSha256").and_then(Value::as_str) == Some(hash.as_str())
      && metadata.get("mode").and_then(Value::as_str) == Some(mode_name(options.mode))
      && metadata.get("kind").and_then(Value::as_str) == Some(kind_name(options.kind))
      && metadata.get("background").and_then(Value::as_str) == Some(options.background.as_str())
      && metadata.get("logoScale").and_then(Value::as_u64) == Some(options.logo_scale as u64),
  )
}

fn read_metadata(path: &Path) -> Result<Value> {
  serde_json::from_slice(
    &fs::read(path).with_context(|| format!("could not read {}", path.display()))?,
  )
  .with_context(|| format!("could not parse {}", path.display()))
}

fn read_icon_paths(value: Option<&Value>) -> Vec<String> {
  match value {
    Some(Value::String(path)) => vec![path.clone()],
    Some(Value::Array(paths)) => paths
      .iter()
      .filter_map(Value::as_str)
      .map(str::to_owned)
      .collect(),
    _ => Vec::new(),
  }
}

fn has_extension(path: &str, extension: &str) -> bool {
  Path::new(path)
    .extension()
    .and_then(|value| value.to_str())
    .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

fn parse_mode(value: &str) -> Result<MacOSMode> {
  match value {
    "auto" => Ok(MacOSMode::Auto),
    "passthrough" => Ok(MacOSMode::Passthrough),
    _ => bail!("bundle.macosIcon.mode must be `auto` or `passthrough`"),
  }
}

fn parse_kind(value: &str) -> Result<MacOSKind> {
  match value {
    "square" => Ok(MacOSKind::Square),
    "logo" => Ok(MacOSKind::Logo),
    _ => bail!("bundle.macosIcon.kind must be `square` or `logo`"),
  }
}

fn mode_name(mode: MacOSMode) -> &'static str {
  match mode {
    MacOSMode::Auto => "auto",
    MacOSMode::Passthrough => "passthrough",
  }
}

fn kind_name(kind: MacOSKind) -> &'static str {
  match kind {
    MacOSKind::Square => "square",
    MacOSKind::Logo => "logo",
  }
}

fn relative_to_project(root: &Path, path: &Path) -> Result<String> {
  let relative = path
    .strip_prefix(root)
    .with_context(|| format!("{} is outside the project root", path.display()))?;
  Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;
  use std::path::PathBuf;

  fn fixture_icon() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tests/fixtures/desktop-api/.icons/icon.png")
  }

  #[test]
  fn auto_adapts_png_and_reuses_the_tracked_output_on_repeated_projection() {
    let root = tempfile::tempdir().unwrap();
    let icons = root.path().join("src-vitul/icons");
    fs::create_dir_all(&icons).unwrap();
    fs::copy(fixture_icon(), icons.join("icon.png")).unwrap();
    let initial = json!({
      "bundle": { "icon": ["src-vitul/icons/icon.png"] }
    });

    let mut first = initial.clone();
    prepare_projection(root.path(), &mut first).unwrap();
    let generated = root.path().join(GENERATED_ICON);
    assert!(generated.is_file());
    assert!(generated.with_file_name("icon-macos-master.png").is_file());
    assert_eq!(first["bundle"]["icon"].as_array().unwrap().len(), 2);
    assert_eq!(
      first["bundle"]["icon"][1],
      "src-vitul/icons/macos/icon.icns"
    );
    assert!(first["bundle"].get("macosIcon").is_none());
    let first_icns = fs::read(&generated).unwrap();

    let mut second = initial;
    prepare_projection(root.path(), &mut second).unwrap();
    assert_eq!(fs::read(&generated).unwrap(), first_icns);
    assert_eq!(second["bundle"]["icon"].as_array().unwrap().len(), 2);

    let alternate = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tests/fixtures/desktop-api/.icons/Square310x310Logo.png");
    fs::copy(alternate, icons.join("icon.png")).unwrap();
    let mut changed_source = json!({
      "bundle": { "icon": ["src-vitul/icons/icon.png"] }
    });
    prepare_projection(root.path(), &mut changed_source).unwrap();
    let changed_source_icns = fs::read(&generated).unwrap();
    assert_ne!(changed_source_icns, first_icns);

    let mut changed_options = json!({
      "bundle": {
        "icon": ["src-vitul/icons/icon.png"],
        "macosIcon": { "mode": "auto", "kind": "logo", "logoScale": 62 }
      }
    });
    prepare_projection(root.path(), &mut changed_options).unwrap();
    assert_ne!(fs::read(&generated).unwrap(), changed_source_icns);
    let metadata: Value = read_metadata(&generated.with_file_name(SIDECAR)).unwrap();
    assert_eq!(metadata["kind"], "logo");
    assert_eq!(metadata["logoScale"], 62);
  }

  #[test]
  fn tracked_icns_keeps_its_original_master_instead_of_a_bundle_thumbnail() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("app-icon.png");
    fs::copy(fixture_icon(), &source).unwrap();
    let icons = root.path().join("src-vitul/icons");
    vitul_desktop_cli::generate_macos_icon(&source, &icons, &MacOSIconOptions::default()).unwrap();
    fs::copy(
      Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../create-vitul-app/templates/_base_/src-tauri/icons/32x32.png"),
      icons.join("32x32.png"),
    )
    .unwrap();
    let config = json!({
      "bundle": { "icon": ["src-vitul/icons/32x32.png", "src-vitul/icons/icon.icns"] }
    });
    let first = fs::read(icons.join("icon.icns")).unwrap();
    prepare_projection(root.path(), &mut config.clone()).unwrap();
    assert_eq!(fs::read(icons.join("icon.icns")).unwrap(), first);

    fs::copy(icons.join("32x32.png"), &source).unwrap();
    prepare_projection(root.path(), &mut config.clone()).unwrap();
    assert_ne!(fs::read(icons.join("icon.icns")).unwrap(), first);
    let metadata = read_metadata(&icons.join(SIDECAR)).unwrap();
    assert_eq!(
      metadata["source"].as_str(),
      source.canonicalize().unwrap().to_str()
    );
    assert_eq!(
      metadata["sourceSha256"],
      format!("{:x}", Sha256::digest(fs::read(&source).unwrap()))
    );
  }

  #[test]
  fn untracked_custom_icns_is_used_without_requiring_or_generating_a_png() {
    let root = tempfile::tempdir().unwrap();
    let custom = json!({
      "bundle": { "icon": ["src-vitul/icons/custom.icns"] }
    });
    let mut projected = custom.clone();
    prepare_projection(root.path(), &mut projected).unwrap();
    assert_eq!(projected, custom);
    assert!(!root.path().join(GENERATED_ICON).exists());
  }
}
