use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
  pub schema_version: u32,
  #[serde(default, alias = "stagedInstallers")]
  pub install_tasks: Vec<InstallTask>,
  #[serde(default)]
  pub installed_apps: Vec<InstalledApp>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallTask {
  pub id: String,
  pub file_name: String,
  pub extension: String,
  pub bytes: u64,
  pub sha256: String,
  pub added_at: u128,
  #[serde(default = "waiting_runtime")]
  pub status: String,
  #[serde(default = "runtime_phase")]
  pub phase: String,
  #[serde(default = "runtime_message")]
  pub message: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub session_id: Option<String>,
}

fn waiting_runtime() -> String {
  "waiting-runtime".to_owned()
}

fn runtime_phase() -> String {
  "runtime".to_owned()
}

fn runtime_message() -> String {
  "Install task is queued and will remain pending until the Wine/FEX runtime is connected."
    .to_owned()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
  pub id: String,
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub version: Option<String>,
  pub installation_id: String,
  pub executable: String,
  pub installed_at: u128,
  #[serde(default)]
  pub runtime_version: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub uninstall_executable: Option<String>,
  #[serde(default)]
  pub uninstall_args: Vec<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub session_id: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub session_kind: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub last_error: Option<String>,
}
