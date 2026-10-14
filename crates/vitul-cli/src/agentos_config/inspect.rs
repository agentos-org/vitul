use anyhow::{Context, Result};
use std::{ffi::OsString, path::PathBuf};

pub fn run(args: &[OsString]) -> Result<()> {
  let root = option_value(args, "--path")
    .map(PathBuf::from)
    .or_else(|| positional_root(args))
    .unwrap_or(std::env::current_dir().context("failed to read current directory")?);
  let root = root
    .canonicalize()
    .with_context(|| format!("could not resolve project directory {}", root.display()))?;
  let config = crate::project_config::package_config(&root)?;
  println!("Project: {}", root.display());
  println!("Config: vitul.conf.json");
  println!("AgentOS config: valid");
  println!("App: {} ({})", config.app.name, config.app.id);
  println!(
    "Type: {}",
    match config.app.app_type {
      super::PackageAppType::Process => "process",
      super::PackageAppType::RemoteWeb => "remote-web",
    }
  );
  println!("Targets: {}", config.target_summary());
  match config.app.app_type {
    super::PackageAppType::Process => {
      println!(
        "Run command: {}",
        config.run.command.as_deref().unwrap_or("")
      );
      println!("Build steps: {}", config.build.steps.len());
      println!("Artifacts: {}", config.build.artifacts.len());
      println!("Execution mode: {}", config.agentos.execution_mode_name());
      if let Some(native) = config
        .agentos
        .execution
        .as_ref()
        .and_then(|execution| execution.native.as_ref())
      {
        println!(
          "Native profile: v{} ({}, {})",
          native.version, native.identity, native.scope
        );
        println!("Last window close: {}", native.last_window_close);
        println!("Stop grace period: {} ms", native.stop_grace_period_ms);
      }
      println!("OpenTool discovery: {}", config.agentos.rpc_discovery);
      println!("Persistent data: {}", config.data.persistent);
      if let Some(network) = &config.agentos.network {
        println!("Network protocol version: {}", network.protocol_version);
        let mut profile_types = Vec::new();
        if !network.proxy_profiles.is_empty() {
          profile_types.push("proxy");
        }
        if !network.tun_profiles.is_empty() {
          profile_types.push("tun");
        }
        println!("Network profile types: {}", profile_types.join(", "));
        println!("Network executables: {}", network.executables.len());
        println!("Network proxy profiles: {}", network.proxy_profiles.len());
        println!("Network TUN profiles: {}", network.tun_profiles.len());
        let network_permissions = config
          .agentos
          .permissions
          .iter()
          .filter(|permission| {
            matches!(
              permission.as_str(),
              "native.providers" | "native.network" | "native.system-proxy" | "native.tun"
            )
          })
          .map(String::as_str)
          .collect::<Vec<_>>();
        println!("Network permissions: {}", network_permissions.join(", "));
      }
    }
    super::PackageAppType::RemoteWeb => {
      println!("Website: {}", config.web.url.as_deref().unwrap_or(""));
      println!("Packages an AgentOS URL app; the website opens in the embedded CEF window.");
    }
  }
  Ok(())
}

fn option_value(args: &[OsString], option: &str) -> Option<String> {
  let mut index = 0;
  while index < args.len() {
    let value = args[index].to_string_lossy();
    if let Some(value) = value.strip_prefix(&format!("{option}=")) {
      return Some(value.to_owned());
    }
    if value == option {
      return args
        .get(index + 1)
        .map(|value| value.to_string_lossy().into_owned());
    }
    index += 1;
  }
  None
}

fn positional_root(args: &[OsString]) -> Option<PathBuf> {
  let mut index = 0;
  while index < args.len() {
    let value = args[index].to_string_lossy();
    if value == "--path" {
      index += 2;
      continue;
    }
    if value.starts_with("--path=") {
      index += 1;
      continue;
    }
    if !value.starts_with('-') {
      return Some(PathBuf::from(value.as_ref()));
    }
    index += 1;
  }
  None
}
