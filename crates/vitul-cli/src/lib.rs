mod agentos;
mod dev;
mod init;
mod project_config;
mod vitulfile;

use anyhow::Context;
use std::{ffi::OsString, path::PathBuf};

const HELP: &str = "Vitul application toolchain\n\nUSAGE:\n  vitul <COMMAND> [OPTIONS]\n\nCOMMANDS:\n  create <PROJECT>    Create an app with Vitul's preset web stack\n  init <PROJECT>      Alias for create\n  agentos <COMMAND>   Inspect or validate AgentOS packaging config\n  dev                 Run a desktop app with its OpenTool backend\n  build               Build a desktop app or an AgentOS package\n  info, icon, plugin, signer, bundle, android, ios, ...\n                      Run desktop and mobile tooling through Vitul\n\nPRESET APP STACK:\n  React + TypeScript, Tailwind CSS, Vite, and pnpm\n\nTARGETS:\n  agentos             Build every declared AgentOS target\n  agentos/arm64       Build an AgentOS ARM64 .agt package\n  agentos/x64         Build an AgentOS x64 .agt package\n\nCREATE OPTIONS:\n  --product-name <NAME>  Set the app display name (default: directory name)\n  --window-title <TITLE> Set the desktop window title (default: product name)\n\nCONFIGURATION:\n  New Vitul apps use one root vitul.conf.json.\n  Standalone web projects can use a Vitulfile.\n";

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
    "create" | "init" => init::run(args)?,
    "agentos" => agentos::run(args)?,
    "dev" => dev::run(args)?,
    "build" => run_build(args)?,
    other => run_desktop_cli(other, args, "vitul")?,
  }
  Ok(())
}

/// Internal compatibility entry used by tooling that embeds Vitul's desktop CLI.
/// User-facing Vitul commands should use [`run_vitul_with_args`].
pub fn run_compat_with_args(args: Vec<OsString>, bin_name: Option<String>) -> anyhow::Result<()> {
  let command = args.first().map(|arg| arg.to_string_lossy().into_owned());
  let wants_help = args
    .iter()
    .any(|arg| matches!(arg.to_string_lossy().as_ref(), "-h" | "--help"));
  let is_vitul_project = is_vitul_project(&std::env::current_dir()?);
  let has_vitulfile = std::env::current_dir()?
    .join(vitulfile::FILE_NAME)
    .is_file();
  let is_agentos_build = command.as_deref() == Some("build")
    && option_value(&args, "--target", "-t")
      .is_some_and(|target| target == "agentos" || target.starts_with("agentos/"));

  if command.as_deref() == Some("agentos")
    || (!wants_help
      && (is_agentos_build
        || (command.as_deref() == Some("build") && (is_vitul_project || has_vitulfile))
        || (command.as_deref() == Some("dev") && is_vitul_project)))
  {
    return run_vitul_with_args(args);
  }

  vitul_desktop_cli::try_run(args, bin_name)?;
  Ok(())
}

fn is_vitul_project(root: &std::path::Path) -> bool {
  root.join("vitul.conf.json").is_file() && root.join("backend/Cargo.toml").is_file()
}

fn run_build(args: Vec<OsString>) -> anyhow::Result<()> {
  match option_value(&args, "--target", "-t").as_deref() {
    Some("agentos") => {
      let root = project_root()?;
      if project_config::is_unified(&root) || root.join(vitulfile::FILE_NAME).is_file() {
        agentos::build_vitulfile(&root, "agentos")
      } else if is_vitul_project(&root) {
        agentos::legacy::build_arm64(&root)
      } else {
        agentos::build_vitulfile(&root, "agentos")
      }
    }
    Some("agentos/arm64") => build_agentos_arm64(&project_root()?),
    Some("agentos/x64") => agentos::build_vitulfile(&project_root()?, "agentos/x64"),
    Some("agentos-arm64") => {
      let root = project_root()?;
      if project_config::is_unified(&root) || root.join(vitulfile::FILE_NAME).is_file() {
        agentos::build_vitulfile(&root, "agentos/arm64")
      } else {
        agentos::legacy::build_arm64(&root)
      }
    }
    Some("agentos-x64") => agentos::build_vitulfile(&project_root()?, "agentos/x64"),
    _ => run_desktop_cli("build", args, "vitul"),
  }
}

fn build_agentos_arm64(root: &std::path::Path) -> anyhow::Result<()> {
  if project_config::is_unified(root) || root.join(vitulfile::FILE_NAME).is_file() {
    agentos::build_vitulfile(root, "agentos/arm64")
  } else if is_vitul_project(root) {
    agentos::legacy::build_arm64(root)
  } else {
    agentos::build_vitulfile(root, "agentos/arm64")
  }
}

fn run_desktop_cli(command: &str, args: Vec<OsString>, bin_name: &str) -> anyhow::Result<()> {
  project_config::write_runtime_config(&project_root()?)?;
  let mut forwarded = Vec::with_capacity(args.len() + 1);
  forwarded.push(OsString::from(command));
  forwarded.extend(args);
  vitul_desktop_cli::try_run(forwarded, Some(bin_name.to_owned()))
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
