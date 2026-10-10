use std::{
  fs::File,
  path::{Component, Path, PathBuf},
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{fs, io::AsyncWriteExt, sync::Mutex};
use uuid::Uuid;

const MAX_RUNTIME_BYTES: u64 = 20 * 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct RuntimeManager {
  root: PathBuf,
  catalog_path: PathBuf,
  prepare_lock: Arc<Mutex<()>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
  pub state: String,
  pub wine_version: Option<String>,
  pub fex_version: Option<String>,
  pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRuntime {
  pub version: String,
  pub host_architecture: String,
  pub wine_version: String,
  pub fex_version: Option<String>,
  pub wine_command: Vec<String>,
  pub wineboot_command: Vec<String>,
  pub fex_command: Option<Vec<String>>,
  pub fex_rootfs: Option<String>,
  pub artifact_sha256: String,
  pub installed_at: u128,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCatalog {
  schema_version: u32,
  version: String,
  host_architecture: String,
  wine_version: String,
  fex_version: Option<String>,
  artifact: RuntimeArtifact,
  wine_command: Vec<String>,
  wineboot_command: Vec<String>,
  fex_command: Option<Vec<String>>,
  fex_rootfs: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeArtifact {
  url: String,
  format: String,
  sha256: String,
  max_bytes: u64,
}

#[derive(Debug)]
pub struct RuntimeError(pub String);

impl RuntimeManager {
  pub fn new(data_dir: PathBuf, catalog_path: PathBuf) -> Self {
    Self {
      root: data_dir.join("runtimes"),
      catalog_path,
      prepare_lock: Arc::new(Mutex::new(())),
    }
  }

  pub async fn status(&self) -> RuntimeStatus {
    match self.installed_runtime().await {
      Ok(Some(runtime)) => {
        return RuntimeStatus {
          state: "ready".to_owned(),
          wine_version: Some(runtime.wine_version),
          fex_version: runtime.fex_version,
          message: "The shared Wine/FEX runtime is ready.".to_owned(),
        };
      }
      Ok(None) => {}
      Err(error) => {
        return RuntimeStatus {
          state: "runtime-invalid".to_owned(),
          wine_version: None,
          fex_version: None,
          message: error.0,
        };
      }
    }

    match self.read_catalog().await {
      Ok(catalog) => match self.validate_catalog(&catalog) {
        Ok(()) => RuntimeStatus {
          state: "not-installed".to_owned(),
          wine_version: Some(catalog.wine_version),
          fex_version: catalog.fex_version,
          message:
            "A SHA-256-pinned runtime package is configured and will be prepared on first use."
              .to_owned(),
        },
        Err(error) => RuntimeStatus {
          state: "catalog-invalid".to_owned(),
          wine_version: None,
          fex_version: None,
          message: error.0,
        },
      },
      Err(error) => RuntimeStatus {
        state: "catalog-unavailable".to_owned(),
        wine_version: None,
        fex_version: None,
        message: error.0,
      },
    }
  }

  pub async fn prepare(&self) -> Result<InstalledRuntime, RuntimeError> {
    let _guard = self.prepare_lock.lock().await;
    if let Some(runtime) = self.installed_runtime().await? {
      return Ok(runtime);
    }

    let catalog = self.read_catalog().await?;
    self.validate_catalog(&catalog)?;
    fs::create_dir_all(self.root.join("versions"))
      .await
      .map_err(io_error)?;

    let operation_id = Uuid::new_v4().to_string();
    let archive_path = self.root.join(format!(".download-{operation_id}.tar.zst"));
    let staging_path = self.root.join(format!(".staging-{operation_id}"));
    let result = async {
      self
        .download_artifact(&catalog.artifact, &archive_path)
        .await?;
      let archive_path_for_extract = archive_path.clone();
      let staging_path_for_extract = staging_path.clone();
      tokio::task::spawn_blocking(move || {
        unpack_tar_zstd(&archive_path_for_extract, &staging_path_for_extract)
      })
      .await
      .map_err(|error| RuntimeError(format!("Runtime extraction task failed: {error}")))??;

      let runtime = Self::installed_record(&catalog)?;
      self.validate_layout(&runtime, &staging_path).await?;

      let version_path = self.root.join("versions").join(&catalog.version);
      if fs::try_exists(&version_path).await.map_err(io_error)? {
        fs::remove_dir_all(&version_path).await.map_err(io_error)?;
      }
      fs::rename(&staging_path, &version_path)
        .await
        .map_err(io_error)?;
      self.write_active_runtime(&runtime).await?;
      Ok(runtime)
    }
    .await;

    let _ = fs::remove_file(&archive_path).await;
    if result.is_err() {
      let _ = fs::remove_dir_all(&staging_path).await;
    }
    result
  }

  pub async fn installed_runtime(&self) -> Result<Option<InstalledRuntime>, RuntimeError> {
    let active_path = self.root.join("active.json");
    let bytes = match fs::read(active_path).await {
      Ok(bytes) => bytes,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(io_error(error)),
    };
    let runtime: InstalledRuntime = serde_json::from_slice(&bytes)
      .map_err(|error| RuntimeError(format!("Invalid installed-runtime record: {error}")))?;
    if !valid_version(&runtime.version) {
      return Err(RuntimeError(
        "Invalid installed runtime version.".to_owned(),
      ));
    }
    if runtime.host_architecture != std::env::consts::ARCH {
      return Err(RuntimeError(format!(
        "Installed runtime targets {}, but this AgentOS process runs on {}.",
        runtime.host_architecture,
        std::env::consts::ARCH
      )));
    }
    let runtime_dir = self.runtime_dir(&runtime);
    self.validate_layout(&runtime, &runtime_dir).await?;
    Ok(Some(runtime))
  }

  pub fn runtime_dir(&self, runtime: &InstalledRuntime) -> PathBuf {
    self.root.join("versions").join(&runtime.version)
  }

  pub fn resolve_command(
    &self,
    runtime: &InstalledRuntime,
    command: &[String],
  ) -> Result<(PathBuf, Vec<String>), RuntimeError> {
    let (program, args) = command
      .split_first()
      .ok_or_else(|| RuntimeError("Runtime command is empty.".to_owned()))?;
    let path = safe_relative_path(program)?;
    Ok((self.runtime_dir(runtime).join(path), args.to_vec()))
  }

  pub fn session_command(
    &self,
    runtime: &InstalledRuntime,
    command: &[String],
  ) -> Result<(String, Vec<String>), RuntimeError> {
    let (program, args) = command
      .split_first()
      .ok_or_else(|| RuntimeError("Runtime command is empty.".to_owned()))?;
    let path = safe_relative_path(program)?;
    let relative = path
      .components()
      .map(|component| component.as_os_str().to_string_lossy())
      .collect::<Vec<_>>()
      .join("/");
    Ok((
      format!("/data/runtimes/versions/{}/{relative}", runtime.version),
      args.to_vec(),
    ))
  }

  pub fn session_data_path(
    &self,
    runtime: &InstalledRuntime,
    relative_path: &str,
  ) -> Result<String, RuntimeError> {
    let path = safe_relative_path(relative_path)?;
    let relative = path
      .components()
      .map(|component| component.as_os_str().to_string_lossy())
      .collect::<Vec<_>>()
      .join("/");
    Ok(format!(
      "/data/runtimes/versions/{}/{relative}",
      runtime.version
    ))
  }

  async fn read_catalog(&self) -> Result<RuntimeCatalog, RuntimeError> {
    let bytes = fs::read(&self.catalog_path).await.map_err(|error| {
      if error.kind() == std::io::ErrorKind::NotFound {
        RuntimeError(format!(
          "No Wine/FEX runtime catalog is configured at {}.",
          self.catalog_path.display()
        ))
      } else {
        io_error(error)
      }
    })?;
    serde_json::from_slice(&bytes)
      .map_err(|error| RuntimeError(format!("Invalid runtime catalog: {error}")))
  }

  fn validate_catalog(&self, catalog: &RuntimeCatalog) -> Result<(), RuntimeError> {
    if catalog.schema_version != 1 {
      return Err(RuntimeError(format!(
        "Unsupported runtime catalog schema version: {}.",
        catalog.schema_version
      )));
    }
    if !valid_version(&catalog.version) {
      return Err(RuntimeError(
        "Invalid runtime version identifier.".to_owned(),
      ));
    }
    if catalog.host_architecture != std::env::consts::ARCH {
      return Err(RuntimeError(format!(
        "Runtime targets {}, but this AgentOS process runs on {}.",
        catalog.host_architecture,
        std::env::consts::ARCH
      )));
    }
    if catalog.wine_version.trim().is_empty()
      || catalog.wine_command.is_empty()
      || catalog.wineboot_command.is_empty()
    {
      return Err(RuntimeError(
        "The runtime catalog must specify Wine versions and launch commands.".to_owned(),
      ));
    }
    if catalog.host_architecture == "aarch64" && catalog.fex_command.is_none() {
      return Err(RuntimeError(
        "An ARM64 runtime must include a Linux FEX command.".to_owned(),
      ));
    }
    if catalog.host_architecture == "aarch64"
      && catalog
        .fex_version
        .as_deref()
        .is_none_or(|version| version.trim().is_empty())
    {
      return Err(RuntimeError(
        "An ARM64 runtime must specify its FEX version.".to_owned(),
      ));
    }
    if catalog.host_architecture == "aarch64" {
      let rootfs = catalog.fex_rootfs.as_deref().ok_or_else(|| {
        RuntimeError("An ARM64 Linux FEX command requires a packaged x86-64 RootFS.".to_owned())
      })?;
      safe_relative_path(rootfs)?;
    }
    if catalog
      .fex_command
      .as_ref()
      .is_some_and(|command| command.is_empty())
    {
      return Err(RuntimeError("FEX command is empty.".to_owned()));
    }
    validate_command_paths(&catalog.wine_command)?;
    validate_command_paths(&catalog.wineboot_command)?;
    if let Some(command) = &catalog.fex_command {
      validate_command_paths(command)?;
    }
    if catalog.artifact.format != "tar.zst" {
      return Err(RuntimeError(
        "Only tar.zst runtime bundles are supported.".to_owned(),
      ));
    }
    let artifact_url = reqwest::Url::parse(&catalog.artifact.url)
      .map_err(|error| RuntimeError(format!("Invalid runtime bundle URL: {error}")))?;
    if artifact_url.scheme() != "https"
      || artifact_url.host_str().is_none()
      || !artifact_url.username().is_empty()
      || artifact_url.password().is_some()
    {
      return Err(RuntimeError(
        "Runtime bundle URLs must use HTTPS and may not contain embedded credentials.".to_owned(),
      ));
    }
    if !valid_sha256(&catalog.artifact.sha256) {
      return Err(RuntimeError(
        "Runtime bundle SHA-256 must contain 64 hexadecimal characters.".to_owned(),
      ));
    }
    if catalog.artifact.max_bytes == 0 || catalog.artifact.max_bytes > MAX_RUNTIME_BYTES {
      return Err(RuntimeError(format!(
        "Runtime bundle size must be between 1 byte and {} GiB.",
        MAX_RUNTIME_BYTES / 1024 / 1024 / 1024
      )));
    }
    Ok(())
  }

  async fn download_artifact(
    &self,
    artifact: &RuntimeArtifact,
    destination: &Path,
  ) -> Result<(), RuntimeError> {
    let client = reqwest::Client::builder()
      .connect_timeout(std::time::Duration::from_secs(30))
      .build()
      .map_err(|error| RuntimeError(format!("Could not initialize download client: {error}")))?;
    let mut response = client
      .get(&artifact.url)
      .send()
      .await
      .map_err(|error| RuntimeError(format!("Runtime download failed: {error}")))?
      .error_for_status()
      .map_err(|error| RuntimeError(format!("Runtime download failed: {error}")))?;
    if response.url().scheme() != "https" {
      return Err(RuntimeError(
        "Runtime download redirected away from HTTPS.".to_owned(),
      ));
    }
    if response
      .content_length()
      .is_some_and(|size| size > artifact.max_bytes)
    {
      return Err(RuntimeError(
        "Runtime bundle exceeds its catalog size limit.".to_owned(),
      ));
    }

    let mut output = fs::File::create(destination).await.map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    while let Some(chunk) = response
      .chunk()
      .await
      .map_err(|error| RuntimeError(format!("Runtime download failed: {error}")))?
    {
      downloaded = downloaded.saturating_add(chunk.len() as u64);
      if downloaded > artifact.max_bytes {
        drop(output);
        let _ = fs::remove_file(destination).await;
        return Err(RuntimeError(
          "Runtime bundle exceeds its catalog size limit.".to_owned(),
        ));
      }
      hasher.update(&chunk);
      output.write_all(&chunk).await.map_err(io_error)?;
    }
    output.flush().await.map_err(io_error)?;
    drop(output);
    if format!("{:x}", hasher.finalize()) != artifact.sha256.to_ascii_lowercase() {
      let _ = fs::remove_file(destination).await;
      return Err(RuntimeError(
        "Runtime bundle SHA-256 does not match the catalog.".to_owned(),
      ));
    }
    Ok(())
  }

  fn installed_record(catalog: &RuntimeCatalog) -> Result<InstalledRuntime, RuntimeError> {
    Ok(InstalledRuntime {
      version: catalog.version.clone(),
      host_architecture: catalog.host_architecture.clone(),
      wine_version: catalog.wine_version.clone(),
      fex_version: catalog.fex_version.clone(),
      wine_command: catalog.wine_command.clone(),
      wineboot_command: catalog.wineboot_command.clone(),
      fex_command: catalog.fex_command.clone(),
      fex_rootfs: catalog.fex_rootfs.clone(),
      artifact_sha256: catalog.artifact.sha256.to_ascii_lowercase(),
      installed_at: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis(),
    })
  }

  async fn validate_layout(
    &self,
    runtime: &InstalledRuntime,
    runtime_dir: &Path,
  ) -> Result<(), RuntimeError> {
    for command in [&runtime.wine_command, &runtime.wineboot_command] {
      self.validate_executable(runtime_dir, command).await?;
    }
    if let Some(command) = &runtime.fex_command {
      self.validate_executable(runtime_dir, command).await?;
    }
    if let Some(rootfs) = &runtime.fex_rootfs {
      self.validate_file_or_directory(runtime_dir, rootfs).await?;
    }
    Ok(())
  }

  async fn validate_executable(
    &self,
    runtime_dir: &Path,
    command: &[String],
  ) -> Result<(), RuntimeError> {
    let (program, _) = command
      .split_first()
      .ok_or_else(|| RuntimeError("Runtime command is empty.".to_owned()))?;
    let relative = safe_relative_path(program)?;
    let path = runtime_dir.join(relative);
    let metadata = fs::metadata(&path).await.map_err(|error| {
      RuntimeError(format!(
        "Runtime executable {} is missing: {error}.",
        path.display()
      ))
    })?;
    if !metadata.is_file() {
      return Err(RuntimeError(format!(
        "Runtime executable {} is not a file.",
        path.display()
      )));
    }
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mut permissions = metadata.permissions();
      permissions.set_mode(permissions.mode() | 0o111);
      fs::set_permissions(path, permissions)
        .await
        .map_err(io_error)?;
    }
    Ok(())
  }

  async fn validate_file_or_directory(
    &self,
    runtime_dir: &Path,
    relative: &str,
  ) -> Result<(), RuntimeError> {
    let path = runtime_dir.join(safe_relative_path(relative)?);
    fs::metadata(&path).await.map_err(|error| {
      RuntimeError(format!(
        "Runtime path {} is missing: {error}.",
        path.display()
      ))
    })?;
    Ok(())
  }

  async fn write_active_runtime(&self, runtime: &InstalledRuntime) -> Result<(), RuntimeError> {
    let path = self.root.join("active.json");
    let temporary = self.root.join(format!(".active-{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(runtime)
      .map_err(|error| RuntimeError(format!("Could not serialize runtime record: {error}")))?;
    fs::write(&temporary, bytes).await.map_err(io_error)?;
    fs::rename(temporary, path).await.map_err(io_error)?;
    Ok(())
  }
}

fn unpack_tar_zstd(archive_path: &Path, destination: &Path) -> Result<(), RuntimeError> {
  std::fs::create_dir_all(destination).map_err(io_error)?;
  let file = File::open(archive_path).map_err(io_error)?;
  let decoder = zstd::stream::read::Decoder::new(file)
    .map_err(|error| RuntimeError(format!("Invalid zstd runtime bundle: {error}")))?;
  let mut archive = tar::Archive::new(decoder);
  let mut unpacked_bytes = 0_u64;
  let entries = archive
    .entries()
    .map_err(|error| RuntimeError(format!("Invalid runtime archive: {error}")))?;
  for entry in entries {
    let mut entry =
      entry.map_err(|error| RuntimeError(format!("Invalid runtime archive entry: {error}")))?;
    unpacked_bytes = unpacked_bytes.saturating_add(entry.size());
    if unpacked_bytes > MAX_RUNTIME_BYTES.saturating_mul(4) {
      return Err(RuntimeError(
        "Runtime bundle expands beyond the extraction size limit.".to_owned(),
      ));
    }
    let unpacked = entry
      .unpack_in(destination)
      .map_err(|error| RuntimeError(format!("Could not extract runtime bundle: {error}")))?;
    if !unpacked {
      return Err(RuntimeError(
        "Runtime bundle contains an unsafe archive path.".to_owned(),
      ));
    }
  }
  Ok(())
}

fn validate_command_paths(command: &[String]) -> Result<(), RuntimeError> {
  let (program, _) = command
    .split_first()
    .ok_or_else(|| RuntimeError("Runtime command is empty.".to_owned()))?;
  safe_relative_path(program)?;
  Ok(())
}

fn safe_relative_path(value: &str) -> Result<PathBuf, RuntimeError> {
  let path = Path::new(value);
  if path.as_os_str().is_empty()
    || path.is_absolute()
    || path
      .components()
      .any(|component| !matches!(component, Component::Normal(_)))
  {
    return Err(RuntimeError(format!(
      "Runtime path must be relative and may not traverse directories: {value}"
    )));
  }
  Ok(path.to_path_buf())
}

fn valid_version(version: &str) -> bool {
  !version.is_empty()
    && version.len() <= 80
    && version
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_sha256(value: &str) -> bool {
  value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn io_error(error: std::io::Error) -> RuntimeError {
  RuntimeError(error.to_string())
}
