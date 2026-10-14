use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentOsPackageConfig {
  #[serde(rename = "schemaVersion")]
  pub schema_version: u32,
  #[serde(default)]
  pub targets: Vec<String>,
  pub app: AppConfig,
  #[serde(default)]
  pub build: BuildConfig,
  #[serde(default)]
  pub run: RunConfig,
  #[serde(default)]
  pub web: WebConfig,
  #[serde(default)]
  pub agentos: AgentOsConfig,
  #[serde(default)]
  pub data: DataConfig,
}

impl AgentOsPackageConfig {
  pub fn declared_targets(&self) -> Vec<&str> {
    self.targets.iter().map(String::as_str).collect()
  }

  pub fn target_summary(&self) -> String {
    self.declared_targets().join(", ")
  }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
  #[serde(rename = "type")]
  pub app_type: AppType,
  pub id: String,
  pub name: String,
  #[serde(default)]
  pub name_zh: Option<String>,
  pub version: String,
  #[serde(default)]
  pub description: Option<String>,
  #[serde(default)]
  pub description_zh: Option<String>,
  #[serde(default)]
  pub icon: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AppType {
  Process,
  RemoteWeb,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
  #[serde(default)]
  pub steps: Vec<BuildStep>,
  #[serde(default)]
  pub artifacts: Vec<Artifact>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildStep {
  pub command: String,
  #[serde(default)]
  pub args: Vec<String>,
  #[serde(default)]
  pub cwd: Option<PathBuf>,
  #[serde(default)]
  pub env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
  pub from: PathBuf,
  pub to: PathBuf,
  #[serde(default)]
  pub targets: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RunConfig {
  #[serde(default)]
  pub command: Option<String>,
  #[serde(default)]
  pub args: Vec<String>,
  #[serde(default = "default_port")]
  pub port: u16,
  #[serde(default = "default_health_check")]
  pub health_check: String,
  #[serde(default = "default_ready_timeout")]
  pub ready_timeout: u32,
  #[serde(default = "default_frontend_entry")]
  pub frontend_entry: String,
  #[serde(default)]
  pub auto_start: bool,
  #[serde(default)]
  pub env: BTreeMap<String, String>,
}

impl Default for RunConfig {
  fn default() -> Self {
    Self {
      command: None,
      args: Vec::new(),
      port: default_port(),
      health_check: default_health_check(),
      ready_timeout: default_ready_timeout(),
      frontend_entry: default_frontend_entry(),
      auto_start: false,
      env: BTreeMap::new(),
    }
  }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebConfig {
  #[serde(default)]
  pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AgentOsConfig {
  #[serde(default = "default_open_mode")]
  pub open_mode: String,
  #[serde(default = "default_title_bar")]
  pub title_bar: String,
  #[serde(default = "default_width")]
  pub default_width: u32,
  #[serde(default = "default_height")]
  pub default_height: u32,
  #[serde(default = "default_min_width")]
  pub min_width: u32,
  #[serde(default = "default_min_height")]
  pub min_height: u32,
  #[serde(default)]
  pub permissions: Vec<String>,
  /// Host Providers this process App registers with AgentOS. Each declaration
  /// narrows the methods; executable providers run as the desktop user.
  #[serde(default)]
  pub providers: Vec<ProviderConfig>,
  /// Versioned, declarative system operations required by this process App.
  /// AgentOS validates and authorizes the closed action set at install time.
  #[serde(default)]
  pub deployment: Option<serde_json::Value>,
  /// Closed AgentOS Native App network declaration, protocol v1.
  #[serde(default)]
  pub network: Option<NetworkConfig>,
  /// Where AgentOS runs the process. Omitted or empty means container.
  #[serde(default)]
  pub execution: Option<ExecutionConfig>,
  #[serde(default = "default_rpc_base_path")]
  pub rpc_base_path: String,
  #[serde(default = "default_rpc_discovery")]
  pub rpc_discovery: String,
}

impl Default for AgentOsConfig {
  fn default() -> Self {
    Self {
      open_mode: default_open_mode(),
      title_bar: default_title_bar(),
      default_width: default_width(),
      default_height: default_height(),
      min_width: default_min_width(),
      min_height: default_min_height(),
      permissions: Vec::new(),
      providers: Vec::new(),
      deployment: None,
      network: None,
      execution: None,
      rpc_base_path: default_rpc_base_path(),
      rpc_discovery: default_rpc_discovery(),
    }
  }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NetworkConfig {
  pub protocol_version: u32,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub executables: Vec<NetworkExecutable>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub proxy_profiles: Vec<NetworkProxyProfile>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub tun_profiles: Vec<NetworkTunProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NetworkExecutable {
  pub id: String,
  pub path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NetworkProxyProfile {
  pub id: String,
  pub executable: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub http: Option<NetworkProxyEndpoint>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub https: Option<NetworkProxyEndpoint>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub socks: Option<NetworkProxyEndpoint>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub bypass: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NetworkProxyEndpoint {
  pub host: String,
  pub port: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NetworkTunProfile {
  pub id: String,
  pub executable: String,
  pub arguments: Vec<String>,
  pub configuration: NetworkConfigurationFile,
  pub interface_name: String,
  pub outer_endpoints: Vec<NetworkTunnelEndpoint>,
  pub ipv4: bool,
  pub ipv6: bool,
  pub dns_servers: Vec<String>,
  #[serde(default = "default_network_exit_policy")]
  pub exit_policy: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NetworkConfigurationFile {
  pub path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NetworkTunnelEndpoint {
  pub address: String,
  pub port: u32,
  pub protocol: String,
}

fn default_network_exit_policy() -> String {
  "restore-previous".to_owned()
}

impl AgentOsConfig {
  pub fn execution_mode(&self) -> ExecutionMode {
    self
      .execution
      .as_ref()
      .and_then(|execution| execution.mode)
      .unwrap_or_default()
  }

  pub fn execution_mode_name(&self) -> &'static str {
    match self.execution_mode() {
      ExecutionMode::Container => "container",
      ExecutionMode::Native => "native",
    }
  }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExecutionConfig {
  pub mode: Option<ExecutionMode>,
  #[serde(default)]
  pub native: Option<NativeExecutionConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NativeExecutionConfig {
  pub version: u32,
  pub identity: String,
  pub scope: String,
  #[serde(default = "default_native_last_window_close")]
  pub last_window_close: String,
  #[serde(default = "default_native_stop_grace_period_ms")]
  pub stop_grace_period_ms: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecutionMode {
  #[default]
  Container,
  Native,
}

impl<'de> Deserialize<'de> for ExecutionMode {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let mode = String::deserialize(deserializer)?;
    match mode.as_str() {
      "" | "container" => Ok(Self::Container),
      "native" => Ok(Self::Native),
      _ => Err(serde::de::Error::unknown_variant(
        &mode,
        &["container", "native"],
      )),
    }
  }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProviderConfig {
  pub id: String,
  #[serde(default)]
  pub methods: Vec<String>,
  /// Optional package-relative executable implementing this Provider.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub executable: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataConfig {
  #[serde(default)]
  pub persistent: bool,
}

fn default_port() -> u16 {
  9627
}

fn default_health_check() -> String {
  "/health".to_owned()
}

fn default_ready_timeout() -> u32 {
  60
}

fn default_frontend_entry() -> String {
  "/".to_owned()
}

fn default_title_bar() -> String {
  "system".to_owned()
}

fn default_open_mode() -> String {
  "window".to_owned()
}

fn default_width() -> u32 {
  960
}

fn default_height() -> u32 {
  680
}

fn default_min_width() -> u32 {
  520
}

fn default_min_height() -> u32 {
  360
}

fn default_rpc_base_path() -> String {
  "/opentool".to_owned()
}

fn default_rpc_discovery() -> String {
  "static".to_owned()
}

fn default_native_last_window_close() -> String {
  "stop".to_owned()
}

fn default_native_stop_grace_period_ms() -> u32 {
  5000
}
