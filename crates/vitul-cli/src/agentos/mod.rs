mod package;

pub mod legacy;

use anyhow::{Context, Result, bail};
use std::{ffi::OsString, path::PathBuf};

pub fn run(args: Vec<OsString>) -> Result<()> {
  let Some(command) = args.first().map(|arg| arg.to_string_lossy().into_owned()) else {
    print_help();
    return Ok(());
  };
  let args = &args[1..];
  if args
    .iter()
    .any(|arg| matches!(arg.to_string_lossy().as_ref(), "-h" | "--help"))
  {
    print_help();
    return Ok(());
  }
  match command.as_str() {
    "init" => run_init(args),
    "inspect" => crate::vitulfile::inspect::run(args),
    "validate" => crate::vitulfile::validate::run(args),
    "-h" | "--help" | "help" => {
      print_help();
      Ok(())
    }
    _ => bail!("unknown AgentOS command `{command}`; run `vitul agentos --help`"),
  }
}

pub fn build_vitulfile(project_root: &std::path::Path, target: &str) -> Result<()> {
  package::build(project_root, target)
}

fn run_init(args: &[OsString]) -> Result<()> {
  let root = option_value(args, "--path")
    .map(PathBuf::from)
    .unwrap_or(std::env::current_dir().context("failed to read current directory")?);
  let kind = option_value(args, "--type").unwrap_or_else(|| "process".to_owned());
  let app_type = match kind.as_str() {
    "process" => crate::vitulfile::VitulfileAppType::Process,
    "remote-web" => crate::vitulfile::VitulfileAppType::RemoteWeb,
    _ => bail!("--type must be `process` or `remote-web`"),
  };
  let url = option_value(args, "--url");
  if app_type == crate::vitulfile::VitulfileAppType::Process && url.is_some() {
    bail!("--url is only valid with `--type remote-web`");
  }
  let path = crate::vitulfile::init(&root, app_type, url.as_deref())?;
  println!("Created {}", path.display());
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

fn print_help() {
  println!(
    "AgentOS packaging\n\nUSAGE:\n  vitul agentos <COMMAND> [OPTIONS]\n\nCOMMANDS:\n  init      Write a starter Vitulfile for a standalone web project\n  inspect   Inspect the current AgentOS configuration\n  validate  Validate declared AgentOS targets\n\nINIT OPTIONS:\n  --path <DIR>             Project directory (default: current directory)\n  --type <process|remote-web>\n                           App profile (default: process)\n  --url <HTTPS_URL>        Required for remote-web\n\nFor apps created with `vitul create`, edit the `agentos` section in the root\nvitul.conf.json instead of creating a Vitulfile.\n\nBUILD:\n  vitul build --target agentos             Build every declared target\n  vitul build --target agentos/arm64       Build only ARM64\n  vitul build --target agentos/x64         Build only x64"
  );
}
