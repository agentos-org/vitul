use super::{FILE_NAME, Vitulfile};
use anyhow::{Context, Result};
use std::{ffi::OsString, fs, path::PathBuf};

const PROJECT_MARKERS: &[&str] = &[
  "package.json",
  "pnpm-lock.yaml",
  "yarn.lock",
  "bun.lock",
  "Cargo.toml",
  "go.mod",
  "pyproject.toml",
  "requirements.txt",
  "Dockerfile",
  "docker-compose.yml",
  "compose.yaml",
];

pub fn run(args: &[OsString]) -> Result<()> {
  let root = option_value(args, "--path")
    .map(PathBuf::from)
    .or_else(|| positional_root(args))
    .unwrap_or(std::env::current_dir().context("failed to read current directory")?);
  let root = root
    .canonicalize()
    .with_context(|| format!("could not resolve project directory {}", root.display()))?;
  println!("Project: {}", root.display());

  for marker in PROJECT_MARKERS {
    if root.join(marker).exists() {
      println!("Detected: {marker}");
    }
  }

  let vitulfile = root.join(FILE_NAME);
  if vitulfile.is_file() || crate::project_config::is_unified(&root) {
    let config = if crate::project_config::is_unified(&root) {
      Vitulfile::load(&root, None)?
    } else {
      let source = fs::read_to_string(&vitulfile)
        .with_context(|| format!("could not read {}", vitulfile.display()))?;
      serde_yaml::from_str(&source)
        .with_context(|| format!("could not parse {}", vitulfile.display()))?
    };
    let unified = crate::project_config::is_unified(&root);
    let label = if unified {
      "AgentOS config"
    } else {
      "Vitulfile"
    };
    match config.validate(&root) {
      Ok(()) => println!("{label}: valid"),
      Err(error) => println!("{label}: needs attention: {error}"),
    }
    println!("App: {} ({})", config.app.name, config.app.id);
    println!(
      "Type: {}",
      match config.app.app_type {
        super::VitulfileAppType::Process => "process",
        super::VitulfileAppType::RemoteWeb => "remote-web",
      }
    );
    println!("Targets: {}", config.target_summary());
    match config.app.app_type {
      super::VitulfileAppType::Process => {
        println!(
          "Run command: {}",
          config.run.command.as_deref().unwrap_or("")
        );
        println!("Build steps: {}", config.build.steps.len());
        println!("Artifacts: {}", config.build.artifacts.len());
        println!("OpenTool discovery: {}", config.agentos.rpc_discovery);
        println!("Persistent data: {}", config.data.persistent);
      }
      super::VitulfileAppType::RemoteWeb => {
        println!("Website: {}", config.web.url.as_deref().unwrap_or(""));
        println!("Packages an AgentOS URL app; the website opens in the embedded CEF window.");
      }
    }
  } else {
    println!("Vitulfile: not found");
    println!("Create one with `vitul agentos init`.");
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
