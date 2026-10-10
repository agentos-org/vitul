use crate::vitulfile::{Config, VitulfileAppType, agentos_icon_filename};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
  fs,
  path::{Component, Path, PathBuf},
  process::Command,
};

pub fn build(project_root: &Path, requested_target: &str) -> Result<()> {
  let project_root = project_root.canonicalize().with_context(|| {
    format!(
      "could not resolve project directory {}",
      project_root.display()
    )
  })?;
  let config = Config::load(&project_root, None)?;
  let declared_targets = config.declared_targets();
  let selected_targets = if requested_target == "agentos" {
    declared_targets
  } else {
    if !declared_targets.contains(&requested_target) {
      bail!(
        "target `{requested_target}` is not declared in Vitulfile; available targets: {}",
        config.target_summary()
      );
    }
    vec![requested_target]
  };

  // URL apps contain no platform-specific process and always produce one `any` package.
  let package_targets = if config.app.app_type == VitulfileAppType::RemoteWeb {
    vec!["any"]
  } else {
    selected_targets
  };
  let archive_staging =
    tempfile::tempdir().context("could not create AgentOS archive staging directory")?;
  let mut archive_names = Vec::new();
  for target in package_targets {
    let architecture = if target == "any" {
      target
    } else {
      target.strip_prefix("agentos/").unwrap_or(target)
    };
    println!(
      "Building {} for AgentOS architecture {architecture}",
      config.app.name
    );
    if config.app.app_type == VitulfileAppType::Process {
      run_build_steps(&project_root, &config, target)?;
      build_project_backend(&project_root, architecture)?;
    }

    let archive_name = format!(
      "{}-{}-agentos-{architecture}.agt",
      archive_name(&config.app.name),
      config.app.version
    );
    let archive_path = archive_staging.path().join(&archive_name);
    let staging =
      tempfile::tempdir().context("could not create AgentOS package staging directory")?;
    stage_package(&project_root, &config, staging.path(), architecture)?;
    let mut command = Command::new("tar");
    command
      .args(["-czf"])
      .arg(&archive_path)
      .args(["-C"])
      .arg(staging.path())
      .arg(".");
    let status = command
      .status()
      .context("failed to start `tar` while creating the AgentOS package")?;
    if !status.success() {
      bail!("creating the AgentOS .agt archive failed with {status}");
    }
    archive_names.push(archive_name);
  }

  let output_dir = project_root.join("dist");
  fs::create_dir_all(&output_dir)
    .with_context(|| format!("could not create {}", output_dir.display()))?;
  for archive_name in archive_names {
    let staged_archive = archive_staging.path().join(&archive_name);
    let archive_path = output_dir.join(&archive_name);
    fs::copy(&staged_archive, &archive_path).with_context(|| {
      format!(
        "could not copy AgentOS package from {} to {}",
        staged_archive.display(),
        archive_path.display()
      )
    })?;
    println!("AgentOS package created: {}", archive_path.display());
  }
  Ok(())
}

fn build_project_backend(project_root: &Path, architecture: &str) -> Result<()> {
  if !crate::project_config::is_unified(project_root) {
    return Ok(());
  }
  let project = crate::project_config::root_config(project_root)?;
  let Some(backend) = project.get("backend").and_then(Value::as_object) else {
    return Ok(());
  };
  if backend.get("type").and_then(Value::as_str) != Some("opentool") {
    bail!("backend.type must be `opentool` when AgentOS process packaging uses the Rust backend");
  }
  let manifest = backend
    .get("manifest")
    .and_then(Value::as_str)
    .context("backend.manifest is required")?;
  let manifest_path = Path::new(manifest);
  if manifest_path.is_absolute()
    || manifest_path
      .components()
      .any(|component| !matches!(component, Component::Normal(_)))
  {
    bail!("backend.manifest must be a relative project path");
  }
  let binary = backend
    .get("binary")
    .and_then(Value::as_str)
    .filter(|binary| !binary.is_empty())
    .context("backend.binary is required")?;
  if binary.contains('/') || binary.contains('\\') || binary.contains('\0') {
    bail!("backend.binary must be a Cargo binary target name");
  }
  let (platform, rust_target) = match architecture {
    "arm64" => ("linux/arm64", "aarch64-unknown-linux-gnu"),
    "x64" => ("linux/amd64", "x86_64-unknown-linux-gnu"),
    _ => bail!("unsupported AgentOS architecture `{architecture}`"),
  };
  let image = project
    .get("agentos")
    .and_then(|agentos| agentos.get("buildImage"))
    .and_then(Value::as_str)
    .filter(|image| !image.trim().is_empty())
    .unwrap_or("rust:1.90-bookworm");
  let mount = format!("{}:/workspace", project_root.display());
  let manifest_relative = manifest_path
    .components()
    .map(|component| component.as_os_str().to_string_lossy())
    .collect::<Vec<_>>()
    .join("/");
  let container_manifest = format!("/workspace/{manifest_relative}");
  let target_dir = project_root.join("agentos-build/cargo-target");
  fs::create_dir_all(&target_dir)
    .with_context(|| format!("could not create {}", target_dir.display()))?;
  let mut command = Command::new("docker");
  command
    .args([
      "run",
      "--rm",
      "--platform",
      platform,
      "-v",
      &mount,
      "-w",
      "/workspace",
    ])
    .args([
      "--env",
      "CARGO_TARGET_DIR=/workspace/agentos-build/cargo-target",
    ])
    .arg(image)
    .args([
      "cargo",
      "build",
      "--release",
      "--target",
      rust_target,
      "--manifest-path",
    ])
    .arg(&container_manifest)
    .args(["--bin", binary]);
  let status = command
    .status()
    .context("failed to start Docker while building the AgentOS Rust backend")?;
  if !status.success() {
    bail!("building the AgentOS Rust backend in Docker failed with {status}");
  }

  let built_binary = target_dir.join(rust_target).join("release").join(binary);
  if !built_binary.is_file() {
    bail!(
      "AgentOS backend binary was not produced at {}",
      built_binary.display()
    );
  }
  let staged_binary = project_root.join("agentos-build/bin").join(binary);
  if let Some(parent) = staged_binary.parent() {
    fs::create_dir_all(parent).with_context(|| format!("could not create {}", parent.display()))?;
  }
  fs::copy(&built_binary, &staged_binary).with_context(|| {
    format!(
      "could not copy AgentOS backend from {} to {}",
      built_binary.display(),
      staged_binary.display()
    )
  })?;
  Ok(())
}

fn run_build_steps(project_root: &Path, config: &Config, target: &str) -> Result<()> {
  let architecture = target
    .strip_prefix("agentos/")
    .context("process build target must use the `agentos/<architecture>` format")?;
  for (index, step) in config.build.steps.iter().enumerate() {
    let cwd = step
      .cwd
      .as_ref()
      .map(|path| project_root.join(path))
      .unwrap_or_else(|| project_root.to_path_buf());
    let mut command = Command::new(&step.command);
    command
      .args(&step.args)
      .current_dir(&cwd)
      .envs(&step.env)
      .env("VITUL_TARGET", target)
      .env("VITUL_TARGET_OS", "linux")
      .env("VITUL_TARGET_ARCH", architecture)
      .env("AGENTOS_TARGET", target);
    println!("Running build step {}: {command:?}", index + 1);
    let status = command.status().with_context(|| {
      format!(
        "failed to start build step {} ({})",
        index + 1,
        step.command
      )
    })?;
    if !status.success() {
      bail!("build step {} failed with {status}", index + 1);
    }
  }
  Ok(())
}

fn stage_package(
  project_root: &Path,
  config: &Config,
  staging_root: &Path,
  architecture: &str,
) -> Result<()> {
  if config.app.app_type == VitulfileAppType::Process {
    for (index, artifact) in config.build.artifacts.iter().enumerate() {
      let source = project_root.join(&artifact.from);
      if !source.exists() {
        bail!(
          "build artifact {} is missing: {}",
          index + 1,
          source.display()
        );
      }
      if fs::symlink_metadata(&source)
        .with_context(|| format!("could not inspect artifact {}", source.display()))?
        .file_type()
        .is_symlink()
      {
        bail!(
          "artifact roots cannot be symbolic links: {}",
          source.display()
        );
      }
      let destination = staging_root.join(&artifact.to);
      if destination.exists() {
        bail!("multiple build artifacts target {}", artifact.to.display());
      }
      let source_root = source
        .canonicalize()
        .with_context(|| format!("could not resolve artifact {}", source.display()))?;
      copy_path(&source, &destination, &source_root)
        .with_context(|| format!("could not package artifact {}", artifact.from.display()))?;
    }

    let command = config
      .run
      .command
      .as_deref()
      .context("run.command is required for process apps")?;
    if is_package_path(command) {
      let command_path = safe_relative(command)?;
      let command_path = staging_root.join(command_path);
      if !command_path.is_file() {
        bail!(
          "run.command `{command}` is a package path, but no build artifact creates it; add it under build.artifacts"
        );
      }
      set_executable(&command_path)?;
    }
    let launcher = staging_root.join("vitul/launch.sh");
    if let Some(parent) = launcher.parent() {
      fs::create_dir_all(parent).context("could not create Vitul package runtime directory")?;
    }
    fs::write(&launcher, launch_script(command, &config.run.args))
      .context("could not write AgentOS process launcher")?;
    set_executable(&launcher)?;
  }

  if let Some(icon) = &config.app.icon {
    let filename =
      agentos_icon_filename(icon).context("app.icon must point to a PNG or SVG file")?;
    fs::copy(project_root.join(icon), staging_root.join(filename))
      .with_context(|| format!("could not package app icon {}", icon.display()))?;
  }

  let mut manifest = manifest(config, architecture)?;
  let sidecars = collect_sidecars(project_root, staging_root, architecture)?;
  if !sidecars.is_empty() {
    if !config
      .agentos
      .permissions
      .iter()
      .any(|permission| permission == "native.sidecars")
    {
      bail!(
        "bundle.externalBin declares AgentOS sidecars; add `native.sidecars` to `agentos.permissions` in vitul.conf.json"
      );
    }
    manifest["sidecars"] = json!(sidecars);
  }
  let manifest_bytes =
    serde_json::to_vec_pretty(&manifest).context("could not encode AgentOS app.json")?;
  fs::write(staging_root.join("app.json"), manifest_bytes)
    .context("could not write AgentOS app.json")?;
  Ok(())
}

fn collect_sidecars(
  project_root: &Path,
  staging_root: &Path,
  architecture: &str,
) -> Result<Vec<Value>> {
  if !crate::project_config::is_unified(project_root) {
    return Ok(Vec::new());
  }
  let project = crate::project_config::root_config(project_root)?;
  let Some(external_bins) = project
    .get("bundle")
    .and_then(|bundle| bundle.get("externalBin"))
  else {
    return Ok(Vec::new());
  };
  let external_bins = external_bins
    .as_array()
    .context("bundle.externalBin must be an array of sidecar base paths")?;
  let rust_target = match architecture {
    "arm64" => "aarch64-unknown-linux-gnu",
    "x64" => "x86_64-unknown-linux-gnu",
    "any" => return Ok(Vec::new()),
    _ => bail!("unsupported AgentOS architecture `{architecture}`"),
  };
  let mut names = std::collections::BTreeSet::new();
  let mut entries = Vec::with_capacity(external_bins.len());
  for external_bin in external_bins {
    let base = external_bin
      .as_str()
      .context("bundle.externalBin entries must be strings")?;
    let base_path = Path::new(base);
    if base_path.is_absolute()
      || base_path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
      bail!("bundle.externalBin entries must be safe project-relative paths");
    }
    let name = base_path
      .file_name()
      .and_then(|name| name.to_str())
      .context("bundle.externalBin path has no valid filename")?
      .to_owned();
    if !valid_sidecar_name(&name) {
      bail!("AgentOS sidecar name `{name}` must match `^[a-z][a-z0-9-]{{0,31}}$`");
    }
    if !names.insert(name.clone()) {
      bail!("bundle.externalBin contains duplicate AgentOS sidecar name `{name}`");
    }
    let mut source_name = name.clone();
    source_name.push('-');
    source_name.push_str(rust_target);
    let source = base_path.with_file_name(source_name);
    let source = project_root.join(source);
    if !source.is_file() {
      bail!(
        "AgentOS {architecture} sidecar is missing: {} (bundle.externalBin entries require the target suffix `-{rust_target}`)",
        source.display()
      );
    }
    let relative_entry = format!("sidecars/{name}");
    let destination = staging_root.join(&relative_entry);
    if let Some(parent) = destination.parent() {
      fs::create_dir_all(parent).context("could not create AgentOS sidecar directory")?;
    }
    fs::copy(&source, &destination)
      .with_context(|| format!("could not copy AgentOS sidecar from {}", source.display()))?;
    entries.push(json!({ "name": name, "executable": relative_entry }));
  }
  Ok(entries)
}

fn valid_sidecar_name(name: &str) -> bool {
  let bytes = name.as_bytes();
  (1..=32).contains(&bytes.len())
    && bytes[0].is_ascii_lowercase()
    && bytes[1..]
      .iter()
      .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn manifest(config: &Config, architecture: &str) -> Result<Value> {
  let is_remote = config.app.app_type == VitulfileAppType::RemoteWeb;
  let mut manifest = if is_remote {
    let url = config
      .web
      .url
      .as_deref()
      .context("web.url is required for remote-web apps")?;
    json!({
      "manifestVersion": 1,
      "appType": "url",
      "architecture": "any",
      "bundleId": config.app.id,
      "name": config.app.name,
      "version": config.app.version,
      "open": {
        "mode": config.agentos.open_mode.as_str(),
        "url": url,
        "titleBar": config.agentos.title_bar,
        "defaultWidth": config.agentos.default_width,
        "defaultHeight": config.agentos.default_height,
        "minWidth": config.agentos.min_width,
        "minHeight": config.agentos.min_height
      }
    })
  } else {
    json!({
      "manifestVersion": 1,
      "appType": "process",
      "architecture": architecture,
      "bundleId": config.app.id,
      "name": config.app.name,
      "version": config.app.version,
      "permissions": config.agentos.permissions,
      "run": {
        "type": "process",
        "entry": "vitul/launch.sh",
        "port": config.run.port,
        "healthCheck": config.run.health_check,
        "readyTimeout": config.run.ready_timeout,
        "env": config.run.env,
        "autoStart": false
      },
      "frontend": { "entry": config.run.frontend_entry },
      "open": {
        "mode": config.agentos.open_mode.as_str(),
        "titleBar": config.agentos.title_bar,
        "defaultWidth": config.agentos.default_width,
        "defaultHeight": config.agentos.default_height,
        "minWidth": config.agentos.min_width,
        "minHeight": config.agentos.min_height
      }
    })
  };
  let object = manifest.as_object_mut().expect("manifest is a JSON object");
  if !is_remote {
    let mut rpc = json!({
      "protocol": "opentool",
      "version": 2,
      "basePath": config.agentos.rpc_base_path
    });
    if config.agentos.rpc_discovery == "runtime" {
      rpc["discovery"] = json!("runtime");
    }
    object.insert("rpc".to_owned(), rpc);
  }
  if let Some(name_zh) = &config.app.name_zh {
    object.insert("name_zh".to_owned(), Value::String(name_zh.clone()));
  }
  if let Some(description) = &config.app.description {
    object.insert("description".to_owned(), Value::String(description.clone()));
  }
  if let Some(description_zh) = &config.app.description_zh {
    object.insert(
      "description_zh".to_owned(),
      Value::String(description_zh.clone()),
    );
  }
  if !is_remote && config.data.persistent {
    object.insert(
      "data".to_owned(),
      json!({ "persistent": true, "deleteOnUninstall": false }),
    );
  }
  Ok(manifest)
}

fn launch_script(command: &str, args: &[String]) -> String {
  let mut script = String::from("#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")/..\"\nexec ");
  script.push_str(&shell_quote(command));
  for arg in args {
    script.push(' ');
    script.push_str(&shell_quote(arg));
  }
  script.push('\n');
  script
}

fn shell_quote(value: &str) -> String {
  format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn is_package_path(command: &str) -> bool {
  command.starts_with("./") || command.starts_with("../") || command.contains('/')
}

fn safe_relative(value: &str) -> Result<PathBuf> {
  let path = Path::new(value.strip_prefix("./").unwrap_or(value));
  if path.is_absolute()
    || path.as_os_str().is_empty()
    || path
      .components()
      .any(|component| !matches!(component, Component::Normal(_)))
  {
    bail!("package path must be relative and must not contain `.` or `..` segments: {value}");
  }
  Ok(path.to_path_buf())
}

fn copy_path(source: &Path, destination: &Path, source_root: &Path) -> Result<()> {
  if source
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(|extension| extension.eq_ignore_ascii_case("agt"))
  {
    return Ok(());
  }
  let metadata = fs::symlink_metadata(source)
    .with_context(|| format!("could not inspect {}", source.display()))?;
  if metadata.file_type().is_symlink() {
    let target = fs::read_link(source)
      .with_context(|| format!("could not read symbolic link {}", source.display()))?;
    if target.is_absolute() {
      bail!(
        "absolute symbolic links are not supported in package artifacts: {}",
        source.display()
      );
    }
    let resolved_target = source
      .parent()
      .unwrap_or(source_root)
      .join(&target)
      .canonicalize()
      .with_context(|| format!("symbolic link target does not exist: {}", source.display()))?;
    if !resolved_target.starts_with(source_root) {
      bail!(
        "symbolic link escapes its build artifact: {}",
        source.display()
      );
    }
    if let Some(parent) = destination.parent() {
      fs::create_dir_all(parent)
        .with_context(|| format!("could not create {}", parent.display()))?;
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, destination)
      .with_context(|| format!("could not package symbolic link {}", source.display()))?;
    #[cfg(not(unix))]
    bail!("symbolic-link package artifacts require a Unix host");
    return Ok(());
  }
  if metadata.is_dir() {
    fs::create_dir_all(destination)
      .with_context(|| format!("could not create {}", destination.display()))?;
    for entry in
      fs::read_dir(source).with_context(|| format!("could not read {}", source.display()))?
    {
      let entry = entry.context("could not read package artifact entry")?;
      copy_path(
        &entry.path(),
        &destination.join(entry.file_name()),
        source_root,
      )?;
    }
  } else if metadata.is_file() {
    if let Some(parent) = destination.parent() {
      fs::create_dir_all(parent)
        .with_context(|| format!("could not create {}", parent.display()))?;
    }
    fs::copy(source, destination)
      .with_context(|| format!("could not copy {}", source.display()))?;
  } else {
    bail!("unsupported package artifact type: {}", source.display());
  }
  Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
  use std::os::unix::fs::PermissionsExt;
  let mut permissions = fs::metadata(path)
    .with_context(|| format!("could not inspect {}", path.display()))?
    .permissions();
  permissions.set_mode(0o755);
  fs::set_permissions(path, permissions)
    .with_context(|| format!("could not mark {} executable", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
  bail!("AgentOS ARM64 packaging requires a Unix host to create an executable launcher")
}

fn archive_name(name: &str) -> String {
  let mut result = String::new();
  for character in name.chars().flat_map(char::to_lowercase) {
    if character.is_ascii_alphanumeric() {
      result.push(character);
    } else if !result.is_empty() && !result.ends_with('-') {
      result.push('-');
    }
  }
  result.truncate(result.trim_end_matches('-').len());
  if result.is_empty() {
    "agentos-app".to_owned()
  } else {
    result
  }
}
