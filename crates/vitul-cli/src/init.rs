use anyhow::{Context, Result, bail};
use std::{
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
};

const BACKEND_CARGO: &str = include_str!("../templates/backend.Cargo.toml");
const BACKEND_LIB: &str = include_str!("../templates/backend-lib.rs");
const BACKEND_MAIN: &str = include_str!("../templates/backend-main.rs");
const FRONTEND_INDEX: &str = include_str!("../templates/index.html");
const FRONTEND_MAIN: &str = include_str!("../templates/frontend-main.ts");
const FRONTEND_OPENTOOL: &str = include_str!("../templates/opentool.ts");
const FRONTEND_HOST: &str = include_str!("../templates/host.ts");
const DESKTOP_HOST: &str = include_str!("../templates/desktop-host.ts");
const VITUL_LIB: &str = include_str!("../templates/vitul-lib.rs");
const VITE_CONFIG: &str = include_str!("../templates/vite.config.ts");
const VITUL_CONFIG: &str = include_str!("../templates/vitul.conf.json");
const PROJECT_README: &str = include_str!("../templates/VITUL.md");
const PROJECT_README_ZH_CN: &str = include_str!("../templates/VITUL-zh_CN.md");

pub fn run(mut args: Vec<OsString>) -> Result<()> {
  if args
    .iter()
    .any(|arg| arg == "--help" || arg == "-h" || arg == "--version" || arg == "-v")
  {
    create_vitul_app::run(args, Some("vitul init".to_owned()), None);
    return Ok(());
  }

  let project_name = project_name(&args).unwrap_or_else(|| "vitul-app".to_owned());
  reject_tauri_version_option(&args)?;
  if option_value(&args, "--identifier", "--identifier").is_none() {
    args.extend([
      OsString::from("--identifier"),
      OsString::from(default_identifier(&project_name)),
    ]);
  }
  if !has_option(&args, "--template", "-t") {
    args.extend([OsString::from("--template"), OsString::from("vanilla-ts")]);
  } else if option_value(&args, "--template", "-t").as_deref() != Some("vanilla-ts") {
    bail!("the OpenTool starter currently supports the `vanilla-ts` template");
  }

  create_vitul_app::run(args, Some("vitul init".to_owned()), None);
  let project_root = std::env::current_dir()?.join(project_name);
  rename_generated_runtime_dir(&project_root)?;
  rename_generated_config(&project_root)?;
  scaffold(&project_root)
}

fn rename_generated_runtime_dir(project_root: &Path) -> Result<()> {
  let generated_runtime_dir = project_root.join("src-tauri");
  let vitul_runtime_dir = project_root.join("src-vitul");
  if vitul_runtime_dir.exists() {
    if generated_runtime_dir.exists() {
      bail!(
        "both generated runtime directories exist: {} and {}",
        generated_runtime_dir.display(),
        vitul_runtime_dir.display()
      );
    }
    return Ok(());
  }
  fs::rename(&generated_runtime_dir, &vitul_runtime_dir).with_context(|| {
    format!(
      "failed to rename generated runtime directory {} to {}",
      generated_runtime_dir.display(),
      vitul_runtime_dir.display()
    )
  })
}

fn rename_generated_config(project_root: &Path) -> Result<()> {
  let tauri_config = project_root.join("src-vitul/tauri.conf.json");
  let vitul_config = project_root.join("src-vitul/vitul.conf.json");
  if vitul_config.is_file() {
    return Ok(());
  }
  fs::rename(&tauri_config, &vitul_config).with_context(|| {
    format!(
      "failed to rename generated config {} to {}",
      tauri_config.display(),
      vitul_config.display()
    )
  })
}

fn reject_tauri_version_option(args: &[OsString]) -> Result<()> {
  if has_option(args, "--tauri-version", "") {
    bail!(
      "`--tauri-version` is not a Vitul option; `vitul init` selects the supported desktop runtime automatically"
    );
  }
  Ok(())
}

fn default_identifier(project_name: &str) -> String {
  let mut suffix: String = project_name
    .chars()
    .map(|character| {
      if character.is_ascii_alphanumeric() || character == '-' {
        character.to_ascii_lowercase()
      } else {
        '-'
      }
    })
    .collect();
  suffix = suffix.trim_matches('-').to_owned();
  if suffix.is_empty() || !suffix.as_bytes()[0].is_ascii_alphabetic() {
    suffix = format!("app-{suffix}");
  }
  format!("com.vitul.{suffix}")
}

fn project_name(args: &[OsString]) -> Option<String> {
  let mut skip_value = false;
  let mut name = None;
  for arg in args {
    let value = arg.to_string_lossy();
    if skip_value {
      skip_value = false;
      continue;
    }
    if [
      "-m",
      "--manager",
      "-t",
      "--template",
      "--identifier",
      "--tauri-version",
    ]
    .contains(&value.as_ref())
    {
      skip_value = true;
    } else if !value.starts_with('-') {
      name = Some(value.into_owned());
    }
  }
  name
}

fn has_option(args: &[OsString], long: &str, short: &str) -> bool {
  args.iter().any(|arg| {
    let value = arg.to_string_lossy();
    value == long || value == short || value.starts_with(&format!("{long}="))
  })
}

fn option_value(args: &[OsString], long: &str, short: &str) -> Option<String> {
  let mut index = 0;
  while index < args.len() {
    let value = args[index].to_string_lossy();
    if let Some(value) = value.strip_prefix(&format!("{long}=")) {
      return Some(value.to_owned());
    }
    if value == long || value == short {
      return args
        .get(index + 1)
        .map(|value| value.to_string_lossy().into_owned());
    }
    index += 1;
  }
  None
}

fn scaffold(project_root: &Path) -> Result<()> {
  if !project_root.join("src-vitul/src/lib.rs").is_file() {
    bail!(
      "create-vitul-app did not create the Vitul desktop scaffold at {}",
      project_root.display()
    );
  }
  write_file(project_root, "backend/Cargo.toml", BACKEND_CARGO)?;
  write_file(project_root, "backend/src/lib.rs", BACKEND_LIB)?;
  write_file(project_root, "backend/src/main.rs", BACKEND_MAIN)?;
  write_file(project_root, "index.html", FRONTEND_INDEX)?;
  write_file(project_root, "src/opentool.ts", FRONTEND_OPENTOOL)?;
  write_file(project_root, "src/main.ts", FRONTEND_MAIN)?;
  write_file(project_root, "src/host.ts", FRONTEND_HOST)?;
  write_file(project_root, "src/desktop-host.ts", DESKTOP_HOST)?;
  write_file(project_root, "src-vitul/src/lib.rs", VITUL_LIB)?;
  write_file(project_root, "vite.config.ts", VITE_CONFIG)?;
  write_file(project_root, "vitul.conf.json", VITUL_CONFIG)?;
  write_file(project_root, "README.md", PROJECT_README)?;
  write_file(project_root, "README-zh_CN.md", PROJECT_README_ZH_CN)?;
  write_file(project_root, "VITUL.md", PROJECT_README)?;
  write_file(project_root, "VITUL-zh_CN.md", PROJECT_README_ZH_CN)?;
  remove_unused_frontend_assets(project_root)?;
  configure_frontend_dependencies(project_root)?;
  configure_desktop_plugins(project_root)?;
  configure_capabilities(project_root)?;

  let mut vitul_config_path = PathBuf::from(project_root);
  vitul_config_path.push("src-vitul/vitul.conf.json");
  let mut config: serde_json::Value = serde_json::from_slice(
    &fs::read(&vitul_config_path).context("failed to read generated desktop runtime config")?,
  )
  .context("failed to parse generated desktop runtime config")?;
  config["$schema"] = serde_json::Value::String("https://schema.vitul.app/config/2.12.0".into());
  if let Some(title) = config
    .get_mut("app")
    .and_then(|app| app.get_mut("windows"))
    .and_then(serde_json::Value::as_array_mut)
    .and_then(|windows| windows.first_mut())
    .and_then(|window| window.get_mut("title"))
  {
    *title = serde_json::Value::String("Vitul OpenTool App".into());
  }
  fs::write(
    vitul_config_path,
    serde_json::to_vec_pretty(&config).context("failed to encode desktop runtime config")?,
  )
  .context("failed to update generated desktop runtime config")?;

  println!(
    "Created OpenTool-first Vitul project at {}",
    project_root.display()
  );
  Ok(())
}

fn remove_unused_frontend_assets(project_root: &Path) -> Result<()> {
  for relative_path in ["src/assets/vitul.svg", "src/assets/vite.svg"] {
    match fs::remove_file(project_root.join(relative_path)) {
      Ok(()) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(error) => {
        return Err(error).with_context(|| format!("failed to remove {relative_path}"));
      }
    }
  }
  Ok(())
}

fn configure_frontend_dependencies(project_root: &Path) -> Result<()> {
  let path = project_root.join("package.json");
  let mut package: serde_json::Value = serde_json::from_slice(
    &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
  )
  .context("failed to parse generated package.json")?;
  let dependencies = package
    .get_mut("dependencies")
    .and_then(serde_json::Value::as_object_mut)
    .context("generated package.json has no dependencies object")?;
  for dependency in [
    "@tauri-apps/plugin-clipboard-manager",
    "@tauri-apps/plugin-dialog",
    "@tauri-apps/plugin-notification",
    "@tauri-apps/plugin-shell",
    "agentos-sdk-ts",
  ] {
    dependencies.insert(
      dependency.to_owned(),
      serde_json::Value::String("^2".into()),
    );
  }
  let dev_dependencies = package
    .get_mut("devDependencies")
    .and_then(serde_json::Value::as_object_mut)
    .context("generated package.json has no devDependencies object")?;
  dev_dependencies.remove("@tauri-apps/cli");
  dev_dependencies.remove("@vitul/cli");
  if let Some(scripts) = package
    .get_mut("scripts")
    .and_then(serde_json::Value::as_object_mut)
  {
    scripts.remove("tauri");
    scripts.insert(
      "vitul".to_owned(),
      serde_json::Value::String("vitul".into()),
    );
  }
  fs::write(
    path,
    serde_json::to_vec_pretty(&package).context("failed to encode package.json")?,
  )
  .context("failed to update generated frontend dependencies")
}

#[cfg(test)]
mod tests {
  use super::configure_frontend_dependencies;
  use serde_json::json;
  use std::fs;

  #[test]
  fn generated_project_uses_installed_rust_cli_without_unpublished_npm_wrapper() {
    let project = tempfile::tempdir().unwrap();
    fs::write(
      project.path().join("package.json"),
      serde_json::to_vec(&json!({
        "dependencies": {},
        "devDependencies": {
          "@tauri-apps/cli": "^2",
          "@vitul/cli": "^2",
          "vite": "^8"
        },
        "scripts": {
          "tauri": "tauri"
        }
      }))
      .unwrap(),
    )
    .unwrap();

    configure_frontend_dependencies(project.path()).unwrap();

    let package: serde_json::Value =
      serde_json::from_slice(&fs::read(project.path().join("package.json")).unwrap()).unwrap();
    assert!(package["devDependencies"].get("@tauri-apps/cli").is_none());
    assert!(package["devDependencies"].get("@vitul/cli").is_none());
    assert_eq!(package["scripts"]["vitul"], "vitul");
    assert!(package["scripts"].get("tauri").is_none());
  }
}

fn configure_desktop_plugins(project_root: &Path) -> Result<()> {
  let path = project_root.join("src-vitul/Cargo.toml");
  let mut cargo_toml =
    fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
  let opener = "tauri-plugin-opener = \"2\"";
  let plugin_dependencies = [
    "tauri-plugin-clipboard-manager = \"2\"",
    "tauri-plugin-dialog = \"2\"",
    "tauri-plugin-notification = \"2\"",
    "tauri-plugin-shell = \"2\"",
  ];
  if !cargo_toml.contains(opener) {
    bail!(
      "Vitul desktop scaffold is missing the `tauri-plugin-opener` dependency in src-vitul/Cargo.toml"
    );
  }
  for dependency in plugin_dependencies {
    if !cargo_toml.contains(dependency) {
      cargo_toml = cargo_toml.replace(opener, &format!("{opener}\n{dependency}"));
    }
  }
  let mobile_dependencies = "[target.'cfg(any(target_os = \"android\", target_os = \"ios\"))'.dependencies]\n\
vitul-backend = { package = \"vitul-backend\", path = \"../backend\" }\n\
axum = \"0.7\"\n\
futures-util = \"0.3\"\n\
tokio = { version = \"1\", features = [\"sync\"] }\n\
tower = { version = \"0.5\", features = [\"util\"] }\n\n";
  if !cargo_toml
    .contains("[target.'cfg(any(target_os = \"android\", target_os = \"ios\"))'.dependencies]")
  {
    if let Some(patch_section) = cargo_toml.find("[patch.crates-io]") {
      cargo_toml.insert_str(patch_section, mobile_dependencies);
    } else {
      cargo_toml.push_str("\n");
      cargo_toml.push_str(mobile_dependencies);
    }
  }
  fs::write(&path, cargo_toml).with_context(|| format!("failed to update {}", path.display()))
}

fn configure_capabilities(project_root: &Path) -> Result<()> {
  let path = project_root.join("src-vitul/capabilities/default.json");
  let mut capability: serde_json::Value = serde_json::from_slice(
    &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
  )
  .context("failed to parse generated Vitul host capability")?;
  let permissions = capability
    .get_mut("permissions")
    .and_then(serde_json::Value::as_array_mut)
    .context("generated Vitul host capability has no permissions array")?;
  for permission in [
    serde_json::json!("clipboard-manager:allow-read-text"),
    serde_json::json!("clipboard-manager:allow-write-text"),
    serde_json::json!("dialog:default"),
    serde_json::json!("notification:default"),
    serde_json::json!("shell:allow-kill"),
    serde_json::json!({
      "identifier": "shell:allow-spawn",
      "allow": [{ "name": "binaries/*", "sidecar": true, "args": true }]
    }),
  ] {
    if !permissions.contains(&permission) {
      permissions.push(permission);
    }
  }
  fs::write(
    path,
    serde_json::to_vec_pretty(&capability).context("failed to encode Vitul capability")?,
  )
  .context("failed to configure Vitul desktop host API permissions")
}

fn write_file(project_root: &Path, relative_path: &str, content: &str) -> Result<()> {
  let path = project_root.join(relative_path);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
  }
  fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))
}
