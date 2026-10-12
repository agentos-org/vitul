mod package;

pub mod legacy;

use anyhow::{Context, Result, bail};
use serde_json::json;
use std::{ffi::OsString, fs, path::PathBuf};

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
    "inspect" => crate::agentos_config::inspect::run(args),
    "validate" => crate::agentos_config::validate::run(args),
    "-h" | "--help" | "help" => {
      print_help();
      Ok(())
    }
    _ => bail!("unknown AgentOS command `{command}`; run `vitul agentos --help`"),
  }
}

pub fn build_package(project_root: &std::path::Path, target: &str) -> Result<()> {
  package::build(project_root, target)
}

fn run_init(args: &[OsString]) -> Result<()> {
  let root = option_value(args, "--path")
    .map(PathBuf::from)
    .unwrap_or(std::env::current_dir().context("failed to read current directory")?);
  let kind = option_value(args, "--type").unwrap_or_else(|| "process".to_owned());
  let url = option_value(args, "--url");
  let app_type = match kind.as_str() {
    "process" => "process",
    "remote-web" => "remote-web",
    _ => bail!("--type must be `process` or `remote-web`"),
  };
  if app_type == "process" && url.is_some() {
    bail!("--url is only valid with `--type remote-web`");
  }
  if app_type == "remote-web" {
    let url = url
      .as_deref()
      .context("--url is required for `remote-web`")?;
    let parsed = url::Url::parse(url).context("--url must be a valid HTTPS URL")?;
    if parsed.scheme() != "https"
      || parsed.host_str().is_none()
      || !parsed.username().is_empty()
      || parsed.password().is_some()
    {
      bail!("--url must use HTTPS and must not contain embedded credentials");
    }
  }

  let path = root.join("vitul.conf.json");
  if path.exists() {
    bail!("{} already exists", path.display());
  }
  let name = root
    .file_name()
    .and_then(|value| value.to_str())
    .filter(|value| !value.is_empty())
    .unwrap_or("my-app");
  let suffix = name
    .chars()
    .map(|character| {
      if character.is_ascii_alphanumeric() {
        character.to_ascii_lowercase()
      } else {
        '-'
      }
    })
    .collect::<String>()
    .trim_matches('-')
    .to_owned();
  let suffix = if suffix.is_empty() { "app" } else { &suffix };
  let mut agentos = json!({
    "type": app_type,
    "targets": ["agentos/arm64"],
    "open": { "mode": "window", "titleBar": "system" }
  });
  if app_type == "process" {
    agentos["permissions"] = json!([]);
    agentos["build"] = json!({
      "steps": [],
      "artifacts": []
    });
    agentos["run"] = json!({
      "command": "node",
      "args": ["server.js"],
      "port": 9627,
      "healthCheck": "/health",
      "readyTimeout": 60,
      "frontendEntry": "/"
    });
  } else if let Some(url) = url {
    agentos["web"] = json!({ "url": url });
  }
  let config = json!({
    "schemaVersion": 2,
    "productName": name,
    "identifier": format!("com.example.{suffix}"),
    "version": "0.1.0",
    "agentos": agentos
  });
  fs::write(&path, serde_json::to_vec_pretty(&config)?)
    .with_context(|| format!("could not write {}", path.display()))?;
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
    "AgentOS packaging\n\nUSAGE:\n  vitul agentos <COMMAND> [OPTIONS]\n\nCOMMANDS:\n  init      Create a root vitul.conf.json AgentOS config\n  inspect   Inspect the current AgentOS configuration\n  validate  Validate the AgentOS configuration\n\nINIT OPTIONS:\n  --path <DIR>             Project directory (default: current directory)\n  --type <process|remote-web>\n                           App profile (default: process)\n  --url <HTTPS_URL>        Required for remote-web\n\nFor apps created with `vitul create`, edit the `agentos` section in the root\nvitul.conf.json instead of initializing another config.\n\nBUILD:\n  vitul build --target agentos             Build every declared target\n  vitul build --target agentos/arm64       Build only ARM64\n  vitul build --target agentos/x64         Build only x64"
  );
}
