use anyhow::{Context, Result};
use std::{ffi::OsString, path::PathBuf};

pub fn run(args: &[OsString]) -> Result<()> {
  let root = option_value(args, "--path")
    .or_else(|| positional_root(args))
    .map(PathBuf::from)
    .unwrap_or(std::env::current_dir().context("failed to read current directory")?);
  let root = root
    .canonicalize()
    .with_context(|| format!("could not resolve project directory {}", root.display()))?;
  let config = crate::project_config::package_config(&root)?;
  println!("AgentOS config is valid for {}", config.target_summary());
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

fn positional_root(args: &[OsString]) -> Option<String> {
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
      return Some(value.into_owned());
    }
    index += 1;
  }
  None
}
