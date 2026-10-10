use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
  collections::HashSet,
  env, fs,
  path::{Path, PathBuf},
  process::{Command, Output},
};

const RUST_TARGET: &str = "aarch64-unknown-linux-gnu";
const ARCHITECTURE: &str = "arm64";
const DEFAULT_BUILD_IMAGE: &str = "rust:1.90-bookworm";
const DEFAULT_AGENTOS_PORT: u16 = 9627;
const BACKEND_BINARY: &str = "vitul-backend";
const DEFAULT_AGENTOS_PERMISSIONS: &[&str] = &[
  "native.window-control",
  "native.file-dialogs",
  "native.clipboard",
  "native.notifications",
  "native.external-links",
];
const SUPPORTED_AGENTOS_PERMISSIONS: &[&str] = &[
  "native.window-control",
  "native.file-dialogs",
  "native.clipboard",
  "native.notifications",
  "native.external-links",
  "native.sidecars",
];

pub fn build_arm64(project_root: &Path) -> Result<()> {
  let project_root = project_root
    .canonicalize()
    .context("failed to resolve project directory")?;
  let package_json = project_root.join("package.json");
  let backend_manifest = project_root.join("backend/Cargo.toml");
  let vitul_config = project_root.join("src-vitul/vitul.conf.json");
  for required in [&package_json, &backend_manifest, &vitul_config] {
    if !required.is_file() {
      bail!(
        "{} is missing; run `vitul init` to create a Vitul project first",
        required.display()
      );
    }
  }

  build_frontend(&project_root)?;
  let build_image = build_image(&project_root)?;
  build_backend(&project_root, &build_image)?;
  package_app(&project_root, &package_json, &vitul_config)?;
  Ok(())
}

fn build_frontend(project_root: &Path) -> Result<()> {
  let (program, args) = if project_root.join("pnpm-lock.yaml").is_file() {
    ("pnpm", vec!["run", "build"])
  } else if project_root.join("yarn.lock").is_file() {
    ("yarn", vec!["build"])
  } else if project_root.join("bun.lock").is_file() || project_root.join("bun.lockb").is_file() {
    ("bun", vec!["run", "build"])
  } else {
    ("npm", vec!["run", "build"])
  };
  run_command(
    Command::new(program).args(args).current_dir(project_root),
    "building the frontend",
  )?;
  Ok(())
}

fn build_image(project_root: &Path) -> Result<String> {
  if let Ok(image) = env::var("VITUL_AGENTOS_BUILD_IMAGE") {
    return Ok(image);
  }
  let config_path = project_root.join("vitul.conf.json");
  if !config_path.is_file() {
    return Ok(DEFAULT_BUILD_IMAGE.into());
  }
  let config: Value =
    serde_json::from_slice(&fs::read(&config_path).context("failed to read vitul.conf.json")?)
      .context("failed to parse vitul.conf.json")?;
  Ok(
    config
      .get("agentos")
      .and_then(|agentos| agentos.get("buildImage"))
      .and_then(Value::as_str)
      .filter(|value| !value.trim().is_empty())
      .unwrap_or(DEFAULT_BUILD_IMAGE)
      .to_owned(),
  )
}

fn build_backend(project_root: &Path, image: &str) -> Result<()> {
  let mount = format!("{}:/workspace", project_root.display());
  let mut command = Command::new("docker");
  command
    .args([
      "run",
      "--rm",
      "--platform",
      "linux/arm64",
      "-v",
      &mount,
      "-w",
      "/workspace/backend",
      image,
      "sh",
      // Keep the Rust image's PATH intact; a login shell resets it and hides
      // cargo under /usr/local/cargo/bin.
      "-c",
      &format!("cargo build --release --target {RUST_TARGET}"),
    ])
    .current_dir(project_root);
  run_command(&mut command, "building the AgentOS ARM64 backend in Docker")?;
  Ok(())
}

fn package_app(project_root: &Path, package_json: &Path, vitul_config: &Path) -> Result<()> {
  let package: Value =
    serde_json::from_slice(&fs::read(package_json).context("failed to read package.json")?)
      .context("failed to parse package.json")?;
  let config: Value =
    serde_json::from_slice(&fs::read(vitul_config).context("failed to read Vitul runtime config")?)
      .context("failed to parse Vitul runtime config")?;
  let agentos_settings = agentos_settings(project_root)?;
  let bundle_id = match agentos_bundle_id_override(&agentos_settings)? {
    Some(bundle_id) => bundle_id,
    None => agentos_bundle_id(&config)?,
  };
  let permissions = agentos_permissions(project_root)?;
  let open_mode = agentos_open_mode(project_root)?;
  let title_bar = agentos_title_bar(project_root)?;
  let rpc_discovery = agentos_rpc_discovery(project_root)?;
  let port = agentos_port(project_root)?;
  let package_name = package
    .get("name")
    .and_then(Value::as_str)
    .context("package.json must declare a name")?;
  let display_name = config
    .get("productName")
    .and_then(Value::as_str)
    .unwrap_or(package_name);
  let name_zh =
    agentos_setting_string(&agentos_settings, "nameZh")?.unwrap_or_else(|| display_name.to_owned());
  let version = package
    .get("version")
    .and_then(Value::as_str)
    .unwrap_or("0.1.0");
  let description = agentos_setting_string(&agentos_settings, "description")?
    .unwrap_or_else(|| format!("{display_name} application"));
  let description_zh = agentos_setting_string(&agentos_settings, "descriptionZh")?;
  let archive_name = package_archive_name(package_name);

  let runtime_dir = vitul_config
    .parent()
    .context("Vitul runtime config path has no parent directory")?;
  let frontend_dist = config
    .get("build")
    .and_then(|build| build.get("frontendDist"))
    .and_then(Value::as_str)
    .context("AgentOS packaging currently requires build.frontendDist to be a path string")?;
  if frontend_dist.starts_with("http://") || frontend_dist.starts_with("https://") {
    bail!("build.frontendDist must point to built frontend files, not a URL");
  }
  let frontend_dir = runtime_dir.join(frontend_dist);
  if !frontend_dir.is_dir() {
    bail!(
      "frontend output directory does not exist: {}",
      frontend_dir.display()
    );
  }

  let backend_binary = project_root
    .join("backend/target")
    .join(RUST_TARGET)
    .join("release")
    .join(BACKEND_BINARY);
  if !backend_binary.is_file() {
    bail!("backend build did not produce {}", backend_binary.display());
  }

  let output_dir = project_root.join("dist");
  fs::create_dir_all(&output_dir).context("failed to create dist directory")?;
  let archive = output_dir.join(format!("{archive_name}-{version}-agentos-arm64.agt"));
  let staging = tempfile::tempdir().context("failed to create package staging directory")?;
  let staging_root = staging.path();
  let packaged_sidecars = collect_sidecars(runtime_dir, &config, staging_root)?;
  copy_agentos_icon(&config, runtime_dir, staging_root)?;
  if !packaged_sidecars.is_empty()
    && !permissions
      .iter()
      .any(|permission| permission == "native.sidecars")
  {
    bail!(
      "bundle.externalBin declares AgentOS sidecars; add `native.sidecars` to `agentos.permissions` in vitul.conf.json to request permission"
    );
  }

  let mut manifest = json!({
    "manifestVersion": 1,
    "appType": "process",
    "architecture": ARCHITECTURE,
    "bundleId": bundle_id,
    "name": display_name,
    "name_zh": name_zh,
    "version": version,
    "description": description,
    "permissions": permissions,
    "sidecars": packaged_sidecars,
    "run": {
      "type": "process",
      "entry": format!("backend/{BACKEND_BINARY}"),
      "port": port,
      "healthCheck": "/opentool/version",
      "readyTimeout": 60,
      "autoStart": false
    },
    "frontend": {
      "entry": "/"
    },
    "rpc": {
      "protocol": "opentool",
      "version": 2,
      "basePath": "/opentool"
    },
    "open": {
      "mode": open_mode,
      "titleBar": title_bar,
      "defaultWidth": 960,
      "defaultHeight": 680
    }
  });
  if let Some(description_zh) = description_zh {
    manifest["description_zh"] = json!(description_zh);
  }
  if rpc_discovery == "runtime" {
    manifest["rpc"]["discovery"] = json!("runtime");
  }
  fs::write(
    staging_root.join("app.json"),
    serde_json::to_vec_pretty(&manifest).context("failed to encode AgentOS app.json")?,
  )
  .context("failed to write AgentOS app.json")?;

  copy_directory_contents(&frontend_dir, &staging_root.join("web"))
    .with_context(|| format!("failed to copy frontend from {}", frontend_dir.display()))?;
  let packaged_backend = staging_root.join("backend").join(BACKEND_BINARY);
  fs::create_dir_all(packaged_backend.parent().expect("backend has a parent"))
    .context("failed to create backend package directory")?;
  fs::copy(&backend_binary, &packaged_backend).context("failed to copy backend binary")?;

  let mut tar = Command::new("tar");
  tar
    .args(["-czf"])
    .arg(&archive)
    .args(["-C"])
    .arg(staging_root)
    .arg(".");
  run_command(&mut tar, "creating the AgentOS .agt archive")?;

  println!("AgentOS ARM64 package created: {}", archive.display());
  Ok(())
}

fn copy_agentos_icon(config: &Value, runtime_dir: &Path, staging_root: &Path) -> Result<()> {
  let Some(icon_config) = config.get("bundle").and_then(|bundle| bundle.get("icon")) else {
    return Ok(());
  };
  let icon_paths = match icon_config {
    Value::String(path) => vec![path.clone()],
    Value::Array(paths) => paths
      .iter()
      .filter_map(Value::as_str)
      .map(str::to_owned)
      .collect(),
    Value::Null => return Ok(()),
    _ => bail!("bundle.icon must be a path or an array of paths"),
  };
  let selected_icon = icon_paths
    .iter()
    .find(|path| {
      Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("icon.png"))
    })
    .or_else(|| icon_paths.iter().find(|path| is_agentos_icon_path(path)));
  let Some(selected_icon) = selected_icon else {
    return Ok(());
  };

  let source = runtime_dir.join(selected_icon);
  if !source.is_file() {
    bail!(
      "configured AgentOS app icon is missing: {}",
      source.display()
    );
  }
  let extension = Path::new(selected_icon)
    .extension()
    .and_then(|extension| extension.to_str())
    .context("configured AgentOS app icon must have a supported extension")?;
  let destination = staging_root.join(format!("icon.{}", extension.to_ascii_lowercase()));
  fs::copy(&source, &destination).with_context(|| {
    format!(
      "failed to copy AgentOS app icon from {} to {}",
      source.display(),
      destination.display()
    )
  })?;
  Ok(())
}

fn is_agentos_icon_path(path: &str) -> bool {
  Path::new(path)
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(|extension| {
      ["png", "svg", "webp", "jpg"]
        .iter()
        .any(|supported| extension.eq_ignore_ascii_case(supported))
    })
}

fn agentos_open_mode(project_root: &Path) -> Result<String> {
  let config_path = project_root.join("vitul.conf.json");
  if !config_path.is_file() {
    return Ok("window".to_owned());
  }
  let config: Value =
    serde_json::from_slice(&fs::read(&config_path).context("failed to read vitul.conf.json")?)
      .context("failed to parse vitul.conf.json")?;
  let Some(open) = config
    .get("agentos")
    .and_then(|agentos| agentos.get("open"))
  else {
    return Ok("window".to_owned());
  };
  let open = open.as_object().context("agentos.open must be an object")?;
  let Some(mode) = open.get("mode") else {
    return Ok("window".to_owned());
  };
  match mode.as_str() {
    Some("window") => Ok("window".to_owned()),
    Some("workspace") => Ok("workspace".to_owned()),
    _ => bail!("agentos.open.mode must be `window` or `workspace`"),
  }
}

fn agentos_title_bar(project_root: &Path) -> Result<String> {
  let config_path = project_root.join("vitul.conf.json");
  if !config_path.is_file() {
    return Ok("system".to_owned());
  }
  let config: Value =
    serde_json::from_slice(&fs::read(&config_path).context("failed to read vitul.conf.json")?)
      .context("failed to parse vitul.conf.json")?;
  let Some(open) = config
    .get("agentos")
    .and_then(|agentos| agentos.get("open"))
  else {
    return Ok("system".to_owned());
  };
  let open = open.as_object().context("agentos.open must be an object")?;
  let Some(title_bar) = open.get("titleBar") else {
    return Ok("system".to_owned());
  };
  match title_bar.as_str() {
    Some("system") => Ok("system".to_owned()),
    Some("overlay") => Ok("overlay".to_owned()),
    _ => bail!("agentos.open.titleBar must be `system` or `overlay`"),
  }
}

fn agentos_rpc_discovery(project_root: &Path) -> Result<String> {
  let config_path = project_root.join("vitul.conf.json");
  if !config_path.is_file() {
    return Ok("static".to_owned());
  }
  let config: Value =
    serde_json::from_slice(&fs::read(&config_path).context("failed to read vitul.conf.json")?)
      .context("failed to parse vitul.conf.json")?;
  let Some(settings) = config.get("agentos") else {
    return Ok("static".to_owned());
  };
  let settings = settings.as_object().context("agentos must be an object")?;
  let Some(value) = settings.get("rpcDiscovery") else {
    return Ok("static".to_owned());
  };
  let discovery = value
    .as_str()
    .context("agentos.rpcDiscovery must be `static` or `runtime`")?;
  if !matches!(discovery, "static" | "runtime") {
    bail!("agentos.rpcDiscovery must be `static` or `runtime`");
  }
  if discovery == "runtime"
    && config
      .get("backend")
      .and_then(|backend| backend.get("type"))
      .and_then(Value::as_str)
      != Some("opentool")
  {
    bail!("agentos.rpcDiscovery runtime requires backend.type `opentool`");
  }
  Ok(discovery.to_owned())
}

fn agentos_port(project_root: &Path) -> Result<u16> {
  let config_path = project_root.join("vitul.conf.json");
  if !config_path.is_file() {
    return Ok(DEFAULT_AGENTOS_PORT);
  }
  let config: Value =
    serde_json::from_slice(&fs::read(&config_path).context("failed to read vitul.conf.json")?)
      .context("failed to parse vitul.conf.json")?;
  let Some(port) = config
    .get("agentos")
    .and_then(|agentos| agentos.get("port"))
  else {
    return Ok(DEFAULT_AGENTOS_PORT);
  };
  let port = port
    .as_u64()
    .and_then(|port| u16::try_from(port).ok())
    .filter(|port| *port > 0)
    .context("agentos.port must be an integer from 1 to 65535")?;
  Ok(port)
}

fn agentos_bundle_id(config: &Value) -> Result<String> {
  let identifier = config
    .get("identifier")
    .and_then(Value::as_str)
    .context("src-vitul/vitul.conf.json must declare a reverse-DNS identifier")?;
  let bundle_id = identifier.to_ascii_lowercase();
  if !is_valid_bundle_id(&bundle_id) {
    bail!(
      "src-vitul/vitul.conf.json identifier `{identifier}` must be a valid reverse-DNS AgentOS bundleId"
    );
  }
  Ok(bundle_id)
}

fn agentos_settings(project_root: &Path) -> Result<Value> {
  let config_path = project_root.join("vitul.conf.json");
  if !config_path.is_file() {
    return Ok(json!({}));
  }
  let config: Value =
    serde_json::from_slice(&fs::read(&config_path).context("failed to read Vitul runtime config")?)
      .context("failed to parse Vitul runtime config")?;
  Ok(config.get("agentos").cloned().unwrap_or_else(|| json!({})))
}

fn agentos_setting_string(settings: &Value, key: &str) -> Result<Option<String>> {
  let Some(value) = settings.get(key) else {
    return Ok(None);
  };
  let value = value
    .as_str()
    .filter(|value| !value.trim().is_empty())
    .with_context(|| format!("agentos.{key} must be a non-empty string"))?;
  Ok(Some(value.to_owned()))
}

fn agentos_bundle_id_override(settings: &Value) -> Result<Option<String>> {
  let Some(bundle_id) = agentos_setting_string(settings, "bundleId")? else {
    return Ok(None);
  };
  if !is_valid_bundle_id(&bundle_id) {
    bail!("agentos.bundleId must be a valid reverse-DNS AgentOS bundle ID");
  }
  Ok(Some(bundle_id))
}

fn is_valid_bundle_id(value: &str) -> bool {
  value.len() <= 253
    && value.is_ascii()
    && value.split('.').count() >= 2
    && value.split('.').all(|label| {
      let bytes = label.as_bytes();
      !bytes.is_empty()
        && bytes.len() <= 63
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
          .iter()
          .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
    })
}

fn agentos_permissions(project_root: &Path) -> Result<Vec<String>> {
  let config_path = project_root.join("vitul.conf.json");
  if !config_path.is_file() {
    return Ok(
      DEFAULT_AGENTOS_PERMISSIONS
        .iter()
        .map(|permission| (*permission).to_owned())
        .collect(),
    );
  }
  let config: Value =
    serde_json::from_slice(&fs::read(&config_path).context("failed to read vitul.conf.json")?)
      .context("failed to parse vitul.conf.json")?;
  let Some(permissions) = config
    .get("agentos")
    .and_then(|agentos| agentos.get("permissions"))
  else {
    return Ok(
      DEFAULT_AGENTOS_PERMISSIONS
        .iter()
        .map(|permission| (*permission).to_owned())
        .collect(),
    );
  };

  let permissions = permissions
    .as_array()
    .context("agentos.permissions must be an array of native permission names")?;
  let mut resolved = Vec::with_capacity(permissions.len());
  for permission in permissions {
    let permission = permission
      .as_str()
      .context("agentos.permissions entries must be strings")?;
    if !SUPPORTED_AGENTOS_PERMISSIONS.contains(&permission) {
      bail!("unsupported AgentOS permission: {permission}");
    }
    if !resolved.iter().any(|existing| existing == permission) {
      resolved.push(permission.to_owned());
    }
  }
  Ok(resolved)
}

fn collect_sidecars(
  runtime_dir: &Path,
  vitul_config: &Value,
  staging_root: &Path,
) -> Result<Vec<Value>> {
  let Some(external_bins) = vitul_config
    .get("bundle")
    .and_then(|bundle| bundle.get("externalBin"))
  else {
    return Ok(Vec::new());
  };
  let external_bins = external_bins
    .as_array()
    .context("bundle.externalBin must be an array of desktop sidecar base paths")?;
  let mut entries = Vec::with_capacity(external_bins.len());
  let mut names = HashSet::with_capacity(external_bins.len());
  for external_bin in external_bins {
    let base = external_bin
      .as_str()
      .context("bundle.externalBin entries must be strings")?;
    let sidecar_base = runtime_dir.join(base);
    let source = PathBuf::from(format!("{}-{RUST_TARGET}", sidecar_base.display()));
    if !source.is_file() {
      bail!(
        "AgentOS ARM64 sidecar is missing: {} (bundle.externalBin entries require the target suffix `-{RUST_TARGET}`)",
        source.display()
      );
    }
    let name = sidecar_base
      .file_name()
      .and_then(|name| name.to_str())
      .context("bundle.externalBin path has no valid filename")?
      .to_owned();
    if !is_agentos_sidecar_name(&name) {
      bail!(
        "AgentOS sidecar name `{name}` must match `^[a-z][a-z0-9-]{{0,31}}$` (use a lowercase basename without the target suffix)"
      );
    }
    if !names.insert(name.clone()) {
      bail!("bundle.externalBin contains duplicate AgentOS sidecar name `{name}`");
    }
    let relative_entry = format!("sidecars/{name}");
    let destination = staging_root.join(&relative_entry);
    if let Some(parent) = destination.parent() {
      fs::create_dir_all(parent).context("failed to create AgentOS sidecar directory")?;
    }
    fs::copy(&source, &destination)
      .with_context(|| format!("failed to copy AgentOS sidecar from {}", source.display()))?;
    entries.push(json!({ "name": name, "executable": relative_entry }));
  }
  Ok(entries)
}

fn is_agentos_sidecar_name(name: &str) -> bool {
  let bytes = name.as_bytes();
  (1..=32).contains(&bytes.len())
    && bytes[0].is_ascii_lowercase()
    && bytes[1..]
      .iter()
      .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn package_archive_name(package_name: &str) -> String {
  let mut result = String::new();
  for character in package_name.chars().flat_map(char::to_lowercase) {
    if character.is_ascii_lowercase() || character.is_ascii_digit() {
      result.push(character);
    } else if !result.is_empty() && !result.ends_with('-') {
      result.push('-');
    }
  }
  while result.ends_with('-') {
    result.pop();
  }
  if result.is_empty() {
    "vitul-app".into()
  } else {
    result
  }
}

fn copy_directory_contents(source: &Path, destination: &Path) -> Result<()> {
  fs::create_dir_all(destination)
    .with_context(|| format!("failed to create {}", destination.display()))?;
  for entry in
    fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
  {
    let entry = entry.context("failed to read frontend directory entry")?;
    let source_path = entry.path();
    let destination_path = destination.join(entry.file_name());
    let file_type = entry
      .file_type()
      .context("failed to inspect frontend output")?;
    if file_type.is_dir() {
      copy_directory_contents(&source_path, &destination_path)?;
    } else if file_type.is_file() {
      if source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("agt"))
      {
        continue;
      }
      fs::copy(&source_path, &destination_path)
        .with_context(|| format!("failed to copy {}", source_path.display()))?;
    } else if file_type.is_symlink() {
      let target = fs::read_link(&source_path)
        .with_context(|| format!("failed to read link {}", source_path.display()))?;
      #[cfg(unix)]
      std::os::unix::fs::symlink(target, &destination_path)
        .with_context(|| format!("failed to create link {}", destination_path.display()))?;
      #[cfg(not(unix))]
      bail!(
        "frontend output contains unsupported symbolic link {}",
        source_path.display()
      );
    }
  }
  Ok(())
}

fn run_command(command: &mut Command, description: &str) -> Result<Output> {
  let printable = format!("{command:?}");
  println!("Running {description}: {printable}");
  let output = command
    .output()
    .with_context(|| format!("failed to start {description}"))?;
  if !output.status.success() {
    if !output.stdout.is_empty() {
      print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
      eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
    bail!("{description} failed with {}", output.status);
  }
  if !output.stdout.is_empty() {
    print!("{}", String::from_utf8_lossy(&output.stdout));
  }
  if !output.stderr.is_empty() {
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
  }
  Ok(output)
}

#[cfg(test)]
mod tests {
  use super::{ARCHITECTURE, RUST_TARGET, agentos_bundle_id, package_app, package_archive_name};
  use serde_json::{Value, json};
  use std::{fs, process::Command};
  use tempfile::tempdir;

  #[test]
  fn agentos_bundle_id_uses_the_reverse_dns_runtime_identifier() {
    let config = json!({ "identifier": "Com.Example.Hello-Vitul" });
    assert_eq!(
      agentos_bundle_id(&config).expect("valid reverse-DNS identifier"),
      "com.example.hello-vitul"
    );
    assert_eq!(package_archive_name("Hello Vitul 2"), "hello-vitul-2");
    assert_eq!(package_archive_name("---"), "vitul-app");
  }

  #[test]
  fn packages_agentos_manifest_web_backend_and_external_sidecars() {
    let root = tempdir().expect("create project temp directory");
    let root = root.path();
    let frontend = root.join("dist");
    let backend = root
      .join("backend/target")
      .join(RUST_TARGET)
      .join("release");
    let vitul_root = root.join("src-vitul");
    let sidecar_dir = vitul_root.join("binaries");
    fs::create_dir_all(&frontend).expect("create frontend dist");
    fs::create_dir_all(&backend).expect("create backend output");
    fs::create_dir_all(&sidecar_dir).expect("create sidecar output");
    fs::write(frontend.join("index.html"), "<main>Vitul</main>").expect("write frontend");
    fs::write(backend.join("vitul-backend"), "backend ELF").expect("write backend binary");
    fs::write(
      sidecar_dir.join(format!("worker-{RUST_TARGET}")),
      "sidecar ELF",
    )
    .expect("write sidecar binary");
    let package_json = root.join("package.json");
    let vitul_config = vitul_root.join("vitul.conf.json");
    fs::write(
      &package_json,
      serde_json::to_vec(&json!({ "name": "Hello Vitul", "version": "1.2.3" }))
        .expect("encode package.json"),
    )
    .expect("write package.json");
    fs::write(
      &vitul_config,
      serde_json::to_vec(&json!({
        "productName": "Hello Vitul",
        "identifier": "com.example.hello-vitul",
        "build": { "frontendDist": "../dist" },
        "bundle": { "externalBin": ["binaries/worker"] }
      }))
      .expect("encode Vitul desktop config"),
    )
    .expect("write Vitul desktop config");
    fs::write(
      root.join("vitul.conf.json"),
      serde_json::to_vec(&json!({
        "agentos": {
          "port": 15237,
          "permissions": [
            "native.window-control",
            "native.file-dialogs",
            "native.clipboard",
            "native.notifications",
            "native.external-links",
            "native.sidecars"
          ]
        }
      }))
      .expect("encode Vitul project config"),
    )
    .expect("write Vitul project config");

    package_app(root, &package_json, &vitul_config).expect("package AgentOS app");

    let archive = root.join("dist/hello-vitul-1.2.3-agentos-arm64.agt");
    let listing = Command::new("tar")
      .args(["-tzf"])
      .arg(&archive)
      .output()
      .expect("list AgentOS package");
    assert!(listing.status.success());
    let listing = String::from_utf8_lossy(&listing.stdout);
    for expected in [
      "./app.json",
      "./web/index.html",
      "./backend/vitul-backend",
      "./sidecars/worker",
    ] {
      assert!(
        listing.lines().any(|line| line == expected),
        "missing {expected}"
      );
    }

    let manifest = Command::new("tar")
      .args(["-xOzf"])
      .arg(&archive)
      .arg("./app.json")
      .output()
      .expect("read packaged AgentOS app.json");
    assert!(manifest.status.success());
    let manifest: Value = serde_json::from_slice(&manifest.stdout).expect("parse app.json");
    assert_eq!(manifest["manifestVersion"], 1);
    assert_eq!(manifest["architecture"], ARCHITECTURE);
    assert_eq!(manifest["bundleId"], "com.example.hello-vitul");
    assert!(manifest.get("id").is_none());
    assert_eq!(manifest["run"]["port"], 15237);
    assert_eq!(manifest["run"]["autoStart"], false);
    assert_eq!(manifest["run"]["healthCheck"], "/opentool/version");
    assert_eq!(manifest["frontend"]["entry"], "/");
    assert_eq!(manifest["rpc"]["protocol"], "opentool");
    assert_eq!(manifest["rpc"]["version"], 2);
    assert_eq!(manifest["rpc"]["basePath"], "/opentool");
    assert_eq!(
      manifest["permissions"],
      json!([
        "native.window-control",
        "native.file-dialogs",
        "native.clipboard",
        "native.notifications",
        "native.external-links",
        "native.sidecars"
      ])
    );
    assert_eq!(manifest["sidecars"][0]["name"], "worker");
    assert_eq!(manifest["sidecars"][0]["executable"], "sidecars/worker");
  }
}
