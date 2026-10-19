use std::{
  collections::VecDeque,
  fs::File,
  path::{Component, Path, PathBuf},
  sync::atomic::{AtomicU8, Ordering},
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{
  fs,
  io::{AsyncReadExt, AsyncWriteExt},
  sync::Mutex,
};
use uuid::Uuid;

use crate::pe_arch::WindowsArchitecture;

const MAX_RUNTIME_BYTES: u64 = 20 * 1024 * 1024 * 1024;
const PREPARATION_IDLE: u8 = 0;
const PREPARATION_ACTIVE: u8 = 1;
const PREPARATION_FAILED: u8 = 2;

#[derive(Clone)]
pub struct RuntimeManager {
  root: PathBuf,
  catalog_path: PathBuf,
  prepare_lock: Arc<Mutex<()>>,
  preparation_state: Arc<AtomicU8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
  pub state: String,
  pub bundled: bool,
  pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRuntime {
  pub version: String,
  pub host_architecture: String,
  pub wine_version: String,
  #[serde(default)]
  pub native_wine_version: Option<String>,
  pub fex_version: Option<String>,
  pub wine_command: Vec<String>,
  pub wineboot_command: Vec<String>,
  #[serde(default)]
  pub native_wine_command: Option<Vec<String>>,
  #[serde(default)]
  pub native_wineboot_command: Option<Vec<String>>,
  #[serde(default)]
  pub native_session_launcher: Option<Vec<String>>,
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
  #[serde(default)]
  native_wine_version: Option<String>,
  fex_version: Option<String>,
  artifact: RuntimeArtifact,
  wine_command: Vec<String>,
  wineboot_command: Vec<String>,
  #[serde(default)]
  native_wine_command: Option<Vec<String>>,
  #[serde(default)]
  native_wineboot_command: Option<Vec<String>>,
  #[serde(default)]
  native_session_launcher: Option<Vec<String>>,
  fex_command: Option<Vec<String>>,
  fex_rootfs: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeArtifact {
  url: Option<String>,
  bundle_path: Option<String>,
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
      preparation_state: Arc::new(AtomicU8::new(PREPARATION_IDLE)),
    }
  }

  pub async fn status(&self) -> RuntimeStatus {
    match self.installed_runtime().await {
      Ok(Some(_runtime)) => {
        return RuntimeStatus {
          state: "ready".to_owned(),
          bundled: false,
          message: "The Windows runtime is ready.".to_owned(),
        };
      }
      Ok(None) => {}
      Err(_error) => {
        return RuntimeStatus {
          state: "runtime-invalid".to_owned(),
          bundled: false,
          message: "The Windows runtime could not be validated.".to_owned(),
        };
      }
    }

    match self.read_catalog().await {
      Ok(catalog) => match self.validate_catalog(&catalog) {
        Ok(()) => {
          let preparation_state = self.preparation_state.load(Ordering::Acquire);
          RuntimeStatus {
            state: match preparation_state {
              PREPARATION_ACTIVE => "preparing",
              PREPARATION_FAILED => "prepare-failed",
              _ => "not-installed",
            }
            .to_owned(),
            bundled: catalog.artifact.bundle_path.is_some(),
            message: if preparation_state == PREPARATION_ACTIVE {
              "The Windows runtime is being prepared.".to_owned()
            } else if preparation_state == PREPARATION_FAILED {
              "The Windows runtime could not be prepared.".to_owned()
            } else if catalog.artifact.bundle_path.is_some() {
              "The Windows runtime is included with WinTone and will be prepared when WinTone starts."
                .to_owned()
            } else {
              "The Windows runtime package will be downloaded when WinTone starts.".to_owned()
            },
          }
        }
        Err(_error) => RuntimeStatus {
          state: "catalog-invalid".to_owned(),
          bundled: false,
          message: "The Windows runtime configuration is invalid.".to_owned(),
        },
      },
      Err(_error) => RuntimeStatus {
        state: "catalog-unavailable".to_owned(),
        bundled: false,
        message: "The Windows runtime is unavailable.".to_owned(),
      },
    }
  }

  pub async fn prepare(&self) -> Result<InstalledRuntime, RuntimeError> {
    let _guard = self.prepare_lock.lock().await;
    let mut preparation = RuntimePreparingGuard {
      state: self.preparation_state.clone(),
      completed: false,
    };
    self
      .preparation_state
      .store(PREPARATION_ACTIVE, Ordering::Release);
    let catalog = self.read_catalog().await?;
    self.validate_catalog(&catalog)?;
    if let Ok(Some(runtime)) = self.installed_runtime_version(&catalog.version).await {
      if runtime.artifact_sha256 != catalog.artifact.sha256.to_ascii_lowercase() {
        return Err(RuntimeError(
          "The runtime catalog reused a version identifier for different bundle contents."
            .to_owned(),
        ));
      }
      self.write_version_runtime(&runtime).await?;
      self.write_active_runtime(&runtime).await?;
      preparation.complete();
      return Ok(runtime);
    }
    // A previously extracted bundle may be incomplete (for example, older
    // releases omitted FEXServer and Wine preloaders). Do not let an invalid
    // active bundle prevent the configured bundle from repairing it.
    if let Ok(Some(runtime)) = self.installed_runtime().await {
      if runtime.version == catalog.version {
        if runtime.artifact_sha256 != catalog.artifact.sha256.to_ascii_lowercase() {
          return Err(RuntimeError(
            "The runtime catalog reused a version identifier for different bundle contents."
              .to_owned(),
          ));
        }
        preparation.complete();
        return Ok(runtime);
      }
      self.write_version_runtime(&runtime).await?;
    }
    fs::create_dir_all(self.root.join("versions"))
      .await
      .map_err(io_error)?;

    let operation_id = Uuid::new_v4().to_string();
    let archive_path = self.root.join(format!(".download-{operation_id}.tar.zst"));
    let staging_path = self.root.join(format!(".staging-{operation_id}"));
    let result = async {
      self
        .acquire_artifact(&catalog.artifact, &archive_path)
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
      self.write_version_runtime(&runtime).await?;
      self.write_active_runtime(&runtime).await?;
      Ok(runtime)
    }
    .await;

    let _ = fs::remove_file(&archive_path).await;
    if result.is_err() {
      let _ = fs::remove_dir_all(&staging_path).await;
    }
    if result.is_ok() {
      preparation.complete();
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

  pub async fn installed_runtime_version(
    &self,
    version: &str,
  ) -> Result<Option<InstalledRuntime>, RuntimeError> {
    if !valid_version(version) {
      return Err(RuntimeError(
        "Invalid runtime version identifier.".to_owned(),
      ));
    }
    let record_path = self
      .root
      .join("versions")
      .join(version)
      .join("runtime.json");
    let bytes = match fs::read(&record_path).await {
      Ok(bytes) => bytes,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
        if let Some(active) = self.installed_runtime().await? {
          if active.version == version {
            return Ok(Some(active));
          }
        }
        return Ok(None);
      }
      Err(error) => return Err(io_error(error)),
    };
    let runtime: InstalledRuntime = serde_json::from_slice(&bytes)
      .map_err(|error| RuntimeError(format!("Invalid installed-runtime record: {error}")))?;
    if runtime.version != version || runtime.host_architecture != std::env::consts::ARCH {
      return Err(RuntimeError(format!(
        "Installed runtime record for {version} has an invalid version or host architecture."
      )));
    }
    self
      .validate_layout(&runtime, &self.runtime_dir(&runtime))
      .await?;
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
    if !valid_version(&runtime.version) {
      return Err(RuntimeError(
        "Invalid installed runtime version.".to_owned(),
      ));
    }
    let (program, args) = command
      .split_first()
      .ok_or_else(|| RuntimeError("Runtime command is empty.".to_owned()))?;
    let path = safe_relative_path(program)?;
    let relative = path
      .components()
      .map(|component| component.as_os_str().to_string_lossy())
      .collect::<Vec<_>>()
      .join("/");
    Ok((format!("/grants/runtime/{relative}"), args.to_vec()))
  }

  /// Returns an absolute path as seen inside the x86-64 FEX RootFS. This is
  /// deliberately different from `session_command`, which returns the host
  /// path of a read-only AgentOS file grant.
  pub fn guest_session_command(
    &self,
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
    Ok((format!("/{relative}"), args.to_vec()))
  }

  pub fn session_source(&self, runtime: &InstalledRuntime) -> Result<String, RuntimeError> {
    if !valid_version(&runtime.version) {
      return Err(RuntimeError(
        "Invalid installed runtime version.".to_owned(),
      ));
    }
    Ok(format!("runtimes/versions/{}", runtime.version))
  }

  pub fn launch_command(
    &self,
    runtime: &InstalledRuntime,
    architecture: WindowsArchitecture,
  ) -> Result<(String, Vec<String>), RuntimeError> {
    if runtime.host_architecture == "aarch64" && architecture == WindowsArchitecture::Arm64 {
      let command = runtime.native_session_launcher.as_ref().ok_or_else(|| {
        RuntimeError("The Windows runtime does not support ARM64 apps on this device.".to_owned())
      })?;
      return self.session_command(runtime, command);
    }
    if runtime.host_architecture == "aarch64" {
      let command = runtime.fex_command.as_ref().ok_or_else(|| {
        RuntimeError("The Windows runtime does not support this app architecture.".to_owned())
      })?;
      return self.session_command(runtime, command);
    }
    if architecture == WindowsArchitecture::Arm64 {
      return Err(RuntimeError(
        "Native Windows ARM64 programs require an ARM64 AgentOS host.".to_owned(),
      ));
    }
    self.session_command(runtime, &runtime.wine_command)
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
    if !valid_version(&runtime.version) {
      return Err(RuntimeError(
        "Invalid installed runtime version.".to_owned(),
      ));
    }
    Ok(format!("/grants/runtime/{relative}"))
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
    match (
      &catalog.native_wine_version,
      &catalog.native_wine_command,
      &catalog.native_wineboot_command,
      &catalog.native_session_launcher,
    ) {
      (Some(version), Some(wine), Some(wineboot), Some(launcher)) if !version.trim().is_empty() => {
        validate_command_paths(wine)?;
        validate_command_paths(wineboot)?;
        validate_command_paths(launcher)?;
      }
      (None, None, None, None) => {}
      _ => {
        return Err(RuntimeError(
          "Native ARM64 Wine metadata must include its version, commands, and session launcher."
            .to_owned(),
        ));
      }
    }
    if catalog.artifact.format != "tar.zst" {
      return Err(RuntimeError(
        "Only tar.zst runtime bundles are supported.".to_owned(),
      ));
    }
    match (&catalog.artifact.url, &catalog.artifact.bundle_path) {
      (Some(url), None) => {
        let artifact_url = reqwest::Url::parse(url)
          .map_err(|error| RuntimeError(format!("Invalid runtime bundle URL: {error}")))?;
        if artifact_url.scheme() != "https"
          || artifact_url.host_str().is_none()
          || !artifact_url.username().is_empty()
          || artifact_url.password().is_some()
        {
          return Err(RuntimeError(
            "Runtime bundle URLs must use HTTPS and may not contain embedded credentials."
              .to_owned(),
          ));
        }
      }
      (None, Some(bundle_path)) => {
        safe_relative_path(bundle_path)?;
      }
      _ => {
        return Err(RuntimeError(
          "A runtime artifact must configure exactly one HTTPS URL or bundled path.".to_owned(),
        ));
      }
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

  async fn acquire_artifact(
    &self,
    artifact: &RuntimeArtifact,
    destination: &Path,
  ) -> Result<(), RuntimeError> {
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut output = fs::File::create(destination).await.map_err(io_error)?;

    if let Some(url) = &artifact.url {
      let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| RuntimeError(format!("Could not initialize download client: {error}")))?;
      let mut response = client
        .get(url)
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
    } else if let Some(bundle_path) = &artifact.bundle_path {
      let relative = safe_relative_path(bundle_path)?;
      let bundle = self
        .catalog_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(relative);
      let metadata = fs::metadata(&bundle).await.map_err(|error| {
        RuntimeError(format!(
          "Bundled runtime artifact {} is unavailable: {error}.",
          bundle.display()
        ))
      })?;
      if !metadata.is_file() || metadata.len() > artifact.max_bytes {
        return Err(RuntimeError(
          "Bundled runtime artifact is not a file or exceeds its catalog size limit.".to_owned(),
        ));
      }
      let mut input = fs::File::open(&bundle).await.map_err(io_error)?;
      let mut buffer = vec![0_u8; 1024 * 1024];
      loop {
        let count = input.read(&mut buffer).await.map_err(io_error)?;
        if count == 0 {
          break;
        }
        downloaded = downloaded.saturating_add(count as u64);
        if downloaded > artifact.max_bytes {
          drop(output);
          let _ = fs::remove_file(destination).await;
          return Err(RuntimeError(
            "Bundled runtime artifact exceeds its catalog size limit.".to_owned(),
          ));
        }
        hasher.update(&buffer[..count]);
        output.write_all(&buffer[..count]).await.map_err(io_error)?;
      }
    } else {
      return Err(RuntimeError(
        "Runtime artifact source is not configured.".to_owned(),
      ));
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
      native_wine_version: catalog.native_wine_version.clone(),
      fex_version: catalog.fex_version.clone(),
      wine_command: catalog.wine_command.clone(),
      wineboot_command: catalog.wineboot_command.clone(),
      native_wine_command: catalog.native_wine_command.clone(),
      native_wineboot_command: catalog.native_wineboot_command.clone(),
      native_session_launcher: catalog.native_session_launcher.clone(),
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
    if runtime.host_architecture == "aarch64" {
      for required_host_tool in ["fex/bin/FEX", "fex/bin/FEXBash", "fex/bin/FEXServer"] {
        self
          .validate_executable(runtime_dir, &[required_host_tool.to_owned()])
          .await?;
      }
      let rootfs = runtime
        .fex_rootfs
        .as_deref()
        .ok_or_else(|| RuntimeError("ARM64 runtime has no x86-64 RootFS.".to_owned()))?;
      let rootfs_path = runtime_dir.join(safe_relative_path(rootfs)?);
      for command in [&runtime.wine_command, &runtime.wineboot_command] {
        self
          .validate_guest_executable(&rootfs_path, command)
          .await?;
      }
      for preloader in [
        "usr/lib/wine/wine64-preloader",
        "usr/lib/wine/wine-preloader",
      ] {
        self
          .validate_guest_executable(&rootfs_path, &[preloader.to_owned()])
          .await?;
      }
      self.validate_rootfs_devices(&rootfs_path).await?;
    } else {
      for command in [&runtime.wine_command, &runtime.wineboot_command] {
        self.validate_executable(runtime_dir, command).await?;
      }
    }
    if let Some(command) = &runtime.fex_command {
      self.validate_executable(runtime_dir, command).await?;
    }
    for command in [
      &runtime.native_wine_command,
      &runtime.native_wineboot_command,
      &runtime.native_session_launcher,
    ]
    .into_iter()
    .flatten()
    {
      self.validate_executable(runtime_dir, command).await?;
    }
    if runtime.native_wine_command.is_some() {
      let native_rootfs = runtime_dir.join("native-arm64/rootfs");
      for required_native_file in [
        "usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1",
        "usr/lib/wine/wine64-preloader",
      ] {
        self
          .validate_guest_executable(&native_rootfs, &[required_native_file.to_owned()])
          .await?;
      }
      self.validate_rootfs_devices(&native_rootfs).await?;
    }
    if let Some(rootfs) = &runtime.fex_rootfs {
      self.validate_file_or_directory(runtime_dir, rootfs).await?;
    }
    Ok(())
  }

  async fn validate_rootfs_devices(&self, rootfs: &Path) -> Result<(), RuntimeError> {
    for device in [
      "full", "ptmx", "console", "null", "urandom", "tty", "zero", "random",
    ] {
      let path = rootfs.join("dev").join(device);
      // Device entries must be absent so FEX resolves them in the session's
      // /dev. A /dev/null symlink inside the guest root resolves to itself.
      match fs::symlink_metadata(&path).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(RuntimeError(format!(
          "Could not inspect runtime device {}: {error}.", path.display()))),
        Ok(_) => return Err(RuntimeError(format!(
          "Runtime device {} must be omitted to use the sandbox device.", path.display()))),
      }
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

  async fn validate_guest_executable(
    &self,
    rootfs: &Path,
    command: &[String],
  ) -> Result<(), RuntimeError> {
    let (program, _) = command
      .split_first()
      .ok_or_else(|| RuntimeError("Runtime command is empty.".to_owned()))?;
    let relative = safe_relative_path(program)?;
    let path = resolve_guest_path(rootfs, &relative)
      .await
      .map_err(|error| {
        RuntimeError(format!(
          "Runtime executable {} is missing from its packaged RootFS: {}.",
          rootfs.join(relative).display(),
          error.0
        ))
      })?;
    let metadata = fs::metadata(&path).await.map_err(io_error)?;
    if !metadata.is_file() {
      return Err(RuntimeError(format!(
        "Wine executable {} is not a file.",
        path.display()
      )));
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

  async fn write_version_runtime(&self, runtime: &InstalledRuntime) -> Result<(), RuntimeError> {
    let directory = self.root.join("versions").join(&runtime.version);
    fs::create_dir_all(&directory).await.map_err(io_error)?;
    let path = directory.join("runtime.json");
    let temporary = directory.join(format!(".runtime-{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(runtime)
      .map_err(|error| RuntimeError(format!("Could not serialize runtime record: {error}")))?;
    fs::write(&temporary, bytes).await.map_err(io_error)?;
    fs::rename(temporary, path).await.map_err(io_error)?;
    Ok(())
  }
}

/// Resolve Linux absolute symlinks as paths inside a guest root filesystem.
/// Calling `metadata(rootfs.join(path))` directly follows `/usr/...` against
/// the AgentOS host root, which makes valid Wine links look broken.
async fn resolve_guest_path(rootfs: &Path, relative: &Path) -> Result<PathBuf, RuntimeError> {
  let mut pending = guest_path_components(relative)?;
  let mut resolved = PathBuf::new();
  let mut symlink_depth = 0_u8;

  while let Some(component) = pending.pop_front() {
    match component.components().next() {
      Some(Component::RootDir) => resolved.clear(),
      Some(Component::CurDir) => {}
      Some(Component::ParentDir) => {
        if !resolved.pop() {
          return Err(RuntimeError(
            "Guest executable symlink escapes its root filesystem.".to_owned(),
          ));
        }
      }
      Some(Component::Normal(name)) => {
        let candidate = rootfs.join(&resolved).join(name);
        let metadata = fs::symlink_metadata(&candidate).await.map_err(io_error)?;
        if metadata.file_type().is_symlink() {
          symlink_depth = symlink_depth.saturating_add(1);
          if symlink_depth > 40 {
            return Err(RuntimeError(
              "Guest executable has too many nested symlinks.".to_owned(),
            ));
          }
          let target = fs::read_link(candidate).await.map_err(io_error)?;
          let mut target_components = guest_path_components(&target)?;
          target_components.append(&mut pending);
          pending = target_components;
        } else {
          if !pending.is_empty() && !metadata.is_dir() {
            return Err(RuntimeError(
              "Guest executable path traverses a non-directory.".to_owned(),
            ));
          }
          resolved.push(name);
        }
      }
      Some(Component::Prefix(_)) => {
        return Err(RuntimeError(
          "Guest executable path contains an unsupported prefix.".to_owned(),
        ));
      }
      None => {}
    }
  }

  Ok(rootfs.join(resolved))
}

fn guest_path_components(path: &Path) -> Result<VecDeque<PathBuf>, RuntimeError> {
  path
    .components()
    .map(|component| match component {
      Component::Prefix(_) => Err(RuntimeError(
        "Guest executable symlink contains an unsupported prefix.".to_owned(),
      )),
      Component::RootDir => Ok(PathBuf::from("/")),
      Component::CurDir => Ok(PathBuf::from(".")),
      Component::ParentDir => Ok(PathBuf::from("..")),
      Component::Normal(name) => Ok(PathBuf::from(name)),
    })
    .collect()
}

struct RuntimePreparingGuard {
  state: Arc<AtomicU8>,
  completed: bool,
}

impl RuntimePreparingGuard {
  fn complete(&mut self) {
    self.completed = true;
    self.state.store(PREPARATION_IDLE, Ordering::Release);
  }
}

impl Drop for RuntimePreparingGuard {
  fn drop(&mut self) {
    if !self.completed {
      self.state.store(PREPARATION_FAILED, Ordering::Release);
    }
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
    let entry_type = entry.header().entry_type();
    let device_link =
      if entry_type.is_character_special() || entry_type.is_block_special() || entry_type.is_fifo()
      {
        if !entry_type.is_character_special() {
          return Err(RuntimeError(
            "Runtime bundle contains an unsupported special file.".to_owned(),
          ));
        }
        let path = entry
          .path()
          .map_err(|error| RuntimeError(format!("Invalid runtime archive path: {error}")))?
          .into_owned();
        let relative = archive_relative_path(&path)?;
        let target = runtime_device_target(&relative).ok_or_else(|| {
          RuntimeError(format!(
            "Runtime bundle contains an unsupported device node: {}.",
            relative.display()
          ))
        })?;
        Some((relative, target))
      } else {
        None
      };
    unpacked_bytes = unpacked_bytes.saturating_add(entry.size());
    if unpacked_bytes > MAX_RUNTIME_BYTES.saturating_mul(4) {
      return Err(RuntimeError(
        "Runtime bundle expands beyond the extraction size limit.".to_owned(),
      ));
    }
    if device_link.is_some() {
      // Recognized legacy character devices are omitted as well; do not
      // create host devices or circular links in the extracted guest root.
      continue;
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

fn archive_relative_path(path: &Path) -> Result<PathBuf, RuntimeError> {
  let mut relative = PathBuf::new();
  for component in path.components() {
    match component {
      Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
      Component::ParentDir => {
        return Err(RuntimeError(
          "Runtime bundle contains an unsafe archive path.".to_owned(),
        ));
      }
      Component::Normal(name) => relative.push(name),
    }
  }
  if relative.as_os_str().is_empty() {
    return Err(RuntimeError(
      "Runtime bundle contains an empty archive path.".to_owned(),
    ));
  }
  Ok(relative)
}

fn runtime_device_target(path: &Path) -> Option<&'static str> {
  let parent = path.parent()?;
  if parent != Path::new("fex/rootfs/dev") && parent != Path::new("native-arm64/rootfs/dev") {
    return None;
  }
  match path.file_name()?.to_str()? {
    "full" => Some("/dev/full"),
    "ptmx" => Some("/dev/ptmx"),
    "console" => Some("/dev/console"),
    "null" => Some("/dev/null"),
    "urandom" => Some("/dev/urandom"),
    "tty" => Some("/dev/tty"),
    "zero" => Some("/dev/zero"),
    "random" => Some("/dev/random"),
    _ => None,
  }
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

#[cfg(test)]
mod tests {
  use super::*;

  fn arm64_runtime() -> InstalledRuntime {
    InstalledRuntime {
      version: "arm64-dual-runtime".to_owned(),
      host_architecture: "aarch64".to_owned(),
      wine_version: "x86-64 Wine".to_owned(),
      native_wine_version: Some("ARM64 Wine".to_owned()),
      fex_version: Some("FEX".to_owned()),
      wine_command: vec!["usr/bin/wine".to_owned()],
      wineboot_command: vec!["usr/bin/wineboot".to_owned(), "-u".to_owned()],
      native_wine_command: Some(vec!["native-arm64/rootfs/usr/lib/wine/wine64".to_owned()]),
      native_wineboot_command: Some(vec![
        "native-arm64/rootfs/usr/lib/wine/wine64".to_owned(),
        "wineboot.exe".to_owned(),
        "-u".to_owned(),
      ]),
      native_session_launcher: Some(vec![
        "native-arm64/bin/wintone-native-session-launch".to_owned()
      ]),
      fex_command: Some(vec!["fex/bin/wintone-session-launch".to_owned()]),
      fex_rootfs: Some("fex/rootfs".to_owned()),
      artifact_sha256: "0".repeat(64),
      installed_at: 0,
    }
  }

  fn add_rootfs_device_directories(runtime_dir: &Path) {
    for rootfs in ["fex/rootfs", "native-arm64/rootfs"] {
      let dev = runtime_dir.join(rootfs).join("dev");
      std::fs::create_dir_all(&dev).unwrap();

    }
  }

  #[tokio::test]
  async fn validates_bundled_arm64_runtime_layout() {
    let root = std::env::temp_dir().join(format!("wintone-arm64-layout-{}", Uuid::new_v4()));
    let runtime_dir = root.join("runtime");
    for directory in [
      "fex/rootfs/usr/bin",
      "fex/rootfs/usr/lib/wine",
      "fex/bin",
      "native-arm64/rootfs/usr/lib/wine",
      "native-arm64/rootfs/usr/lib/aarch64-linux-gnu",
      "native-arm64/bin",
    ] {
      std::fs::create_dir_all(runtime_dir.join(directory)).unwrap();
    }
    for file in [
      "fex/rootfs/usr/bin/wine",
      "fex/rootfs/usr/bin/wineboot",
      "fex/rootfs/usr/lib/wine/wine64-preloader",
      "fex/rootfs/usr/lib/wine/wine-preloader",
      "fex/bin/FEX",
      "fex/bin/FEXBash",
      "fex/bin/FEXServer",
      "fex/bin/wintone-session-launch",
      "native-arm64/rootfs/usr/lib/wine/wine64",
      "native-arm64/rootfs/usr/lib/wine/wine64-preloader",
      "native-arm64/rootfs/usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1",
      "native-arm64/bin/wintone-native-session-launch",
    ] {
      std::fs::write(runtime_dir.join(file), "runtime fixture").unwrap();
    }
    add_rootfs_device_directories(&runtime_dir);

    let manager = RuntimeManager::new(root.join("data"), PathBuf::from("catalog"));
    manager
      .validate_layout(&arm64_runtime(), &runtime_dir)
      .await
      .unwrap();

    let _ = std::fs::remove_dir_all(root);
  }

  #[tokio::test]
  async fn rejects_arm64_runtime_missing_fex_server() {
    let root = std::env::temp_dir().join(format!("wintone-fex-server-{}", Uuid::new_v4()));
    let runtime_dir = root.join("runtime");
    for directory in [
      "fex/rootfs/usr/bin",
      "fex/rootfs/usr/lib/wine",
      "fex/bin",
      "native-arm64/rootfs/usr/lib/wine",
      "native-arm64/rootfs/usr/lib/aarch64-linux-gnu",
      "native-arm64/bin",
    ] {
      std::fs::create_dir_all(runtime_dir.join(directory)).unwrap();
    }
    for file in [
      "fex/rootfs/usr/bin/wine",
      "fex/rootfs/usr/bin/wineboot",
      "fex/rootfs/usr/lib/wine/wine64-preloader",
      "fex/rootfs/usr/lib/wine/wine-preloader",
      "fex/bin/FEX",
      "fex/bin/FEXBash",
      "fex/bin/wintone-session-launch",
      "native-arm64/rootfs/usr/lib/wine/wine64",
      "native-arm64/rootfs/usr/lib/wine/wine64-preloader",
      "native-arm64/rootfs/usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1",
      "native-arm64/bin/wintone-native-session-launch",
    ] {
      std::fs::write(runtime_dir.join(file), "runtime fixture").unwrap();
    }
    add_rootfs_device_directories(&runtime_dir);

    let manager = RuntimeManager::new(root.join("data"), PathBuf::from("catalog"));
    let error = manager
      .validate_layout(&arm64_runtime(), &runtime_dir)
      .await
      .unwrap_err();
    assert!(error.0.contains("fex/bin/FEXServer"));

    let _ = std::fs::remove_dir_all(root);
  }

  #[tokio::test]
  async fn rejects_legacy_device_self_links() {
    let root = std::env::temp_dir().join(format!("wintone-self-device-{}", Uuid::new_v4()));
    std::fs::create_dir_all(root.join("dev")).unwrap();
    let manager = RuntimeManager::new(root.join("data"), PathBuf::from("catalog"));
    manager.validate_rootfs_devices(&root).await.unwrap();
    std::os::unix::fs::symlink("/dev/null", root.join("dev/null")).unwrap();
    assert!(manager.validate_rootfs_devices(&root).await.unwrap_err().0.contains("must be omitted"));
    std::fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn extraction_omits_rootfs_character_devices_for_sandbox_fallback() {
    let root = std::env::temp_dir().join(format!("wintone-device-unpack-{}", Uuid::new_v4()));
    let archive_path = root.join("runtime.tar.zst");
    let destination = root.join("runtime");
    std::fs::create_dir_all(&root).unwrap();
    let output = File::create(&archive_path).unwrap();
    let encoder = zstd::stream::write::Encoder::new(output, 0).unwrap();
    let mut archive = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::character_special());
    header.set_mode(0o666);
    header.set_uid(0);
    header.set_gid(0);
    header.set_size(0);
    header.set_device_major(1).unwrap();
    header.set_device_minor(3).unwrap();
    header.set_cksum();
    archive
      .append_data(&mut header, "./fex/rootfs/dev/null", std::io::empty())
      .unwrap();
    let mut file_header = tar::Header::new_gnu();
    file_header.set_mode(0o644);
    file_header.set_size(12);
    file_header.set_cksum();
    archive
      .append_data(
        &mut file_header,
        "./fex/rootfs/etc/wintone-test",
        &b"runtime file"[..],
      )
      .unwrap();
    archive.into_inner().unwrap().finish().unwrap();

    unpack_tar_zstd(&archive_path, &destination).unwrap();
    assert!(std::fs::symlink_metadata(destination.join("fex/rootfs/dev/null")).is_err());
    assert_eq!(
      std::fs::read(destination.join("fex/rootfs/etc/wintone-test")).unwrap(),
      b"runtime file"
    );
    let _ = std::fs::remove_dir_all(root);
  }

  #[test]
  fn rejects_character_devices_outside_the_expected_rootfs_device_set() {
    let root = std::env::temp_dir().join(format!("wintone-unsafe-device-{}", Uuid::new_v4()));
    let archive_path = root.join("runtime.tar.zst");
    let destination = root.join("runtime");
    std::fs::create_dir_all(&root).unwrap();
    let output = File::create(&archive_path).unwrap();
    let encoder = zstd::stream::write::Encoder::new(output, 0).unwrap();
    let mut archive = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::character_special());
    header.set_mode(0o666);
    header.set_uid(0);
    header.set_gid(0);
    header.set_size(0);
    header.set_device_major(1).unwrap();
    header.set_device_minor(3).unwrap();
    header.set_cksum();
    archive
      .append_data(
        &mut header,
        "./fex/rootfs/etc/unexpected-device",
        std::io::empty(),
      )
      .unwrap();
    archive.into_inner().unwrap().finish().unwrap();

    let error = unpack_tar_zstd(&archive_path, &destination).unwrap_err();
    assert!(error.0.contains("unsupported device node"));
    assert!(!destination
      .join("fex/rootfs/etc/unexpected-device")
      .exists());
    let _ = std::fs::remove_dir_all(root);
  }

  #[test]
  fn picks_fex_for_x86_and_native_wine_for_arm64_pe() {
    let manager = RuntimeManager::new(PathBuf::from("/tmp/wintone"), PathBuf::from("catalog"));
    let runtime = arm64_runtime();
    assert_eq!(
      manager
        .launch_command(&runtime, WindowsArchitecture::X86_64)
        .unwrap()
        .0,
      "/grants/runtime/fex/bin/wintone-session-launch"
    );
    assert_eq!(
      manager
        .launch_command(&runtime, WindowsArchitecture::Arm64)
        .unwrap()
        .0,
      "/grants/runtime/native-arm64/bin/wintone-native-session-launch"
    );
  }

  #[test]
  fn reports_missing_native_wine_in_older_arm64_runtime() {
    let manager = RuntimeManager::new(PathBuf::from("/tmp/wintone"), PathBuf::from("catalog"));
    let mut runtime = arm64_runtime();
    runtime.native_session_launcher = None;
    assert!(manager
      .launch_command(&runtime, WindowsArchitecture::Arm64)
      .unwrap_err()
      .0
      .contains("does not support ARM64 apps"));
  }

  #[tokio::test]
  async fn status_reports_runtime_preparation_in_progress() {
    let root = std::env::temp_dir().join(format!("wintone-runtime-status-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let catalog_path = root.join("runtime-catalog.json");
    let arm64 = std::env::consts::ARCH == "aarch64";
    let catalog = serde_json::json!({
      "schemaVersion": 1,
      "version": "test-runtime",
      "hostArchitecture": std::env::consts::ARCH,
      "wineVersion": "test",
      "nativeWineVersion": if arm64 { Some("test") } else { None },
      "fexVersion": if arm64 { Some("test") } else { None },
      "artifact": {
        "bundlePath": "runtime-bundle.tar.zst",
        "format": "tar.zst",
        "sha256": "0".repeat(64),
        "maxBytes": 1
      },
      "wineCommand": ["usr/bin/wine"],
      "winebootCommand": ["usr/bin/wineboot", "-u"],
      "nativeWineCommand": if arm64 { Some(vec!["native-arm64/rootfs/usr/lib/wine/wine64"]) } else { None },
      "nativeWinebootCommand": if arm64 { Some(vec!["native-arm64/rootfs/usr/lib/wine/wine64", "wineboot.exe", "-u"]) } else { None },
      "nativeSessionLauncher": if arm64 { Some(vec!["native-arm64/bin/session-launch"]) } else { None },
      "fexCommand": if arm64 { Some(vec!["fex/bin/session-launch"]) } else { None },
      "fexRootfs": if arm64 { Some("fex/rootfs") } else { None }
    });
    std::fs::write(&catalog_path, serde_json::to_vec(&catalog).unwrap()).unwrap();

    let manager = RuntimeManager::new(root.join("data"), catalog_path);
    assert_eq!(manager.status().await.state, "not-installed");
    manager
      .preparation_state
      .store(PREPARATION_ACTIVE, Ordering::Release);
    let status = manager.status().await;
    assert_eq!(status.state, "preparing");
    assert!(status.bundled);
    assert_eq!(status.message, "The Windows runtime is being prepared.");
    assert!(manager.prepare().await.is_err());
    let failed = manager.status().await;
    assert_eq!(failed.state, "prepare-failed");
    assert_eq!(failed.message, "The Windows runtime could not be prepared.");

    let _ = std::fs::remove_dir_all(root);
  }

  #[tokio::test]
  async fn validates_absolute_symlinks_inside_guest_rootfs() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("wintone-guest-rootfs-{}", Uuid::new_v4()));
    let rootfs = root.join("rootfs");
    std::fs::create_dir_all(rootfs.join("usr/bin")).unwrap();
    std::fs::create_dir_all(rootfs.join("etc/alternatives")).unwrap();
    std::fs::write(rootfs.join("usr/bin/wine-stable"), "#!/bin/sh\n").unwrap();
    symlink("/etc/alternatives/wine", rootfs.join("usr/bin/wine")).unwrap();
    symlink("/usr/bin/wine-stable", rootfs.join("etc/alternatives/wine")).unwrap();

    let manager = RuntimeManager::new(root.join("data"), PathBuf::from("catalog"));
    manager
      .validate_guest_executable(&rootfs, &["usr/bin/wine".to_owned()])
      .await
      .unwrap();

    let _ = std::fs::remove_dir_all(root);
  }

  #[tokio::test]
  async fn rejects_guest_symlinks_that_escape_rootfs() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("wintone-guest-rootfs-{}", Uuid::new_v4()));
    let rootfs = root.join("rootfs");
    std::fs::create_dir_all(rootfs.join("usr/bin")).unwrap();
    symlink("../../../../etc/passwd", rootfs.join("usr/bin/wine")).unwrap();

    let manager = RuntimeManager::new(root.join("data"), PathBuf::from("catalog"));
    let error = manager
      .validate_guest_executable(&rootfs, &["usr/bin/wine".to_owned()])
      .await
      .unwrap_err();
    assert!(error.0.contains("escapes its root filesystem"));

    let _ = std::fs::remove_dir_all(root);
  }
}
