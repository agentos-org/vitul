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

use crate::{
  model::InstallTask,
  provider::{unavailable_status, ProviderError},
  AppState,
};

const MAX_APK_BYTES: usize = 1024 * 1024 * 1024;

pub fn router(state: AppState) -> Router {
  Router::new()
    .route("/api/status", get(status))
    .route("/api/apps", get(apps))
    .route("/api/install-tasks", get(tasks).post(import_apk))
    .route("/api/install-tasks/:id/install", post(install_task))
    .route("/api/install-tasks/:id", delete(delete_task))
    .route("/api/apps/:id/launch", post(launch_app))
    .route("/api/apps/:id/stop", post(stop_app))
    .route("/api/apps/:id", delete(uninstall_app))
    .route("/api/runtime/start", post(start_runtime))
    .route("/api/runtime/recover", post(recover_runtime))
    .route("/api/runtime/reauthorize", post(reauthorize_runtime))
    .route("/api/runtime/stop", post(stop_runtime))
    .route("/api/runtime/install", post(install_runtime))
    .route("/api/runtime/initialize", post(initialize_runtime))
    .layer(DefaultBodyLimit::max(MAX_APK_BYTES + 2 * 1024 * 1024))
    .with_state(state)
}

pub async fn health() -> Json<Value> {
  Json(json!({"ok": true, "service": "androidtone"}))
}

async fn status(State(state): State<AppState>) -> Json<Value> {
  let runtime = match state.provider.status().await {
    Ok(value) => value,
    Err(error) => unavailable_status(Some(&error)),
  };
  Json(json!({ "service": "androidtone", "provider": runtime, "uploadLimitBytes": MAX_APK_BYTES }))
}

async fn apps(State(state): State<AppState>) -> Json<Value> {
  match state.provider.apps().await {
    Ok(value) => Json(value),
    Err(error) => Json(json!({"available": false, "error": error.as_value(), "apps": []})),
  }
}

async fn tasks(State(state): State<AppState>) -> Json<Value> {
  let local = state.repository.snapshot().await.install_tasks;
  match state.provider.install_tasks().await {
    Ok(remote) => {
      let mut merged = local
        .into_iter()
        .map(|task| {
          (
            task.id.clone(),
            serde_json::to_value(task).unwrap_or(Value::Null),
          )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
      if let Some(remote_tasks) = remote["installTasks"].as_array() {
        for task in remote_tasks {
          if let Some(id) = task["id"].as_str() {
            merged.insert(id.to_owned(), task.clone());
          }
        }
      }
      Json(json!({ "installTasks": merged.into_values().collect::<Vec<_>>() }))
    }
    Err(_) => Json(json!({"installTasks": local})),
  }
}

async fn import_apk(
  State(state): State<AppState>,
  mut multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), ApiError> {
  let mut saved = None;
  while let Some(mut field) = multipart
    .next_field()
    .await
    .map_err(ApiError::bad_request)?
  {
    let Some(file_name) = field.file_name().map(str::to_owned) else {
      continue;
    };
    if Path::new(&file_name)
      .extension()
      .and_then(|ext| ext.to_str())
      .map_or(true, |ext| !ext.eq_ignore_ascii_case("apk"))
    {
      return Err(ApiError::bad_request("请选择 .apk 安装包。"));
    }
    let id = Uuid::new_v4().to_string();
    let path = state.data_dir.join("imports").join(format!("{id}.apk"));
    let mut file = fs::File::create(&path).await.map_err(ApiError::internal)?;
    let mut length = 0usize;
    let mut prefix = Vec::with_capacity(4);
    let mut hasher = Sha256::new();
    while let Some(chunk) = field.chunk().await.map_err(ApiError::bad_request)? {
      length = length.saturating_add(chunk.len());
      if length > MAX_APK_BYTES {
        let _ = fs::remove_file(&path).await;
        return Err(ApiError::new(
          StatusCode::PAYLOAD_TOO_LARGE,
          "APK 超过 1 GiB 上传限制。",
        ));
      }
      if prefix.len() < 4 {
        prefix.extend_from_slice(&chunk[..chunk.len().min(4 - prefix.len())]);
      }
      hasher.update(&chunk);
      file.write_all(&chunk).await.map_err(ApiError::internal)?;
    }
    file.flush().await.map_err(ApiError::internal)?;
    if prefix.get(..2) != Some(&b"PK"[..]) {
      let _ = fs::remove_file(&path).await;
      return Err(ApiError::bad_request(
        "文件内容不像 APK（未找到 ZIP 文件头）。",
      ));
    }
    let digest = format!("{:x}", hasher.finalize());
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_millis();
    let task = InstallTask {
      id: id.clone(),
      file_name,
      bytes: length as u64,
      sha256: digest,
      added_at: now,
      status: "waiting-provider".into(),
      message: "APK 已保存。等待 AgentOS Android Provider 接入后安装。".into(),
      diagnostic: None,
    };
    state
      .repository
      .insert(task.clone())
      .await
      .map_err(ApiError::internal)?;
    saved = Some((task, path));
  }
  let Some((mut task, _path)) = saved else {
    return Err(ApiError::bad_request("没有收到 APK 文件。"));
  };
  // The host Provider receives a stable, app-owned import identifier. The host
  // bridge must expose this file through a scoped grant before performing install.
  let provider_result = state
    .provider
    .invoke(
      "installations.register",
      json!({
        "id": task.id, "fileName": task.file_name, "sha256": task.sha256, "bytes": task.bytes,
        "dataRelativePath": format!("imports/{}.apk", task.id)
      }),
    )
    .await;
  match provider_result {
    Ok(result) => {
      let install = state
        .provider
        .invoke("installations.install", json!({"id": task.id}))
        .await;
      match install {
        Ok(install) => {
          task.status = install["status"]
            .as_str()
            .unwrap_or("installing")
            .to_owned();
          task.message = install["message"]
            .as_str()
            .unwrap_or("APK 已提交给 Waydroid。")
            .to_owned();
          state
            .repository
            .update_status(&task.id, &task.status, &task.message)
            .await
            .map_err(ApiError::internal)?;
          Ok((
            StatusCode::ACCEPTED,
            Json(json!({"task": task, "provider": result, "install": install})),
          ))
        }
        Err(error) => {
          task.status = "failed".into();
          task.message = error.message.clone();
          state
            .repository
            .update_status(&task.id, &task.status, &task.message)
            .await
            .map_err(ApiError::internal)?;
          Ok((
            StatusCode::ACCEPTED,
            Json(json!({"task": task, "provider": result, "installError": error.message})),
          ))
        }
      }
    }
    Err(_) => Ok((
      StatusCode::ACCEPTED,
      Json(json!({"task": task, "provider": unavailable_status(None)})),
    )),
  }
}

async fn install_task(
  State(state): State<AppState>,
  RoutePath(id): RoutePath<String>,
) -> Result<Json<Value>, ApiError> {
  let id = safe_id(&id)?.to_owned();
  let store = state.repository.snapshot().await;
  let task = store
    .install_tasks
    .iter()
    .find(|task| task.id == id)
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "安装任务不存在。"))?;
  state
    .provider
    .invoke(
      "installations.register",
      json!({
        "id": task.id,
        "fileName": task.file_name,
        "sha256": task.sha256,
        "bytes": task.bytes,
        "dataRelativePath": format!("imports/{}.apk", task.id)
      }),
    )
    .await
    .map_err(ApiError::unavailable)?;
  let response = match state
    .provider
    .invoke("installations.install", json!({"id": task.id}))
    .await
  {
    Ok(response) => response,
    Err(error) => {
      state
        .repository
        .update_status(&id, "failed", &error.message)
        .await
        .map_err(ApiError::internal)?;
      return Err(ApiError::unavailable(error));
    }
  };
  let status = response["status"].as_str().unwrap_or("installing");
  let message = response["message"]
    .as_str()
    .unwrap_or("APK 已提交给 Waydroid。");
  state
    .repository
    .update_status(&id, status, message)
    .await
    .map_err(ApiError::internal)?;
  Ok(Json(response))
}

async fn delete_task(
  State(state): State<AppState>,
  RoutePath(id): RoutePath<String>,
) -> Result<Json<Value>, ApiError> {
  let id = safe_id(&id)?;
  if state.provider.configured() {
    state
      .provider
      .invoke("installations.remove", json!({"id": id}))
      .await
      .map_err(ApiError::unavailable)?;
  }
  let Some(task) = state
    .repository
    .remove(&id)
    .await
    .map_err(ApiError::internal)?
  else {
    return Err(ApiError::new(StatusCode::NOT_FOUND, "安装任务不存在。"));
  };
  let _ = fs::remove_file(
    state
      .data_dir
      .join("imports")
      .join(format!("{}.apk", task.id)),
  )
  .await;
  Ok(Json(json!({"removed": true})))
}

async fn launch_app(
  State(state): State<AppState>,
  RoutePath(id): RoutePath<String>,
) -> Result<Json<Value>, ApiError> {
  proxy_app_action(&state, "applications.launch", json!({"id": safe_id(&id)?})).await
}
async fn stop_app(
  State(state): State<AppState>,
  RoutePath(id): RoutePath<String>,
) -> Result<Json<Value>, ApiError> {
  proxy_app_action(&state, "applications.stop", json!({"id": safe_id(&id)?})).await
}
async fn uninstall_app(
  State(state): State<AppState>,
  RoutePath(id): RoutePath<String>,
) -> Result<Json<Value>, ApiError> {
  proxy_app_action(
    &state,
    "applications.uninstall",
    json!({"id": safe_id(&id)?}),
  )
  .await
}
async fn recover_runtime(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  proxy_app_action(&state, "runtime.recover", json!({})).await
}
async fn reauthorize_runtime(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  proxy_app_action(&state, "runtime.reauthorize", json!({})).await
}
async fn start_runtime(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  proxy_app_action(&state, "runtime.start", json!({})).await
}
async fn stop_runtime(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  proxy_app_action(&state, "runtime.stop", json!({})).await
}
async fn install_runtime(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  proxy_app_action(&state, "runtime.install", json!({})).await
}
async fn initialize_runtime(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  proxy_app_action(&state, "runtime.initialize", json!({})).await
}

async fn proxy_app_action(
  state: &AppState,
  method: &str,
  params: Value,
) -> Result<Json<Value>, ApiError> {
  state
    .provider
    .invoke(method, params)
    .await
    .map(Json)
    .map_err(ApiError::unavailable)
}

fn safe_id(value: &str) -> Result<&str, ApiError> {
  if value.len() > 256
    || value.is_empty()
    || value
      .bytes()
      .any(|byte| !(byte.is_ascii_alphanumeric() || b"._-".contains(&byte)))
  {
    return Err(ApiError::bad_request("应用标识无效。"));
  }
  Ok(value)
}

struct ApiError {
  status: StatusCode,
  error: Value,
}
impl ApiError {
  fn new(status: StatusCode, message: impl Into<String>) -> Self {
    Self {
      status,
      error: json!({
        "code": match status {
          StatusCode::BAD_REQUEST => "bad-request",
          StatusCode::NOT_FOUND => "not-found",
          StatusCode::SERVICE_UNAVAILABLE => "service-unavailable",
          _ => "request-failed",
        },
        "message": message.into()
      }),
    }
  }
  fn bad_request(error: impl std::fmt::Display) -> Self {
    Self::new(StatusCode::BAD_REQUEST, error.to_string())
  }
  fn internal(error: impl std::fmt::Display) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
  }
  fn unavailable(error: ProviderError) -> Self {
    Self {
      status: StatusCode::SERVICE_UNAVAILABLE,
      error: error.as_value(),
    }
  }
}
impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    (self.status, Json(json!({"error": self.error}))).into_response()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn provider_error_envelope_preserves_agentos_code_and_message() {
    let error = ProviderError {
      code: "external-wayland-session-unavailable".into(),
      message: "The private Wayland display session could not be created.".into(),
      provider_id: Some("org.agentos.external-wayland".into()),
      details: None,
      data: None,
      diagnostics: None,
      response: None,
    };
    let response = ApiError::unavailable(error).into_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
      .await
      .unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
      value["error"]["code"],
      "external-wayland-session-unavailable"
    );
    assert_eq!(
      value["error"]["message"],
      "The private Wayland display session could not be created."
    );
    assert_eq!(value["error"]["providerId"], "org.agentos.external-wayland");
  }

  #[tokio::test]
  async fn provider_error_response_keeps_structured_diagnostics() {
    let error = ProviderError {
      code: "preflight-blocked".into(),
      message: "The operation did not pass host preflight.".into(),
      provider_id: Some("org.agentos.system-deployment".into()),
      details: Some(
        serde_json::json!([{"code":"package-version-conflict","package":"androidtone-waydroid-images"}]),
      ),
      data: Some(
        serde_json::json!({"details":[{"code":"package-version-conflict","package":"androidtone-waydroid-images"}]}),
      ),
      diagnostics: Some(serde_json::json!({"phase":"preflight"})),
      response: None,
    };
    let response = ApiError::unavailable(error).into_response();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
      .await
      .unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
      value["error"]["details"][0]["package"],
      "androidtone-waydroid-images"
    );
    assert_eq!(
      value["error"]["data"]["details"][0]["code"],
      "package-version-conflict"
    );
    assert_eq!(value["error"]["diagnostics"]["phase"], "preflight");
  }
}
