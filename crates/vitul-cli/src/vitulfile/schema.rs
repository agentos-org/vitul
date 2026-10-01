use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vitulfile {
  #[serde(rename = "schemaVersion")]
  pub schema_version: u32,
  pub target: String,
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
  #[serde(default = "default_rpc_base_path")]
  pub rpc_base_path: String,
}

impl Default for AgentOsConfig {
  fn default() -> Self {
    Self {
      title_bar: default_title_bar(),
      default_width: default_width(),
      default_height: default_height(),
      min_width: default_min_width(),
      min_height: default_min_height(),
      permissions: Vec::new(),
      rpc_base_path: default_rpc_base_path(),
    }
  }
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
