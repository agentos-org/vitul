use crate::vitulfile::{Config, VitulfileAppType};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
  fs,
  path::{Component, Path, PathBuf},
  process::Command,
};

const ARCHITECTURE: &str = "arm64";

pub fn build(project_root: &Path) -> Result<()> {
  let project_root = project_root.canonicalize().with_context(|| {
    format!(
      "could not resolve project directory {}",
      project_root.display()
    )
  })?;
  let config = Config::load(&project_root, None)?;
  println!("Building {} for {}", config.app.name, config.target);
  let architecture = if config.app.app_type == VitulfileAppType::RemoteWeb {
    "any"
  } else {
    ARCHITECTURE
  };

  if config.app.app_type == VitulfileAppType::Process {
    run_build_steps(&project_root, &config)?;
  }

  let output_dir = project_root.join("dist");
  fs::create_dir_all(&output_dir)
    .with_context(|| format!("could not create {}", output_dir.display()))?;
  let archive_path = output_dir.join(format!(
    "{}-{}-agentos-{architecture}.agt",
    archive_name(&config.app.name),
    config.app.version
  ));
  let staging =
    tempfile::tempdir().context("could not create AgentOS package staging directory")?;
  stage_package(&project_root, &config, staging.path())?;
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
  println!(
    "AgentOS {architecture} package created: {}",
    archive_path.display()
  );
  Ok(())
}

fn run_build_steps(project_root: &Path, config: &Config) -> Result<()> {
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
      .env("VITUL_TARGET", "agentos/arm64")
      .env("VITUL_TARGET_OS", "linux")
      .env("VITUL_TARGET_ARCH", ARCHITECTURE)
      .env("AGENTOS_TARGET", "agentos/arm64");
    for (name, value) in &step.env {
      command.env(name, value);
    }
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

fn stage_package(project_root: &Path, config: &Config, staging_root: &Path) -> Result<()> {
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
    fs::copy(project_root.join(icon), staging_root.join("icon.png"))
      .with_context(|| format!("could not package app icon {}", icon.display()))?;
  }

  let manifest = manifest(config)?;
  let manifest_bytes =
    serde_json::to_vec_pretty(&manifest).context("could not encode AgentOS app.json")?;
  fs::write(staging_root.join("app.json"), manifest_bytes)
    .context("could not write AgentOS app.json")?;
  Ok(())
}

fn manifest(config: &Config) -> Result<Value> {
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
      "bundleId": config.app.id.to_ascii_lowercase(),
      "name": config.app.name,
      "version": config.app.version,
      "open": {
        "mode": "window",
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
      "architecture": ARCHITECTURE,
      "bundleId": config.app.id.to_ascii_lowercase(),
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
        "mode": "window",
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
    object.insert(
      "rpc".to_owned(),
      json!({
        "protocol": "opentool",
        "version": 2,
        "basePath": config.agentos.rpc_base_path
      }),
    );
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
