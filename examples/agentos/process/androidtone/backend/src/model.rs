use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
  pub schema_version: u32,
  #[serde(default)]
  pub install_tasks: Vec<InstallTask>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallTask {
  pub id: String,
  pub file_name: String,
  pub bytes: u64,
  pub sha256: String,
  pub added_at: u128,
  pub status: String,
  pub message: String,
}
