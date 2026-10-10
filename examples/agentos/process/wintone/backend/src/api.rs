use std::{
  path::Path,
  time::{SystemTime, UNIX_EPOCH},
};

use axum::{
  extract::{DefaultBodyLimit, Multipart, Path as RoutePath, State},
  http::StatusCode,
  response::{IntoResponse, Response},
  routing::{delete, get, post},
  Json, Router,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::{fs, io::AsyncWriteExt};
use uuid::Uuid;

use crate::{model::InstallTask, AppState};

const MAX_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = MAX_UPLOAD_BYTES as usize + 2 * 1024 * 1024;

pub fn router(state: AppState) -> Router {
  Router::new()
    .route("/api/status", get(status))
    .route("/api/runtime/prepare", post(prepare_runtime))
    .route(
      "/api/install-tasks",
      get(list_install_tasks).post(import_and_enqueue_install_task),
    )
    .route("/api/install-tasks/:id", delete(delete_install_task))
    .route("/api/apps", get(list_apps))
    .route("/api/apps/:id/launch", post(launch_app))
    .route("/api/apps/:id", delete(uninstall_app))
    .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
    .with_state(state)
}

pub async fn health() -> Json<Value> {
  Json(json!({ "ok": true, "service": "wintone" }))
}

async fn status(State(state): State<AppState>) -> Json<Value> {
  let wayland = if state.external_wayland.is_configured() {
    state
      .external_wayland
      .capabilities()
      .await
      .unwrap_or_else(|error| {
        json!({
          "available": false,
          "message": error.message,
        })
      })
  } else {
    json!({
      "available": false,
      "message": "AgentOS external Wayland permission is not enabled for WinTone.",
    })
  };
  Json(json!({
      "service": "wintone",
      "runtime": state.runtime_manager.status().await,
      "windowIntegration": wayland,
      "storage": "ready"
  }))
}

async fn prepare_runtime(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  match state.runtime_manager.prepare().await {
    Ok(runtime) => {
      crate::installation::resume_waiting_tasks(state.clone()).await;
      Ok(Json(json!({
        "runtime": state.runtime_manager.status().await,
        "version": runtime.version,
      })))
    }
    Err(error) => Err(ApiError::new(StatusCode::BAD_GATEWAY, error.0)),
  }
}

async fn list_install_tasks(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  let store = state.snapshot().await?;
  Ok(Json(json!({ "installTasks": store.install_tasks })))
}

async fn list_apps(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  let store = state.snapshot().await?;
  Ok(Json(json!({ "apps": store.installed_apps })))
}

async fn launch_app(
  State(state): State<AppState>,
  RoutePath(id): RoutePath<String>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
  let session = crate::installation::launch_installed_app(&state, &id)
    .await
    .map_err(|message| ApiError::new(StatusCode::CONFLICT, message))?;
  Ok((StatusCode::ACCEPTED, Json(json!({ "session": session }))))
}

async fn uninstall_app(
  State(state): State<AppState>,
  RoutePath(id): RoutePath<String>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
  let session = crate::installation::uninstall_installed_app(&state, &id)
    .await
    .map_err(|message| ApiError::new(StatusCode::CONFLICT, message))?;
  Ok((StatusCode::ACCEPTED, Json(json!({ "session": session }))))
}

async fn import_and_enqueue_install_task(
  State(state): State<AppState>,
  mut multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), ApiError> {
  let mut record = None;
  while let Some(field) = multipart
    .next_field()
    .await
    .map_err(ApiError::bad_request)?
  {
    if field.name() != Some("file") {
      continue;
    }
    if let Some((previous_path, _)) = record.take() {
      let _ = fs::remove_file(previous_path).await;
      return Err(ApiError::bad_request(
        "Upload exactly one installer at a time.",
      ));
    }
    let name = field
      .file_name()
      .map(str::to_owned)
      .ok_or_else(|| ApiError::bad_request("The uploaded file has no name."))?;
    let (file_name, extension) = validate_file_name(&name)?;
    let id = Uuid::new_v4().to_string();
    let stored_file = format!("{id}.{extension}");
    let path = state.installers_dir().join(&stored_file);
    let mut output = fs::File::create(&path).await?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut signature = Vec::with_capacity(8);
    let mut field = field;
    while let Some(chunk) = field.chunk().await.map_err(ApiError::bad_request)? {
      bytes = bytes.saturating_add(chunk.len() as u64);
      if bytes > MAX_UPLOAD_BYTES {
        drop(output);
        let _ = fs::remove_file(&path).await;
        return Err(ApiError::new(
          StatusCode::PAYLOAD_TOO_LARGE,
          "Installer exceeds the 2 GiB upload limit.",
        ));
      }
      if signature.len() < 8 {
        let remaining = 8 - signature.len();
        signature.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
      }
      hasher.update(&chunk);
      output.write_all(&chunk).await?;
    }
    output.flush().await?;
    drop(output);
    if !has_expected_signature(&extension, &signature) {
      let _ = fs::remove_file(&path).await;
      return Err(ApiError::bad_request(
        "The file contents do not match an EXE or MSI installer.",
      ));
    }

    record = Some((
      path,
      InstallTask {
        id,
        file_name,
        extension,
        bytes,
        sha256: format!("{:x}", hasher.finalize()),
        added_at: SystemTime::now()
          .duration_since(UNIX_EPOCH)
          .unwrap_or_default()
          .as_millis(),
        status: "preparing-runtime".to_owned(),
        phase: "runtime".to_owned(),
        message: "Preparing the shared Wine/FEX runtime.".to_owned(),
        session_id: None,
      },
    ));
  }

  let Some((path, task)) = record else {
    return Err(ApiError::bad_request("Choose an .exe or .msi file."));
  };
  if let Err(error) = state.repository.insert_install_task(task.clone()).await {
    let _ = fs::remove_file(path).await;
    return Err(error.into());
  }
  let task_id = task.id.clone();
  let task_state = state.clone();
  tokio::spawn(async move {
    crate::installation::prepare_imported_task(task_state, task_id).await;
  });
  Ok((StatusCode::ACCEPTED, Json(json!({ "installTask": task }))))
}

async fn delete_install_task(
  State(state): State<AppState>,
  RoutePath(id): RoutePath<String>,
) -> Result<StatusCode, ApiError> {
  if state.install_task_is_active(&id) {
    return Err(ApiError::new(
      StatusCode::CONFLICT,
      "Wine/FEX 运行环境正在准备，请等待准备结束后再移除任务。",
    ));
  }
  if state
    .snapshot()
    .await?
    .install_tasks
    .iter()
    .find(|task| task.id == id)
    .is_some_and(|task| {
      matches!(
        task.status.as_str(),
        "starting-installer" | "awaiting-user" | "discovering-apps"
      )
    })
  {
    return Err(ApiError::new(
      StatusCode::CONFLICT,
      "请先关闭 Windows 安装向导，安装任务完成后再移除。",
    ));
  }
  let Some(task) = state.repository.remove_install_task(&id).await? else {
    return Err(ApiError::new(
      StatusCode::NOT_FOUND,
      "Install task was not found.",
    ));
  };
  let still_installed = state
    .snapshot()
    .await?
    .installed_apps
    .iter()
    .any(|app| app.installation_id == task.id);
  if !still_installed {
    if let Err(error) = state.prefix_manager.remove(&task.id).await {
      eprintln!("WinTone could not remove task prefix {}: {error}", task.id);
    }
  }
  let path = state
    .installers_dir()
    .join(format!("{}.{}", task.id, task.extension));
  match fs::remove_file(path).await {
    Ok(()) => {}
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
    Err(error) => eprintln!("WinTone could not remove installer {}: {error}", task.id),
  }
  Ok(StatusCode::NO_CONTENT)
}

fn validate_file_name(name: &str) -> Result<(String, String), ApiError> {
  let normalized = name.replace('\\', "/");
  let file_name = Path::new(&normalized)
    .file_name()
    .and_then(|value| value.to_str())
    .filter(|value| !value.is_empty() && *value != "." && *value != "..")
    .ok_or_else(|| ApiError::bad_request("Invalid installer filename."))?;
  let extension = Path::new(file_name)
    .extension()
    .and_then(|value| value.to_str())
    .map(str::to_ascii_lowercase)
    .filter(|value| matches!(value.as_str(), "exe" | "msi"))
    .ok_or_else(|| ApiError::bad_request("Only .exe and .msi installers are accepted."))?;
  Ok((file_name.to_owned(), extension))
}

fn has_expected_signature(extension: &str, signature: &[u8]) -> bool {
  match extension {
    "exe" => signature.starts_with(b"MZ"),
    "msi" => signature.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]),
    _ => false,
  }
}

pub struct ApiError(StatusCode, String);

impl ApiError {
  fn new(status: StatusCode, message: impl Into<String>) -> Self {
    Self(status, message.into())
  }

  fn bad_request(error: impl std::fmt::Display) -> Self {
    Self::new(StatusCode::BAD_REQUEST, error.to_string())
  }
}

impl From<std::io::Error> for ApiError {
  fn from(error: std::io::Error) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
  }
}

impl From<crate::storage::StorageError> for ApiError {
  fn from(error: crate::storage::StorageError) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.0)
  }
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    (self.0, Json(json!({ "error": self.1 }))).into_response()
  }
}
