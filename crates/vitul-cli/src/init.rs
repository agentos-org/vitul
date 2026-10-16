use anyhow::{Context, Result, bail};
use std::{
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
};

const BACKEND_CARGO: &str = include_str!("../templates/backend.Cargo.toml");
const BACKEND_LIB: &str = include_str!("../templates/backend-lib.rs");
const BACKEND_MAIN: &str = include_str!("../templates/backend-main.rs");
const BACKEND_TOOLS_MOD: &str = include_str!("../templates/backend-tools-mod.rs");
const BACKEND_TOOL_GREET: &str = include_str!("../templates/backend-tool-greet.rs");
const FRONTEND_INDEX: &str = include_str!("../templates/index.html");
const FRONTEND_MAIN: &str = include_str!("../templates/frontend-main.tsx");
const FRONTEND_CONTEXT: &str = include_str!("../templates/host-context.ts");
const FRONTEND_MESSAGES: &str = include_str!("../templates/messages.ts");
const FRONTEND_APP: &str = include_str!("../templates/App.tsx");
const FRONTEND_BUTTON: &str = include_str!("../templates/components/ui/Button.tsx");
const FRONTEND_CARD: &str = include_str!("../templates/components/ui/Card.tsx");
const FRONTEND_INPUT: &str = include_str!("../templates/components/ui/Input.tsx");
const FRONTEND_STYLES: &str = include_str!("../templates/styles.css");
const FRONTEND_OPENTOOL: &str = include_str!("../templates/opentool.ts");
const FRONTEND_AGENTOS: &str = include_str!("../templates/agentos.ts");
const FRONTEND_HOST: &str = include_str!("../templates/host.ts");
const DESKTOP_HOST: &str = include_str!("../templates/desktop-host.ts");
const VITUL_LIB: &str = include_str!("../templates/vitul-lib.rs");
const VITE_CONFIG: &str = include_str!("../templates/vite.config.ts");
const PROJECT_README: &str = include_str!("../templates/VITUL.md");
const PROJECT_README_ZH_CN: &str = include_str!("../templates/VITUL-zh_CN.md");
const AGENT_GUIDE: &str = include_str!("../templates/AGENTS.md");
const CREATE_HELP: &str = "Create an OpenTool-first Vitul app\n\nUSAGE:\n  vitul create [OPTIONS] <PROJECT>\n\nPRESET:\n  React + TypeScript, Tailwind CSS, Vite, and pnpm\n\nOPTIONS:\n      --identifier <IDENTIFIER> Set the bundle identifier (preserved as given)\n      --product-name <NAME>     Set the app display name (default: directory name)\n      --window-title <TITLE>    Set the desktop title (default: product name)\n  -y, --yes                     Skip prompts\n  -f, --force                   Overwrite a non-empty target directory\n";

const PRESET_PACKAGE_MANAGER: &str = "pnpm";
const PRESET_TEMPLATE: &str = "react-ts";
const PRESET_IOS_MINIMUM_VERSION: &str = "26.0";

pub fn run(args: Vec<OsString>) -> Result<()> {
  if args.iter().any(|arg| arg == "--help" || arg == "-h") {
    print!("{CREATE_HELP}");
    return Ok(());
  }
  if args.iter().any(|arg| arg == "--version" || arg == "-v") {
    create_vitul_app::run(args, Some("vitul create".to_owned()), None);
    return Ok(());
  }

  let (mut args, product_name, window_title) = extract_display_options(args)?;
  let project_name = project_name(&args).unwrap_or_else(|| "vitul-app".to_owned());
  let cwd = std::env::current_dir()?;
  let project_root = cwd.join(&project_name);
  let directory_name = target_directory_name(&cwd, &project_root)?;
  let package_name = package_name(&directory_name);
  let product_name = product_name.unwrap_or_else(|| display_name(&directory_name));
  let window_title = window_title.unwrap_or_else(|| product_name.clone());
  reject_unsupported_runtime_version_option(&args)?;
  if option_value(&args, "--identifier", "--identifier").is_none() {
    args.extend([
      OsString::from("--identifier"),
      OsString::from(default_identifier(&package_name)),
    ]);
  }
  if has_option(&args, "--manager", "-m") {
    let manager = option_value(&args, "--manager", "-m");
    if manager.as_deref() != Some(PRESET_PACKAGE_MANAGER) {
      bail!(
        "Vitul's preset package manager is `pnpm`; remove `--manager` or pass `--manager pnpm`"
      );
    }
  } else {
    args.extend([
      OsString::from("--manager"),
      OsString::from(PRESET_PACKAGE_MANAGER),
    ]);
  }
  if has_option(&args, "--template", "-t") {
    let template = option_value(&args, "--template", "-t");
    if template.as_deref() != Some(PRESET_TEMPLATE) {
      bail!(
        "Vitul's preset UI template is `react-ts`; remove `--template` or pass `--template react-ts`"
      );
    }
  } else {
    args.extend([
      OsString::from("--template"),
      OsString::from(PRESET_TEMPLATE),
    ]);
  }

  create_vitul_app::run(args, Some("vitul create".to_owned()), None);
  let project_root = fs::canonicalize(&project_root).unwrap_or(project_root);
  rename_generated_runtime_dir(&project_root)?;
  rename_generated_config(&project_root)?;
  scaffold(&project_root, &package_name, &product_name, &window_title)
}

fn extract_display_options(
  args: Vec<OsString>,
) -> Result<(Vec<OsString>, Option<String>, Option<String>)> {
  let mut remaining = Vec::with_capacity(args.len());
  let mut product_name = None;
  let mut window_title = None;
  let mut args = args.into_iter();
  while let Some(arg) = args.next() {
    let value = arg.to_string_lossy();
    if let Some(value) = value.strip_prefix("--product-name=") {
      product_name = Some(non_empty_option("--product-name", value)?);
    } else if value == "--product-name" {
      product_name = Some(non_empty_option(
        "--product-name",
        &args
          .next()
          .context("--product-name requires a value")?
          .to_string_lossy(),
      )?);
    } else if let Some(value) = value.strip_prefix("--window-title=") {
      window_title = Some(non_empty_option("--window-title", value)?);
    } else if value == "--window-title" {
      window_title = Some(non_empty_option(
        "--window-title",
        &args
          .next()
          .context("--window-title requires a value")?
          .to_string_lossy(),
      )?);
    } else {
      remaining.push(arg);
    }
  }
  Ok((remaining, product_name, window_title))
}

fn non_empty_option(name: &str, value: &str) -> Result<String> {
  let value = value.trim();
  if value.is_empty() {
    bail!("{name} cannot be empty");
  }
  Ok(value.to_owned())
}

fn target_directory_name(cwd: &Path, target: &Path) -> Result<String> {
  let name =
    if target == cwd || target == Path::new(".") || target.file_name() == Some(".".as_ref()) {
      cwd.file_name()
    } else {
      target.file_name()
    }
    .context("the project directory must have a name")?;
  name
    .to_str()
    .map(str::to_owned)
    .context("project directory name is not valid UTF-8")
}

fn package_name(directory_name: &str) -> String {
  let mut name = String::new();
  let mut previous_dash = false;
  for character in directory_name.chars() {
    if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
      name.push(character.to_ascii_lowercase());
      previous_dash = character == '-';
    } else if character.is_whitespace() || matches!(character, ':' | ';' | '~') {
      if !previous_dash && !name.is_empty() {
        name.push('-');
      }
      previous_dash = true;
    }
  }
  let name = name.trim_start_matches(|ch: char| ch.is_ascii_digit() || ch == '-');
  if name.is_empty() {
    "vitul-app".to_owned()
  } else if name.starts_with('_') {
    format!("app-{name}")
  } else {
    name.to_owned()
  }
}

fn display_name(directory_name: &str) -> String {
  directory_name
    .split(['-', '_', ' ', '.'])
    .filter(|part| !part.is_empty())
    .map(|part| {
      let mut characters = part.chars();
      characters
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
        .unwrap_or_default()
    })
    .collect::<Vec<_>>()
    .join(" ")
}

fn rename_generated_runtime_dir(project_root: &Path) -> Result<()> {
  let generated_runtime_dir = project_root.join("src-tauri");
  let vitul_runtime_dir = project_root.join("src-vitul");
  if vitul_runtime_dir.exists() {
    if generated_runtime_dir.exists() {
      bail!(
        "both generated runtime directories exist: {} and {}",
        generated_runtime_dir.display(),
        vitul_runtime_dir.display()
      );
    }
    return Ok(());
  }
  fs::rename(&generated_runtime_dir, &vitul_runtime_dir).with_context(|| {
    format!(
      "failed to rename generated runtime directory {} to {}",
      generated_runtime_dir.display(),
      vitul_runtime_dir.display()
    )
  })
}

fn rename_generated_config(project_root: &Path) -> Result<()> {
  let legacy_config = project_root.join("src-vitul/tauri.conf.json");
  let vitul_config = project_root.join("src-vitul/vitul.conf.json");
  if vitul_config.is_file() {
    return Ok(());
  }
  fs::rename(&legacy_config, &vitul_config).with_context(|| {
    format!(
      "failed to rename generated config {} to {}",
      legacy_config.display(),
      vitul_config.display()
    )
  })
}

fn reject_unsupported_runtime_version_option(args: &[OsString]) -> Result<()> {
  if has_option(args, "--tauri-version", "") {
    bail!(
      "`--tauri-version` is not a Vitul option; `vitul create` selects the supported desktop runtime automatically"
    );
  }
  Ok(())
}

fn default_identifier(project_name: &str) -> String {
  let mut suffix: String = project_name
    .chars()
    .map(|character| {
      if character.is_ascii_alphanumeric() || character == '-' {
        character.to_ascii_lowercase()
      } else {
        '-'
      }
    })
    .collect();
  suffix = suffix.trim_matches('-').to_owned();
  if suffix.is_empty() || !suffix.as_bytes()[0].is_ascii_alphabetic() {
    suffix = format!("app-{suffix}");
  }
  format!("com.vitul.{suffix}")
}

fn project_name(args: &[OsString]) -> Option<String> {
  let mut skip_value = false;
  let mut name = None;
  for arg in args {
    let value = arg.to_string_lossy();
    if skip_value {
      skip_value = false;
      continue;
    }
    if [
      "-m",
      "--manager",
      "-t",
      "--template",
      "--identifier",
      "--tauri-version",
    ]
    .contains(&value.as_ref())
    {
      skip_value = true;
    } else if !value.starts_with('-') {
      name = Some(value.into_owned());
    }
  }
  name
}

fn has_option(args: &[OsString], long: &str, short: &str) -> bool {
  args.iter().any(|arg| {
    let value = arg.to_string_lossy();
    value == long || value == short || value.starts_with(&format!("{long}="))
  })
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

fn scaffold(
  project_root: &Path,
  package_name: &str,
  product_name: &str,
  window_title: &str,
) -> Result<()> {
  if !project_root.join("src-vitul/src/lib.rs").is_file() {
    bail!(
      "create-vitul-app did not create the Vitul desktop scaffold at {}",
      project_root.display()
    );
  }
  write_file(project_root, "backend/Cargo.toml", BACKEND_CARGO)?;
  write_file(project_root, "backend/src/lib.rs", BACKEND_LIB)?;
  write_file(project_root, "backend/src/main.rs", BACKEND_MAIN)?;
  write_file(project_root, "backend/src/tools/mod.rs", BACKEND_TOOLS_MOD)?;
  write_file(
    project_root,
    "backend/src/tools/greet.rs",
    BACKEND_TOOL_GREET,
  )?;
  write_file(project_root, "index.html", FRONTEND_INDEX)?;
  write_file(project_root, "src/opentool.ts", FRONTEND_OPENTOOL)?;
  write_file(project_root, "src/lib/agentos.ts", FRONTEND_AGENTOS)?;
  write_file(project_root, "src/main.tsx", FRONTEND_MAIN)?;
  write_file(project_root, "src/App.tsx", FRONTEND_APP)?;
  write_file(project_root, "src/host-context.ts", FRONTEND_CONTEXT)?;
  write_file(project_root, "src/messages.ts", FRONTEND_MESSAGES)?;
  write_file(project_root, "src/styles.css", FRONTEND_STYLES)?;
  write_file(
    project_root,
    "src/components/ui/Button.tsx",
    FRONTEND_BUTTON,
  )?;
  write_file(project_root, "src/components/ui/Card.tsx", FRONTEND_CARD)?;
  write_file(project_root, "src/components/ui/Input.tsx", FRONTEND_INPUT)?;
  write_file(project_root, "src/host.ts", FRONTEND_HOST)?;
  write_file(project_root, "src/desktop-host.ts", DESKTOP_HOST)?;
  write_file(project_root, "src-vitul/src/lib.rs", VITUL_LIB)?;
  write_file(project_root, "vite.config.ts", VITE_CONFIG)?;
  write_file(project_root, "README.md", PROJECT_README)?;
  write_file(project_root, "README-zh_CN.md", PROJECT_README_ZH_CN)?;
  write_file(project_root, "VITUL.md", PROJECT_README)?;
  write_file(project_root, "VITUL-zh_CN.md", PROJECT_README_ZH_CN)?;
  write_file(project_root, "AGENTS.md", AGENT_GUIDE)?;
  write_file(
    project_root,
    ".env.example",
    "# Optional AgentOS Gateway address. This value is public in the built app.\nVITE_AGENTOS_GATEWAY_URL=http://127.0.0.1:8888\n",
  )?;
  remove_unused_frontend_assets(project_root)?;
  configure_frontend_dependencies(project_root)?;
  apply_project_names(project_root, package_name, product_name, window_title)?;
  configure_desktop_plugins(project_root)?;
  configure_capabilities(project_root)?;

  let mut vitul_config_path = PathBuf::from(project_root);
  vitul_config_path.push("src-vitul/vitul.conf.json");
  let mut config: serde_json::Value = serde_json::from_slice(
    &fs::read(&vitul_config_path).context("failed to read generated desktop runtime config")?,
  )
  .context("failed to parse generated desktop runtime config")?;
  config["$schema"] = serde_json::Value::String("https://schema.vitul.app/config/2.12.0".into());
  config["productName"] = serde_json::Value::String(product_name.to_owned());
  config["bundle"]["iOS"]["minimumSystemVersion"] =
    serde_json::Value::String(PRESET_IOS_MINIMUM_VERSION.into());
  if let Some(title) = config
    .get_mut("app")
    .and_then(|app| app.get_mut("windows"))
    .and_then(serde_json::Value::as_array_mut)
    .and_then(|windows| windows.first_mut())
    .and_then(|window| window.get_mut("title"))
  {
    *title = serde_json::Value::String(window_title.to_owned());
  }
  fs::write(
    &vitul_config_path,
    serde_json::to_vec_pretty(&config).context("failed to encode desktop runtime config")?,
  )
  .context("failed to update generated desktop runtime config")?;
  crate::project_config::create_project_config(project_root)?;
  ignore_generated_runtime_config(project_root)?;

  println!(
    "Created OpenTool-first Vitul project at {}",
    project_root.display()
  );
  Ok(())
}

fn apply_project_names(
  project_root: &Path,
  package_name: &str,
  product_name: &str,
  window_title: &str,
) -> Result<()> {
  let package_path = project_root.join("package.json");
  let mut package: serde_json::Value = serde_json::from_slice(
    &fs::read(&package_path)
      .with_context(|| format!("failed to read {}", package_path.display()))?,
  )
  .context("failed to parse generated package.json")?;
  package["name"] = serde_json::Value::String(package_name.to_owned());
  fs::write(&package_path, serde_json::to_vec_pretty(&package)?)
    .with_context(|| format!("failed to update {}", package_path.display()))?;

  let cargo_path = project_root.join("src-vitul/Cargo.toml");
  let mut cargo = fs::read_to_string(&cargo_path)
    .with_context(|| format!("failed to read {}", cargo_path.display()))?;
  let old_lib_name =
    toml_section_name(&cargo, "lib").context("generated src-vitul/Cargo.toml has no [lib] name")?;
  replace_toml_section_name(&mut cargo, "package", package_name)?;
  let lib_name = format!("{}_lib", package_name.replace('-', "_"));
  replace_toml_section_name(&mut cargo, "lib", &lib_name)?;
  fs::write(&cargo_path, cargo)
    .with_context(|| format!("failed to update {}", cargo_path.display()))?;

  let main_path = project_root.join("src-vitul/src/main.rs");
  if main_path.is_file() {
    let main = fs::read_to_string(&main_path)
      .with_context(|| format!("failed to read {}", main_path.display()))?;
    fs::write(&main_path, main.replace(&old_lib_name, &lib_name))
      .with_context(|| format!("failed to update {}", main_path.display()))?;
  }

  let config_path = project_root.join("src-vitul/vitul.conf.json");
  let mut config: serde_json::Value = serde_json::from_slice(
    &fs::read(&config_path).with_context(|| format!("failed to read {}", config_path.display()))?,
  )
  .context("failed to parse generated desktop config")?;
  config["productName"] = serde_json::Value::String(product_name.to_owned());
  if let Some(title) = config.pointer_mut("/app/windows/0/title") {
    *title = serde_json::Value::String(window_title.to_owned());
  }
  fs::write(&config_path, serde_json::to_vec_pretty(&config)?)
    .with_context(|| format!("failed to update {}", config_path.display()))
}

fn toml_section_name(contents: &str, section: &str) -> Option<String> {
  let mut active = false;
  for line in contents.lines() {
    if line.trim().starts_with('[') {
      active = line.trim() == format!("[{section}]");
    } else if active {
      if let Some(value) = line.trim().strip_prefix("name = ") {
        return value.trim().trim_matches('"').to_owned().into();
      }
    }
  }
  None
}

fn replace_toml_section_name(contents: &mut String, section: &str, name: &str) -> Result<()> {
  let mut active = false;
  let mut replaced = false;
  let lines = contents
    .lines()
    .map(|line| {
      if line.trim().starts_with('[') {
        active = line.trim() == format!("[{section}]");
      } else if active && !replaced && line.trim().starts_with("name = ") {
        replaced = true;
        return format!("name = \"{name}\"");
      }
      line.to_owned()
    })
    .collect::<Vec<_>>();
  if !replaced {
    bail!("generated Cargo.toml has no name in [{section}]");
  }
  *contents = lines.join("\n") + "\n";
  Ok(())
}

fn remove_unused_frontend_assets(project_root: &Path) -> Result<()> {
  for relative_path in [
    "src/assets/vitul.svg",
    "src/assets/vite.svg",
    "src/assets/react.svg",
    "src/App.css",
    "tailwind.config.cjs",
    "postcss.config.cjs",
  ] {
    match fs::remove_file(project_root.join(relative_path)) {
      Ok(()) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(error) => {
        return Err(error).with_context(|| format!("failed to remove {relative_path}"));
      }
    }
  }
  Ok(())
}

fn ignore_generated_runtime_config(project_root: &Path) -> Result<()> {
  let path = project_root.join(".gitignore");
  let mut contents = fs::read_to_string(&path).unwrap_or_default();
  for entry in [
    "/src-vitul/vitul.conf.json",
    "/src-vitul/tauri.conf.json",
    "/agentos-build/",
    "/.env",
  ] {
    if !contents.lines().any(|line| line.trim() == entry) {
      if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
      }
      contents.push_str(entry);
      contents.push('\n');
    }
  }
  fs::write(&path, contents).with_context(|| format!("failed to update {}", path.display()))
}

fn configure_frontend_dependencies(project_root: &Path) -> Result<()> {
  let path = project_root.join("package.json");
  let mut package: serde_json::Value = serde_json::from_slice(
    &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
  )
  .context("failed to parse generated package.json")?;
  let dependencies = package
    .get_mut("dependencies")
    .and_then(serde_json::Value::as_object_mut)
    .context("generated package.json has no dependencies object")?;
  for dependency in [
    "agentos-sdk-ts",
    "@tauri-apps/plugin-clipboard-manager",
    "@tauri-apps/plugin-dialog",
    "@tauri-apps/plugin-notification",
    "@tauri-apps/plugin-shell",
  ] {
    dependencies.insert(
      dependency.to_owned(),
      serde_json::Value::String(
        if dependency == "agentos-sdk-ts" {
          "^2.1.0"
        } else {
          "^2"
        }
        .into(),
      ),
    );
  }
  let dependencies = package
    .get_mut("devDependencies")
    .and_then(serde_json::Value::as_object_mut)
    .context("generated package.json has no devDependencies object")?;
  dependencies.insert(
    "tailwindcss".to_owned(),
    serde_json::Value::String("^4.3.3".into()),
  );
  dependencies.insert(
    "@tailwindcss/vite".to_owned(),
    serde_json::Value::String("^4.3.3".into()),
  );
  dependencies.remove("postcss");
  dependencies.remove("autoprefixer");
  dependencies.insert(
    "@types/node".to_owned(),
    serde_json::Value::String("^22.19.0".into()),
  );
  dependencies.remove("@tauri-apps/cli");
  dependencies.insert(
    "@litevar/vitul-cli".to_owned(),
    serde_json::Value::String("^3.4.0".into()),
  );
  package["packageManager"] = serde_json::Value::String("pnpm@12.4.2".into());
  package["engines"] = serde_json::json!({ "node": ">=22.13.0", "pnpm": ">=12.4.2" });
  let scripts = package
    .get_mut("scripts")
    .and_then(serde_json::Value::as_object_mut)
    .context("generated package.json has no scripts object")?;
  scripts.remove("tauri");
  scripts.insert(
    "vitul".to_owned(),
    serde_json::Value::String("vitul".into()),
  );
  scripts.insert(
    "check".to_owned(),
    serde_json::Value::String("tsc -b --pretty false".into()),
  );
  fs::write(
    path,
    serde_json::to_vec_pretty(&package).context("failed to encode package.json")?,
  )
  .context("failed to update generated frontend dependencies")
}

#[cfg(test)]
mod tests {
  use super::{
    configure_frontend_dependencies, display_name, extract_display_options, package_name,
    target_directory_name,
  };
  use serde_json::json;
  use std::{ffi::OsString, fs};

  #[test]
  fn generated_project_uses_local_vitul_cli() {
    let project = tempfile::tempdir().unwrap();
    fs::write(
      project.path().join("package.json"),
      serde_json::to_vec(&json!({
        "dependencies": {},
        "devDependencies": {
          "@tauri-apps/cli": "^2",
          "@litevar/vitul-cli": "^2",
          "vite": "^8"
        },
        "scripts": {
          "tauri": "tauri"
        }
      }))
      .unwrap(),
    )
    .unwrap();

    configure_frontend_dependencies(project.path()).unwrap();

    let package: serde_json::Value =
      serde_json::from_slice(&fs::read(project.path().join("package.json")).unwrap()).unwrap();
    assert!(package["devDependencies"].get("@tauri-apps/cli").is_none());
    assert_eq!(package["devDependencies"]["@litevar/vitul-cli"], "^3.4.0");
    assert_eq!(package["scripts"]["vitul"], "vitul");
    assert_eq!(package["scripts"]["check"], "tsc -b --pretty false");
    assert!(package["scripts"].get("tauri").is_none());
    assert_eq!(package["dependencies"]["agentos-sdk-ts"], "^2.1.0");
    assert_eq!(package["devDependencies"]["tailwindcss"], "^4.3.3");
    assert_eq!(package["devDependencies"]["@tailwindcss/vite"], "^4.3.3");
    assert!(package["devDependencies"].get("postcss").is_none());
    assert!(package["devDependencies"].get("autoprefixer").is_none());
    assert_eq!(package["devDependencies"]["@types/node"], "^22.19.0");
    assert_eq!(package["packageManager"], "pnpm@12.4.2");
    assert_eq!(package["engines"]["node"], ">=22.13.0");
  }

  #[test]
  fn project_names_are_derived_from_the_target_directory() {
    assert_eq!(package_name("time-memo"), "time-memo");
    assert_eq!(package_name("123 Time Memo"), "time-memo");
    assert_eq!(display_name("time-memo"), "Time Memo");

    let root = tempfile::tempdir().unwrap();
    let current = root.path().join("time-memo");
    fs::create_dir_all(&current).unwrap();
    assert_eq!(
      target_directory_name(&current, &current.join(".")).unwrap(),
      "time-memo"
    );
  }

  #[test]
  fn generated_names_update_frontend_rust_and_runtime_config() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("src-vitul/src")).unwrap();
    fs::write(root.path().join("package.json"), r#"{"name":"vitul-app"}"#).unwrap();
    fs::write(
      root.path().join("src-vitul/Cargo.toml"),
      "[package]\nname = \"vitul-app\"\n\n[lib]\nname = \"vitul_app_lib\"\n",
    )
    .unwrap();
    fs::write(
      root.path().join("src-vitul/src/main.rs"),
      "fn main() { vitul_app_lib::run() }\n",
    )
    .unwrap();
    fs::write(
      root.path().join("src-vitul/vitul.conf.json"),
      r#"{"productName":"old","app":{"windows":[{"title":"old"}]}}"#,
    )
    .unwrap();

    super::apply_project_names(root.path(), "time-memo", "Time Memo", "Clock").unwrap();

    let package: serde_json::Value =
      serde_json::from_slice(&fs::read(root.path().join("package.json")).unwrap()).unwrap();
    let config: serde_json::Value =
      serde_json::from_slice(&fs::read(root.path().join("src-vitul/vitul.conf.json")).unwrap())
        .unwrap();
    let cargo = fs::read_to_string(root.path().join("src-vitul/Cargo.toml")).unwrap();
    let main = fs::read_to_string(root.path().join("src-vitul/src/main.rs")).unwrap();
    assert_eq!(package["name"], "time-memo");
    assert!(cargo.contains("name = \"time-memo\""));
    assert!(cargo.contains("name = \"time_memo_lib\""));
    assert!(main.contains("time_memo_lib::run()"));
    assert_eq!(config["productName"], "Time Memo");
    assert_eq!(config["app"]["windows"][0]["title"], "Clock");
  }

  #[test]
  fn product_name_options_are_removed_before_project_creation() {
    let (remaining, product_name, window_title) = extract_display_options(vec![
      ".".into(),
      "--product-name=Time Memo".into(),
      "--window-title".into(),
      "Clock".into(),
    ])
    .unwrap();
    assert_eq!(remaining, vec![OsString::from(".")]);
    assert_eq!(product_name.as_deref(), Some("Time Memo"));
    assert_eq!(window_title.as_deref(), Some("Clock"));
  }
}

fn configure_desktop_plugins(project_root: &Path) -> Result<()> {
  let path = project_root.join("src-vitul/Cargo.toml");
  let mut cargo_toml =
    fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
  let opener = "tauri-plugin-opener = \"2\"";
  let plugin_dependencies = [
    "tauri-plugin-clipboard-manager = \"2\"",
    "tauri-plugin-dialog = \"2\"",
    "tauri-plugin-notification = \"2\"",
    "tauri-plugin-shell = \"2\"",
  ];
  if !cargo_toml.contains(opener) {
    bail!(
      "Vitul desktop scaffold is missing the `tauri-plugin-opener` dependency in src-vitul/Cargo.toml"
    );
  }
  for dependency in plugin_dependencies {
    if !cargo_toml.contains(dependency) {
      cargo_toml = cargo_toml.replace(opener, &format!("{opener}\n{dependency}"));
    }
  }
  let mobile_dependencies = "[target.'cfg(any(target_os = \"android\", target_os = \"ios\"))'.dependencies]\n\
vitul-backend = { package = \"vitul-backend\", path = \"../backend\" }\n\
axum = \"0.7\"\n\
futures-util = \"0.3\"\n\
tokio = { version = \"1\", features = [\"sync\"] }\n\
tower = { version = \"0.5\", features = [\"util\"] }\n\n";
  if !cargo_toml
    .contains("[target.'cfg(any(target_os = \"android\", target_os = \"ios\"))'.dependencies]")
  {
    if let Some(patch_section) = cargo_toml.find("[patch.crates-io]") {
      cargo_toml.insert_str(patch_section, mobile_dependencies);
    } else {
      cargo_toml.push_str("\n");
      cargo_toml.push_str(mobile_dependencies);
    }
  }
  fs::write(&path, cargo_toml).with_context(|| format!("failed to update {}", path.display()))
}

fn configure_capabilities(project_root: &Path) -> Result<()> {
  let path = project_root.join("src-vitul/capabilities/default.json");
  let mut capability: serde_json::Value = serde_json::from_slice(
    &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
  )
  .context("failed to parse generated Vitul host capability")?;
  let permissions = capability
    .get_mut("permissions")
    .and_then(serde_json::Value::as_array_mut)
    .context("generated Vitul host capability has no permissions array")?;
  for permission in [
    serde_json::json!("clipboard-manager:allow-read-text"),
    serde_json::json!("clipboard-manager:allow-write-text"),
    serde_json::json!("dialog:default"),
    serde_json::json!("notification:default"),
    serde_json::json!("shell:allow-kill"),
    serde_json::json!({
      "identifier": "shell:allow-spawn",
      "allow": [{ "name": "binaries/*", "sidecar": true, "args": true }]
    }),
  ] {
    if !permissions.contains(&permission) {
      permissions.push(permission);
    }
  }
  fs::write(
    path,
    serde_json::to_vec_pretty(&capability).context("failed to encode Vitul capability")?,
  )
  .context("failed to configure Vitul desktop host API permissions")
}

fn write_file(project_root: &Path, relative_path: &str, content: &str) -> Result<()> {
  let path = project_root.join(relative_path);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
  }
  fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))
}
