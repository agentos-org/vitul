// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{Result, error::ErrorExt};
use clap::{Command, Parser};
use clap_complete::{Shell, generate};

use std::{fs::write, path::PathBuf};

const PKG_MANAGERS: &[&str] = &["cargo", "pnpm", "npm", "yarn", "bun", "deno"];

#[derive(Debug, Clone, Parser)]
#[clap(about = "Generate Vitul CLI shell completions for Bash, Zsh, PowerShell or Fish")]
pub struct Options {
  /// Shell to generate a completion script for.
  #[clap(short, long, verbatim_doc_comment)]
  shell: Shell,
  /// Output file for the shell completions. By default the completions are printed to stdout.
  #[clap(short, long)]
  output: Option<PathBuf>,
}

fn completions_for(shell: Shell, manager: &'static str, cmd: Command) -> Vec<u8> {
  let vitul_bin_name = match manager {
    "npm" | "bun" => format!("{manager} run vitul"),
    "deno" => format!("{manager} task vitul"),
    _ => format!("{manager} vitul"),
  };
  // Override the bin name inherited from how the CLI was invoked (e.g. `cargo-vitul`),
  // clap_complete resolves subcommands from the words of the bin name
  let vitul = cmd.name("vitul").bin_name(vitul_bin_name);
  let mut command = if manager == "npm" || manager == "bun" {
    Command::new(manager)
      .bin_name(manager)
      .subcommand(Command::new("run").subcommand(vitul))
  } else if manager == "deno" {
    Command::new(manager)
      .bin_name(manager)
      .subcommand(Command::new("task").subcommand(vitul))
  } else {
    Command::new(manager).bin_name(manager).subcommand(vitul)
  };

  let mut buf = Vec::new();
  generate(shell, &mut command, manager, &mut buf);
  buf
}

// Bash completions for the commands that invoke the Tauri CLI directly.
//
// Bash completion functions are registered per command name, so registering one for the package
// managers (`cargo`, `npm`...) would replace their own completions; those are not registered.
fn bash_completions(cmd: Command) -> String {
  let mut command = cmd.name("vitul").bin_name("vitul");
  let mut buf = Vec::new();
  generate(Shell::Bash, &mut command, "vitul", &mut buf);
  // Use a function name that does not clash with other completions.
  String::from_utf8_lossy(&buf)
    .replace("_vitul() {", "_vitul_cli() {")
    .replace("complete -F _vitul ", "complete -F _vitul_cli ")
    .replace(" -o default vitul\n", " -o default vitul cargo-vitul\n")
}

fn get_completions(shell: Shell, cmd: Command) -> Result<String> {
  let completions = if shell == Shell::Bash {
    bash_completions(cmd)
  } else {
    let mut buffer = String::new();

    for (i, manager) in PKG_MANAGERS.iter().enumerate() {
      let buf = String::from_utf8_lossy(&completions_for(shell, manager, cmd.clone())).into_owned();

      let completions = match shell {
        Shell::PowerShell => {
          if i != 0 {
            // namespaces have already been imported
            buf
              .replace("using namespace System.Management.Automation.Language", "")
              .replace("using namespace System.Management.Automation", "")
          } else {
            buf
          }
        }
        _ => buf,
      };

      buffer.push_str(&completions);
      buffer.push('\n');
    }

    buffer
  };

  Ok(completions)
}

pub fn command(options: Options, cmd: Command) -> Result<()> {
  log::info!("Generating completion file for {}...", options.shell);

  let completions = get_completions(options.shell, cmd)?;
  if let Some(output) = options.output {
    write(&output, completions).fs_context("failed to write to completions", output)?;
  } else {
    print!("{completions}");
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use clap::CommandFactory;

  #[test]
  fn bash_completions_do_not_hijack_other_commands() {
    for bin_name in ["cargo-vitul", "cargo vitul", "vitul"] {
      let completions =
        get_completions(Shell::Bash, crate::Cli::command().bin_name(bin_name)).unwrap();
      assert!(completions.contains("_vitul_cli() {"));
      assert!(!completions.contains("_vitul() {"));
      assert!(!completions.contains("_cargo"));

      let registrations: Vec<_> = completions
        .lines()
        .filter(|line| line.trim_start().starts_with("complete "))
        .collect();
      assert!(!registrations.is_empty());
      for registration in registrations {
        assert!(
          registration
            .trim_start()
            .starts_with("complete -F _vitul_cli ")
        );
        assert!(registration.ends_with(" -o default vitul cargo-vitul"));
      }
    }
  }

  #[test]
  fn completions_generate_for_every_shell() {
    for shell in [Shell::Zsh, Shell::Fish, Shell::PowerShell, Shell::Elvish] {
      for bin_name in ["cargo-vitul", "cargo vitul", "vitul"] {
        assert!(
          !get_completions(shell, crate::Cli::command().bin_name(bin_name))
            .unwrap()
            .is_empty()
        );
      }
    }
  }
}
