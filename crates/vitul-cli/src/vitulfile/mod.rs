mod schema;

pub mod inspect;
pub mod validate;

use anyhow::{Context, Result, bail};
use schema::{AppType, Vitulfile};
use std::{
  fs,
  path::{Path, PathBuf},
};

pub const FILE_NAME: &str = "Vitulfile";

pub use schema::{AppType as VitulfileAppType, Vitulfile as Config};

impl Vitulfile {
  pub fn load(project_root: &Path, file: Option<&Path>) -> Result<Self> {
    let path = file
      .map(|file| {
        if file.is_absolute() {
          file.to_path_buf()
        } else {
          project_root.join(file)
        }
      })
      .unwrap_or_else(|| project_root.join(FILE_NAME));
    if !path.is_file() {
      bail!(
        "{} is missing; run `vitul agentos init` to create it",
        path.display()
      );
    }
    let source =
      fs::read_to_string(&path).with_context(|| format!("could not read {}", path.display()))?;
    let config: Self = serde_yaml::from_str(&source)
      .with_context(|| format!("could not parse {}", path.display()))?;
    config.validate(project_root)?;
    Ok(config)
  }

  pub fn validate(&self, project_root: &Path) -> Result<()> {
    if self.schema_version != 1 {
      bail!("Vitulfile schemaVersion must be 1");
    }
    if self.target.is_some() && !self.targets.is_empty() {
      bail!("use either `target` or `targets`, not both");
    }
    let targets = self.declared_targets();
    if targets.is_empty() {
      bail!("Vitulfile must declare `target` or a non-empty `targets` list");
    }
    let mut unique_targets = std::collections::BTreeSet::new();
    for target in targets {
      if !matches!(target, "agentos/arm64" | "agentos/x64") {
        bail!("unsupported target {target}; expected agentos/arm64 or agentos/x64");
      }
      if !unique_targets.insert(target) {
        bail!("duplicate target `{target}` in Vitulfile");
      }
    }
    if !valid_bundle_id(&self.app.id) {
      bail!("app.id must be a reverse-DNS identifier, for example com.example.app");
    }
    if self.app.name.trim().is_empty() {
      bail!("app.name must not be empty");
    }
    if self.app.version.trim().is_empty() {
      bail!("app.version must not be empty");
    }
    if self.app.version.len() > 64
      || !self
        .app
        .version
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-' | b'_'))
    {
      bail!("app.version may contain only letters, numbers, `.`, `+`, `-`, and `_`");
    }
    if !matches!(self.agentos.open_mode.as_str(), "window" | "workspace") {
      bail!("agentos.openMode must be `window` or `workspace`");
    }
    if !matches!(self.agentos.title_bar.as_str(), "system" | "overlay") {
      bail!("agentos.titleBar must be `system` or `overlay`");
    }
    if !matches!(self.agentos.rpc_discovery.as_str(), "static" | "runtime") {
      bail!("agentos.rpcDiscovery must be `static` or `runtime`");
    }
    if self.agentos.rpc_discovery == "runtime" && self.app.app_type != AppType::Process {
      bail!("agentos.rpcDiscovery runtime requires a process app");
    }
    if self.agentos.default_width < 200
      || self.agentos.default_height < 200
      || self.agentos.min_width < 200
      || self.agentos.min_height < 200
      || self.agentos.min_width > self.agentos.default_width
      || self.agentos.min_height > self.agentos.default_height
    {
      bail!(
        "AgentOS window dimensions must be at least 200 pixels and min dimensions cannot exceed defaults"
      );
    }
    const SUPPORTED_PERMISSIONS: &[&str] = &[
      "native.window-control",
      "native.file-dialogs",
      "native.clipboard",
      "native.notifications",
      "native.external-links",
      "native.sidecars",
      "native.audio",
      "native.audio.exclusive",
      "native.tray",
    ];
    for permission in &self.agentos.permissions {
      if !SUPPORTED_PERMISSIONS.contains(&permission.as_str()) {
        bail!("unsupported AgentOS permission `{permission}`");
      }
    }
    if let Some(icon) = &self.app.icon {
      let icon_path = ensure_project_file(project_root, icon, "app.icon")?;
      if !icon_path.is_file()
        || !icon
          .extension()
          .and_then(|extension| extension.to_str())
          .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
      {
        bail!("app.icon must point to a PNG file");
      }
    }

    for (index, step) in self.build.steps.iter().enumerate() {
      if step.command.trim().is_empty() || step.command.contains('\0') {
        bail!("build.steps[{index}].command must not be empty or contain NUL characters");
      }
      if step.args.iter().any(|argument| argument.contains('\0')) {
        bail!("build.steps[{index}].args must not contain NUL characters");
      }
      if let Some(cwd) = &step.cwd {
        ensure_project_directory(project_root, cwd)
          .with_context(|| format!("invalid build.steps[{index}].cwd"))?;
      }
      validate_env(&step.env).with_context(|| format!("invalid build.steps[{index}].env"))?;
    }
    for (index, artifact) in self.build.artifacts.iter().enumerate() {
      if artifact.from.as_os_str().is_empty() {
        bail!("build.artifacts[{index}].from must not be empty");
      }
      validate_package_relative_path(&artifact.from)
        .with_context(|| format!("invalid build.artifacts[{index}].from"))?;
      validate_package_relative_path(&artifact.to)
        .with_context(|| format!("invalid build.artifacts[{index}].to"))?;
      let source = project_root.join(&artifact.from);
      if source.exists() {
        ensure_project_file(project_root, &artifact.from, "build artifact")?;
      } else if self.build.steps.is_empty() {
        bail!(
          "build artifact is missing and no build step will create it: {}",
          source.display()
        );
      }
    }

    match self.app.app_type {
      AppType::Process => {
        if !valid_rpc_base_path(&self.agentos.rpc_base_path) {
          bail!("agentos.rpcBasePath must be a safe absolute path");
        }
        if !(1..=65535).contains(&self.run.port) {
          bail!("run.port must be an integer from 1 to 65535");
        }
        if self.run.ready_timeout == 0 || self.run.ready_timeout > 600 {
          bail!("run.readyTimeout must be between 1 and 600 seconds");
        }
        if !valid_http_path(&self.run.health_check) {
          bail!("run.healthCheck must be an absolute HTTP path");
        }
        if !valid_http_path(&self.run.frontend_entry) {
          bail!("run.frontendEntry must be an absolute HTTP path");
        }
        if self
          .run
          .command
          .as_deref()
          .unwrap_or_default()
          .trim()
          .is_empty()
        {
          bail!("run.command is required when app.type is `process`");
        }
        if self.web.url.is_some() {
          bail!("web.url is only valid when app.type is `remote-web`");
        }
        if let Some(command) = &self.run.command {
          if command.contains('\0') || command.starts_with('/') || command.contains('\\') {
            bail!("run.command must not contain NUL and must be a relative POSIX command");
          }
          if command.starts_with("./") || command.starts_with("../") || command.contains('/') {
            let command_path = command.strip_prefix("./").unwrap_or(command);
            validate_package_relative_path(Path::new(command_path))
              .context("run.command package paths must stay inside the package")?;
            let command_path = Path::new(command_path);
            if !self.build.artifacts.iter().any(|artifact| {
              let target = artifact.to.to_string_lossy();
              let target = Path::new(target.strip_prefix("./").unwrap_or(&target));
              command_path.starts_with(target)
            }) {
              bail!("run.command package paths must be covered by a build.artifacts.to path");
            }
          }
        }
        if self.run.args.iter().any(|argument| argument.contains('\0')) {
          bail!("run.args must not contain NUL characters");
        }
        validate_env(&self.run.env).context("invalid run.env")?;
      }
      AppType::RemoteWeb => {
        if !self.build.steps.is_empty() || !self.build.artifacts.is_empty() {
          bail!("remote-web apps do not accept build steps or source artifacts");
        }
        if self.run.command.is_some() || !self.run.args.is_empty() {
          bail!("remote-web apps do not accept run.command or run.args");
        }
        if !self.run.env.is_empty() {
          bail!("remote-web apps do not accept run.env");
        }
        if !self.agentos.permissions.is_empty() {
          bail!(
            "remote-web apps cannot request AgentOS native permissions; remote CEF pages do not receive the native capability bridge"
          );
        }
        if self.data.persistent {
          bail!("remote-web apps do not use AgentOS process data storage");
        }
        let url = self
          .web
          .url
          .as_deref()
          .context("web.url is required when app.type is `remote-web`")?;
        let parsed = url::Url::parse(url).context("web.url must be a valid HTTPS URL")?;
        if parsed.scheme() != "https"
          || parsed.host_str().is_none()
          || !parsed.username().is_empty()
          || parsed.password().is_some()
        {
          bail!("web.url must be an HTTPS URL without embedded credentials");
        }
      }
    }
    Ok(())
  }
}

pub fn init(project_root: &Path, app_type: AppType, remote_url: Option<&str>) -> Result<PathBuf> {
  let path = project_root.join(FILE_NAME);
  if path.exists() {
    bail!("{} already exists", path.display());
  }
  let name = project_root
    .file_name()
    .and_then(|value| value.to_str())
    .filter(|value| !value.is_empty())
    .unwrap_or("my-app");
  let bundle_suffix = name
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
  let bundle_suffix = if bundle_suffix.is_empty() {
    "app"
  } else {
    &bundle_suffix
  };
  let remote_fields = match app_type {
    AppType::Process => "run:\n  command: ./bin/server\n  args: []\n  port: 9627\n  healthCheck: /health\n  readyTimeout: 60\n  frontendEntry: /\nbuild:\n  steps: []\n  artifacts:\n    - from: bin/server\n      to: bin/server\n".to_owned(),
    AppType::RemoteWeb => {
      let url = remote_url.context("--url is required for `remote-web`")?;
      let parsed = url::Url::parse(url).context("--url must be a valid HTTPS URL")?;
      if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
      {
        bail!("--url must use HTTPS and must not contain embedded credentials");
      }
      format!("web:\n  url: {}\n", yaml_string(url))
    }
  };
  let agentos_fields = match app_type {
    AppType::Process => {
      "agentos:\n  openMode: window\n  titleBar: system\n  permissions: []\n  rpcBasePath: /opentool\n  rpcDiscovery: static\n"
    }
    AppType::RemoteWeb => "agentos:\n  openMode: window\n  titleBar: system\n",
  };
  let contents = format!(
    "schemaVersion: 1\ntargets:\n  - agentos/arm64\napp:\n  type: {}\n  id: com.example.{}\n  name: {}\n  version: 0.1.0\n{}{}",
    match app_type {
      AppType::Process => "process",
      AppType::RemoteWeb => "remote-web",
    },
    bundle_suffix,
    yaml_string(name),
    remote_fields,
    agentos_fields,
  );
  fs::write(&path, contents).with_context(|| format!("could not write {}", path.display()))?;
  Ok(path)
}

fn yaml_string(value: &str) -> String {
  serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}

fn ensure_project_file(project_root: &Path, relative: &Path, label: &str) -> Result<PathBuf> {
  if relative.is_absolute() {
    bail!("{label} path must be relative to the project directory");
  }
  let resolved = project_root.join(relative);
  let canonical = resolved
    .canonicalize()
    .with_context(|| format!("{label} does not exist: {}", resolved.display()))?;
  let root = project_root
    .canonicalize()
    .context("could not resolve project directory")?;
  if !canonical.starts_with(root) {
    bail!(
      "{label} path must stay inside the project directory: {}",
      relative.display()
    );
  }
  Ok(canonical)
}

fn ensure_project_directory(project_root: &Path, relative: &Path) -> Result<PathBuf> {
  validate_package_relative_path(relative).context("path must be a safe project-relative path")?;
  let resolved = project_root.join(relative);
  let canonical = resolved
    .canonicalize()
    .with_context(|| format!("directory does not exist: {}", resolved.display()))?;
  let root = project_root
    .canonicalize()
    .context("could not resolve project directory")?;
  if !canonical.starts_with(root) || !canonical.is_dir() {
    bail!(
      "directory must stay inside the project directory: {}",
      relative.display()
    );
  }
  Ok(canonical)
}

fn validate_env(values: &std::collections::BTreeMap<String, String>) -> Result<()> {
  for (name, value) in values {
    if name.is_empty() || name.contains('=') || name.contains('\0') || value.contains('\0') {
      bail!("environment variable names and values must be valid OS strings");
    }
  }
  Ok(())
}

fn validate_package_relative_path(path: &Path) -> Result<()> {
  let normalized = path.to_string_lossy();
  let path = Path::new(normalized.strip_prefix("./").unwrap_or(&normalized));
  if path.is_absolute() || path.as_os_str().is_empty() || path.to_string_lossy().contains('\\') {
    bail!("path must be a non-empty relative package path");
  }
  if path
    .components()
    .any(|component| !matches!(component, std::path::Component::Normal(_)))
  {
    bail!("path must not contain `.` or `..` segments");
  }
  Ok(())
}

fn valid_bundle_id(value: &str) -> bool {
  if value.len() > 253 || value.trim() != value {
    return false;
  }
  let labels = value.split('.').collect::<Vec<_>>();
  labels.len() >= 2
    && labels.iter().all(|label| {
      !label.is_empty()
        && label.len() <= 63
        && label.as_bytes()[0].is_ascii_alphanumeric()
        && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
        && label
          .bytes()
          .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn valid_http_path(value: &str) -> bool {
  value.starts_with('/')
    && !value.starts_with("//")
    && value.len() <= 2048
    && !value.contains("..")
    && !value.contains('\\')
    && !value.contains('?')
    && !value.contains('#')
    && value.chars().all(|character| !character.is_control())
}

fn valid_rpc_base_path(value: &str) -> bool {
  value.len() <= 512
    && value.starts_with('/')
    && !value.starts_with("//")
    && !value.ends_with('/')
    && !value.contains("..")
    && !value.contains('\\')
    && !value.contains('?')
    && !value.contains('#')
    && value.strip_prefix('/').is_some_and(|path| {
      !path.is_empty()
        && path.split('/').all(|segment| {
          !segment.is_empty()
            && segment != "."
            && segment
              .bytes()
              .all(|byte| byte.is_ascii_alphanumeric() || b"._~-".contains(&byte))
        })
    })
}
