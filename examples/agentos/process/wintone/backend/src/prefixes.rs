use std::{
  path::PathBuf,
  time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::{pe_arch::WindowsArchitecture, runtime::InstalledRuntime};

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
  #[serde(default)]
  pub windows_architecture: WindowsArchitecture,
  pub created_at: u128,
}

impl PrefixManager {
  pub fn new(data_dir: PathBuf) -> Self {
    Self { data_dir }
  }

  pub fn session_path(installation_id: &str) -> Result<String, String> {
    Uuid::parse_str(installation_id).map_err(|_| "Invalid installation identifier.".to_owned())?;
    Ok("/grants/prefix".to_owned())
  }

  pub fn grant_source(installation_id: &str) -> Result<String, String> {
    let id = Uuid::parse_str(installation_id)
      .map_err(|_| "Invalid installation identifier.".to_owned())?;
    Ok(format!("prefixes/{id}"))
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
    windows_architecture: WindowsArchitecture,
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
      let mut existing: InstallationPrefix = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Invalid prefix record: {error}"))?;
      if existing.installation_id != installation_id || existing.path != prefix_path {
        return Err("Prefix record points outside its installation directory.".to_owned());
      }
      if existing.windows_architecture != windows_architecture {
        return Err(format!(
          "Installation prefix {} is configured for {}, not {}.",
          existing.installation_id,
          existing.windows_architecture.display_name(),
          windows_architecture.display_name()
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
      if existing.runtime_version != runtime.version {
        if !same_runtime_build(&existing.runtime_version, &runtime.version) {
          return Err(format!(
            "Installation prefix {} is tied to Windows runtime version {}, not {}.",
            existing.installation_id, existing.runtime_version, runtime.version
          ));
        }
        // Runtime bundle IDs include the bundle hash. A launcher-only fix can
        // change that hash without changing Wine or FEX, so keep the prefix and
        // advance its record when the runtime's version components are equal.
        existing.runtime_version = runtime.version.clone();
        write_prefix_record(&metadata_path, &metadata_dir, &existing).await?;
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
      windows_architecture,
      created_at: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis(),
    };
    write_prefix_record(&metadata_path, &metadata_dir, &record).await?;
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

fn same_runtime_build(existing: &str, current: &str) -> bool {
  fn version_without_bundle_hash(version: &str) -> Option<&str> {
    let (components, hash) = version.rsplit_once('-')?;
    (hash.len() == 12 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(components)
  }

  matches!(
    (
      version_without_bundle_hash(existing),
      version_without_bundle_hash(current)
    ),
    (Some(existing), Some(current)) if existing == current
  )
}

async fn write_prefix_record(
  metadata_path: &std::path::Path,
  metadata_dir: &std::path::Path,
  record: &InstallationPrefix,
) -> Result<(), String> {
  let bytes = serde_json::to_vec_pretty(record)
    .map_err(|error| format!("Could not encode prefix record: {error}"))?;
  let temporary_path = metadata_dir.join(format!(
    ".{}.{}.tmp",
    record.installation_id,
    Uuid::new_v4()
  ));
  fs::write(&temporary_path, bytes)
    .await
    .map_err(|error| error.to_string())?;
  if let Err(error) = fs::rename(&temporary_path, metadata_path).await {
    let _ = fs::remove_file(temporary_path).await;
    return Err(error.to_string());
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::runtime::InstalledRuntime;

  fn runtime(version: &str) -> InstalledRuntime {
    InstalledRuntime {
      version: version.to_owned(),
      host_architecture: "aarch64".to_owned(),
      wine_version: "9.0~repack-4build3".to_owned(),
      native_wine_version: Some("9.0~repack-4build3".to_owned()),
      fex_version: Some("2610-1~n".to_owned()),
      wine_command: vec!["usr/bin/wine".to_owned()],
      wineboot_command: vec!["usr/bin/wineboot".to_owned(), "-u".to_owned()],
      native_wine_command: None,
      native_wineboot_command: None,
      native_session_launcher: None,
      fex_command: None,
      fex_rootfs: None,
      artifact_sha256: String::new(),
      installed_at: 0,
    }
  }

  async fn make_prefix(root: &std::path::Path, id: &str, version: &str) {
    let prefix_path = root.join("prefixes").join(id);
    let metadata_dir = root.join("installations");
    fs::create_dir_all(&prefix_path).await.unwrap();
    fs::create_dir_all(&metadata_dir).await.unwrap();
    let record = InstallationPrefix {
      installation_id: id.to_owned(),
      path: prefix_path,
      runtime_version: version.to_owned(),
      windows_architecture: WindowsArchitecture::Arm64,
      created_at: 1,
    };
    fs::write(
      metadata_dir.join(format!("{id}.json")),
      serde_json::to_vec_pretty(&record).unwrap(),
    )
    .await
    .unwrap();
  }

  #[tokio::test]
  async fn prepare_migrates_prefix_when_only_bundle_hash_changes() {
    let root = std::env::temp_dir().join(format!("wintone-prefix-{}", Uuid::new_v4()));
    let id = Uuid::new_v4().to_string();
    let old_version = "arm64-fex-2610-1-n-w9.0-repack-4build3-n9.0-repack-4build3-405aaaa935db";
    let new_version = "arm64-fex-2610-1-n-w9.0-repack-4build3-n9.0-repack-4build3-bcdcbc813306";
    make_prefix(&root, &id, old_version).await;

    let record = PrefixManager::new(root.clone())
      .prepare(&id, &runtime(new_version), WindowsArchitecture::Arm64)
      .await
      .unwrap();

    assert_eq!(record.runtime_version, new_version);
    assert_eq!(record.path, root.join("prefixes").join(&id));
    let _ = fs::remove_dir_all(root).await;
  }

  #[tokio::test]
  async fn prepare_rejects_prefix_when_runtime_components_change() {
    let root = std::env::temp_dir().join(format!("wintone-prefix-{}", Uuid::new_v4()));
    let id = Uuid::new_v4().to_string();
    make_prefix(
      &root,
      &id,
      "arm64-fex-2610-1-n-w9.0-repack-4build3-n9.0-repack-4build3-405aaaa935db",
    )
    .await;

    let error = PrefixManager::new(root.clone())
      .prepare(
        &id,
        &runtime("arm64-fex-2610-1-n-w9.1-repack-4build3-n9.1-repack-4build3-bcdcbc813306"),
        WindowsArchitecture::Arm64,
      )
      .await
      .unwrap_err();

    assert!(error.contains("is tied to Windows runtime version"));
    let _ = fs::remove_dir_all(root).await;
  }
}
