mod agentos;
mod dev;
mod init;

use anyhow::{bail, Context};
use std::{ffi::OsString, path::PathBuf};

const HELP: &str = "Vitul application toolchain\n\nUSAGE:\n  vitul <COMMAND> [OPTIONS]\n\nCOMMANDS:\n  init <PROJECT>     Create an OpenTool-first Vitul project\n  dev                Run a desktop app with its OpenTool backend\n  build              Build a desktop app or an AgentOS package\n  info, icon, plugin, signer, bundle, android, ios, ...\n                     Run Tauri-compatible tooling through Vitul\n\nTARGETS:\n  Desktop targets use Vitul's Tauri-compatible runtime.\n  AgentOS v2 .agt manifests support ARM64 and x64.\n  agentos-arm64      Build an AgentOS ARM64 .agt package\n  agentos-x64        x64 package target is not provided by this CLI\n\nCOMPATIBILITY:\n  Tauri-compatible desktop tooling is invoked internally by Vitul.\n";

pub fn run() -> anyhow::Result<()> {
  let args = std::env::args_os().skip(1).collect::<Vec<_>>();
  run_vitul_with_args(args)
}

/// Run the Vitul CLI using arguments supplied by a shell or package-manager wrapper.
pub fn run_vitul_with_args(args: Vec<OsString>) -> anyhow::Result<()> {
  let mut args = args.into_iter();
  let Some(command) = args.next() else {
    print!("{HELP}");
    return Ok(());
  };
  let command = command.to_string_lossy();
  let args = args.collect::<Vec<_>>();

  match command.as_ref() {
    "-h" | "--help" | "help" => print!("{HELP}"),
    "-V" | "--version" => println!("vitul {}", env!("CARGO_PKG_VERSION")),
    "init" => init::run(args)?,
    "dev" => dev::run(args)?,
    "build" => run_build(args)?,
    other => run_tauri(other, args, "vitul")?,
  }
  Ok(())
}

/// Internal compatibility entry used by tooling that embeds the Tauri CLI.
/// User-facing Vitul commands should use [`run_vitul_with_args`].
pub fn run_compat_with_args(args: Vec<OsString>, bin_name: Option<String>) -> anyhow::Result<()> {
  let command = args.first().map(|arg| arg.to_string_lossy().into_owned());
  let wants_help = args
    .iter()
    .any(|arg| matches!(arg.to_string_lossy().as_ref(), "-h" | "--help"));
  let is_vitul_project = is_vitul_project(&std::env::current_dir()?);

  if is_vitul_project && !wants_help && matches!(command.as_deref(), Some("dev" | "build")) {
    return run_vitul_with_args(args);
  }

  tauri_cli::try_run(args, bin_name)?;
  Ok(())
}

fn is_vitul_project(root: &std::path::Path) -> bool {
  root.join("vitul.conf.json").is_file() && root.join("backend/Cargo.toml").is_file()
}

fn run_build(args: Vec<OsString>) -> anyhow::Result<()> {
  match option_value(&args, "--target", "-t").as_deref() {
    Some("agentos-arm64") => agentos::build_arm64(&project_root()?),
    Some("agentos-x64") => {
      bail!("AgentOS x64 package creation is not supported by this Vitul CLI; use `agentos-arm64`")
    }
    _ => run_tauri("build", args, "vitul"),
  }
}

fn run_tauri(command: &str, args: Vec<OsString>, bin_name: &str) -> anyhow::Result<()> {
  let mut forwarded = Vec::with_capacity(args.len() + 1);
  forwarded.push(OsString::from(command));
  forwarded.extend(args);
  tauri_cli::try_run(forwarded, Some(bin_name.to_owned()))
    .with_context(|| format!("{bin_name} desktop {command} failed"))
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

fn project_root() -> anyhow::Result<PathBuf> {
  std::env::current_dir().context("failed to read current directory")
}

#[cfg(test)]
mod tests {
  use super::option_value;
  use std::ffi::OsString;

  #[test]
  fn reads_long_and_short_target_options() {
    assert_eq!(
      option_value(
        &["--target".into(), "agentos-arm64".into()],
        "--target",
        "-t"
      ),
      Some("agentos-arm64".to_owned())
    );
    assert_eq!(
      option_value(
        &[OsString::from("-t"), OsString::from("agentos-arm64")],
        "--target",
        "-t"
      ),
      Some("agentos-arm64".to_owned())
    );
  }

  #[test]
  fn reads_equals_form() {
    assert_eq!(
      option_value(&["--target=agentos-arm64".into()], "--target", "-t"),
      Some("agentos-arm64".to_owned())
    );
  }
}
