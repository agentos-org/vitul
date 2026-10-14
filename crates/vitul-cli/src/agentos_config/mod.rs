pub(crate) mod network;
mod schema;

pub mod inspect;
pub mod validate;

use anyhow::{Context, Result, bail};
use schema::{AgentOsPackageConfig, AppType};
use std::path::{Path, PathBuf};

pub use schema::{AgentOsPackageConfig as Config, AppType as PackageAppType, ExecutionMode};

pub(crate) fn agentos_icon_filename(icon: &Path) -> Option<&'static str> {
  let extension = icon.extension()?.to_str()?;
  if extension.eq_ignore_ascii_case("png") {
    Some("icon.png")
  } else if extension.eq_ignore_ascii_case("svg") {
    Some("icon.svg")
  } else {
    None
  }
}

impl AgentOsPackageConfig {
  pub fn validate(&self, project_root: &Path) -> Result<()> {
    if self.schema_version != 1 {
      bail!("projected AgentOS config schemaVersion must be 1");
    }
    let targets = self.declared_targets();
    if targets.is_empty() {
      bail!("agentos.targets must contain at least one supported target");
    }
    let mut unique_targets = std::collections::BTreeSet::new();
    for target in targets {
      if !matches!(target, "agentos/arm64" | "agentos/x64") {
        bail!("unsupported target {target}; expected agentos/arm64 or agentos/x64");
      }
      if !unique_targets.insert(target) {
        bail!("duplicate target `{target}` in agentos.targets");
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
      "native.external-wayland",
      "native.external-wayland.network",
      "native.external-wayland.gpu",
      "native.external-wayland.clipboard.read",
      "native.external-wayland.clipboard.write",
      "native.providers",
      "native.network",
      "native.system-proxy",
      "native.tun",
      "native.system-deployment",
      "native.desktop-session",
    ];
    for permission in &self.agentos.permissions {
      if !SUPPORTED_PERMISSIONS.contains(&permission.as_str()) {
        bail!("unsupported AgentOS permission `{permission}`");
      }
    }
    let execution_mode = self.agentos.execution_mode();
    let has_network_config = self.agentos.network.is_some();
    let has_network_permission = self.agentos.permissions.iter().any(|permission| {
      matches!(
        permission.as_str(),
        "native.network" | "native.system-proxy" | "native.tun"
      )
    });
    if has_network_config {
      let network = self
        .agentos
        .network
        .as_ref()
        .expect("checked network config");
      network.validate()?;
      if self.app.app_type != AppType::Process {
        bail!(
          "agentos.network requires app.type `process`; remove the network declaration from remote-web apps"
        );
      }
      if execution_mode != schema::ExecutionMode::Native {
        bail!(
          "agentos.network requires agentos.execution.mode `native`; add the native profile v1"
        );
      }
      let native = self
        .agentos
        .execution
        .as_ref()
        .and_then(|execution| execution.native.as_ref())
        .context("agentos.network requires agentos.execution.native profile v1 with identity `desktop-user`")?;
      if native.version != 1 || native.identity != "desktop-user" {
        bail!(
          "agentos.network requires agentos.execution.native.version 1 and identity `desktop-user`; update the native profile"
        );
      }
      for required in ["native.providers", "native.network"] {
        if !self
          .agentos
          .permissions
          .iter()
          .any(|permission| permission == required)
        {
          bail!(
            "agentos.network requires agentos.permissions to include `{required}`; add it to the permissions array"
          );
        }
      }
      let has_proxy_profiles = !network.proxy_profiles.is_empty();
      let has_tun_profiles = !network.tun_profiles.is_empty();
      for (permission, profile_family, declared) in [
        ("native.system-proxy", "proxyProfiles", has_proxy_profiles),
        ("native.tun", "tunProfiles", has_tun_profiles),
      ] {
        let requested = self
          .agentos
          .permissions
          .iter()
          .any(|item| item == permission);
        if declared && !requested {
          bail!(
            "agentos.network.{profile_family} requires `{permission}` in agentos.permissions; add that permission"
          );
        }
        if requested && !declared {
          bail!(
            "agentos.permissions includes `{permission}` but agentos.network.{profile_family} is empty; remove the unused permission or declare a matching profile"
          );
        }
      }
    } else if has_network_permission {
      bail!(
        "agentos.permissions network permissions require agentos.network; add a protocol v1 network declaration or remove the unused permission"
      );
    }
    if let Some(execution) = &self.agentos.execution {
      if execution.mode == Some(schema::ExecutionMode::Native) && execution.native.is_none() {
        bail!(
          "agentos.execution.mode `native` requires the AgentOS native process profile v1 in agentos.execution.native"
        );
      }
      if let Some(native) = &execution.native {
        if execution.mode != Some(schema::ExecutionMode::Native) {
          bail!("agentos.execution.native requires agentos.execution.mode `native`");
        }
        if native.version != 1
          || native.identity != "desktop-user"
          || native.scope != "agentos-account"
          || !matches!(native.last_window_close.as_str(), "stop" | "background")
          || native.stop_grace_period_ms > 30000
        {
          bail!(
            "agentos.execution.native requires version 1, identity `desktop-user`, scope `agentos-account`, lastWindowClose `stop` or `background`, and stopGracePeriodMs from 0 to 30000"
          );
        }
        if !self.data.persistent {
          bail!("agentos.execution.native requires agentos.data.persistent: true");
        }
        const RESERVED_NATIVE_ENV: &[&str] = &[
          "HOME",
          "USER",
          "LOGNAME",
          "PATH",
          "TMPDIR",
          "XDG_RUNTIME_DIR",
          "DBUS_SESSION_BUS_ADDRESS",
          "DISPLAY",
          "WAYLAND_DISPLAY",
          "WAYLAND_SOCKET",
          "XAUTHORITY",
          "XDG_SESSION_ID",
          "AGENTOS_SESSION_ID",
          "AGENTOS_TRAY_SOCKET",
          "LD_PRELOAD",
          "LD_AUDIT",
        ];
        if let Some(name) = self
          .run
          .env
          .keys()
          .find(|name| RESERVED_NATIVE_ENV.contains(&name.as_str()))
        {
          bail!(
            "run.env.{name} is reserved by AgentOS and cannot be set for agentos.execution.native"
          );
        }
      }
    }
    if execution_mode == schema::ExecutionMode::Native {
      if self.app.app_type != AppType::Process {
        bail!("agentos.execution.mode native requires a process App");
      }
      if let Some(permission) = self.agentos.permissions.iter().find(|permission| {
        matches!(permission.as_str(), "native.audio" | "native.audio.exclusive")
      }) {
        bail!(
          "agentos.permissions includes `{permission}`, which is not available to desktop-user native Apps"
        );
      }
      if let Some(deployment) = &self.agentos.deployment {
        const CONTAINER_ONLY_ACTIONS: &[&str] = &[
          "linux.device.assign",
          "linux.bind-mount",
          "linux.lxc.ensure",
          "linux.lxc.lifecycle",
        ];
        if let Some(action) = deployment
          .get("operations")
          .and_then(serde_json::Value::as_array)
          .into_iter()
          .flatten()
          .flat_map(|operation| operation.get("actions").and_then(serde_json::Value::as_array))
          .into_iter()
          .flatten()
          .find(|action| {
            action
              .get("type")
              .and_then(serde_json::Value::as_str)
              .is_some_and(|kind| CONTAINER_ONLY_ACTIONS.contains(&kind))
          })
        {
          bail!(
            "desktop-user native deployment cannot request container-only action `{}`",
            action.get("type").and_then(serde_json::Value::as_str).unwrap_or("unknown")
          );
        }
      }
    }
    if self.app.app_type != AppType::Process && !self.agentos.providers.is_empty() {
      bail!("AgentOS Providers require a process app");
    }
    if !self.agentos.providers.is_empty()
      && !self
        .agentos
        .permissions
        .iter()
        .any(|permission| permission == "native.providers")
    {
      bail!("agentos.providers requires the native.providers permission");
    }
    if self
      .agentos
      .permissions
      .iter()
      .any(|permission| permission == "native.providers")
      && self.agentos.providers.is_empty()
      && !has_network_config
    {
      bail!("native.providers requires at least one agentos.providers declaration");
    }
    let mut provider_ids = std::collections::BTreeSet::new();
    for (index, provider) in self.agentos.providers.iter().enumerate() {
      let provider_labels = provider.id.split('.').collect::<Vec<_>>();
      let valid_provider_id = provider.id.len() <= 128
        && provider_labels.len() >= 2
        && provider_labels.iter().all(|label| {
          let bytes = label.as_bytes();
          bytes.len() <= 63
            && !bytes.is_empty()
            && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
            && (bytes[bytes.len() - 1].is_ascii_lowercase()
              || bytes[bytes.len() - 1].is_ascii_digit())
            && bytes
              .iter()
              .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        });
      if !valid_provider_id || !provider_ids.insert(provider.id.clone()) {
        bail!("agentos.providers[{index}].id must be a unique reverse-DNS identifier");
      }
      if let Some(executable) = &provider.executable {
        if executable.is_empty()
          || executable.starts_with('/')
          || executable.contains('\\')
          || executable
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
          || executable.contains('\0')
        {
          bail!("agentos.providers[{index}].executable must be a safe package-relative path");
        }
      }
      if provider.methods.is_empty() || provider.methods.len() > 64 {
        bail!("agentos.providers[{index}].methods must contain 1 to 64 method names");
      }
      let mut method_names = std::collections::BTreeSet::new();
      for method in &provider.methods {
        let method_parts = method.split('.').collect::<Vec<_>>();
        if method.len() > 128
          || method_parts.len() < 2
          || !method_parts.iter().all(|part| {
            let mut bytes = part.bytes();
            matches!(bytes.next(), Some(b'a'..=b'z'))
              && bytes
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
          })
          || !method_names.insert(method)
        {
          bail!("agentos.providers[{index}].methods contains an invalid or duplicate method");
        }
      }
    }
    validate_platform_providers(self)?;
    if let Some(icon) = &self.app.icon {
      let icon_path = ensure_project_file(project_root, icon, "app.icon")?;
      if !icon_path.is_file() || agentos_icon_filename(icon).is_none() {
        bail!("app.icon must point to a PNG or SVG file");
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
      let mut artifact_targets = std::collections::BTreeSet::new();
      for target in &artifact.targets {
        if !matches!(target.as_str(), "agentos/arm64" | "agentos/x64")
          || !self.declared_targets().contains(&target.as_str())
          || !artifact_targets.insert(target)
        {
          bail!("build.artifacts[{index}].targets must contain unique declared AgentOS targets");
        }
      }
      let source = project_root.join(&artifact.from);
      if source.exists() {
        ensure_project_file(project_root, &artifact.from, "build artifact")?;
      } else if self.build.steps.is_empty()
        && !crate::project_config::is_generated_backend_artifact(project_root, &artifact.from)
      {
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
        if self.run.auto_start {
          bail!("manifest v1 process apps require run.autoStart false");
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

fn validate_platform_providers(config: &AgentOsPackageConfig) -> Result<()> {
  const DEPLOYMENT_ID: &str = "org.agentos.system-deployment";
  const DESKTOP_SESSION_ID: &str = "org.agentos.desktop-session";
  const EXTERNAL_WAYLAND_ID: &str = "org.agentos.external-wayland";
  const NETWORK_ID: &str = "org.agentos.network";
  const EXTERNAL_WAYLAND_METHODS: &[&str] = &[
    "session.start",
    "session.prepare_restart",
    "session.restart",
    "session.stop",
  ];
  const DEPLOYMENT_METHODS: &[&str] = &[
    "capabilities.get",
    "operation.plan",
    "authorization.status",
    "task.get",
    "task.cancel",
    "task.events",
    "resources.list",
  ];
  let has_permission = |permission: &str| {
    config
      .agentos
      .permissions
      .iter()
      .any(|item| item == permission)
  };
  let find_provider = |id: &str| {
    config
      .agentos
      .providers
      .iter()
      .find(|provider| provider.id == id)
  };

  let system_deployment = find_provider(DEPLOYMENT_ID);
  let desktop_session = find_provider(DESKTOP_SESSION_ID);
  let external_wayland = find_provider(EXTERNAL_WAYLAND_ID);
  if config.agentos.network.is_some() && find_provider(NETWORK_ID).is_some() {
    bail!("org.agentos.network is generated from agentos.network; remove its manual provider declaration");
  }
  if config.agentos.network.is_none() && find_provider(NETWORK_ID).is_some() {
    bail!("org.agentos.network requires an agentos.network declaration");
  }
  if let Some(provider) = external_wayland {
    let declared = provider.methods.iter().map(String::as_str).collect::<std::collections::BTreeSet<_>>();
    let expected = EXTERNAL_WAYLAND_METHODS.iter().copied().collect::<std::collections::BTreeSet<_>>();
    if provider.executable.is_some()
      || declared != expected
      || !has_permission("native.external-wayland")
    {
      bail!("{EXTERNAL_WAYLAND_ID} requires native.external-wayland and its fixed built-in methods without an executable");
    }
  }
  match (
    &config.agentos.deployment,
    has_permission("native.system-deployment"),
  ) {
    (Some(declaration), true) => {
      if config.app.app_type != AppType::Process
        || !has_permission("native.providers")
        || declaration
          .get("protocolVersion")
          .and_then(serde_json::Value::as_u64)
          != Some(1)
        || declaration
          .get("operations")
          .and_then(serde_json::Value::as_array)
          .is_none_or(|operations| operations.is_empty() || operations.len() > 32)
      {
        bail!(
          "agentos.deployment requires a process App, protocolVersion 1, 1-32 operations, and native.providers/native.system-deployment"
        );
      }
      let Some(provider) = system_deployment else {
        bail!("native.system-deployment requires the built-in {DEPLOYMENT_ID} Provider");
      };
      if provider.executable.is_some()
        || DEPLOYMENT_METHODS
          .iter()
          .any(|method| !provider.methods.iter().any(|declared| declared == method))
        || provider
          .methods
          .iter()
          .any(|method| !DEPLOYMENT_METHODS.contains(&method.as_str()))
      {
        bail!("{DEPLOYMENT_ID} must register its fixed built-in method set without an executable");
      }
    }
    (Some(_), false) => bail!("agentos.deployment requires native.system-deployment"),
    (None, true) => bail!("native.system-deployment requires agentos.deployment"),
    (None, false) => {
      if let Some(provider) = system_deployment {
        if provider.executable.is_some() || provider.methods.as_slice() != ["capabilities.get"] {
          bail!("{DEPLOYMENT_ID} may declare only capabilities.get without agentos.deployment");
        }
      }
    }
  }

  if has_permission("native.desktop-session") {
    let Some(provider) = desktop_session else {
      bail!("native.desktop-session requires the built-in {DESKTOP_SESSION_ID} Provider");
    };
    if provider.executable.is_some() || provider.methods.as_slice() != ["session.get"] {
      bail!("{DESKTOP_SESSION_ID} must register only its built-in session.get method");
    }
  } else if desktop_session.is_some() {
    bail!("{DESKTOP_SESSION_ID} requires native.desktop-session");
  }
  Ok(())
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
