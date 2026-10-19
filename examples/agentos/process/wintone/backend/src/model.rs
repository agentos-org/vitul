use serde::{Deserialize, Serialize};

use crate::pe_arch::WindowsArchitecture;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
  pub schema_version: u32,
  #[serde(default, alias = "stagedInstallers")]
  pub install_tasks: Vec<InstallTask>,
  #[serde(default)]
  pub installed_apps: Vec<InstalledApp>,
  #[serde(default)]
  pub session_preferences: SessionPreferences,
  #[serde(default)]
  pub session_preferences_configured: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPreferences {
  #[serde(default)]
  pub gpu: bool,
  #[serde(default)]
  pub clipboard_read: bool,
  #[serde(default)]
  pub clipboard_write: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallTask {
  pub id: String,
  pub file_name: String,
  pub extension: String,
  #[serde(default = "installer_task_kind")]
  pub kind: String,
  pub bytes: u64,
  pub sha256: String,
  pub added_at: u128,
  #[serde(default)]
  pub windows_architecture: WindowsArchitecture,
  #[serde(default = "waiting_runtime")]
  pub status: String,
  #[serde(default = "runtime_phase")]
  pub phase: String,
  #[serde(default = "runtime_message")]
  pub message: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub session_id: Option<String>,
}

fn installer_task_kind() -> String {
  "installer".to_owned()
}

fn waiting_runtime() -> String {
  "waiting-runtime".to_owned()
}

fn runtime_phase() -> String {
  "runtime".to_owned()
}

fn runtime_message() -> String {
  "Install task is waiting for the Windows runtime to be ready.".to_owned()
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
  pub windows_architecture: WindowsArchitecture,
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
