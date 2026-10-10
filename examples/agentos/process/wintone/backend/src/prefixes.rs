use std::{
  path::PathBuf,
  time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::runtime::InstalledRuntime;

#[derive(Clone)]
pub struct PrefixManager {
  data_dir: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationPrefix {
  pub installation_id: String,
  pub path: PathBuf,
  pub runtime_version: String,
  pub created_at: u128,
}

impl PrefixManager {
  pub fn new(data_dir: PathBuf) -> Self {
    Self { data_dir }
  }

  pub fn session_path(installation_id: &str) -> Result<String, String> {
    let id = Uuid::parse_str(installation_id)
      .map_err(|_| "Invalid installation identifier.".to_owned())?;
    Ok(format!("/data/prefixes/{id}"))
  }

  pub async fn path(&self, installation_id: &str) -> Result<PathBuf, String> {
    let id = Uuid::parse_str(installation_id)
      .map_err(|_| "Invalid installation identifier.".to_owned())?
      .to_string();
    let path = self.data_dir.join("prefixes").join(id);
    if !fs::try_exists(&path)
      .await
      .map_err(|error| error.to_string())?
    {
      return Err("Installation prefix is missing.".to_owned());
    }
    Ok(path)
  }

  pub async fn prepare(
    &self,
    installation_id: &str,
    runtime: &InstalledRuntime,
  ) -> Result<InstallationPrefix, String> {
    let id = Uuid::parse_str(installation_id)
      .map_err(|_| "Invalid installation identifier.".to_owned())?;
    let installation_id = id.to_string();
    let prefix_path = self.data_dir.join("prefixes").join(&installation_id);
    let metadata_dir = self.data_dir.join("installations");
    let metadata_path = metadata_dir.join(format!("{installation_id}.json"));

    let existing_bytes = match fs::read(&metadata_path).await {
      Ok(bytes) => Some(bytes),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
      Err(error) => return Err(error.to_string()),
    };
    if let Some(bytes) = existing_bytes {
      let existing: InstallationPrefix = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Invalid prefix record: {error}"))?;
      if existing.installation_id != installation_id || existing.path != prefix_path {
        return Err("Prefix record points outside its installation directory.".to_owned());
      }
      if existing.runtime_version != runtime.version {
        return Err(format!(
          "Installation prefix {} is tied to Wine/FEX runtime {}, not {}.",
          existing.installation_id, existing.runtime_version, runtime.version
        ));
      }
      if !fs::try_exists(&existing.path)
        .await
        .map_err(|error| error.to_string())?
      {
        return Err(format!(
          "Installation prefix {} is missing.",
          existing.installation_id
        ));
      }
      return Ok(existing);
    }

    fs::create_dir_all(&prefix_path)
      .await
      .map_err(|error| error.to_string())?;
    fs::create_dir_all(&metadata_dir)
      .await
      .map_err(|error| error.to_string())?;
    let record = InstallationPrefix {
      installation_id: installation_id.clone(),
      path: prefix_path,
      runtime_version: runtime.version.clone(),
      created_at: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis(),
    };
    let bytes = serde_json::to_vec_pretty(&record)
      .map_err(|error| format!("Could not encode prefix record: {error}"))?;
    let temporary_path = metadata_dir.join(format!(".{installation_id}.{}.tmp", Uuid::new_v4()));
    fs::write(&temporary_path, bytes)
      .await
      .map_err(|error| error.to_string())?;
    if let Err(error) = fs::rename(&temporary_path, &metadata_path).await {
      let _ = fs::remove_file(temporary_path).await;
      return Err(error.to_string());
    }
    Ok(record)
  }

  pub async fn remove(&self, installation_id: &str) -> Result<(), String> {
    let installation_id = Uuid::parse_str(installation_id)
      .map_err(|_| "Invalid installation identifier.".to_owned())?
      .to_string();
    let prefix_path = self.data_dir.join("prefixes").join(&installation_id);
    let metadata_path = self
      .data_dir
      .join("installations")
      .join(format!("{installation_id}.json"));

    match fs::remove_dir_all(&prefix_path).await {
      Ok(()) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(error) => return Err(error.to_string()),
    }
    match fs::remove_file(&metadata_path).await {
      Ok(()) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(error) => return Err(error.to_string()),
    }
    Ok(())
  }
}
