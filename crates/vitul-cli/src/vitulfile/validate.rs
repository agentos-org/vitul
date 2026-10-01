use super::{FILE_NAME, Vitulfile};
use anyhow::{Context, Result, bail};
use std::{ffi::OsString, path::PathBuf};

pub fn run(args: &[OsString]) -> Result<()> {
  let root = option_value(args, "--path")
    .map(PathBuf::from)
    .or_else(|| positional_root(args))
    .unwrap_or(std::env::current_dir().context("failed to read current directory")?);
  let file = option_value(args, "--file").map(PathBuf::from);
  let root = root
    .canonicalize()
    .with_context(|| format!("could not resolve project directory {}", root.display()))?;
  let path = file
    .as_ref()
    .map(|file| root.join(file))
    .unwrap_or_else(|| root.join(FILE_NAME));
  if !path.is_file() {
    bail!(
      "{} is missing; run `vitul agentos init` to create it",
      path.display()
    );
  }
  let config = Vitulfile::load(&root, file.as_deref())?;
  println!("Vitulfile is valid for {}", config.target);
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
    if value == "--file" || value == "--path" {
      index += 2;
      continue;
    }
    if let Some((name, _)) = value.split_once('=') {
      if name == "--file" || name == "--path" {
        index += 1;
        continue;
      }
    }
    if !value.starts_with('-') {
      return Some(PathBuf::from(value.as_ref()));
    }
    index += 1;
  }
  None
}
