use std::{
  env,
  fs::{self, File, OpenOptions},
  future::Future,
  io::{Read, Write},
  os::fd::AsRawFd,
  os::unix::fs::{DirBuilderExt, PermissionsExt},
  path::{Path, PathBuf},
  process::{Output, Stdio},
  time::Duration,
};

use crate::platform::{
  kernel_observations, private_wayland_environment, pulse_runtime_path, AdministrativeOperation,
  SessionContext,
};
use bytes::Bytes;
use hyper::{body::Incoming, client::conn::http1, header::HOST, Method, Request};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::{
  io::AsyncReadExt,
  net::UnixStream,
  process::{Child, Command},
};
use uuid::Uuid;
use zip::ZipArchive;

const ANDROIDTONE_PROVIDER_ID: &str = "org.agentos.androidtone";
const EXTERNAL_WAYLAND_PROVIDER_ID: &str = "org.agentos.external-wayland";
const DESKTOP_SESSION_PROVIDER_ID: &str = "org.agentos.desktop-session";
const SYSTEM_DEPLOYMENT_PROVIDER_ID: &str = "org.agentos.system-deployment";
const MAX_APK_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_ANDROID_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const APK_INSTALL_VERIFY_TIMEOUT: Duration = Duration::from_secs(45);
const APK_INSTALL_VERIFY_INTERVAL: Duration = Duration::from_secs(2);
const WAYDROID_IMAGE_PACKAGE_VERSION: &str = "0.1.8-1";
const ANDROID_RUNTIME_SETUP_OPERATION_ID: &str = "android-runtime-setup";
const WAYDROID_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
const WAYDROID_SESSION_STOP_TIMEOUT: Duration = Duration::from_secs(8);
const WAYDROID_SESSION_CHILD_GRACE: Duration = Duration::from_secs(2);
const WAYDROID_CHILD_OUTPUT_LIMIT: usize = 16 * 1024;
const EXTERNAL_WAYLAND_CALL_TIMEOUT: Duration = Duration::from_secs(12);
const ANDROID_STARTUP_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone, Debug)]
pub struct ProviderClient {
  socket: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
  pub code: String,
  pub message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provider_id: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub details: Option<Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub diagnostics: Option<Value>,
  #[serde(skip)]
  pub(crate) response: Option<Value>,
}

impl ProviderError {
  fn local(message: impl Into<String>) -> Self {
    Self {
      code: "provider-request-failed".into(),
      message: message.into(),
      provider_id: None,
      details: None,
      data: None,
      diagnostics: None,
      response: None,
    }
  }

  fn for_provider(code: &str, provider_id: &str, message: impl Into<String>) -> Self {
    Self {
      code: code.into(),
      message: message.into(),
      provider_id: Some(provider_id.into()),
      details: None,
      data: None,
      diagnostics: None,
      response: None,
    }
  }

  fn from_payload(payload: &Value, provider_id: Option<&str>) -> Self {
    let error = payload.get("error").unwrap_or(payload);
    Self {
      code: error["code"]
        .as_str()
        .unwrap_or("provider-request-failed")
        .to_owned(),
      message: error["message"]
        .as_str()
        .or_else(|| error.as_str())
        .unwrap_or("AgentOS Host Provider 拒绝了请求")
        .to_owned(),
      provider_id: error["providerId"]
        .as_str()
        .or(provider_id)
        .map(str::to_owned),
      details: error.get("details").cloned().or_else(|| {
        error
          .get("data")
          .and_then(|data| data.get("details"))
          .cloned()
      }),
      data: error.get("data").cloned(),
      diagnostics: error.get("diagnostics").cloned().or_else(|| {
        error
          .get("data")
          .and_then(|data| data.get("diagnostics"))
          .cloned()
      }),
      response: Some(payload.clone()),
    }
  }

  pub(crate) fn as_value(&self) -> Value {
    serde_json::to_value(self).unwrap_or_else(
      |_| json!({ "code": self.code, "message": self.message, "providerId": self.provider_id }),
    )
  }
}

impl std::fmt::Display for ProviderError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl std::error::Error for ProviderError {}

impl ProviderClient {
  pub fn from_env() -> Self {
    Self {
      socket: env::var_os("AGENTOS_PROVIDER_SOCKET")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from),
    }
  }

  pub fn configured(&self) -> bool {
    self.socket.is_some()
  }

  pub async fn status(&self) -> Result<Value, ProviderError> {
    self.invoke("runtime.status", json!({})).await
  }

  pub async fn apps(&self) -> Result<Value, ProviderError> {
    self.invoke("applications.list", json!({})).await
  }

  pub async fn install_tasks(&self) -> Result<Value, ProviderError> {
    self.invoke("installations.list", json!({})).await
  }

  pub async fn desktop_session(&self) -> Result<SessionContext, ProviderError> {
    let value = self
      .invoke_provider(DESKTOP_SESSION_PROVIDER_ID, "session.get", json!({}))
      .await?;
    SessionContext::from_provider(&value).map_err(ProviderError::local)
  }

  pub async fn system_deployment_capabilities(&self) -> Result<Value, ProviderError> {
    self
      .invoke_provider(SYSTEM_DEPLOYMENT_PROVIDER_ID, "capabilities.get", json!({}))
      .await
  }

  pub async fn system_deployment_invoke(
    &self,
    method: &str,
    params: Value,
  ) -> Result<Value, ProviderError> {
    self
      .invoke_provider(SYSTEM_DEPLOYMENT_PROVIDER_ID, method, params)
      .await
  }

  pub async fn invoke(&self, method: &str, params: Value) -> Result<Value, ProviderError> {
    self
      .invoke_provider(ANDROIDTONE_PROVIDER_ID, method, params)
      .await
  }

  pub async fn invoke_provider(
    &self,
    provider_id: &str,
    method: &str,
    params: Value,
  ) -> Result<Value, ProviderError> {
    let body =
      serde_json::to_vec(&json!({ "method": method, "params": params })).map_err(protocol_error)?;
    let response = self
      .request(
        Method::POST,
        &format!("/v1/providers/{provider_id}/invoke"),
        Some(body),
      )
      .await?;
    if response.get("androidtoneProviderError").is_some() {
      return Err(ProviderError::from_payload(
        &response["androidtoneProviderError"],
        Some(provider_id),
      ));
    }
    if response["error"].is_object()
      && (response["error"]["code"].is_string() || response["error"]["message"].is_string())
    {
      return Err(ProviderError::from_payload(&response, Some(provider_id)));
    }
    Ok(response)
  }

  async fn request(
    &self,
    method: Method,
    path: &str,
    body: Option<Vec<u8>>,
  ) -> Result<Value, ProviderError> {
    tokio::time::timeout(
      Duration::from_secs(480),
      self.request_inner(method, path, body),
    )
    .await
    .map_err(|_| ProviderError::local("AgentOS Host Provider 请求超时"))?
  }

  async fn request_inner(
    &self,
    method: Method,
    path: &str,
    body: Option<Vec<u8>>,
  ) -> Result<Value, ProviderError> {
    let socket = self
      .socket
      .as_ref()
      .ok_or_else(|| ProviderError::local("AgentOS 尚未向 AndroidTone 授予 Host Provider。"))?;
    let stream = UnixStream::connect(socket)
      .await
      .map_err(|error| ProviderError::local(format!("无法连接 AgentOS Host Provider：{error}")))?;
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream))
      .await
      .map_err(protocol_error)?;
    tokio::spawn(async move {
      let _ = connection.await;
    });
    let bytes = body.unwrap_or_default();
    let mut builder = Request::builder()
      .method(method)
      .uri(format!("http://agentos{path}"))
      .header(HOST, "agentos")
      .header("connection", "close");
    if !bytes.is_empty() {
      builder = builder.header("content-type", "application/json");
    }
    let request = builder
      .body(http_body_util::Full::new(Bytes::from(bytes)))
      .map_err(protocol_error)?;
    let response = sender.send_request(request).await.map_err(protocol_error)?;
    decode(response, path).await
  }
}

async fn decode(
  response: hyper::Response<Incoming>,
  provider_path: &str,
) -> Result<Value, ProviderError> {
  let status = response.status();
  let body = http_body_util::BodyExt::collect(response.into_body())
    .await
    .map_err(protocol_error)?
    .to_bytes();
  let value: Value = serde_json::from_slice(&body).map_err(protocol_error)?;
  if !status.is_success() {
    let provider_id = provider_path
      .strip_prefix("/v1/providers/")
      .and_then(|value| value.split('/').next());
    return Err(ProviderError::from_payload(&value, provider_id));
  }
  if let Some(error) = structured_provider_error(&value, provider_path) {
    return Err(error);
  }
  Ok(value)
}

fn structured_provider_error(value: &Value, provider_path: &str) -> Option<ProviderError> {
  let error = value.get("error")?;
  if !error.is_object()
    || !(error.get("code").is_some_and(Value::is_string)
      || error.get("message").is_some_and(Value::is_string))
  {
    return None;
  }
  let provider_id = provider_path
    .strip_prefix("/v1/providers/")
    .and_then(|value| value.split('/').next());
  Some(ProviderError::from_payload(value, provider_id))
}

fn protocol_error(error: impl std::fmt::Display) -> ProviderError {
  ProviderError::local(format!("AndroidTone Provider 协议错误：{error}"))
}

pub fn unavailable_status(error: Option<&ProviderError>) -> Value {
  json!({ "available": false, "installed": false, "running": false, "state": "provider-unavailable", "runtimeBundle": crate::bundle::status(), "canInstall": false, "message": error.map(|e| e.message.as_str()).unwrap_or("等待 AndroidTone Host Provider 授权"), "providerError": error.map(ProviderError::as_value) })
}

#[derive(Debug)]
enum HostProviderError {
  Message(String),
  Coded { code: String, message: String },
  Agent(ProviderError),
}

impl From<String> for HostProviderError {
  fn from(message: String) -> Self {
    Self::Message(message)
  }
}

impl From<&str> for HostProviderError {
  fn from(message: &str) -> Self {
    Self::Message(message.to_owned())
  }
}

impl HostProviderError {
  fn coded(code: &str, message: impl Into<String>) -> Self {
    Self::Coded {
      code: code.into(),
      message: message.into(),
    }
  }
}

impl From<ProviderError> for HostProviderError {
  fn from(error: ProviderError) -> Self {
    Self::Agent(error)
  }
}

impl std::fmt::Display for HostProviderError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Message(message) => formatter.write_str(message),
      Self::Coded { message, .. } => formatter.write_str(message),
      Self::Agent(error) => error.fmt(formatter),
    }
  }
}

impl std::error::Error for HostProviderError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderRequest {
  #[serde(rename = "providerId")]
  provider_id: String,
  method: String,
  #[serde(default)]
  params: Value,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct HostState {
  display_session_id: Option<String>,
  desktop_session_id: Option<String>,
  owns_container: bool,
  deployment_task_id: Option<String>,
  // Retained as a migration aid for older state files. New submissions use
  // deployment_request_binding and never reuse this key without a fingerprint.
  deployment_request_key: Option<String>,
  runtime_operation_version: u32,
  deployment_request_binding: Option<DeploymentRequestBinding>,
  deployment_request_conflicted: bool,
  runtime_deployed: bool,
  recovery_task_id: Option<String>,
  recovery_request_key: Option<String>,
  reauthorizations: Vec<ReauthorizationRecord>,
  install_tasks: Vec<InstallRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeSetupRequestIdentity {
  protocol_version: u32,
  operation_id: String,
  operation_version: u32,
  parameters: Value,
  deployment_fingerprint: String,
  request_fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeploymentRequestBinding {
  request: RuntimeSetupRequestIdentity,
  idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReauthorizationRecord {
  denied_task_id: String,
  idempotency_key: String,
  target: String,
  #[serde(default)]
  request_id: Option<String>,
  #[serde(default)]
  state: Option<String>,
  #[serde(default)]
  response: Option<Value>,
  #[serde(default)]
  new_task_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallRecord {
  id: String,
  file_name: String,
  bytes: u64,
  sha256: String,
  status: String,
  message: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  diagnostic: Option<String>,
}

struct HostProvider {
  client: ProviderClient,
  data_dir: PathBuf,
  state_path: PathBuf,
}

impl HostProvider {
  fn from_env() -> Result<Self, String> {
    let data_dir = env::var_os("AGENTOS_APP_DATA_DIR")
      .map(PathBuf::from)
      .ok_or_else(|| "AgentOS did not provide AndroidTone's private data directory.".to_owned())?;
    fs::create_dir_all(&data_dir)
      .map_err(|error| format!("Cannot open AndroidTone data directory: {error}"))?;
    Ok(Self {
      client: ProviderClient::from_env(),
      state_path: data_dir.join(".androidtone-host-state.json"),
      data_dir,
    })
  }

  async fn invoke(&self, method: &str, params: Value) -> Result<Value, HostProviderError> {
    if method.starts_with("runtime.") {
      crate::platform::validate_runtime_params(&params)?;
    }
    match method {
      "runtime.status" => self.status().await.map_err(Into::into),
      "runtime.install" => self.install_runtime().await,
      "runtime.initialize" => self.initialize_runtime().await,
      "runtime.start" => self.start_runtime().await,
      "runtime.recover" => self.recover_runtime().await.map_err(Into::into),
      "runtime.reauthorize" => self.reauthorize_denied_task().await.map_err(Into::into),
      "runtime.stop" => self.stop_runtime().await.map_err(Into::into),
      "applications.list" => self.applications().await.map_err(Into::into),
      "applications.launch" => self.app_action("launch", &params).await.map_err(Into::into),
      "applications.stop" => self.app_action("stop", &params).await.map_err(Into::into),
      "applications.uninstall" => self
        .app_action("uninstall", &params)
        .await
        .map_err(Into::into),
      "installations.register" => self.register_installation(params).await.map_err(Into::into),
      "installations.install" => self.install_apk(&params).await.map_err(Into::into),
      "installations.list" => self.list_installations().await.map_err(Into::into),
      "installations.remove" => self.remove_installation(&params).await.map_err(Into::into),
      _ => Err("AndroidTone Host Provider does not register this method.".into()),
    }
  }

  pub(crate) async fn reauthorize_denied_task(&self) -> Result<Value, String> {
    let mut state = self.load_state()?;
    let mut denied = None;
    for (target, id) in [
      ("recovery", state.recovery_task_id.clone()),
      ("setup", state.deployment_task_id.clone()),
    ] {
      let Some(id) = id else { continue };
      let task = self
        .client
        .system_deployment_invoke("task.get", json!({"taskId": id}))
        .await
        .map_err(|error| {
          format!(
            "Cannot verify denied deployment task {id}: {}",
            error.message
          )
        })?;
      if task["state"] == "denied" {
        denied = Some((target.to_owned(), id));
        break;
      }
    }
    let (target, denied_task_id) =
      denied.ok_or("重新授权只适用于当前确认为 denied 的 AndroidTone 部署任务。")?;

    let existing = state
      .reauthorizations
      .iter()
      .rposition(|request| request.denied_task_id == denied_task_id);
    if let Some(index) = existing {
      let request_id = state.reauthorizations[index].request_id.clone();
      if let Some(request_id) = request_id {
        let current = self
          .client
          .system_deployment_invoke(
            "authorization.reauthorization.get",
            json!({"requestId": request_id}),
          )
          .await
          .map_err(|error| error.message)?;
        if reauthorization_pending(&current) {
          update_reauthorization_record(&mut state, index, &current);
          self.save_state(&state)?;
          return Ok(current);
        }
        if let Some(record) = state.reauthorizations.get_mut(index) {
          record.state = current["state"].as_str().map(str::to_owned);
          record.response = Some(current);
        }
      } else {
        // A previous Provider response may have been lost. Reuse the key saved
        // before the call so AgentOS returns the existing request, if any.
        return self.submit_reauthorization(&mut state, index).await;
      }
    }

    let key = format!("androidtone-reauth-{}", Uuid::new_v4());
    if !(8..=128).contains(&key.len()) || !key.bytes().all(safe_idempotency_byte) {
      return Err("无法生成符合 AgentOS 要求的重新授权幂等键。".into());
    }
    state.reauthorizations.push(ReauthorizationRecord {
      denied_task_id,
      idempotency_key: key,
      target,
      request_id: None,
      state: Some("requestUnconfirmed".into()),
      response: None,
      new_task_id: None,
    });
    let index = state.reauthorizations.len() - 1;
    self.save_state(&state)?;
    self.submit_reauthorization(&mut state, index).await
  }

  async fn submit_reauthorization(
    &self,
    state: &mut HostState,
    index: usize,
  ) -> Result<Value, String> {
    let record = state
      .reauthorizations
      .get(index)
      .ok_or("重新授权记录不存在。")?;
    let result = self
      .client
      .system_deployment_invoke(
        "authorization.reauthorize",
        json!({
          "protocolVersion": 1,
          "taskId": record.denied_task_id,
          "idempotencyKey": record.idempotency_key
        }),
      )
      .await
      .map_err(|error| format!("AgentOS 重新授权请求未确认，可点击重试：{}", error.message))?;
    if result["requestId"].as_str().is_none() || result["state"].as_str().is_none() {
      return Err("AgentOS 重新授权响应缺少 requestId 或 state；可使用已保存的幂等键重试。".into());
    }
    update_reauthorization_record(state, index, &result);
    self.save_state(state)?;
    Ok(result)
  }

  async fn poll_reauthorization(&self, state: &mut HostState) -> Result<Option<Value>, String> {
    let Some(index) = state.reauthorizations.len().checked_sub(1) else {
      return Ok(None);
    };
    let Some(request_id) = state.reauthorizations[index].request_id.clone() else {
      return Ok(Some(reauthorization_record_view(
        &state.reauthorizations[index],
      )));
    };
    if !matches!(
      state.reauthorizations[index].state.as_deref(),
      Some("awaitingShellConfirmation" | "validating" | "requestUnconfirmed")
    ) {
      return Ok(Some(reauthorization_record_view(
        &state.reauthorizations[index],
      )));
    }
    let result = self
      .client
      .system_deployment_invoke(
        "authorization.reauthorization.get",
        json!({"requestId": request_id}),
      )
      .await
      .map_err(|error| {
        format!(
          "Cannot track AgentOS reauthorization request: {}",
          error.message
        )
      })?;
    update_reauthorization_record(state, index, &result);
    if let Some(new_task_id) = state.reauthorizations[index].new_task_id.clone() {
      match state.reauthorizations[index].target.as_str() {
        "recovery" => state.recovery_task_id = Some(new_task_id),
        "setup" => state.deployment_task_id = Some(new_task_id),
        _ => return Err("AgentOS reauthorization has an unknown task target.".into()),
      }
    }
    self.save_state(state)?;
    Ok(Some(result))
  }

  fn load_state(&self) -> Result<HostState, String> {
    match fs::read(&self.state_path) {
      Ok(bytes) => serde_json::from_slice(&bytes)
        .map_err(|error| format!("AndroidTone host state is invalid: {error}")),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HostState::default()),
      Err(error) => Err(format!("Cannot read AndroidTone host state: {error}")),
    }
  }

  fn load_owned_state(&self, session: &SessionContext) -> Result<HostState, String> {
    let state = self.load_state()?;
    if (state.display_session_id.is_some() || state.owns_container)
      && state.desktop_session_id.as_deref() != Some(session.session_id.as_str())
    {
      return Err(
        "platform-dependency: 持久化的运行环境归属与可信桌面会话不匹配；不接管已有会话。".into(),
      );
    }
    Ok(state)
  }

  fn save_state(&self, state: &HostState) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(state).map_err(|error| error.to_string())?;
    let temporary = self.state_path.with_extension("tmp");
    let mut file = OpenOptions::new()
      .create(true)
      .truncate(true)
      .write(true)
      .open(&temporary)
      .map_err(|error| format!("Cannot write AndroidTone host state: {error}"))?;
    file
      .write_all(&bytes)
      .and_then(|_| file.sync_all())
      .map_err(|error| format!("Cannot save AndroidTone host state: {error}"))?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
      .map_err(|error| error.to_string())?;
    fs::rename(&temporary, &self.state_path)
      .map_err(|error| format!("Cannot commit AndroidTone host state: {error}"))
  }

  /// AgentOS may stop exposing an expired authorization task through task.get.
  /// Consult the read-only authorization status index before classifying it;
  /// this keeps the persisted task ID and lets the UI distinguish an expired
  /// task from an unavailable Provider.
  async fn tracked_task(&self, task_id: &str) -> Result<Value, ProviderError> {
    match self
      .client
      .system_deployment_invoke("task.get", json!({"taskId": task_id}))
      .await
    {
      Ok(task) => Ok(task),
      Err(mut task_error) => {
        let authorization_status = self
          .client
          .system_deployment_invoke("authorization.status", json!({}))
          .await;
        if let Ok(status) = &authorization_status {
          if let Some(mut task) = task_from_authorization_status(status, task_id) {
            task["trackingDiagnostic"] = json!({
              "source": "authorization.status",
              "taskGetCode": task_error.code,
              "taskGetMessage": task_error.message,
              "matchedTask": true
            });
            return Ok(task);
          }
        }

        let status_diagnostic = match authorization_status {
          Ok(status) => json!({
            "source": "authorization.status",
            "matchedTask": false,
            "recentTaskCount": status["recent"].as_array().map(Vec::len),
            "pendingTaskCount": status["pending"].as_array().map(Vec::len)
          }),
          Err(error) => json!({
            "source": "authorization.status",
            "queryErrorCode": error.code,
            "queryErrorMessage": error.message
          }),
        };
        if authorization_expired_error(&task_error) {
          return Ok(json!({
            "taskId": task_id,
            "state": "expired",
            "error": {"code": task_error.code, "message": task_error.message},
            "trackingDiagnostic": {
              "source": "task.get-expiration-fallback",
              "authorizationStatus": status_diagnostic
            }
          }));
        }
        task_error.diagnostics = Some(json!({
          "taskGet": {"code": task_error.code, "message": task_error.message},
          "authorizationStatus": status_diagnostic
        }));
        Err(task_error)
      }
    }
  }

  async fn status(&self) -> Result<Value, String> {
    let kernel = kernel_observations();
    let mut host_state = self.load_state()?;
    let reauthorization = self.poll_reauthorization(&mut host_state).await?;
    let session = self
      .client
      .desktop_session()
      .await
      .map_err(|error| error.message);
    let deployment = self.client.system_deployment_capabilities().await;
    let deployment_status = match deployment {
      Ok(value) => value,
      Err(error) => json!({
        "protocolVersion": null,
        "implemented": false,
        "available": false,
        "reason": "provider-unavailable",
        "message": error.message
      }),
    };
    let runtime_usable = waydroid_binary().is_some() && waydroid_initialized();
    let installed_image_package_version = installed_waydroid_image_package_version();
    let image_package_current =
      installed_image_package_version.as_deref() == Some(WAYDROID_IMAGE_PACKAGE_VERSION);
    let mut deployment_actions = deployment_status
      .get("actions")
      .cloned()
      .unwrap_or_else(|| json!({}));
    if let Some(actions) = deployment_actions.as_object_mut() {
      for (name, action) in actions {
        if let Some(fields) = action.as_object_mut() {
          fields.insert(
            "requiredForAndroidToneSetup".into(),
            Value::Bool(if runtime_usable {
              RECOVERY_ACTIONS.contains(&name.as_str())
            } else {
              SETUP_ACTIONS.contains(&name.as_str())
            }),
          );
        }
      }
    }
    let can_install = deployment_actions_available(&deployment_actions);
    let blockers = deployment_blockers(&deployment_status);
    let dependencies = json!({
      "administratorOperations": if deployment_status["implemented"] == true { "capability-reported" } else { "platform-dependency" },
      "trustedSessionContext": if session.is_ok() { "provided" } else { "platform-dependency" },
      "privateWaylandContext": "provided-on-session-start",
      "windowPresentation": "unverified",
      "systemDeployment": deployment_status
    });
    let deployment_task = match host_state.deployment_task_id.as_deref() {
      Some(task_id) => match self.tracked_task(task_id).await {
        Ok(task) => Some(task),
        Err(error) => {
          return Ok(json!({
            "available": false, "installed": true, "initialized": true,
            "running": false, "state": "deployment-tracking-failed",
            "canInstall": false, "canInitialize": false, "canStart": false,
            "deploymentTask": {"taskId": task_id},
            "deploymentTrackingDiagnostic": error.as_value(),
            "kernel": kernel, "platformDependencies": dependencies,
            "deploymentActions": deployment_actions,
            "runtimeBundle": crate::bundle::status(),
            "message": "暂时无法读取 AgentOS 部署任务状态；AndroidTone 未重新提交部署。请展开详情查看诊断并手动刷新。"
          }));
        }
      },
      None => None,
    };
    if host_state.deployment_request_conflicted && deployment_task.is_none() {
      return Ok(json!({
        "available": false, "installed": false, "initialized": false,
        "running": false, "state": "idempotency-conflict",
        "canInstall": can_install, "canInitialize": false, "canStart": false,
        "deploymentTask": null, "kernel": kernel,
        "platformDependencies": dependencies, "deploymentActions": deployment_actions,
        "runtimeBundle": crate::bundle::status(),
        "message": "AgentOS 检测到部署请求标识冲突。请刷新 Android 运行环境状态，再手动重新检查。"
      }));
    }
    if let Some(task) = deployment_task
      .as_ref()
      .filter(|task| task["state"].as_str() == Some("blocked"))
    {
      return Ok(json!({
        "available": false, "installed": false, "initialized": false,
        "running": false, "state": "preflight-blocked",
        "canInstall": can_install, "canInitialize": false, "canStart": false,
        "deploymentTask": task, "kernel": kernel,
        "platformDependencies": dependencies, "deploymentActions": deployment_actions,
        "runtimeBundle": crate::bundle::status(),
        "message": preflight_blocked_summary(task)
      }));
    }
    if let Some(task) = deployment_task.as_ref().filter(|task| {
      !runtime_usable
        || matches!(
          task["state"].as_str(),
          Some(
            "awaitingAuthorization"
              | "running"
              | "denied"
              | "cancelled"
              | "canceled"
              | "expired"
              | "failed"
              | "unsupported"
          )
        )
    }) {
      match task.get("state").and_then(Value::as_str) {
        Some("completed") => {
          host_state.runtime_deployed = true;
          self.save_state(&host_state)?;
        }
        Some("awaitingAuthorization" | "running") => {
          let state = task["state"].as_str().unwrap_or("running");
          return Ok(json!({
            "available": false, "installed": false, "initialized": false,
            "running": false, "state": if state == "awaitingAuthorization" { "awaiting-authorization" } else { "deploying" },
            "canInstall": false, "canInitialize": false, "canStart": false,
            "deploymentTask": task, "kernel": kernel,
            "platformDependencies": dependencies, "deploymentActions": deployment_actions,
            "runtimeBundle": crate::bundle::status(),
            "message": if state == "awaitingAuthorization" { "Android 环境安装请求已提交，请在 AgentOS 系统授权窗口中确认。" } else { "AgentOS 正在安装离线组件并初始化 Android 系统镜像。" }
          }));
        }
        Some("denied") => {
          let reauth = reauthorization_for_task(&host_state, task);
          let mut view = json!({
            "available": false, "installed": false, "initialized": false,
            "running": false, "state": if reauth.as_ref().is_some_and(reauthorization_pending) { "reauthorization-pending" } else { "authorization-denied" },
            "canInstall": false, "canInitialize": false, "canStart": false,
            "deploymentTask": task, "kernel": kernel,
            "platformDependencies": dependencies, "deploymentActions": deployment_actions,
            "runtimeBundle": crate::bundle::status(),
            "message": if reauth.as_ref().is_some_and(reauthorization_pending) { reauthorization_message(reauth.as_ref().unwrap()) } else { "AgentOS 已记录管理员对这项操作的拒绝结果；重复提交不会再次弹窗。".into() }
          });
          attach_reauthorization(&mut view, reauth);
          return Ok(view);
        }
        Some("expired") => {
          let lineage = deployment_lineage(&host_state, task);
          let mut view = json!({
            "available": false, "installed": false, "initialized": false,
            "running": false, "state": "authorization-expired",
            "canInstall": false, "canInitialize": false, "canStart": false,
            "canReapply": can_install, "deploymentKind": "setup",
            "deploymentTask": task, "deploymentTrackingDiagnostic": task.get("trackingDiagnostic"),
            "kernel": kernel, "platformDependencies": dependencies,
            "deploymentActions": deployment_actions,
            "runtimeBundle": crate::bundle::status(),
            "message": "系统授权请求已过期；本次部署已停止。只有点击“重新申请”后才会创建新的授权请求。"
          });
          if let Some(lineage) = lineage {
            view["deploymentLineage"] = lineage;
          }
          attach_reauthorization(&mut view, reauthorization_for_task(&host_state, task));
          return Ok(view);
        }
        Some("cancelled" | "canceled") => {
          return Ok(json!({
            "available": false, "installed": false, "initialized": false,
            "running": false, "state": "authorization-cancelled",
            "canInstall": false, "canInitialize": false, "canStart": false,
            "deploymentTask": task, "kernel": kernel,
            "platformDependencies": dependencies, "deploymentActions": deployment_actions,
            "runtimeBundle": crate::bundle::status(),
            "message": "系统授权已取消；AndroidTone 已停止本次操作，不会自动重新申请。"
          }));
        }
        Some("unsupported") => {
          return Ok(json!({
            "available": false, "installed": false, "initialized": false,
            "running": false, "state": "platform-dependency",
            "canInstall": false, "canInitialize": false, "canStart": false,
            "deploymentTask": task, "kernel": kernel,
            "platformDependencies": dependencies, "deploymentActions": deployment_actions,
            "runtimeBundle": crate::bundle::status(),
            "message": "AgentOS 未接受本次操作所需的系统能力；未请求管理员授权。"
          }));
        }
        Some("failed") => {
          let detail = deployment_failure_summary(task);
          return Ok(json!({
            "available": false, "installed": false, "initialized": false,
            "running": false, "state": "deployment-failed",
            "canInstall": can_install, "canInitialize": false, "canStart": false,
            "deploymentTask": task, "kernel": kernel,
            "platformDependencies": dependencies, "deploymentActions": deployment_actions,
            "runtimeBundle": crate::bundle::status(), "message": detail
          }));
        }
        _ => {}
      }
    }
    let recovery_task = match host_state.recovery_task_id.as_deref() {
      Some(id) => match self.tracked_task(id).await {
        Ok(task) => Some(task),
        Err(error) => {
          return Ok(json!({
            "available": false, "installed": true, "initialized": true,
            "running": false, "state": "deployment-tracking-failed",
            "canInstall": false, "canInitialize": false, "canStart": false,
            "deploymentKind": "recovery", "deploymentTask": {"taskId": id},
            "deploymentTrackingDiagnostic": error.as_value(),
            "kernel": kernel, "platformDependencies": dependencies,
            "deploymentActions": deployment_actions,
            "runtimeBundle": crate::bundle::status(),
            "message": "暂时无法读取 Android 环境恢复任务状态；AndroidTone 未重复提交恢复请求。请展开详情查看诊断并手动刷新。"
          }));
        }
      },
      None => None,
    };
    if let Some(task) = recovery_task.as_ref() {
      if let Some(mut view) = recovery_task_view(task, &deployment_status) {
        view["kernel"] = kernel.clone();
        view["platformDependencies"] = dependencies.clone();
        view["deploymentActions"] = deployment_actions.clone();
        view["deploymentTrackingDiagnostic"] = task
          .get("trackingDiagnostic")
          .cloned()
          .unwrap_or(Value::Null);
        if let Some(lineage) = deployment_lineage(&host_state, task) {
          view["deploymentLineage"] = lineage;
        }
        attach_reauthorization(&mut view, reauthorization_for_task(&host_state, task));
        return Ok(view);
      }
    }
    let initialized = waydroid_initialized();
    let Some(binary) = waydroid_binary() else {
      if host_state.runtime_deployed {
        return Ok(json!({
          "available": false, "installed": true, "initialized": true,
          "running": false, "state": "runtime-ready", "canInstall": false,
          "canInitialize": false, "canStart": false, "deploymentTask": deployment_task,
          "kernel": kernel, "platformDependencies": dependencies,
          "deploymentActions": deployment_actions, "runtimeBundle": crate::bundle::status(),
          "message": "AgentOS 已完成 Waydroid 用户态组件和 Android 镜像部署，但 AndroidTone 进程环境无法访问宿主 Waydroid 命令；需要平台提供受限的运行时会话接口。"
        }));
      }
      return Ok(
        json!({"available": false, "installed": false, "initialized": false,
        "running": false, "state": "runtime-missing", "canInstall": can_install,
        "kernel": kernel, "platformDependencies": dependencies, "runtimeBundle": crate::bundle::status(),
        "message": if can_install { "Android 运行环境尚未安装；AndroidTone 将通过 AgentOS 提交受限的离线安装和镜像初始化任务。" } else { blockers.as_str() },
        "deploymentTask": deployment_task,
        "deploymentActions": deployment_actions}),
      );
    };
    let session = match session {
      Ok(session) => session,
      Err(error) => {
        return Ok(
          json!({"available": false, "installed": true, "initialized": initialized,
        "running": false, "state": "platform-dependency", "canInstall": false,
        "canInitialize": false, "canStart": false, "kernel": kernel,
        "platformDependencies": dependencies, "deploymentActions": deployment_actions,
        "runtimeBundle": crate::bundle::status(), "message": error}),
        );
      }
    };
    if !initialized {
      return Ok(
        json!({"available": false, "installed": true, "initialized": false,
        "running": false, "state": "uninitialized", "canInitialize": can_install,
        "kernel": kernel, "platformDependencies": dependencies, "runtimeBundle": crate::bundle::status(),
        "message": if can_install { "Android 系统镜像尚未初始化；将通过 AgentOS 请求授权部署。" } else { blockers.as_str() },
        "deploymentActions": deployment_actions}),
      );
    }
    let output = run_user(&session, &binary, &["status".into()]).await;
    let status_text = output
      .as_ref()
      .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
      .unwrap_or_default();
    let (container_running, session_running) = match output {
      Ok(output) if output.status.success() => {
        parse_status(&String::from_utf8_lossy(&output.stdout))
      }
      Ok(output) => {
        let message = diagnostic(&output);
        return Ok(
          json!({ "available": false, "installed": true, "initialized": true, "running": false, "state": "error", "message": message }),
        );
      }
      Err(error) => {
        return Ok(
          json!({ "available": false, "installed": true, "initialized": true, "running": false, "state": "error", "message": error }),
        )
      }
    };
    let state = self.load_owned_state(&session)?;
    if !image_package_current {
      let compatibility = waydroid_aarch32_image_mismatch().await;
      let can_update =
        can_install && !container_running && !session_running && state.display_session_id.is_none();
      let (runtime_state, message) = if let Some(message) = compatibility {
        ("image-incompatible", message)
      } else {
        (
          "runtime-update-required",
          format!(
            "Waydroid 镜像包需要更新：当前为 {}，AndroidTone 随包提供 {}。更新后会替换共享 system/vendor 镜像，保留 Waydroid 用户数据。",
            installed_image_package_version.as_deref().unwrap_or("未知版本"),
            WAYDROID_IMAGE_PACKAGE_VERSION
          ),
        )
      };
      let message = if container_running || session_running || state.display_session_id.is_some() {
        format!("{message} 请先停止 Android 环境，再执行镜像更新。")
      } else if !can_install {
        let blocked_by = deployment_blockers(&deployment_status);
        format!("{message} 镜像更新所需的 AgentOS 能力不可用：{blocked_by}")
      } else {
        message
      };
      return Ok(json!({
        "available": true, "installed": true, "initialized": true,
        "running": false, "state": runtime_state,
        "canInstall": can_update, "canInitialize": false, "canStart": false,
        "canRecover": false, "canStop": state.display_session_id.is_some() || state.owns_container,
        "installedImagePackageVersion": installed_image_package_version,
        "requiredImagePackageVersion": WAYDROID_IMAGE_PACKAGE_VERSION,
        "deploymentTask": deployment_task, "kernel": kernel,
        "platformDependencies": dependencies, "deploymentActions": deployment_actions,
        "runtimeBundle": crate::bundle::status(), "message": message
      }));
    }
    if let Some(message) = waydroid_aarch32_image_mismatch().await {
      let blockers = recovery_blockers(&deployment_status);
      let detail = if blockers.is_empty() {
        format!(
          "{message} Waydroid 仍使用旧 vendor rootfs；请通过 AgentOS 授权重启容器服务刷新镜像。"
        )
      } else {
        format!("{message} Waydroid 仍使用旧 vendor rootfs；刷新所需能力不可用：{blockers}")
      };
      return Ok(json!({
        "available": true, "installed": true, "initialized": true,
        "running": false, "state": "image-restart-required",
        "canInstall": false, "canInitialize": false,
        "canRecover": blockers.is_empty()
          && !container_running
          && !session_running
          && state.display_session_id.is_none(),
        "canStart": false, "canStop": state.display_session_id.is_some() || state.owns_container,
        "installedImagePackageVersion": installed_image_package_version,
        "requiredImagePackageVersion": WAYDROID_IMAGE_PACKAGE_VERSION,
        "deploymentTask": recovery_task, "kernel": kernel,
        "platformDependencies": dependencies, "deploymentActions": deployment_actions,
        "runtimeBundle": crate::bundle::status(),
        "message": if container_running || session_running || state.display_session_id.is_some() {
          format!("{message} Waydroid 仍使用旧 vendor rootfs；请先停止当前 Android 会话，再通过 AgentOS 重启容器服务刷新镜像。")
        } else {
          detail
        }
      }));
    }
    if session_running && state.display_session_id.is_none() {
      return Ok(
        json!({ "available": true, "installed": true, "initialized": true, "running": false, "state": "external-session", "kernel": kernel, "platformDependencies": dependencies, "deploymentActions": deployment_actions, "runtimeBundle": crate::bundle::status(), "message": "Waydroid 报告已有未绑定当前平台显示会话的 session；AndroidTone 不接管该会话。" }),
      );
    }
    let session_ready = recovery_allows_session(recovery_task.as_ref(), &status_text, &kernel);
    if !container_running && !session_running && !session_ready {
      let reason = recovery_blockers(&deployment_status);
      let allowed = reason.is_empty();
      let completed_but_stopped = recovery_task
        .as_ref()
        .is_some_and(|task| task["state"] == "completed")
        && binder_prepared(&kernel);
      let mut view = json!({
        "available": true, "installed": true, "initialized": true, "running": false,
        "state": if completed_but_stopped { "recovery-failed" } else { "recovery-required" },
        "canRecover": allowed, "canStart": allowed, "canInstall": false,
        "deploymentKind": "recovery", "deploymentTask": recovery_task,
        "kernel": kernel, "platformDependencies": dependencies, "deploymentActions": deployment_actions,
        "message": if !allowed { reason } else if completed_but_stopped {
          "系统恢复任务已完成，但 Waydroid 容器仍停止；请检查详情后重试恢复。".into()
        } else { "Android 运行环境已安装，需要系统授权准备 Binder 并重启容器服务。".into() }
      });
      attach_reauthorization(&mut view, reauthorization);
      return Ok(view);
    }
    let running = session_running && container_running && state.display_session_id.is_some();
    let current_state = if running {
      "process-running"
    } else if container_running {
      "container-running"
    } else if session_ready {
      "session-ready"
    } else {
      "stopped"
    };
    let mut view = json!({ "available": true, "installed": true, "initialized": true, "running": running, "windowVerification": {"mapping": "unknown", "flutterPresentation": "unknown", "inputFocus": "unknown", "scaling": "unknown", "multiWindow": "unknown", "gpuPresentation": "unknown", "exitCleanup": "unknown"}, "state": current_state, "deploymentKind": "recovery", "deploymentTask": recovery_task, "displaySessionId": state.display_session_id, "kernel": kernel, "platformDependencies": dependencies, "deploymentActions": deployment_actions, "runtimeBundle": crate::bundle::status(), "canStart": (container_running || session_ready) && !session_running && state.display_session_id.is_none(), "canStop": state.display_session_id.is_some() || state.owns_container, "message": if running { "Waydroid 进程报告运行；窗口映射、Shell 呈现及输入尚未验证。" } else if container_running { "Waydroid 容器已运行，可以启动 AgentOS 显示会话。" } else { "系统恢复已完成，正在创建 Android 用户会话；尚未验证窗口呈现。" } });
    attach_reauthorization(&mut view, reauthorization);
    Ok(view)
  }

  async fn install_runtime(&self) -> Result<Value, HostProviderError> {
    tokio::task::spawn_blocking(crate::bundle::verify)
      .await
      .map_err(|e| HostProviderError::Message(e.to_string()))??;
    let request = runtime_setup_request_identity().map_err(HostProviderError::Message)?;
    self.plan_runtime_setup(&request).await?;
    self.status().await.map_err(Into::into)
  }

  async fn plan_runtime_setup(
    &self,
    request: &RuntimeSetupRequestIdentity,
  ) -> Result<Option<Value>, HostProviderError> {
    let mut state = self.load_state().map_err(HostProviderError::Message)?;
    let mut retire_existing_task = false;
    if let Some(task_id) = state.deployment_task_id.as_deref() {
      let task = self.tracked_task(task_id).await?;
      match task["state"].as_str() {
        Some("awaitingAuthorization" | "running" | "denied") => return Ok(Some(task)),
        Some("blocked" | "failed" | "cancelled" | "canceled" | "expired") => {
          // Only this explicit install/recheck call can turn a known terminal
          // task into a new request. Since AgentOS has confirmed the old task's
          // result, the next attempt needs a fresh idempotency key. Keep the
          // old binding until capabilities are rechecked below.
          retire_existing_task = true;
        }
        Some("completed") => {
          let previous_version = state
            .deployment_request_binding
            .as_ref()
            .map(|binding| binding.request.operation_version)
            .unwrap_or(state.runtime_operation_version);
          let request_changed = state
            .deployment_request_binding
            .as_ref()
            .is_some_and(|binding| {
              binding.request.request_fingerprint != request.request_fingerprint
            });
          if !request_changed
            && previous_version == request.operation_version
            && installed_waydroid_image_package_version().as_deref()
              == Some(WAYDROID_IMAGE_PACKAGE_VERSION)
          {
            return Ok(Some(task));
          }
          retire_existing_task = true;
        }
        _ => return Ok(Some(task)),
      }
    }
    // Check again at the mutation boundary; frontend status can be stale and
    // callers may invoke this method without going through the UI.
    let capabilities = self.client.system_deployment_capabilities().await?;
    if !deployment_actions_available(&capabilities["actions"]) {
      return Ok(None);
    }
    if retire_existing_task {
      clear_deployment_request(&mut state);
      self
        .save_state(&state)
        .map_err(HostProviderError::Message)?;
    }
    let reuse_existing_key = !state.deployment_request_conflicted
      && state
        .deployment_request_binding
        .as_ref()
        .is_some_and(|binding| binding.request.request_fingerprint == request.request_fingerprint);
    let request_key = if reuse_existing_key {
      state
        .deployment_request_binding
        .as_ref()
        .expect("checked above")
        .idempotency_key
        .clone()
    } else {
      Uuid::new_v4().to_string()
    };
    state.deployment_request_binding = Some(DeploymentRequestBinding {
      request: request.clone(),
      idempotency_key: request_key.clone(),
    });
    state.deployment_request_key = Some(request_key.clone());
    state.runtime_operation_version = request.operation_version;
    state.deployment_request_conflicted = false;
    // Save the complete request identity before sending it. A subsequent
    // explicit retry can then distinguish an uncertain response from a new
    // operation version or changed action/package manifest.
    self
      .save_state(&state)
      .map_err(HostProviderError::Message)?;
    let task = match self
      .client
      .system_deployment_invoke("operation.plan", request.plan_parameters(&request_key))
      .await
    {
      Ok(task) => task,
      Err(error) if is_operation_already_active(&error.code) => {
        if let Some(task) = active_task_from_error(&error) {
          state.deployment_task_id = task["taskId"].as_str().map(str::to_owned);
          state.runtime_deployed = task["state"].as_str() == Some("completed");
          self
            .save_state(&state)
            .map_err(HostProviderError::Message)?;
          return Ok(Some(task));
        }
        return Err(HostProviderError::Agent(error));
      }
      Err(error) if is_idempotency_conflict(&error.code) => {
        // This key is known to be unusable for this request. Keep the binding
        // and rotate it only when the user explicitly invokes install/recheck.
        state.deployment_request_conflicted = true;
        self
          .save_state(&state)
          .map_err(HostProviderError::Message)?;
        return Err(HostProviderError::Agent(error));
      }
      Err(error) => return Err(HostProviderError::Agent(error)),
    };
    if is_operation_already_active(task["code"].as_str().unwrap_or("")) {
      if let Some(active) = active_task_from_value(&task) {
        state.deployment_task_id = active["taskId"].as_str().map(str::to_owned);
        state.runtime_deployed = active["state"].as_str() == Some("completed");
        self
          .save_state(&state)
          .map_err(HostProviderError::Message)?;
        return Ok(Some(active));
      }
    }
    if task
      .get("taskId")
      .and_then(Value::as_str)
      .filter(|id| !id.is_empty())
      .is_none()
      || task.get("state").and_then(Value::as_str).is_none()
    {
      return Err(HostProviderError::Message(
        "AgentOS returned an invalid deployment task; authorization was not assumed.".into(),
      ));
    }
    state.deployment_task_id = task
      .get("taskId")
      .and_then(Value::as_str)
      .map(str::to_owned);
    state.runtime_deployed = task.get("state").and_then(Value::as_str) == Some("completed");
    self
      .save_state(&state)
      .map_err(HostProviderError::Message)?;
    Ok(Some(task))
  }

  async fn initialize_runtime(&self) -> Result<Value, HostProviderError> {
    self.install_runtime().await
  }

  async fn recover_runtime(&self) -> Result<Value, String> {
    if waydroid_binary().is_none() || !waydroid_initialized() {
      return Err("恢复需要已安装的 Waydroid 和已初始化的镜像；不会重新安装离线包。".into());
    }
    let current = self.status().await?;
    // Repeated API calls after completion or during authorization only observe
    // the existing task. A new recovery is allowed solely for an actual stop
    // or an explicit retry of a recoverable failure.
    if !recovery_request_needed(&current) {
      return Ok(current);
    }
    let capabilities = self
      .client
      .system_deployment_capabilities()
      .await
      .map_err(|error| error.message)?;
    self.plan_recovery(&capabilities).await?;
    self.status().await
  }

  async fn plan_recovery(&self, capabilities: &Value) -> Result<Option<Value>, String> {
    let mut state = self.load_state()?;
    if !recovery_blockers(capabilities).is_empty() {
      return Ok(None);
    }
    let mut retire_existing_task = false;
    if let Some(id) = state.recovery_task_id.as_deref() {
      let task = self.tracked_task(id).await.map_err(|error| error.message)?;
      // Only an explicit recovery invocation may retry a failed/blocked task,
      // or a completed recovery whose actual container is still stopped.
      if !matches!(
        task["state"].as_str(),
        Some(
          "failed" | "blocked" | "unsupported" | "completed" | "cancelled" | "canceled" | "expired"
        )
      ) {
        return Ok(Some(task));
      }
      retire_existing_task = true;
    }
    if retire_existing_task {
      state.recovery_task_id = None;
      state.recovery_request_key = None;
      self.save_state(&state)?;
    }
    let key = state
      .recovery_request_key
      .clone()
      .unwrap_or_else(|| Uuid::new_v4().to_string());
    state.recovery_request_key = Some(key.clone());
    self.save_state(&state)?;
    let task = self
      .client
      .system_deployment_invoke("operation.plan", recovery_plan_parameters(&key))
      .await
      .map_err(|error| error.message)?;
    let id = task["taskId"]
      .as_str()
      .filter(|id| !id.is_empty())
      .ok_or("AgentOS recovery response has no task ID; authorization was not assumed.")?;
    if task["state"].as_str().is_none() {
      return Err("AgentOS recovery response has no state.".into());
    }
    state.recovery_task_id = Some(id.to_owned());
    self.save_state(&state)?;
    Ok(Some(task))
  }

  async fn start_runtime(&self) -> Result<Value, HostProviderError> {
    let binary = waydroid_binary().ok_or("宿主尚未安装 Waydroid。")?;
    if !waydroid_initialized() {
      return Err("请先初始化 Waydroid Android 系统镜像。".into());
    }
    if let Some(message) = waydroid_aarch32_image_mismatch().await {
      return Err(HostProviderError::coded(
        "waydroid-image-incompatible",
        message,
      ));
    }
    if installed_waydroid_image_package_version().as_deref() != Some(WAYDROID_IMAGE_PACKAGE_VERSION)
    {
      return Err(HostProviderError::coded(
        "waydroid-runtime-update-required",
        "AndroidTone 的 ARM64-only 镜像更新尚未安装；请先在页面中执行 Android 运行环境更新。",
      ));
    }
    let desktop_session = self.client.desktop_session().await?;
    // The package transaction starts the Waydroid system service. Reuse it when
    // active; only request the bounded platform operation if it is stopped.
    let mut state = self.load_owned_state(&desktop_session)?;
    let output = run_user(&desktop_session, &binary, &["status".into()])
      .await
      .map_err(|error| format!("无法读取 Waydroid 状态：{error}"))?;
    ensure_success(&output, "无法读取 Waydroid 状态")?;
    let (container_running, session_running) =
      parse_status(&String::from_utf8_lossy(&output.stdout));
    if let Some(session_id) = state.display_session_id.take() {
      if session_running && container_running {
        state.display_session_id = Some(session_id);
        return self.status().await.map_err(Into::into);
      }
      let _ = external_wayland_call(
        &self.client,
        "session.stop",
        json!({ "sessionId": session_id }),
        EXTERNAL_WAYLAND_CALL_TIMEOUT,
      )
      .await;
      state.desktop_session_id = None;
      self.save_state(&state)?;
    }
    if session_running {
      return Err(
        "Waydroid 报告已有未归属的 session；需要其所属会话退出，AndroidTone 不自行接管。".into(),
      );
    }
    if !container_running {
      let recovery_task = match state.recovery_task_id.as_deref() {
        Some(id) => Some(
          self
            .client
            .system_deployment_invoke("task.get", json!({"taskId": id}))
            .await?,
        ),
        None => None,
      };
      if !recovery_allows_session(
        recovery_task.as_ref(),
        &String::from_utf8_lossy(&output.stdout),
        &kernel_observations(),
      ) {
        return self.recover_runtime().await.map_err(Into::into);
      }
    } else if !state.owns_container {
      state.owns_container = false;
    }
    self.save_state(&state)?;
    let launch = json!({
      "version": 1,
      "applicationId": "com.agentos.androidtone",
      "executable": "/app/androidtone-server",
      "workingDirectory": "/app",
      "args": [
        "--agentos-wayland-client",
        serde_json::to_string(&desktop_session).map_err(|error| error.to_string())?
      ],
      "environment": {},
      "files": [],
      "permissions": ["gpu"],
      "windowHints": {"title": "Android application", "width": 900, "height": 680},
      "resources": {}
    });
    let startup_deadline = tokio::time::Instant::now() + ANDROID_STARTUP_TIMEOUT;
    let display = match tokio::time::timeout_at(
      startup_deadline,
      self.client.invoke_provider(
        EXTERNAL_WAYLAND_PROVIDER_ID,
        "session.start",
        json!({"launch": launch}),
      ),
    )
    .await
    {
      Ok(Ok(display)) => display,
      Ok(Err(error)) => {
        self
          .cleanup_failed_start(&desktop_session, &binary, &mut state, None)
          .await;
        return Err(error.into());
      }
      Err(_) => {
        self
          .cleanup_failed_start(&desktop_session, &binary, &mut state, None)
          .await;
        return Err(HostProviderError::Agent(ProviderError::for_provider(
          "external-wayland-session-start-timeout",
          EXTERNAL_WAYLAND_PROVIDER_ID,
          "AgentOS 创建 Android 私有显示会话超过 180 秒。Waydroid 命令和会话清理均已设置超时；请展开诊断并手动重试。",
        )));
      }
    };
    let session_id = display["sessionId"]
      .as_str()
      .ok_or("AgentOS did not create the Android display session.")?
      .to_owned();
    state.desktop_session_id = Some(desktop_session.session_id.clone());
    state.display_session_id = Some(session_id.clone());
    self.save_state(&state)?;
    let setup_result: Result<(), HostProviderError> = async {
      self
        .wait_until_ready(&desktop_session, &binary, startup_deadline)
        .await?;
      configure_multi_windows(&desktop_session, &binary, startup_deadline).await?;
      startup_wayland_call(
        &self.client,
        "session.prepare_restart",
        json!({"sessionId": session_id}),
        startup_deadline,
      )
      .await?;
      let stop = startup_user_command(
        &desktop_session,
        &binary,
        &["session".into(), "stop".into()],
        startup_deadline,
        "session-stop-for-restart",
      )
      .await?;
      ensure_success(&stop, "Waydroid 用户会话重启失败")?;
      startup_wayland_call(
        &self.client,
        "session.restart",
        json!({"sessionId": session_id}),
        startup_deadline,
      )
      .await?;
      self
        .wait_until_ready(&desktop_session, &binary, startup_deadline)
        .await?;
      wait_for_android_framework(&desktop_session, &binary, startup_deadline).await?;
      Ok::<(), HostProviderError>(())
    }
    .await;
    if let Err(error) = setup_result {
      self
        .cleanup_failed_start(&desktop_session, &binary, &mut state, Some(&session_id))
        .await;
      return Err(classify_start_error(error));
    }
    self.status().await.map_err(Into::into)
  }

  async fn cleanup_failed_start(
    &self,
    desktop_session: &SessionContext,
    binary: &Path,
    state: &mut HostState,
    display_session_id: Option<&str>,
  ) {
    let _ = run_user(desktop_session, binary, &["session".into(), "stop".into()]).await;
    if let Some(session_id) = display_session_id {
      let _ = external_wayland_call(
        &self.client,
        "session.stop",
        json!({"sessionId": session_id}),
        EXTERNAL_WAYLAND_CALL_TIMEOUT,
      )
      .await;
    }
    // Never retain a stale private Wayland session handle after a failed start,
    // even when either best-effort stop request times out.
    clear_display_session(state);
    let _ = self.save_state(state);
  }

  async fn stop_runtime(&self) -> Result<Value, String> {
    let desktop_session = self
      .client
      .desktop_session()
      .await
      .map_err(|error| error.message)?;
    let binary = waydroid_binary().ok_or("Waydroid is not installed.")?;
    let mut state = self.load_owned_state(&desktop_session)?;
    if let Some(session_id) = state.display_session_id.take() {
      let _ = run_user(
        &desktop_session,
        &binary,
        &["session".into(), "stop".into()],
      )
      .await;
      let _ = external_wayland_call(
        &self.client,
        "session.stop",
        json!({"sessionId": session_id}),
        EXTERNAL_WAYLAND_CALL_TIMEOUT,
      )
      .await;
      state.desktop_session_id = None;
    }
    if state.owns_container {
      let output = request_administrative_operation(AdministrativeOperation::StopContainer).await?;
      ensure_success(&output, "Waydroid 容器停止失败")?;
    }
    state.owns_container = false;
    self.save_state(&state)?;
    self.status().await
  }

  async fn wait_until_ready(
    &self,
    desktop_session: &SessionContext,
    binary: &Path,
    deadline: tokio::time::Instant,
  ) -> Result<(), HostProviderError> {
    let mut last_message = String::new();
    loop {
      let remaining = startup_remaining(deadline, "container-session-ready", &last_message)?;
      match run_user_with_timeout(
        desktop_session,
        binary,
        &["status".into()],
        remaining.min(WAYDROID_COMMAND_TIMEOUT),
      )
      .await
      {
        Ok(output) if output.status.success() => {
          let (container, session) = parse_status(&String::from_utf8_lossy(&output.stdout));
          if container && session {
            return Ok(());
          }
          last_message = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        }
        Ok(output) => last_message = diagnostic(&output),
        Err(error) => last_message = error,
      }
      startup_poll_delay(deadline, "container-session-ready", &last_message).await?;
    }
  }

  async fn applications(&self) -> Result<Value, String> {
    let binary = waydroid_binary().ok_or("Waydroid is not installed.")?;
    let desktop_session = self
      .client
      .desktop_session()
      .await
      .map_err(|error| error.message)?;
    let state = self.load_owned_state(&desktop_session)?;
    if state.display_session_id.is_none() {
      return Ok(json!({"available": false, "error": "请先启动 Waydroid 环境。", "apps": []}));
    }
    let output = run_user(&desktop_session, &binary, &["app".into(), "list".into()]).await?;
    ensure_success(&output, "无法读取 Waydroid 应用列表")?;
    let process_names: Vec<String> = Vec::new(); // No privileged Android shell probing.
    let mut apps = Vec::new();
    let mut name: Option<String> = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
      if let Some(value) = line.strip_prefix("Name: ") {
        name = Some(value.trim().to_owned());
      } else if let Some(value) = line.strip_prefix("packageName: ") {
        let package = value.trim();
        if valid_package_id(package) {
          let running = process_names
            .iter()
            .any(|process| process == package || process.starts_with(&format!("{package}:")));
          apps.push(json!({"id": package, "packageName": package, "name": name.take().filter(|v| !v.is_empty()).unwrap_or_else(|| package.to_owned()), "version": "Unknown", "running": running, "runningStateAvailable": !process_names.is_empty()}));
        }
        name = None;
      }
    }
    Ok(json!({"available": true, "apps": apps}))
  }

  async fn app_action(&self, action: &str, params: &Value) -> Result<Value, String> {
    let id = params
      .get("id")
      .and_then(Value::as_str)
      .filter(|id| valid_package_id(id))
      .ok_or("Android package ID is invalid.")?;
    let binary = waydroid_binary().ok_or("Waydroid is not installed.")?;
    let desktop_session = self
      .client
      .desktop_session()
      .await
      .map_err(|error| error.message)?;
    let state = self.load_owned_state(&desktop_session)?;
    if state.display_session_id.is_none() {
      return Err("请先启动 Waydroid 环境。".into());
    }
    if action == "stop" {
      request_administrative_operation(AdministrativeOperation::StopApplication).await?;
    }
    let args = match action {
      "launch" => vec!["app".into(), "launch".into(), id.into()],

      "uninstall" => vec!["app".into(), "remove".into(), id.into()],
      _ => return Err("Unsupported Android app action.".into()),
    };
    let output = run_user(&desktop_session, &binary, &args).await?;
    ensure_success(&output, "Waydroid 应用操作失败")?;
    Ok(match action {
      "launch" => {
        json!({"launchRequested": true, "windowVerified": false, "id": id, "message": "已提交启动请求；窗口呈现尚未验证。"})
      }
      "stop" => json!({"stopped": true, "id": id, "message": "Android 应用已停止。"}),
      _ => json!({"removed": true, "id": id, "message": "Android 应用已卸载。"}),
    })
  }

  async fn register_installation(&self, params: Value) -> Result<Value, String> {
    let id = params
      .get("id")
      .and_then(Value::as_str)
      .filter(|id| valid_task_id(id))
      .ok_or("Installation task ID is invalid.")?
      .to_owned();
    let file_name = params
      .get("fileName")
      .and_then(Value::as_str)
      .filter(|name| valid_apk_name(name))
      .ok_or("APK file name is invalid.")?
      .to_owned();
    let bytes = params
      .get("bytes")
      .and_then(Value::as_u64)
      .filter(|bytes| *bytes > 0 && *bytes <= MAX_APK_BYTES)
      .ok_or("APK size is invalid.")?;
    let sha = params
      .get("sha256")
      .and_then(Value::as_str)
      .filter(|sha| valid_sha256(sha))
      .ok_or("APK SHA-256 is invalid.")?
      .to_owned();
    let relative = format!("imports/{id}.apk");
    if params.get("dataRelativePath").and_then(Value::as_str) != Some(&relative) {
      return Err("APK reference is outside its approved AndroidTone import path.".into());
    }
    let source = self.resolve_app_data_file(&relative)?;
    verify_file(&source, bytes, &sha)?;
    let mut state = self.load_state()?;
    let record = InstallRecord {
      id: id.clone(),
      file_name,
      bytes,
      sha256: sha,
      status: "queued".into(),
      message: "APK is ready to install.".into(),
      diagnostic: None,
    };
    if let Some(previous) = state.install_tasks.iter_mut().find(|task| task.id == id) {
      *previous = record;
    } else {
      state.install_tasks.push(record);
    }
    self.save_state(&state)?;
    Ok(json!({"id": id, "status": "queued", "message": "APK 已通过完整性检查，可以安装。"}))
  }

  async fn install_apk(&self, params: &Value) -> Result<Value, String> {
    let id = params
      .get("id")
      .and_then(Value::as_str)
      .filter(|id| valid_task_id(id))
      .ok_or("Installation task ID is invalid.")?
      .to_owned();
    let binary = waydroid_binary().ok_or("Waydroid is not installed.")?;
    let desktop_session = self
      .client
      .desktop_session()
      .await
      .map_err(|error| error.message)?;
    if self
      .load_owned_state(&desktop_session)?
      .display_session_id
      .is_none()
    {
      return Err("请先启动 Waydroid 环境。".into());
    }
    let mut state = self.load_state()?;
    let index = state
      .install_tasks
      .iter()
      .position(|task| task.id == id)
      .ok_or("请重新导入 APK 后再安装。")?;
    if state.install_tasks[index].status == "installing" {
      return Err("此 APK 正在安装。".into());
    }
    state.install_tasks[index].status = "installing".into();
    state.install_tasks[index].message = "正在复制并校验 APK。".into();
    state.install_tasks[index].diagnostic = None;
    self.save_state(&state)?;
    let task = state.install_tasks[index].clone();
    let stage_dir = env::temp_dir().join(format!("androidtone-apk-{}", uuid::Uuid::new_v4()));
    let install_result = async {
      let source = self.resolve_app_data_file(&format!("imports/{id}.apk"))?;
      verify_file(&source, task.bytes, &task.sha256)?;
      let package_name = apk_package_name(&source)?;
      fs::DirBuilder::new()
        .mode(0o700)
        .create(&stage_dir)
        .map_err(|error| format!("Cannot prepare private APK staging directory: {error}"))?;
      let staged = stage_dir.join(format!("{id}.apk"));
      fs::copy(&source, &staged).map_err(|error| format!("Cannot stage APK: {error}"))?;
      fs::set_permissions(&staged, fs::Permissions::from_mode(0o400))
        .map_err(|error| error.to_string())?;
      verify_file(&staged, task.bytes, &task.sha256)?;
      let output = run_user(
        &desktop_session,
        &binary,
        &[
          "app".into(),
          "install".into(),
          staged.to_string_lossy().into_owned(),
        ],
      )
      .await?;
      ensure_waydroid_install_success(&output)?;
      self.wait_until_application_available(&package_name).await
    }
    .await;
    let _ = fs::remove_dir_all(&stage_dir);
    let mut state = self.load_state()?;
    if let Some(record) = state
      .install_tasks
      .iter_mut()
      .find(|record| record.id == id)
    {
      match &install_result {
        Ok(()) => {
          record.status = "installed".into();
          record.message = "APK 安装完成，已在 Android 应用列表中确认。".into();
          record.diagnostic = None;
        }
        Err(error) => {
          record.status = "failed".into();
          record.message = "APK 安装未能确认；请展开任务详情查看诊断后重试。".into();
          record.diagnostic = Some(error.clone());
        }
      }
    }
    self.save_state(&state)?;
    install_result.map_err(|_| "APK 安装未能确认；请展开任务详情查看诊断后重试。".to_owned())?;
    Ok(
      json!({"id": id, "status": "installed", "message": "APK 安装完成，已在 Android 应用列表中确认。", "verified": true}),
    )
  }

  async fn wait_until_application_available(&self, package_name: &str) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + APK_INSTALL_VERIFY_TIMEOUT;
    loop {
      let last_error = match self.applications().await {
        Ok(applications) => {
          if application_available(&applications, package_name) {
            return Ok(());
          }
          format!("Waydroid has not exposed `{package_name}` as a launchable application yet.")
        }
        Err(error) => error,
      };
      let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
      if remaining.is_zero() {
        return Err(format!(
          "APK install was not confirmed within {} seconds: {last_error}",
          APK_INSTALL_VERIFY_TIMEOUT.as_secs()
        ));
      }
      tokio::time::sleep(remaining.min(APK_INSTALL_VERIFY_INTERVAL)).await;
    }
  }

  async fn list_installations(&self) -> Result<Value, String> {
    Ok(json!({"installTasks": self.load_state()?.install_tasks}))
  }

  async fn remove_installation(&self, params: &Value) -> Result<Value, String> {
    let id = params
      .get("id")
      .and_then(Value::as_str)
      .filter(|id| valid_task_id(id))
      .ok_or("Installation task ID is invalid.")?;
    let mut state = self.load_state()?;
    state.install_tasks.retain(|task| task.id != id);
    self.save_state(&state)?;
    Ok(json!({"removed": true}))
  }

  fn resolve_app_data_file(&self, relative: &str) -> Result<PathBuf, String> {
    if relative.starts_with('/')
      || relative.contains('\\')
      || relative
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
      return Err("APK path is invalid.".into());
    }
    let root = fs::canonicalize(&self.data_dir).map_err(|error| error.to_string())?;
    let requested = root.join(relative);
    let canonical =
      fs::canonicalize(&requested).map_err(|error| format!("APK file is unavailable: {error}"))?;
    if !canonical.starts_with(&root) || !canonical.is_file() {
      return Err("APK file resolves outside AndroidTone's private data directory.".into());
    }
    Ok(canonical)
  }
}

pub async fn run_host_provider() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let mut input = String::new();
  std::io::stdin()
    .take(1024 * 1024 + 1)
    .read_to_string(&mut input)?;
  if input.len() > 1024 * 1024 {
    return Err("Provider request is larger than 1 MiB.".into());
  }
  let request: ProviderRequest = serde_json::from_str(&input)?;
  if request.provider_id != ANDROIDTONE_PROVIDER_ID
    || env::var("AGENTOS_PROVIDER_ID").as_deref() != Ok(ANDROIDTONE_PROVIDER_ID)
  {
    return Err("This executable is not authorized for the requested Host Provider ID.".into());
  }
  let app_id = env::var("AGENTOS_APP_ID").unwrap_or_default();
  if app_id != "com.agentos.androidtone" {
    return Err("AndroidTone Host Provider was invoked for a different App.".into());
  }
  let provider = HostProvider::from_env().map_err(std::io::Error::other)?;
  let lock_path = provider.data_dir.join(".androidtone-host-provider.lock");
  let lock = OpenOptions::new()
    .create(true)
    .read(true)
    .write(true)
    .open(lock_path)?;
  if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
    return Err(std::io::Error::last_os_error().into());
  }
  let result = provider.invoke(&request.method, request.params).await;
  match result {
    Ok(value) => {
      println!("{}", serde_json::to_string(&value)?);
      Ok(())
    }
    Err(HostProviderError::Agent(error)) => {
      println!(
        "{}",
        serde_json::to_string(&json!({
          "androidtoneProviderError": error.as_value()
        }))?
      );
      Ok(())
    }
    Err(HostProviderError::Coded { code, message }) => {
      println!(
        "{}",
        serde_json::to_string(&json!({
          "androidtoneProviderError": {
            "code": code,
            "message": message,
            "providerId": ANDROIDTONE_PROVIDER_ID
          }
        }))?
      );
      Ok(())
    }
    Err(HostProviderError::Message(message)) => {
      println!(
        "{}",
        serde_json::to_string(&json!({
          "androidtoneProviderError": {
            "code": "androidtone-host-provider-failed",
            "message": message,
            "providerId": ANDROIDTONE_PROVIDER_ID
          }
        }))?
      );
      Ok(())
    }
  }
}

async fn external_wayland_call(
  client: &ProviderClient,
  method: &str,
  params: Value,
  timeout: Duration,
) -> Result<Value, ProviderError> {
  tokio::time::timeout(
    timeout,
    client.invoke_provider(EXTERNAL_WAYLAND_PROVIDER_ID, method, params),
  )
  .await
  .map_err(|_| {
    ProviderError::for_provider(
      "external-wayland-request-timeout",
      EXTERNAL_WAYLAND_PROVIDER_ID,
      format!(
        "AgentOS Android 显示会话请求超过 {} 秒。",
        timeout.as_secs()
      ),
    )
  })?
}

pub async fn run_wayland_client(
  desktop_session_json: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let session =
    SessionContext::from_launch_argument(desktop_session_json).map_err(std::io::Error::other)?;
  let (runtime_dir, display) =
    private_wayland_environment(&session).map_err(std::io::Error::other)?;
  let pulse_runtime_dir = pulse_runtime_path(&session).map_err(std::io::Error::other)?;
  let binary = waydroid_binary().ok_or("Waydroid is not installed.")?;
  if let Some(message) = waydroid_aarch32_image_mismatch().await {
    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, message).into());
  }
  let mut command = Command::new(&binary);
  command
    .args(["session", "start"])
    .env_clear()
    .envs(session.environment())
    .env("XDG_RUNTIME_DIR", runtime_dir.clone())
    .env("PULSE_RUNTIME_PATH", pulse_runtime_dir.clone())
    .env("WAYLAND_DISPLAY", display.clone())
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let stop_signal = waydroid_termination_signal().map_err(std::io::Error::other)?;
  supervise_waydroid_session(
    command,
    &session,
    &binary,
    &runtime_dir,
    &pulse_runtime_dir,
    &display,
    ANDROID_STARTUP_TIMEOUT,
    stop_signal,
  )
  .await
  .map_err(std::io::Error::other)?;
  Ok(())
}

#[cfg(unix)]
fn waydroid_termination_signal() -> std::io::Result<impl Future<Output = ()> + Send + 'static> {
  let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
  let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
  Ok(async move {
    tokio::select! {
      _ = terminate.recv() => {},
      _ = interrupt.recv() => {},
    }
  })
}

#[cfg(not(unix))]
fn waydroid_termination_signal() -> impl Future<Output = ()> + Send + 'static {
  std::future::pending()
}

async fn supervise_waydroid_session<F>(
  mut command: Command,
  session: &SessionContext,
  binary: &Path,
  runtime_dir: &str,
  pulse_runtime_dir: &Path,
  display: &str,
  startup_timeout: Duration,
  stop_signal: F,
) -> Result<(), String>
where
  F: Future<Output = ()> + Send,
{
  command.kill_on_drop(true);
  let mut child = match command.spawn() {
    Ok(child) => child,
    Err(error) => {
      let cleanup =
        stop_waydroid_session(session, binary, runtime_dir, pulse_runtime_dir, display).await;
      return Err(match cleanup {
        Ok(()) => format!("无法启动 Waydroid 用户会话：{error}"),
        Err(cleanup_error) => {
          format!("无法启动 Waydroid 用户会话：{error}；有界清理也失败：{cleanup_error}")
        }
      });
    }
  };
  let stdout = child.stdout.take().map(spawn_output_drain);
  let stderr = child.stderr.take().map(spawn_output_drain);
  let startup_deadline = tokio::time::Instant::now() + startup_timeout;
  tokio::pin!(stop_signal);
  let framework_ready = wait_for_android_framework(session, binary, startup_deadline);
  tokio::pin!(framework_ready);

  let readiness = tokio::select! {
    status = child.wait() => {
      let message = cli_exit_before_ready(status, String::new());
      let cleanup = stop_waydroid_session(session, binary, runtime_dir, pulse_runtime_dir, display).await;
      let message = match cleanup {
        Ok(()) => message,
        Err(error) => format!("{message}；有界清理也失败：{error}"),
      };
      let logs = finish_child_output(stdout, stderr).await;
      return Err(append_child_diagnostics(message, logs));
    }
    result = &mut framework_ready => result.map_err(|error| host_provider_error_diagnostic(&error)),
    _ = &mut stop_signal => {
      let _ = cleanup_waydroid_child(&mut child, session, binary, runtime_dir, pulse_runtime_dir, display).await;
      let _ = finish_child_output(stdout, stderr).await;
      return Ok(());
    }
  };

  if let Err(error) = readiness {
    let _ = cleanup_waydroid_child(
      &mut child,
      session,
      binary,
      runtime_dir,
      pulse_runtime_dir,
      display,
    )
    .await;
    let logs = finish_child_output(stdout, stderr).await;
    return Err(append_child_diagnostics(error, logs));
  }

  // Framework readiness ends the startup deadline. The Waydroid CLI is the
  // live session process and may remain here indefinitely until a real exit
  // or an AgentOS/user stop signal arrives.
  let status = tokio::select! {
    status = child.wait() => status,
    _ = &mut stop_signal => {
      let _ = cleanup_waydroid_child(
        &mut child,
        session,
        binary,
        runtime_dir,
        pulse_runtime_dir,
        display,
      ).await;
      let _ = finish_child_output(stdout, stderr).await;
      return Ok(());
    }
  };
  let logs = finish_child_output(stdout, stderr).await;
  match status {
    Ok(status) if status.success() => Ok(()),
    Ok(status) => {
      let _ = stop_waydroid_session(session, binary, runtime_dir, pulse_runtime_dir, display).await;
      Err(append_child_diagnostics(
        format!("Waydroid 用户会话在 Android framework 就绪后异常退出（{status}）。"),
        logs,
      ))
    }
    Err(error) => {
      let _ = stop_waydroid_session(session, binary, runtime_dir, pulse_runtime_dir, display).await;
      Err(append_child_diagnostics(
        format!("无法监督 Waydroid 用户会话进程：{error}"),
        logs,
      ))
    }
  }
}

fn host_provider_error_diagnostic(error: &HostProviderError) -> String {
  match error {
    HostProviderError::Message(message) => message.clone(),
    HostProviderError::Coded { code, message } => format!("{code}: {message}"),
    HostProviderError::Agent(error) => format!("{}: {}", error.code, error.message),
  }
}

fn spawn_output_drain<R>(reader: R) -> tokio::task::JoinHandle<Vec<u8>>
where
  R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
  tokio::spawn(async move {
    let mut reader = reader;
    let mut tail = std::collections::VecDeque::with_capacity(WAYDROID_CHILD_OUTPUT_LIMIT);
    let mut chunk = [0_u8; 4096];
    loop {
      let count = match reader.read(&mut chunk).await {
        Ok(0) | Err(_) => break,
        Ok(count) => count,
      };
      for byte in &chunk[..count] {
        if tail.len() == WAYDROID_CHILD_OUTPUT_LIMIT {
          tail.pop_front();
        }
        tail.push_back(*byte);
      }
    }
    tail.into_iter().collect()
  })
}

async fn finish_child_output(
  stdout: Option<tokio::task::JoinHandle<Vec<u8>>>,
  stderr: Option<tokio::task::JoinHandle<Vec<u8>>>,
) -> String {
  let mut diagnostic = Vec::new();
  for output in [stdout, stderr].into_iter().flatten() {
    if let Ok(Ok(bytes)) = tokio::time::timeout(Duration::from_secs(2), output).await {
      if !bytes.is_empty() {
        diagnostic.extend(bytes);
      }
    }
  }
  String::from_utf8_lossy(&diagnostic).trim().to_owned()
}

fn cli_exit_before_ready(
  status: std::io::Result<std::process::ExitStatus>,
  logs: String,
) -> String {
  match status {
    Ok(status) => append_child_diagnostics(
      format!("Waydroid 用户会话在 Android framework 就绪前退出（{status}）。"),
      logs,
    ),
    Err(error) => {
      append_child_diagnostics(format!("无法监督 Waydroid 用户会话进程：{error}"), logs)
    }
  }
}

fn append_child_diagnostics(message: String, logs: String) -> String {
  if logs.is_empty() {
    message
  } else {
    format!(
      "{message} Waydroid 输出：{}",
      logs.chars().take(4000).collect::<String>()
    )
  }
}

async fn cleanup_waydroid_child(
  child: &mut Child,
  session: &SessionContext,
  binary: &Path,
  runtime_dir: &str,
  pulse_runtime_dir: &Path,
  display: &str,
) -> Result<(), String> {
  let stop = stop_waydroid_session(session, binary, runtime_dir, pulse_runtime_dir, display).await;
  if tokio::time::timeout(WAYDROID_SESSION_CHILD_GRACE, child.wait())
    .await
    .is_err()
  {
    let _ = child.start_kill();
    let _ = tokio::time::timeout(WAYDROID_SESSION_CHILD_GRACE, child.wait()).await;
  }
  stop
}

async fn stop_waydroid_session(
  session: &SessionContext,
  binary: &Path,
  runtime_dir: &str,
  pulse_runtime_dir: &Path,
  display: &str,
) -> Result<(), String> {
  let mut stop = Command::new(binary);
  stop
    .args(["session", "stop"])
    .env_clear()
    .envs(session.environment())
    .env("XDG_RUNTIME_DIR", runtime_dir)
    .env("PULSE_RUNTIME_PATH", pulse_runtime_dir)
    .env("WAYLAND_DISPLAY", display);
  let output = bounded_output(
    &mut stop,
    WAYDROID_SESSION_STOP_TIMEOUT,
    "Waydroid 用户会话清理",
  )
  .await?;
  ensure_success(&output, "Waydroid 用户会话清理失败")
}

fn waydroid_binary() -> Option<PathBuf> {
  ["/usr/bin/waydroid", "/usr/local/bin/waydroid"]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
}

async fn run_user(
  session: &SessionContext,
  binary: &Path,
  args: &[String],
) -> Result<Output, String> {
  run_user_with_timeout(session, binary, args, waydroid_command_timeout(args)).await
}

async fn run_user_with_timeout(
  session: &SessionContext,
  binary: &Path,
  args: &[String],
  timeout: Duration,
) -> Result<Output, String> {
  let mut command = Command::new(binary);
  command.args(args).env_clear().envs(session.environment());
  bounded_output(
    &mut command,
    timeout,
    &format!("Waydroid {}", args.join(" ")),
  )
  .await
}

fn startup_remaining(
  deadline: tokio::time::Instant,
  phase: &str,
  observation: &str,
) -> Result<Duration, HostProviderError> {
  let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
  if remaining.is_zero() {
    Err(startup_timeout(phase, observation))
  } else {
    Ok(remaining)
  }
}

fn startup_timeout(phase: &str, observation: &str) -> HostProviderError {
  let mut error = ProviderError::for_provider(
    "waydroid-startup-timeout",
    ANDROIDTONE_PROVIDER_ID,
    "Android 未在 180 秒启动总时限内完成准备。已停止本次启动；请查看诊断后手动重试。",
  );
  error.details = Some(json!({
    "phase": phase,
    "startupBudgetSeconds": ANDROID_STARTUP_TIMEOUT.as_secs(),
    "lastObservation": observation.chars().take(1600).collect::<String>(),
    "cleanup": "bounded-session-stop-and-private-wayland-release",
  }));
  HostProviderError::Agent(error)
}

async fn startup_poll_delay(
  deadline: tokio::time::Instant,
  phase: &str,
  observation: &str,
) -> Result<(), HostProviderError> {
  let remaining = startup_remaining(deadline, phase, observation)?;
  tokio::time::sleep(remaining.min(Duration::from_secs(1))).await;
  startup_remaining(deadline, phase, observation).map(|_| ())
}

async fn startup_user_command(
  session: &SessionContext,
  binary: &Path,
  args: &[String],
  deadline: tokio::time::Instant,
  phase: &str,
) -> Result<Output, HostProviderError> {
  let remaining = startup_remaining(deadline, phase, "command-not-started")?;
  // Property commands may wait for Android services for longer than the normal
  // CLI limit. Use the shared startup deadline; do not give each call a new budget.
  let result = run_user_with_timeout(session, binary, args, remaining).await;
  match result {
    Err(message) if tokio::time::Instant::now() >= deadline => {
      Err(startup_timeout(phase, &message))
    }
    Err(message) => Err(classify_start_error(message.into())),
    Ok(output) => Ok(output),
  }
}

async fn startup_wayland_call(
  client: &ProviderClient,
  method: &str,
  params: Value,
  deadline: tokio::time::Instant,
) -> Result<Value, HostProviderError> {
  let timeout =
    startup_remaining(deadline, method, "request-not-started")?.min(EXTERNAL_WAYLAND_CALL_TIMEOUT);
  match external_wayland_call(client, method, params, timeout).await {
    Err(error) if tokio::time::Instant::now() >= deadline => {
      Err(startup_timeout(method, &error.message))
    }
    result => result.map_err(Into::into),
  }
}

async fn wait_for_android_framework(
  session: &SessionContext,
  binary: &Path,
  deadline: tokio::time::Instant,
) -> Result<(), HostProviderError> {
  loop {
    let output = startup_user_command(
      session,
      binary,
      &["prop".into(), "get".into(), "sys.boot_completed".into()],
      deadline,
      "android-framework-ready",
    )
    .await?;
    if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "1" {
      return Ok(());
    }
    let observation = format!("sys.boot_completed != 1; {}", diagnostic(&output));
    startup_poll_delay(deadline, "android-framework-ready", &observation).await?;
  }
}

async fn configure_multi_windows(
  session: &SessionContext,
  binary: &Path,
  deadline: tokio::time::Instant,
) -> Result<(), HostProviderError> {
  // Container/Session RUNNING alone does not mean Android framework is ready.
  wait_for_android_framework(session, binary, deadline).await?;
  let property = "persist.waydroid.multi_windows";
  let output = startup_user_command(
    session,
    binary,
    &["prop".into(), "set".into(), property.into(), "true".into()],
    deadline,
    "multi-windows-property-set",
  )
  .await?;
  ensure_success(&output, "Waydroid 多窗口设置失败")?;
  loop {
    let output = startup_user_command(
      session,
      binary,
      &["prop".into(), "get".into(), property.into()],
      deadline,
      "multi-windows-property-verify",
    )
    .await?;
    if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true" {
      return Ok(());
    }
    let observation = format!("{property} != true; {}", diagnostic(&output));
    startup_poll_delay(deadline, "multi-windows-property-verify", &observation).await?;
  }
}

async fn bounded_output(
  command: &mut Command,
  timeout: Duration,
  description: &str,
) -> Result<Output, String> {
  command.kill_on_drop(true);
  match tokio::time::timeout(timeout, command.output()).await {
    Ok(Ok(output)) => Ok(output),
    Ok(Err(error)) => Err(format!("无法运行 {description}：{error}")),
    Err(_) => Err(format!(
      "{description} 超时（{} 秒）；AndroidTone 已终止该 CLI 进程。",
      timeout.as_secs()
    )),
  }
}

fn waydroid_command_timeout(args: &[String]) -> Duration {
  match args
    .iter()
    .map(String::as_str)
    .collect::<Vec<_>>()
    .as_slice()
  {
    ["session", "stop"] => WAYDROID_SESSION_STOP_TIMEOUT,
    ["app", "install", ..] => Duration::from_secs(180),
    ["app", "launch", ..] | ["app", "remove", ..] => Duration::from_secs(60),
    _ => WAYDROID_COMMAND_TIMEOUT,
  }
}

fn classify_start_error(error: HostProviderError) -> HostProviderError {
  match error {
    HostProviderError::Agent(error) => HostProviderError::Agent(error),
    HostProviderError::Coded { code, message } => HostProviderError::Coded { code, message },
    HostProviderError::Message(message) if message.to_ascii_lowercase().contains("超时") => {
      HostProviderError::coded("waydroid-command-timeout", message)
    }
    HostProviderError::Message(message) => {
      HostProviderError::coded("waydroid-session-start-failed", message)
    }
  }
}

async fn request_administrative_operation(
  operation: AdministrativeOperation,
) -> Result<Output, String> {
  // No approved platform contract currently exists. Never execute an application
  // script as root or attempt elevation from this no-new-privs Provider.
  Err(operation.platform_dependency())
}

fn ensure_success(output: &Output, context: &str) -> Result<(), String> {
  if output.status.success() {
    Ok(())
  } else {
    Err(format!("{context}：{}", diagnostic(output)))
  }
}

fn diagnostic(output: &Output) -> String {
  let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
  let text = if stderr.is_empty() { stdout } else { stderr };
  if text.is_empty() {
    format!("命令退出状态 {}", output.status)
  } else {
    text.chars().take(1600).collect()
  }
}

fn parse_status(output: &str) -> (bool, bool) {
  let running = |key: &str| {
    output.lines().any(|line| {
      line.split_once(':').is_some_and(|(name, value)| {
        name.trim().eq_ignore_ascii_case(key) && value.trim().eq_ignore_ascii_case("RUNNING")
      })
    })
  };
  (running("Container"), running("Session"))
}

fn binder_prepared(kernel: &Value) -> bool {
  kernel["moduleLoading"]["observed"] == true && kernel["binderfsMounting"]["state"] == "mounted"
}

fn recovery_allows_session(task: Option<&Value>, output: &str, kernel: &Value) -> bool {
  binder_prepared(kernel)
    && task.is_some_and(|task| task["state"] == "completed")
    && output.lines().any(|line| {
      line.split_once(':').is_some_and(|(key, value)| {
        key.trim().eq_ignore_ascii_case("Session") && value.trim().eq_ignore_ascii_case("STOPPED")
      })
    })
    && !output.lines().any(|line| {
      line
        .split_once(':')
        .is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("Container"))
    })
}

const RECOVERY_ACTIONS: [&str; 3] = [
  "linux.module.load",
  "linux.binderfs.ensure",
  "systemd.service.control",
];
const WAYDROID_UNIT: &str = "waydroid-container.service";

fn recovery_request_needed(status: &Value) -> bool {
  status["canRecover"] == true
    && matches!(
      status["state"].as_str(),
      Some(
        "recovery-required"
          | "recovery-failed"
          | "image-restart-required"
          | "authorization-expired"
      )
    )
}

fn recovery_plan_parameters(key: &str) -> Value {
  json!({"protocolVersion": 1, "operationId": "android-runtime-recover", "operationVersion": 1,
    "idempotencyKey": key, "parameters": {}})
}

fn recovery_blockers(capabilities: &Value) -> String {
  let mut reasons = Vec::new();
  if let Some(message) = capabilities["message"].as_str() {
    reasons.push(message.to_owned());
  }
  for name in RECOVERY_ACTIONS {
    let action = &capabilities["actions"][name];
    if action["available"] != true {
      reasons.push(format!(
        "{name}: {}",
        action["reason"]
          .as_str()
          .unwrap_or("capability-not-reported")
      ));
    }
  }
  let control = &capabilities["actions"]["systemd.service.control"];
  if control["available"] == true {
    if !control["allowedUnits"]
      .as_array()
      .is_some_and(|units| units.iter().any(|unit| unit == WAYDROID_UNIT))
    {
      reasons.push(format!(
        "systemd.service.control: systemd-unit-not-allowlisted ({WAYDROID_UNIT})"
      ));
    }
    if !control["operations"]
      .as_array()
      .is_some_and(|ops| ops.iter().any(|op| op == "restart"))
    {
      reasons.push("systemd.service.control: restart-not-supported".into());
    }
  }
  reasons.join("；")
}

fn recovery_task_view(task: &Value, capabilities: &Value) -> Option<Value> {
  let (state, message, retry) = match task["state"].as_str() {
    Some("completed") => return None,
    Some("awaitingAuthorization") => (
      "awaiting-authorization",
      "恢复请求已提交，请在 AgentOS Shell 中批准系统授权。".into(),
      false,
    ),
    Some("running") => (
      "deploying",
      "AgentOS 正在准备 Binder 并重启 Android 容器服务。".into(),
      false,
    ),
    Some("denied") => (
      "authorization-denied",
      "系统恢复授权已拒绝；不会重复申请。".into(),
      false,
    ),
    Some("expired") => (
      "authorization-expired",
      "系统恢复授权请求已过期；本次操作已停止。点击“重新申请”才会创建新的恢复任务。".into(),
      true,
    ),
    Some("cancelled" | "canceled") => (
      "authorization-cancelled",
      "系统恢复授权已取消；本次操作已停止。".into(),
      false,
    ),
    Some("failed" | "blocked" | "unsupported") => {
      ("recovery-failed", deployment_failure_summary(task), true)
    }
    _ => (
      "recovery-failed",
      "AgentOS 返回了未知恢复任务状态；已停止自动准备。".into(),
      false,
    ),
  };
  Some(
    json!({"available": false, "installed": true, "initialized": true, "running": false,
    "state": state, "message": message, "canInstall": false, "canStart": false,
    "canRecover": retry && recovery_blockers(capabilities).is_empty(),
    "canReapply": retry && recovery_blockers(capabilities).is_empty(),
    "deploymentKind": "recovery", "deploymentTask": task}),
  )
}

fn task_from_authorization_status(status: &Value, task_id: &str) -> Option<Value> {
  ["pending", "recent"]
    .into_iter()
    .filter_map(|key| status[key].as_array())
    .flat_map(|tasks| tasks.iter())
    .find(|task| task["taskId"].as_str() == Some(task_id) || task["id"].as_str() == Some(task_id))
    .cloned()
}

fn authorization_expired_error(error: &ProviderError) -> bool {
  error.code.to_ascii_lowercase().contains("expired")
    || error
      .message
      .to_ascii_lowercase()
      .contains("authorization request has expired")
    || error.message.contains("授权请求已过期")
}

fn deployment_lineage(state: &HostState, task: &Value) -> Option<Value> {
  let task_id = task["taskId"].as_str();
  let request_id = task["reauthorizationRequestId"].as_str();
  let retry_of = task["retryOfTaskId"].as_str();
  let record = state.reauthorizations.iter().rev().find(|record| {
    request_id.is_some_and(|request_id| record.request_id.as_deref() == Some(request_id))
      || task_id.is_some_and(|task_id| record.new_task_id.as_deref() == Some(task_id))
  });
  if retry_of.is_none() && request_id.is_none() && record.is_none() {
    return None;
  }
  Some(json!({
    "retryOfTaskId": retry_of.or_else(|| record.map(|record| record.denied_task_id.as_str())),
    "reauthorizationRequestId": request_id.or_else(|| record.and_then(|record| record.request_id.as_deref())),
    "reauthorizationState": record.and_then(|record| record.state.as_deref()),
    "newTaskId": record.and_then(|record| record.new_task_id.as_deref()),
    "agentOsResponse": record.and_then(|record| record.response.as_ref())
  }))
}

fn safe_idempotency_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || b"-._~".contains(&byte)
}

fn reauthorization_pending(request: &Value) -> bool {
  matches!(
    request["state"].as_str(),
    Some("awaitingShellConfirmation" | "validating")
  )
}

fn update_reauthorization_record(state: &mut HostState, index: usize, response: &Value) {
  let Some(record) = state.reauthorizations.get_mut(index) else {
    return;
  };
  if let Some(request_id) = response["requestId"].as_str() {
    record.request_id = Some(request_id.to_owned());
  }
  if let Some(status) = response["state"].as_str() {
    record.state = Some(status.to_owned());
  }
  if let Some(task_id) = response["newTaskId"]
    .as_str()
    .or_else(|| response["newTask"]["taskId"].as_str())
  {
    record.new_task_id = Some(task_id.to_owned());
  }
  record.response = Some(response.clone());
}

fn reauthorization_record_view(record: &ReauthorizationRecord) -> Value {
  let state = record.state.as_deref().unwrap_or("requestUnconfirmed");
  let mut view = record
    .response
    .clone()
    .filter(Value::is_object)
    .unwrap_or_else(|| json!({"state": state}));
  view["state"] = json!(state);
  view["deniedTaskId"] = json!(record.denied_task_id);
  view["canRetryRequest"] = json!(record.request_id.is_none() && state == "requestUnconfirmed");
  view["canReauthorize"] = json!(matches!(state, "denied" | "expired" | "revalidationFailed"));
  if let Some(request_id) = &record.request_id {
    view["requestId"] = json!(request_id);
  }
  if let Some(task_id) = &record.new_task_id {
    view["newTaskId"] = json!(task_id);
  }
  view
}

fn reauthorization_for_task(state: &HostState, task: &Value) -> Option<Value> {
  let task_id = task["taskId"].as_str()?;
  let record = state.reauthorizations.iter().rev().find(|record| {
    record.denied_task_id == task_id || record.new_task_id.as_deref() == Some(task_id)
  })?;
  let mut view = reauthorization_record_view(record);
  if task["state"] == "denied" {
    view["canReauthorize"] = Value::Bool(true);
  }
  Some(view)
}

fn attach_reauthorization(view: &mut Value, reauthorization: Option<Value>) {
  if let Some(reauthorization) = reauthorization {
    view["reauthorization"] = reauthorization;
  }
}

fn reauthorization_message(reauthorization: &Value) -> String {
  match reauthorization["state"].as_str() {
    Some("awaitingShellConfirmation") => "等待 AgentOS Shell 确认重新申请系统授权。".into(),
    Some("validating") => "AgentOS 正在重新检查能力和主机策略。".into(),
    Some("confirmed") => "AgentOS 已确认重新申请；正在跟踪新的部署任务。".into(),
    Some("denied") => "重新授权请求已被拒绝；如需重试，请再次主动点击重新授权。".into(),
    Some("expired") => "重新授权请求已过期；如需重试，请再次主动点击重新授权。".into(),
    Some("revalidationFailed") => reauthorization["error"]["message"]
      .as_str()
      .or(reauthorization["message"].as_str())
      .unwrap_or("AgentOS 重新检查未通过；当前能力或主机策略仍阻止此操作。")
      .to_owned(),
    Some("requestUnconfirmed") => {
      "AgentOS 尚未确认重新授权请求；点击重试会复用原请求幂等键。".into()
    }
    _ => "AgentOS 重新授权状态已更新。".into(),
  }
}

fn deployment_blockers(capabilities: &Value) -> String {
  if let Some(message) = capabilities.get("message").and_then(Value::as_str) {
    return format!("系统部署服务不可用：{message}");
  }
  let blocked: Vec<String> = SETUP_ACTIONS
    .iter()
    .filter_map(|name| {
      let action = &capabilities["actions"][*name];
      if action["available"] == true
        && (*name != "debian.packages.ensure" || ensure_schema_v1_available(action))
      {
        return None;
      }
      let reason = if *name == "debian.packages.ensure" && action["available"] == true {
        "schema-version-1-unavailable"
      } else {
        action["reason"]
          .as_str()
          .unwrap_or("capability-not-reported")
      };
      Some(format!("{name}: {}", reason))
    })
    .collect();
  format!("系统能力阻塞：{}", blocked.join("；"))
}

fn preflight_blocked_summary(_task: &Value) -> &'static str {
  "AgentOS 离线包预检查发现版本或依赖冲突；任务未进入系统授权队列。请查看详情，修正清单或宿主状态后，再手动重新检查。"
}

fn deployment_failure_summary(task: &Value) -> String {
  let error = task.get("error").unwrap_or(&Value::Null).to_string();
  if error.to_ascii_lowercase().contains("enospc")
    || error.to_ascii_lowercase().contains("no space left")
  {
    return "ENOSPC：设备磁盘空间不足，Android 环境部署失败；这不是授权失败。".into();
  }
  let message = task["error"]["message"]
    .as_str()
    .unwrap_or("AgentOS 未能完成 Android 环境部署。");
  // Keep full platform diagnostics in deploymentTask, not the main banner.
  message
    .lines()
    .next()
    .unwrap_or(message)
    .chars()
    .take(240)
    .collect()
}

const SETUP_ACTIONS: [&str; 3] = [
  "debian.packages.ensure",
  "linux.module.load",
  "linux.binderfs.ensure",
];

fn runtime_setup_request_identity() -> Result<RuntimeSetupRequestIdentity, String> {
  let manifest: Value = serde_json::from_str(include_str!("../../vitul.conf.json"))
    .map_err(|error| format!("AndroidTone deployment manifest is invalid: {error}"))?;
  let deployment = manifest
    .pointer("/agentos/deployment")
    .ok_or("AndroidTone deployment declaration is missing.")?;
  let protocol_version = deployment["protocolVersion"]
    .as_u64()
    .and_then(|value| u32::try_from(value).ok())
    .ok_or("AndroidTone deployment protocol version is invalid.")?;
  let operations = deployment["operations"]
    .as_array()
    .ok_or("AndroidTone deployment operations are missing.")?;
  let operation = operations
    .iter()
    .find(|operation| operation["id"] == ANDROID_RUNTIME_SETUP_OPERATION_ID)
    .ok_or("Android runtime setup operation is missing from the manifest.")?;
  runtime_setup_request_identity_from_operation(protocol_version, operation)
}

fn runtime_setup_request_identity_from_operation(
  protocol_version: u32,
  operation: &Value,
) -> Result<RuntimeSetupRequestIdentity, String> {
  let operation_id = operation["id"]
    .as_str()
    .filter(|value| !value.is_empty())
    .ok_or("Android runtime setup operation ID is invalid.")?
    .to_owned();
  let operation_version = operation["version"]
    .as_u64()
    .and_then(|value| u32::try_from(value).ok())
    .ok_or("Android runtime setup operation version is invalid.")?;
  let parameters = operation
    .get("parameters")
    .cloned()
    .ok_or("Android runtime setup parameters are missing.")?;
  let actions = operation
    .get("actions")
    .ok_or("Android runtime setup actions are missing.")?;
  let deployment_fingerprint = stable_json_fingerprint(actions)?;
  // serde_json's default map representation sorts object keys; arrays retain
  // manifest order, which also matters for the declared action sequence.
  let request_fingerprint = stable_json_fingerprint(&json!({
    "protocolVersion": protocol_version,
    "operationId": operation_id,
    "operationVersion": operation_version,
    "parameters": parameters,
    "deploymentFingerprint": deployment_fingerprint
  }))?;
  Ok(RuntimeSetupRequestIdentity {
    protocol_version,
    operation_id,
    operation_version,
    parameters,
    deployment_fingerprint,
    request_fingerprint,
  })
}

fn stable_json_fingerprint(value: &Value) -> Result<String, String> {
  let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
  Ok(format!("{:x}", Sha256::digest(bytes)))
}

impl RuntimeSetupRequestIdentity {
  fn plan_parameters(&self, idempotency_key: &str) -> Value {
    json!({
      "protocolVersion": self.protocol_version,
      "operationId": self.operation_id,
      "operationVersion": self.operation_version,
      "idempotencyKey": idempotency_key,
      "parameters": self.parameters
    })
  }
}

fn clear_deployment_request(state: &mut HostState) {
  state.deployment_task_id = None;
  state.deployment_request_key = None;
  state.deployment_request_binding = None;
  state.deployment_request_conflicted = false;
}

fn is_operation_already_active(code: &str) -> bool {
  matches!(
    code.to_ascii_lowercase().as_str(),
    "operation-already-active" | "operation_active" | "operation-active"
  )
}

fn is_idempotency_conflict(code: &str) -> bool {
  let normalized = code.to_ascii_lowercase();
  normalized.contains("idempotency") && normalized.contains("conflict")
}

fn active_task_from_error(error: &ProviderError) -> Option<Value> {
  error.response.as_ref().and_then(active_task_from_value)
}

fn active_task_from_value(value: &Value) -> Option<Value> {
  fn find(value: &Value, nested_task: bool, depth: usize) -> Option<Value> {
    if depth > 8 {
      return None;
    }
    if let Some(object) = value.as_object() {
      let task_id = object
        .get("taskId")
        .or_else(|| object.get("task_id"))
        .and_then(Value::as_str)
        .or_else(|| {
          nested_task
            .then(|| object.get("id").and_then(Value::as_str))
            .flatten()
        });
      if let (Some(task_id), Some(state)) = (task_id, object.get("state").and_then(Value::as_str)) {
        let mut task = value.clone();
        task["taskId"] = Value::String(task_id.to_owned());
        task["state"] = Value::String(state.to_owned());
        return Some(task);
      }
      for (key, child) in object {
        let child_is_task = matches!(
          key.as_str(),
          "task" | "activeTask" | "existingTask" | "activeOperation" | "taskResult"
        );
        if child_is_task
          || matches!(
            key.as_str(),
            "error" | "data" | "details" | "result" | "response"
          )
        {
          if let Some(task) = find(child, child_is_task, depth + 1) {
            return Some(task);
          }
        }
      }
    } else if let Some(items) = value.as_array() {
      for item in items {
        if let Some(task) = find(item, nested_task, depth + 1) {
          return Some(task);
        }
      }
    }
    None
  }

  find(value, false, 0)
}

fn ensure_schema_v1_available(action: &Value) -> bool {
  action["schemaVersions"]
    .as_array()
    .is_some_and(|versions| versions.iter().any(|version| version.as_u64() == Some(1)))
}

fn deployment_actions_available(actions: &Value) -> bool {
  SETUP_ACTIONS.iter().all(|name| {
    let Some(action) = actions.get(*name) else {
      return false;
    };
    action["available"].as_bool() == Some(true)
      && (*name != "debian.packages.ensure" || ensure_schema_v1_available(action))
  })
}

fn waydroid_initialized() -> bool {
  let Ok(config) = fs::read_to_string("/var/lib/waydroid/waydroid.cfg") else {
    return false;
  };
  let Some(images) = configured_waydroid_images_path(&config) else {
    return false;
  };
  images.join("system.img").is_file() && images.join("vendor.img").is_file()
}

fn installed_waydroid_image_package_version() -> Option<String> {
  let config = fs::read_to_string("/var/lib/waydroid/waydroid.cfg").ok()?;
  let images = configured_waydroid_images_path(&config)?;
  let metadata: Value =
    serde_json::from_slice(&fs::read(images.join("androidtone-image-info.json")).ok()?).ok()?;
  metadata["packageVersion"].as_str().map(str::to_owned)
}

fn configured_waydroid_architecture(config: &str) -> Option<String> {
  let mut in_waydroid_section = false;
  for line in config.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
      continue;
    }
    if line.starts_with('[') && line.ends_with(']') {
      in_waydroid_section = line == "[waydroid]";
      continue;
    }
    if !in_waydroid_section {
      continue;
    }
    if let Some((key, value)) = line.split_once('=') {
      if key.trim() == "arch" {
        return Some(value.trim().to_owned());
      }
    }
  }
  None
}

fn is_elf32_arm(bytes: &[u8]) -> bool {
  bytes.len() >= 20
    && bytes.get(..4) == Some(b"\x7fELF")
    && bytes[4] == 1
    && bytes[5] == 1
    && u16::from_le_bytes([bytes[18], bytes[19]]) == 40
}

fn aarch32_el0_supported_from_lscpu(output: &str) -> Option<bool> {
  output.lines().find_map(|line| {
    let (key, value) = line.split_once(':')?;
    if !key.trim().eq_ignore_ascii_case("CPU op-mode(s)") {
      return None;
    }
    Some(
      value
        .split(',')
        .any(|mode| mode.trim().eq_ignore_ascii_case("32-bit")),
    )
  })
}

fn clear_display_session(state: &mut HostState) {
  state.display_session_id = None;
  state.desktop_session_id = None;
}

async fn aarch32_el0_supported() -> Option<bool> {
  let mut command = Command::new("/usr/bin/lscpu");
  command.kill_on_drop(true);
  let output = tokio::time::timeout(Duration::from_secs(3), command.output())
    .await
    .ok()?
    .ok()?;
  if !output.status.success() {
    return None;
  }
  aarch32_el0_supported_from_lscpu(&String::from_utf8_lossy(&output.stdout))
}

async fn waydroid_aarch32_image_mismatch() -> Option<String> {
  let config = fs::read_to_string("/var/lib/waydroid/waydroid.cfg").ok()?;
  if configured_waydroid_architecture(&config).as_deref() != Some("arm64_only") {
    return None;
  }
  let has_32_bit_self_test = [
    "/var/lib/waydroid/rootfs/vendor/bin/boringssl_self_test32",
    "/var/lib/waydroid/rootfs/vendor/bin/hw/boringssl_self_test32",
  ]
  .into_iter()
  .any(|path| fs::read(path).is_ok_and(|bytes| is_elf32_arm(&bytes)));
  if !has_32_bit_self_test {
    return None;
  }
  match aarch32_el0_supported().await {
    Some(true) => None,
    Some(false) => Some(
      "当前 Waydroid 配置为 arm64_only，但已初始化的 vendor 镜像仍包含 ELF32 ARM EABI5 的 boringssl_self_test32。此虚拟 CPU 的 lscpu 只报告 64-bit CPU op-mode，不提供 AArch32 EL0；Android init 执行该加密自检会收到 Exec format error，并因 reboot_on_failure 反复重启。请更新到 AndroidTone 随包的 ARM64-only 镜像后再启动。".into(),
    ),
    None => Some(
      "当前 Waydroid 配置为 arm64_only，但已初始化的 vendor 镜像仍包含 ELF32 ARM EABI5 的 boringssl_self_test32。系统未能确认虚拟 CPU 是否提供 AArch32 EL0；为避免 Android init 因 Exec format error 重启，请更新到 AndroidTone 随包的 ARM64-only 镜像，并检查虚拟 CPU 的 32-bit op-mode。".into(),
    ),
  }
}

fn configured_waydroid_images_path(config: &str) -> Option<PathBuf> {
  let mut in_waydroid_section = false;
  for line in config.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
      continue;
    }
    if line.starts_with('[') && line.ends_with(']') {
      in_waydroid_section = line == "[waydroid]";
      continue;
    }
    if !in_waydroid_section {
      continue;
    }
    let Some((key, value)) = line.split_once('=') else {
      continue;
    };
    if key.trim() != "images_path" {
      continue;
    }
    let path = value.trim();
    return match path {
      "/var/lib/waydroid/images"
      | "/etc/waydroid-extra/images"
      | "/usr/share/waydroid-extra/images" => Some(PathBuf::from(path)),
      _ => None,
    };
  }
  None
}

fn valid_task_id(id: &str) -> bool {
  id.len() >= 16
    && id.len() <= 64
    && id.bytes().all(|byte| {
      byte.is_ascii_lowercase() && byte <= b'f' || byte.is_ascii_digit() || byte == b'-'
    })
}

fn apk_package_name(path: &Path) -> Result<String, String> {
  let file = File::open(path).map_err(|error| format!("Cannot open imported APK: {error}"))?;
  let mut archive =
    ZipArchive::new(file).map_err(|error| format!("Invalid APK archive: {error}"))?;
  let mut manifest = archive
    .by_name("AndroidManifest.xml")
    .map_err(|error| format!("APK is missing AndroidManifest.xml: {error}"))?;
  if manifest.size() > MAX_ANDROID_MANIFEST_BYTES {
    return Err("APK AndroidManifest.xml exceeds the 16 MiB validation limit.".into());
  }
  let mut bytes = Vec::with_capacity(manifest.size() as usize);
  manifest
    .read_to_end(&mut bytes)
    .map_err(|error| format!("Cannot read APK AndroidManifest.xml: {error}"))?;
  parse_android_manifest_package(&bytes)
}

fn parse_android_manifest_package(bytes: &[u8]) -> Result<String, String> {
  let xml_size = read_u32(bytes, 4)? as usize;
  let root_header = read_u16(bytes, 2)? as usize;
  if read_u16(bytes, 0)? != 0x0003 || root_header < 8 || xml_size > bytes.len() {
    return Err("APK AndroidManifest.xml is not a valid binary Android XML document.".into());
  }

  let mut strings = None;
  let mut offset = root_header;
  while offset + 8 <= xml_size {
    let kind = read_u16(bytes, offset)?;
    let header_size = read_u16(bytes, offset + 2)? as usize;
    let chunk_size = read_u32(bytes, offset + 4)? as usize;
    let end = offset
      .checked_add(chunk_size)
      .filter(|end| header_size >= 8 && chunk_size >= header_size && *end <= xml_size)
      .ok_or("APK AndroidManifest.xml has an invalid chunk size.")?;

    match kind {
      0x0001 => strings = Some(parse_android_string_pool(bytes, offset, header_size, end)?),
      0x0102 => {
        let Some(pool) = strings.as_ref() else {
          offset = end;
          continue;
        };
        let tag_name = android_string(pool, read_u32(bytes, offset + 20)?)?;
        if tag_name == "manifest" {
          let attr_start = read_u16(bytes, offset + 24)? as usize;
          let attr_size = read_u16(bytes, offset + 26)? as usize;
          let attr_count = read_u16(bytes, offset + 28)? as usize;
          if attr_size < 20 {
            return Err("APK manifest attributes are malformed.".into());
          }
          let attrs = offset
            .checked_add(16 + attr_start)
            .ok_or("APK manifest attribute offset overflowed.")?;
          for index in 0..attr_count {
            let attr = attrs
              .checked_add(index.saturating_mul(attr_size))
              .filter(|at| at + 20 <= end)
              .ok_or("APK manifest attribute is truncated.")?;
            if android_string(pool, read_u32(bytes, attr + 4)?)? != "package" {
              continue;
            }
            let raw_value = read_u32(bytes, attr + 8)?;
            let value_type = *bytes
              .get(attr + 15)
              .ok_or("APK manifest package value is truncated.")?;
            let value_index = if raw_value != u32::MAX {
              raw_value
            } else if value_type == 0x03 {
              read_u32(bytes, attr + 16)?
            } else {
              return Err("APK manifest package attribute is not a string.".into());
            };
            let package = android_string(pool, value_index)?.to_owned();
            if !valid_package_id(&package) {
              return Err("APK manifest contains an invalid Android package ID.".into());
            }
            return Ok(package);
          }
          return Err("APK manifest does not declare a package ID.".into());
        }
      }
      _ => {}
    }
    offset = end;
  }
  Err("APK manifest does not contain a package declaration.".into())
}

fn parse_android_string_pool(
  bytes: &[u8],
  offset: usize,
  header_size: usize,
  end: usize,
) -> Result<Vec<String>, String> {
  if header_size < 28 {
    return Err("APK manifest string pool header is malformed.".into());
  }
  let count = read_u32(bytes, offset + 8)? as usize;
  let flags = read_u32(bytes, offset + 16)?;
  let strings_start = offset
    .checked_add(read_u32(bytes, offset + 20)? as usize)
    .filter(|start| *start <= end)
    .ok_or("APK manifest string pool is out of bounds.")?;
  let offsets_start = offset + header_size;
  if count > 1_000_000 || offsets_start.saturating_add(count.saturating_mul(4)) > end {
    return Err("APK manifest string pool offsets are malformed.".into());
  }
  let mut strings = Vec::with_capacity(count);
  for index in 0..count {
    let relative = read_u32(bytes, offsets_start + index * 4)? as usize;
    let at = strings_start
      .checked_add(relative)
      .filter(|at| *at < end)
      .ok_or("APK manifest string is out of bounds.")?;
    let value = if flags & 0x0000_0100 != 0 {
      let (_, next) = android_utf8_length(bytes, at, end)?;
      let (length, start) = android_utf8_length(bytes, next, end)?;
      let stop = start
        .checked_add(length)
        .filter(|stop| *stop < end)
        .ok_or("APK manifest UTF-8 string is truncated.")?;
      std::str::from_utf8(&bytes[start..stop])
        .map_err(|_| "APK manifest contains invalid UTF-8.")?
        .to_owned()
    } else {
      let (units, start) = android_utf16_length(bytes, at, end)?;
      let byte_count = units
        .checked_mul(2)
        .ok_or("APK manifest UTF-16 length overflowed.")?;
      let stop = start
        .checked_add(byte_count)
        .filter(|stop| *stop <= end)
        .ok_or("APK manifest UTF-16 string is truncated.")?;
      let code_units = bytes[start..stop]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
      String::from_utf16(code_units.collect::<Vec<_>>().as_slice())
        .map_err(|_| "APK manifest contains invalid UTF-16.")?
    };
    strings.push(value);
  }
  Ok(strings)
}

fn android_utf8_length(bytes: &[u8], offset: usize, end: usize) -> Result<(usize, usize), String> {
  let first = *bytes
    .get(offset)
    .filter(|_| offset < end)
    .ok_or("APK manifest string length is truncated.")?;
  if first & 0x80 == 0 {
    Ok((first as usize, offset + 1))
  } else {
    let second = *bytes
      .get(offset + 1)
      .filter(|_| offset + 1 < end)
      .ok_or("APK manifest string length is truncated.")?;
    Ok((
      (((first & 0x7f) as usize) << 8) | second as usize,
      offset + 2,
    ))
  }
}

fn android_utf16_length(bytes: &[u8], offset: usize, end: usize) -> Result<(usize, usize), String> {
  if offset.saturating_add(2) > end {
    return Err("APK manifest UTF-16 length is truncated.".into());
  }
  let first = read_u16(bytes, offset)?;
  if first & 0x8000 == 0 {
    Ok((first as usize, offset + 2))
  } else {
    if offset.saturating_add(4) > end {
      return Err("APK manifest UTF-16 length is truncated.".into());
    }
    let second = read_u16(bytes, offset + 2)?;
    Ok((
      ((first & 0x7fff) as usize) << 16 | second as usize,
      offset + 4,
    ))
  }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
  let slice = bytes
    .get(offset..offset.saturating_add(2))
    .ok_or("APK manifest is truncated.")?;
  Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
  let slice = bytes
    .get(offset..offset.saturating_add(4))
    .ok_or("APK manifest is truncated.")?;
  Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn android_string(strings: &[String], index: u32) -> Result<&str, String> {
  strings
    .get(index as usize)
    .map(String::as_str)
    .ok_or_else(|| "APK manifest refers to an unknown string.".into())
}

fn application_ids(value: &Value) -> Vec<String> {
  value["apps"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|app| {
      app["packageName"]
        .as_str()
        .or_else(|| app["id"].as_str())
        .filter(|id| valid_package_id(id))
        .map(str::to_owned)
    })
    .collect()
}

fn application_available(value: &Value, package_name: &str) -> bool {
  application_ids(value).iter().any(|id| id == package_name)
}

fn ensure_waydroid_install_success(output: &Output) -> Result<(), String> {
  ensure_success(output, "Waydroid APK 安装失败")?;
  let combined = format!(
    "{}\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  if combined.lines().any(|line| {
    let normalized = line.to_ascii_lowercase();
    normalized.contains("error:")
      || normalized.contains("[error]")
      || normalized.contains("traceback (most recent call last)")
      || normalized.contains("failed with code:")
  }) {
    return Err(diagnostic(output));
  }
  Ok(())
}
fn valid_package_id(id: &str) -> bool {
  id.len() <= 256
    && id.split('.').count() >= 2
    && id.split('.').all(|part| {
      !part.is_empty()
        && part
          .bytes()
          .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}
fn valid_apk_name(name: &str) -> bool {
  !name.is_empty()
    && name.len() <= 255
    && !name.contains('/')
    && !name.contains('\\')
    && name.to_ascii_lowercase().ends_with(".apk")
}
fn valid_sha256(sha: &str) -> bool {
  sha.len() == 64 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn verify_file(path: &Path, expected_bytes: u64, expected_sha: &str) -> Result<(), String> {
  let metadata = fs::metadata(path).map_err(|error| format!("APK file is unavailable: {error}"))?;
  if !metadata.is_file() || metadata.len() != expected_bytes {
    return Err("APK size changed after import.".into());
  }
  if metadata.len() > MAX_APK_BYTES {
    return Err("APK exceeds the 1 GiB limit.".into());
  }
  let mut file = File::open(path).map_err(|error| error.to_string())?;
  let mut hasher = Sha256::new();
  let mut prefix = Vec::with_capacity(4);
  let mut buffer = [0u8; 64 * 1024];
  loop {
    let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
    if read == 0 {
      break;
    }
    if prefix.len() < 4 {
      prefix.extend_from_slice(&buffer[..read.min(4 - prefix.len())]);
    }
    hasher.update(&buffer[..read]);
  }
  let actual = format!("{:x}", hasher.finalize());
  if prefix.get(..2) != Some(&b"PK"[..]) {
    return Err("File contents do not look like an APK.".into());
  }
  if actual != expected_sha {
    return Err("APK SHA-256 changed after import.".into());
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn put_u16(target: &mut Vec<u8>, value: u16) {
    target.extend_from_slice(&value.to_le_bytes());
  }

  fn put_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
  }

  fn binary_manifest(package: &str, utf8: bool) -> Vec<u8> {
    let values = ["manifest", "package", package];
    let mut string_data = Vec::new();
    let mut string_offsets = Vec::new();
    for value in values {
      string_offsets.push(string_data.len() as u32);
      if utf8 {
        string_data.push(value.len() as u8);
        string_data.push(value.len() as u8);
        string_data.extend_from_slice(value.as_bytes());
        string_data.push(0);
      } else {
        let utf16 = value.encode_utf16().collect::<Vec<_>>();
        put_u16(&mut string_data, utf16.len() as u16);
        for code_unit in utf16 {
          put_u16(&mut string_data, code_unit);
        }
        put_u16(&mut string_data, 0);
      }
    }

    let string_offsets_bytes = string_offsets.len() * 4;
    let strings_start = 28 + string_offsets_bytes;
    let pool_size = strings_start + string_data.len();
    let mut pool = Vec::new();
    put_u16(&mut pool, 0x0001);
    put_u16(&mut pool, 28);
    put_u32(&mut pool, pool_size as u32);
    put_u32(&mut pool, values.len() as u32);
    put_u32(&mut pool, 0);
    put_u32(&mut pool, if utf8 { 0x100 } else { 0 });
    put_u32(&mut pool, strings_start as u32);
    put_u32(&mut pool, 0);
    for offset in string_offsets {
      put_u32(&mut pool, offset);
    }
    pool.extend_from_slice(&string_data);

    let node_size = 16 + 20 + 20;
    let mut start = Vec::new();
    put_u16(&mut start, 0x0102);
    put_u16(&mut start, 16);
    put_u32(&mut start, node_size as u32);
    put_u32(&mut start, 1);
    put_u32(&mut start, u32::MAX);
    put_u32(&mut start, u32::MAX);
    put_u32(&mut start, 0);
    put_u16(&mut start, 20);
    put_u16(&mut start, 20);
    put_u16(&mut start, 1);
    put_u16(&mut start, 0);
    put_u16(&mut start, 0);
    put_u16(&mut start, 0);
    put_u32(&mut start, u32::MAX);
    put_u32(&mut start, 1);
    put_u32(&mut start, 2);
    put_u16(&mut start, 8);
    start.push(0);
    start.push(0x03);
    put_u32(&mut start, 2);

    let total_size = 8 + pool.len() + start.len();
    let mut xml = Vec::with_capacity(total_size);
    put_u16(&mut xml, 0x0003);
    put_u16(&mut xml, 8);
    put_u32(&mut xml, total_size as u32);
    xml.extend_from_slice(&pool);
    xml.extend_from_slice(&start);
    xml
  }

  #[test]
  fn apk_binary_manifest_yields_exact_package_id() {
    for utf8 in [false, true] {
      assert_eq!(
        parse_android_manifest_package(&binary_manifest("com.example.chat", utf8)).unwrap(),
        "com.example.chat"
      );
    }
    // Invalid chunk sizes must fail rather than leave the chunk iterator stuck.
    let mut invalid = vec![3, 0, 8, 0, 16, 0, 0, 0];
    invalid.extend_from_slice(&[0; 8]);
    assert!(parse_android_manifest_package(&invalid).is_err());
  }

  #[test]
  fn successful_waydroid_cli_exit_with_error_log_is_not_install_success() {
    let mut output = std::process::Command::new("true").output().unwrap();
    output.stderr = b"[10:12:19] ERROR: Failed with code: -1".to_vec();
    assert!(ensure_waydroid_install_success(&output).is_err());

    let clean = std::process::Command::new("true").output().unwrap();
    assert!(ensure_waydroid_install_success(&clean).is_ok());
  }

  #[test]
  fn application_install_is_verified_by_exact_package_id() {
    let list = json!({"available": true, "apps": [
      {"id": "com.android.settings", "packageName": "com.android.settings"},
      {"id": "com.tencent.mm", "packageName": "com.tencent.mm"}
    ]});
    assert!(application_available(&list, "com.tencent.mm"));
    assert!(!application_available(&list, "com.tencent.mobileqq"));
  }

  #[test]
  fn structured_provider_error_keeps_external_wayland_code_and_message() {
    let error = structured_provider_error(
      &json!({
        "error": {
          "code": "external-wayland-session-unavailable",
          "message": "The private Wayland display session could not be created."
        }
      }),
      "/v1/providers/org.agentos.external-wayland/invoke",
    )
    .unwrap();
    assert_eq!(error.code, "external-wayland-session-unavailable");
    assert_eq!(
      error.provider_id.as_deref(),
      Some(EXTERNAL_WAYLAND_PROVIDER_ID)
    );
    assert_eq!(
      error.message,
      "The private Wayland display session could not be created."
    );
    let detailed = ProviderError::from_payload(
      &json!({"error": {
        "code": "preflight-blocked",
        "message": "The operation did not pass host preflight.",
        "data": {"details": [{"code": "package-version-conflict", "package": "example", "installedVersion": "1.0", "requestedVersion": "2.0"}], "diagnostics": {"phase": "preflight"}}
      }}),
      Some(SYSTEM_DEPLOYMENT_PROVIDER_ID),
    );
    assert_eq!(detailed.code, "preflight-blocked");
    assert_eq!(detailed.details.as_ref().unwrap()[0]["package"], "example");
    assert_eq!(detailed.diagnostics.as_ref().unwrap()["phase"], "preflight");
    assert_eq!(
      detailed.data.as_ref().unwrap()["details"][0]["requestedVersion"],
      "2.0"
    );
    assert!(structured_provider_error(
      &json!({"result": "ok"}),
      "/v1/providers/org.agentos.external-wayland/invoke"
    )
    .is_none());
  }
  fn recovery_capabilities() -> Value {
    json!({"actions": {
      "linux.module.load": {"available": true},
      "linux.binderfs.ensure": {"available": true},
      "systemd.service.control": {"available": true, "allowedUnits": [WAYDROID_UNIT], "operations": ["start", "stop", "restart"]},
      "linux.lxc.ensure": {"available": false}, "linux.lxc.lifecycle": {"available": false},
      "debian.packages.ensure": {"available": false, "schemaVersions": [1]}
    }})
  }

  #[test]
  fn repeated_recovery_only_plans_for_stopped_runtime_or_explicit_failure_retry() {
    for state in ["recovery-required", "recovery-failed"] {
      assert!(recovery_request_needed(
        &json!({"state":state,"canRecover":true})
      ));
      assert!(!recovery_request_needed(
        &json!({"state":state,"canRecover":false})
      ));
    }
    for state in [
      "session-ready",
      "process-running",
      "awaiting-authorization",
      "deploying",
      "authorization-denied",
      "authorization-cancelled",
      "error",
    ] {
      assert!(!recovery_request_needed(
        &json!({"state":state,"canRecover":true})
      ));
    }
    assert!(recovery_request_needed(
      &json!({"state":"image-restart-required","canRecover":true})
    ));
    assert!(!recovery_request_needed(
      &json!({"state":"image-restart-required","canRecover":false})
    ));
  }

  #[test]
  fn recovery_requires_only_fixed_binder_and_service_capabilities() {
    let capabilities = recovery_capabilities();
    assert!(recovery_blockers(&capabilities).is_empty());
    for name in RECOVERY_ACTIONS {
      let mut unavailable = capabilities.clone();
      unavailable["actions"][name] = json!({"available": false, "reason": "policy-blocked"});
      assert!(recovery_blockers(&unavailable).contains(name));
      assert!(recovery_blockers(&unavailable).contains("policy-blocked"));
    }
    let mut wrong_unit = capabilities.clone();
    wrong_unit["actions"]["systemd.service.control"]["allowedUnits"] = json!(["another.service"]);
    assert!(recovery_blockers(&wrong_unit).contains("systemd-unit-not-allowlisted"));
    let mut wrong_operation = capabilities;
    wrong_operation["actions"]["systemd.service.control"]["operations"] = json!(["start"]);
    assert!(recovery_blockers(&wrong_operation).contains("restart-not-supported"));
  }

  #[test]
  fn recovery_task_states_do_not_assume_authorization_or_retry_terminal_decisions() {
    let capabilities = recovery_capabilities();
    for (state, expected, retry) in [
      ("awaitingAuthorization", "awaiting-authorization", false),
      ("running", "deploying", false),
      ("denied", "authorization-denied", false),
      ("cancelled", "authorization-cancelled", false),
      ("expired", "authorization-expired", true),
      ("failed", "recovery-failed", true),
      ("blocked", "recovery-failed", true),
    ] {
      let error = json!({"message": "error", "details": {"example": "full details"}});
      let task = json!({"taskId": "test", "state": state, "error": error});
      let view = recovery_task_view(&task, &capabilities).unwrap();
      assert_eq!(view["state"], expected);
      assert_eq!(view["canRecover"], retry);
      assert_eq!(view["canReapply"], retry);
      assert_eq!(view["deploymentTask"]["error"], error);
      assert_eq!(view["canInstall"], false);
    }
    assert!(recovery_task_view(&json!({"state":"completed"}), &capabilities).is_none());
  }

  #[test]
  fn waydroid_tabs_and_stopped_session_are_not_misread_as_failed_recovery() {
    assert_eq!(
      parse_status("Session:\tRUNNING\nContainer:\tRUNNING\n"),
      (true, true)
    );
    assert_eq!(
      parse_status("Session: STOPPED\nVendor type: MAINLINE"),
      (false, false)
    );
    let completed = json!({"state":"completed"});
    let kernel = json!({"moduleLoading":{"observed":true},"binderfsMounting":{"state":"mounted"}});
    assert!(recovery_allows_session(
      Some(&completed),
      "Session:\tSTOPPED\nVendor type: MAINLINE",
      &kernel
    ));
    assert!(!recovery_allows_session(None, "Session:\tSTOPPED", &kernel));
    assert!(!recovery_allows_session(
      Some(&completed),
      "Session:\tSTOPPED",
      &json!({})
    ));
    assert!(!recovery_allows_session(
      Some(&completed),
      "Session:\tSTOPPED",
      &json!({"moduleLoading":{"observed":true},"binderfsMounting":{"state":"not-observed"}})
    ));
    assert!(!recovery_allows_session(
      Some(&completed),
      "Session:\tSTOPPED",
      &json!({"moduleLoading":{"observed":false},"binderfsMounting":{"state":"mounted"}})
    ));
    assert!(!recovery_allows_session(
      Some(&completed),
      "Session:\tSTOPPED\nContainer: STOPPED",
      &kernel
    ));
  }

  enum BrokerReply {
    Json(Value),
    Disconnect,
  }

  async fn mock_agentos_broker(
    replies: Vec<BrokerReply>,
  ) -> (HostProvider, tokio::task::JoinHandle<Vec<Value>>) {
    mock_agentos_broker_for(SYSTEM_DEPLOYMENT_PROVIDER_ID, replies).await
  }

  async fn mock_agentos_broker_for(
    provider_id: &str,
    replies: Vec<BrokerReply>,
  ) -> (HostProvider, tokio::task::JoinHandle<Vec<Value>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let invoke_path = format!("/v1/providers/{provider_id}/invoke");
    let dir = env::temp_dir().join(format!("at-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let socket = dir.join("p.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let server = tokio::spawn(async move {
      let mut requests = Vec::new();
      for reply in replies {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut data = Vec::new();
        loop {
          let mut buf = [0; 4096];
          let size = stream.read(&mut buf).await.unwrap();
          assert!(size > 0);
          data.extend_from_slice(&buf[..size]);
          if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&data[..end]);
            assert!(headers.contains(&invoke_path));
            let length: usize = headers
              .lines()
              .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key
                  .eq_ignore_ascii_case("content-length")
                  .then(|| value.trim().parse().unwrap())
              })
              .unwrap();
            if data.len() >= end + 4 + length {
              requests.push(serde_json::from_slice(&data[end + 4..end + 4 + length]).unwrap());
              break;
            }
          }
        }
        let BrokerReply::Json(response) = reply else {
          drop(stream);
          continue;
        };
        let body = serde_json::to_vec(&response).unwrap();
        let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        stream.write_all(headers.as_bytes()).await.unwrap();
        stream.write_all(&body).await.unwrap();
      }
      requests
    });
    (
      HostProvider {
        client: ProviderClient {
          socket: Some(socket),
        },
        state_path: dir.join("state.json"),
        data_dir: dir,
      },
      server,
    )
  }

  async fn mock_recovery_broker(
    responses: Vec<Value>,
  ) -> (HostProvider, tokio::task::JoinHandle<Vec<Value>>) {
    mock_agentos_broker(responses.into_iter().map(BrokerReply::Json).collect()).await
  }

  fn runtime_setup_capabilities() -> Value {
    json!({"actions": {
      "debian.packages.ensure": {"available": true, "schemaVersions": [1]},
      "linux.module.load": {"available": true},
      "linux.binderfs.ensure": {"available": true}
    }})
  }

  #[test]
  fn runtime_setup_request_fingerprint_tracks_actions_and_package_manifest() {
    let manifest: Value = serde_json::from_str(include_str!("../../vitul.conf.json")).unwrap();
    let deployment = &manifest["agentos"]["deployment"];
    let operation = deployment["operations"]
      .as_array()
      .unwrap()
      .iter()
      .find(|operation| operation["id"] == ANDROID_RUNTIME_SETUP_OPERATION_ID)
      .unwrap();
    let original = runtime_setup_request_identity_from_operation(
      deployment["protocolVersion"].as_u64().unwrap() as u32,
      operation,
    )
    .unwrap();
    assert_eq!(original.operation_version, 3);
    assert_eq!(original.operation_id, ANDROID_RUNTIME_SETUP_OPERATION_ID);

    let mut changed = operation.clone();
    changed["actions"][0]["name"] = Value::String("different-module".into());
    let changed = runtime_setup_request_identity_from_operation(
      deployment["protocolVersion"].as_u64().unwrap() as u32,
      &changed,
    )
    .unwrap();
    assert_ne!(
      original.deployment_fingerprint,
      changed.deployment_fingerprint
    );
    assert_ne!(original.request_fingerprint, changed.request_fingerprint);
  }

  #[tokio::test]
  async fn old_v2_persisted_key_is_replaced_and_v3_binding_is_saved() {
    let old_key = "legacy-runtime-v2-key";
    let response = json!({"taskId":"setup-v3-task", "state":"awaitingAuthorization"});
    let (provider, server) = mock_agentos_broker(vec![
      BrokerReply::Json(runtime_setup_capabilities()),
      BrokerReply::Json(response),
    ])
    .await;
    provider
      .save_state(&HostState {
        deployment_request_key: Some(old_key.into()),
        runtime_operation_version: 2,
        ..HostState::default()
      })
      .unwrap();

    let identity = runtime_setup_request_identity().unwrap();
    let task = provider
      .plan_runtime_setup(&identity)
      .await
      .unwrap()
      .unwrap();
    assert_eq!(task["state"], "awaitingAuthorization");
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["method"], "capabilities.get");
    assert_eq!(requests[1]["method"], "operation.plan");
    assert_eq!(requests[1]["params"]["operationId"], identity.operation_id);
    assert_eq!(requests[1]["params"]["operationVersion"], 3);
    assert_eq!(requests[1]["params"]["parameters"], identity.parameters);
    let new_key = requests[1]["params"]["idempotencyKey"].as_str().unwrap();
    assert_ne!(new_key, old_key);
    assert!(Uuid::parse_str(new_key).is_ok());

    let saved = provider.load_state().unwrap();
    assert_eq!(saved.runtime_operation_version, 3);
    assert_eq!(saved.deployment_task_id.as_deref(), Some("setup-v3-task"));
    assert_eq!(saved.deployment_request_key.as_deref(), Some(new_key));
    let binding = saved.deployment_request_binding.unwrap();
    assert_eq!(binding.request, identity);
    assert_eq!(binding.idempotency_key, new_key);
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn uncertain_response_retry_reuses_the_same_key_for_the_same_request() {
    let response = json!({"taskId":"setup-task", "state":"awaitingAuthorization"});
    let (provider, server) = mock_agentos_broker(vec![
      BrokerReply::Json(runtime_setup_capabilities()),
      BrokerReply::Disconnect,
      BrokerReply::Json(runtime_setup_capabilities()),
      BrokerReply::Json(response),
    ])
    .await;
    let identity = runtime_setup_request_identity().unwrap();
    assert!(provider.plan_runtime_setup(&identity).await.is_err());
    let saved_after_lost_response = provider.load_state().unwrap();
    assert!(saved_after_lost_response.deployment_task_id.is_none());
    let first_key = saved_after_lost_response
      .deployment_request_binding
      .as_ref()
      .unwrap()
      .idempotency_key
      .clone();

    let task = provider
      .plan_runtime_setup(&identity)
      .await
      .unwrap()
      .unwrap();
    assert_eq!(task["taskId"], "setup-task");
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[1]["method"], "operation.plan");
    assert_eq!(requests[3]["method"], "operation.plan");
    assert_eq!(requests[1]["params"]["idempotencyKey"], first_key);
    assert_eq!(requests[3]["params"]["idempotencyKey"], first_key);
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn active_setup_task_is_queried_and_reused_without_a_second_plan() {
    for active_state in ["awaitingAuthorization", "running"] {
      let task = json!({"taskId":"active-setup-task", "state":active_state});
      let (provider, server) = mock_agentos_broker(vec![BrokerReply::Json(task.clone())]).await;
      provider
        .save_state(&HostState {
          deployment_task_id: Some("active-setup-task".into()),
          deployment_request_key: Some("existing-plan-key".into()),
          runtime_operation_version: 3,
          ..HostState::default()
        })
        .unwrap();

      assert_eq!(
        provider
          .plan_runtime_setup(&runtime_setup_request_identity().unwrap())
          .await
          .unwrap()
          .unwrap(),
        task
      );
      let requests = server.await.unwrap();
      assert_eq!(requests.len(), 1);
      assert_eq!(requests[0]["method"], "task.get");
      assert_eq!(requests[0]["params"]["taskId"], "active-setup-task");
      fs::remove_dir_all(provider.data_dir).unwrap();
    }
  }

  #[tokio::test]
  async fn operation_already_active_task_id_is_saved_and_followed() {
    let (provider, server) = mock_agentos_broker(vec![
      BrokerReply::Json(runtime_setup_capabilities()),
      BrokerReply::Json(json!({"error": {
        "code":"operation-already-active",
        "message":"An operation is already active.",
        "details":{"taskId":"existing-active-task", "state":"running"}
      }})),
    ])
    .await;
    let task = provider
      .plan_runtime_setup(&runtime_setup_request_identity().unwrap())
      .await
      .unwrap()
      .unwrap();
    assert_eq!(task["taskId"], "existing-active-task");
    assert_eq!(task["state"], "running");
    assert_eq!(
      provider.load_state().unwrap().deployment_task_id.as_deref(),
      Some("existing-active-task")
    );
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1]["method"], "operation.plan");
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn known_idempotency_conflict_rotates_only_on_the_next_explicit_plan_call() {
    let response = json!({"taskId":"recovered-task", "state":"awaitingAuthorization"});
    let (provider, server) = mock_agentos_broker(vec![
      BrokerReply::Json(runtime_setup_capabilities()),
      BrokerReply::Json(json!({"error": {
        "code":"idempotency-key-conflict",
        "message":"This idempotency key was already used for a different operation request."
      }})),
      BrokerReply::Json(runtime_setup_capabilities()),
      BrokerReply::Json(response),
    ])
    .await;
    let identity = runtime_setup_request_identity().unwrap();
    assert!(provider.plan_runtime_setup(&identity).await.is_err());
    let first_key = provider
      .load_state()
      .unwrap()
      .deployment_request_binding
      .unwrap()
      .idempotency_key;
    assert!(provider.load_state().unwrap().deployment_request_conflicted);

    assert_eq!(
      provider
        .plan_runtime_setup(&identity)
        .await
        .unwrap()
        .unwrap()["taskId"],
      "recovered-task"
    );
    let requests = server.await.unwrap();
    let second_key = requests[3]["params"]["idempotencyKey"].as_str().unwrap();
    assert_ne!(first_key, second_key);
    assert!(!provider.load_state().unwrap().deployment_request_conflicted);
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn recovery_submits_fixed_plan_and_polls_without_duplicates() {
    let pending = json!({"taskId":"recovery-task", "state":"awaitingAuthorization"});
    let (provider, server) = mock_recovery_broker(vec![pending.clone(), pending]).await;
    assert_eq!(
      provider
        .plan_recovery(&recovery_capabilities())
        .await
        .unwrap()
        .unwrap()["state"],
      "awaitingAuthorization"
    );
    assert_eq!(
      provider
        .plan_recovery(&recovery_capabilities())
        .await
        .unwrap()
        .unwrap()["state"],
      "awaitingAuthorization"
    );
    let requests = server.await.unwrap();
    assert_eq!(requests[0]["method"], "operation.plan");
    assert_eq!(
      requests[0]["params"]["operationId"],
      "android-runtime-recover"
    );
    assert_eq!(requests[0]["params"]["operationVersion"], 1);
    assert_eq!(requests[0]["params"]["parameters"], json!({}));
    assert_eq!(requests[1]["method"], "task.get");
    assert_eq!(requests[1]["params"]["taskId"], "recovery-task");
    let saved = provider.load_state().unwrap();
    assert_eq!(
      requests[0]["params"]["idempotencyKey"],
      saved.recovery_request_key.unwrap()
    );
    assert_eq!(saved.recovery_task_id.as_deref(), Some("recovery-task"));
    assert!(saved.deployment_task_id.is_none());
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn recovery_capability_blocker_never_contacts_the_broker() {
    let (provider, server) = mock_recovery_broker(vec![]).await;
    assert!(provider
      .plan_recovery(&json!({"actions":{}}))
      .await
      .unwrap()
      .is_none());
    assert!(server.await.unwrap().is_empty());
    assert!(provider
      .load_state()
      .unwrap()
      .recovery_request_key
      .is_none());
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn explicit_reauthorization_uses_exact_parameters_and_reuses_pending_request() {
    let denied = json!({"taskId":"denied-task", "state":"denied"});
    let pending = json!({
      "requestId":"reauth-1",
      "deniedTaskId":"denied-task",
      "state":"awaitingShellConfirmation"
    });
    let (provider, server) = mock_recovery_broker(vec![
      denied.clone(),
      json!({}), // Simulates a response that cannot confirm whether AgentOS accepted the request.
      denied.clone(),
      pending.clone(),
      denied,
      pending.clone(),
    ])
    .await;
    provider
      .save_state(&HostState {
        recovery_task_id: Some("denied-task".into()),
        ..HostState::default()
      })
      .unwrap();

    assert!(provider.reauthorize_denied_task().await.is_err());
    assert_eq!(
      provider.reauthorize_denied_task().await.unwrap()["state"],
      "awaitingShellConfirmation"
    );
    assert_eq!(
      provider.reauthorize_denied_task().await.unwrap()["requestId"],
      "reauth-1"
    );

    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 6);
    assert_eq!(requests[0]["method"], "task.get");
    assert_eq!(requests[1]["method"], "authorization.reauthorize");
    let params = &requests[1]["params"];
    assert_eq!(params["protocolVersion"], 1);
    assert_eq!(params["taskId"], "denied-task");
    let key = params["idempotencyKey"].as_str().unwrap();
    assert!((8..=128).contains(&key.len()));
    assert!(key.bytes().all(safe_idempotency_byte));
    assert_eq!(
      params
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>(),
      ["idempotencyKey", "protocolVersion", "taskId"]
        .into_iter()
        .collect()
    );
    assert_eq!(requests[3]["method"], "authorization.reauthorize");
    assert_eq!(requests[3]["params"]["idempotencyKey"], key);
    assert_eq!(requests[2]["method"], "task.get");
    assert_eq!(requests[4]["method"], "task.get");
    assert_eq!(requests[5]["method"], "authorization.reauthorization.get");
    assert_eq!(requests[5]["params"], json!({"requestId":"reauth-1"}));

    let saved = provider.load_state().unwrap();
    assert_eq!(saved.reauthorizations.len(), 1);
    assert_eq!(saved.reauthorizations[0].denied_task_id, "denied-task");
    assert_eq!(saved.reauthorizations[0].idempotency_key, key);
    assert_eq!(
      saved.reauthorizations[0].request_id.as_deref(),
      Some("reauth-1")
    );
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn confirmed_reauthorization_tracks_linked_task_and_keeps_denied_audit_id() {
    let denied = json!({"taskId":"denied-task", "state":"denied"});
    let pending = json!({"requestId":"reauth-2", "state":"awaitingShellConfirmation"});
    let confirmed = json!({
      "requestId":"reauth-2",
      "deniedTaskId":"denied-task",
      "state":"confirmed",
      "newTaskId":"new-task",
      "newTask":{"taskId":"new-task", "state":"awaitingAuthorization", "retryOfTaskId":"denied-task", "reauthorizationRequestId":"reauth-2"}
    });
    let new_task = json!({"taskId":"new-task", "state":"awaitingAuthorization"});
    let (provider, server) =
      mock_recovery_broker(vec![denied, pending, confirmed.clone(), new_task.clone()]).await;
    provider
      .save_state(&HostState {
        recovery_task_id: Some("denied-task".into()),
        ..HostState::default()
      })
      .unwrap();

    provider.reauthorize_denied_task().await.unwrap();
    let mut state = provider.load_state().unwrap();
    assert_eq!(
      provider
        .poll_reauthorization(&mut state)
        .await
        .unwrap()
        .unwrap()["state"],
      "confirmed"
    );
    let current_task_id = state.recovery_task_id.clone().unwrap();
    let tracked_task = provider
      .client
      .system_deployment_invoke("task.get", json!({"taskId": current_task_id}))
      .await
      .unwrap();
    assert_eq!(tracked_task, new_task);
    let saved = provider.load_state().unwrap();
    assert_eq!(saved.recovery_task_id.as_deref(), Some("new-task"));
    assert_eq!(saved.reauthorizations[0].denied_task_id, "denied-task");
    assert_eq!(
      saved.reauthorizations[0].new_task_id.as_deref(),
      Some("new-task")
    );
    assert_eq!(
      reauthorization_for_task(&saved, &confirmed["newTask"]).unwrap()["deniedTaskId"],
      "denied-task"
    );
    let requests = server.await.unwrap();
    assert_eq!(requests[2]["method"], "authorization.reauthorization.get");
    assert_eq!(requests[2]["params"], json!({"requestId":"reauth-2"}));
    assert_eq!(requests[3]["method"], "task.get");
    assert_eq!(requests[3]["params"], json!({"taskId":"new-task"}));
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn terminal_reauthorization_starts_a_new_flow_only_on_a_new_explicit_call() {
    let denied = json!({"taskId":"denied-task", "state":"denied"});
    let first = json!({"requestId":"reauth-old", "state":"awaitingShellConfirmation"});
    let rejected = json!({"requestId":"reauth-old", "state":"denied"});
    let second = json!({"requestId":"reauth-new", "state":"awaitingShellConfirmation"});
    let (provider, server) = mock_recovery_broker(vec![
      denied.clone(),
      first,
      denied.clone(),
      rejected,
      second,
    ])
    .await;
    provider
      .save_state(&HostState {
        recovery_task_id: Some("denied-task".into()),
        ..HostState::default()
      })
      .unwrap();

    provider.reauthorize_denied_task().await.unwrap();
    provider.reauthorize_denied_task().await.unwrap();
    let requests = server.await.unwrap();
    let old_key = requests[1]["params"]["idempotencyKey"].as_str().unwrap();
    let new_key = requests[4]["params"]["idempotencyKey"].as_str().unwrap();
    assert_eq!(requests[3]["params"]["requestId"], "reauth-old");
    assert_ne!(old_key, new_key);
    let saved = provider.load_state().unwrap();
    assert_eq!(saved.reauthorizations.len(), 2);
    assert_eq!(saved.reauthorizations[0].denied_task_id, "denied-task");
    assert_eq!(saved.reauthorizations[0].state.as_deref(), Some("denied"));
    assert_eq!(saved.reauthorizations[1].denied_task_id, "denied-task");
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[test]
  fn deployment_requires_every_action_to_be_explicitly_available() {
    let mut actions = json!({});
    assert!(!deployment_actions_available(&actions));
    for name in SETUP_ACTIONS {
      actions[name] = if name == "debian.packages.ensure" {
        json!({"available": true, "schemaVersions": [1]})
      } else {
        json!({"available": true})
      };
    }
    assert!(deployment_actions_available(&actions));
    for name in SETUP_ACTIONS {
      let mut unavailable = actions.clone();
      unavailable[name] = json!({"available": false, "reason": "disabled-by-host-policy"});
      assert!(!deployment_actions_available(&unavailable));
      unavailable[name] = json!({"available": "true"});
      assert!(!deployment_actions_available(&unavailable));
    }
  }

  #[test]
  fn blockers_preserve_missing_actions_and_transport_errors() {
    let mut capabilities =
      json!({"protocolVersion": 1, "actions": {}, "authorization": {"passwordSentToApp": false}});
    for name in SETUP_ACTIONS {
      capabilities["actions"][name] = if name == "debian.packages.ensure" {
        json!({"available": true, "schemaVersions": [1], "state": {"present": false}})
      } else {
        json!({"available": true, "state": {"present": false}})
      };
    }
    capabilities["actions"]["linux.module.load"] = json!({"available": false, "reason": "no-modules-allowlisted-by-host-policy", "state": {"moduleFileInstalled": true}});
    let message = deployment_blockers(&capabilities);
    assert!(message.contains("linux.module.load: no-modules-allowlisted-by-host-policy"));
    assert!(!message.contains("linux.device.assign"));
    assert_eq!(
      capabilities["actions"]["linux.module.load"]["state"]["moduleFileInstalled"],
      true
    );
    assert!(
      deployment_blockers(&json!({"message": "Permission denied (os error 13)"}))
        .contains("Permission denied")
    );
    assert!(deployment_blockers(&json!({})).contains("capability-not-reported"));
  }

  #[test]
  fn ensure_requires_schema_v1_and_blocked_plans_are_not_authorized() {
    let base = json!({
      "debian.packages.ensure": {"available": true, "schemaVersions": [1]},
      "linux.module.load": {"available": true},
      "linux.binderfs.ensure": {"available": true}
    });
    assert!(deployment_actions_available(&base));
    let mut unsupported = base.clone();
    unsupported["debian.packages.ensure"]["schemaVersions"] = json!([2]);
    assert!(!deployment_actions_available(&unsupported));
    assert!(deployment_blockers(&json!({"actions": unsupported}))
      .contains("schema-version-1-unavailable"));

    let task = json!({"state": "blocked", "error": {"code": "preflight-blocked", "details": [
      {"code": "package-version-conflict", "package": "androidtone-waydroid-images", "installedVersion": "0.1.7", "requestedVersion": "0.1.8-1", "reasonCode": "package-upgrade-disallowed"}
    ]}});
    assert!(preflight_blocked_summary(&task).contains("未进入系统授权队列"));
    assert_eq!(task["error"]["details"][0]["installedVersion"], "0.1.7");
  }

  #[test]
  fn disk_failure_is_distinct_from_authorization_and_verbose_logs_are_bounded() {
    let task = json!({"error": {"message": "deployment failed", "details": {"stderr": "dpkg: No space left on device (ENOSPC)"}}});
    assert!(deployment_failure_summary(&task).starts_with("ENOSPC"));
    let task = json!({"error": {"message": "authorization denied"}});
    assert_eq!(deployment_failure_summary(&task), "authorization denied");
    let task = json!({"error": {"message": "x".repeat(2000)}});
    assert_eq!(deployment_failure_summary(&task).chars().count(), 240);
  }

  #[test]
  fn reads_only_supported_waydroid_image_locations_from_config() {
    let config = "# Waydroid config\n[waydroid]\nimages_path = /usr/share/waydroid-extra/images\n\n[other]\nimages_path = /tmp/untrusted\n";
    assert_eq!(
      configured_waydroid_images_path(config),
      Some(PathBuf::from("/usr/share/waydroid-extra/images"))
    );
    assert_eq!(
      configured_waydroid_images_path(
        "[waydroid]\nimages_path = /tmp/untrusted\n[other]\nimages_path = /usr/share/waydroid-extra/images\n"
      ),
      None
    );
  }

  #[test]
  fn detects_aarch32_el0_from_cpu_mode_and_recognizes_the_android_self_test_elf() {
    assert_eq!(
      aarch32_el0_supported_from_lscpu("Architecture: aarch64\nCPU op-mode(s): 64-bit\n"),
      Some(false)
    );
    assert_eq!(
      aarch32_el0_supported_from_lscpu("Architecture: aarch64\nCPU op-mode(s): 32-bit, 64-bit\n"),
      Some(true)
    );
    assert_eq!(
      aarch32_el0_supported_from_lscpu("Architecture: aarch64\n"),
      None
    );

    let mut elf32_arm = vec![0; 20];
    elf32_arm[..4].copy_from_slice(b"\x7fELF");
    elf32_arm[4] = 1;
    elf32_arm[5] = 1;
    elf32_arm[18..20].copy_from_slice(&40u16.to_le_bytes());
    assert!(is_elf32_arm(&elf32_arm));
    elf32_arm[4] = 2;
    assert!(!is_elf32_arm(&elf32_arm));
  }

  #[test]
  fn waydroid_cli_operations_have_bounded_durations_and_failed_start_clears_session_ids() {
    assert_eq!(ANDROID_STARTUP_TIMEOUT, Duration::from_secs(180));
    assert_eq!(
      waydroid_command_timeout(&["session".into(), "stop".into()]),
      WAYDROID_SESSION_STOP_TIMEOUT
    );
    assert_eq!(
      waydroid_command_timeout(&["status".into()]),
      WAYDROID_COMMAND_TIMEOUT
    );

    let mut state = HostState {
      display_session_id: Some("old-wayland-session".into()),
      desktop_session_id: Some("old-desktop-session".into()),
      ..HostState::default()
    };
    clear_display_session(&mut state);
    assert!(state.display_session_id.is_none());
    assert!(state.desktop_session_id.is_none());
  }

  #[tokio::test]
  async fn bounded_output_kills_a_cli_that_exceeds_its_deadline() {
    let mut command = Command::new("/bin/sleep");
    command.arg("5");
    let started = tokio::time::Instant::now();
    let error = bounded_output(
      &mut command,
      Duration::from_millis(30),
      "test Waydroid command",
    )
    .await
    .unwrap_err();
    assert!(error.contains("超时"));
    assert!(tokio::time::Instant::now().duration_since(started) < Duration::from_secs(1));
  }

  fn startup_test_session(dir: &Path) -> SessionContext {
    SessionContext {
      session_id: "startup-test-desktop".into(),
      uid: unsafe { libc::geteuid() },
      gid: unsafe { libc::getegid() },
      home: dir.to_string_lossy().into_owned(),
      runtime_dir: dir.to_string_lossy().into_owned(),
      dbus_session_bus_address: "unix:path=/test/no-bus-required".into(),
    }
  }

  fn fake_waydroid(dir: &Path, script: &str) -> PathBuf {
    let binary = dir.join("waydroid-test");
    fs::write(&binary, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    binary
  }

  fn supervised_test_command(binary: &Path, session: &SessionContext) -> Command {
    let mut command = Command::new(binary);
    command
      .args(["session", "start"])
      .env_clear()
      .envs(session.environment())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    command
  }

  async fn wait_for_test_file(path: &Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !path.is_file() && tokio::time::Instant::now() < deadline {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(path.is_file(), "timed out waiting for {}", path.display());
  }

  async fn assert_test_pid_stopped(path: &Path) {
    let pid = fs::read_to_string(path).unwrap().trim().to_owned();
    let status = Command::new("/bin/kill")
      .args(["-0", &pid])
      .stderr(Stdio::null())
      .status()
      .await
      .unwrap();
    assert!(!status.success(), "Waydroid CLI child {pid} remains alive");
  }

  #[tokio::test]
  async fn ready_waydroid_session_outlives_startup_deadline_until_user_stop() {
    let dir = env::temp_dir().join(format!("at-session-supervisor-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let session = startup_test_session(&dir);
    let binary = fake_waydroid(
      &dir,
      r#"
      case "$1 $2" in
        "session start") echo $$ > "$HOME/start.pid"; exec sleep 30 ;;
        "prop get") touch "$HOME/framework-ready"; echo 1 ;;
        "session stop") echo stop >> "$HOME/commands"; kill "$(cat "$HOME/start.pid")" 2>/dev/null || true ;;
      esac
    "#,
    );
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let started = tokio::time::Instant::now();
    let supervisor = supervise_waydroid_session(
      supervised_test_command(&binary, &session),
      &session,
      &binary,
      &session.runtime_dir,
      Path::new(&session.runtime_dir),
      "wayland-test",
      Duration::from_secs(1),
      async move {
        let _ = stop_rx.await;
      },
    );
    tokio::pin!(supervisor);
    let framework_ready_file = dir.join("framework-ready");
    tokio::select! {
      result = &mut supervisor => panic!("supervisor exited before framework readiness: {result:?}"),
      _ = wait_for_test_file(&framework_ready_file) => {},
    }
    tokio::select! {
      result = &mut supervisor => panic!("supervisor exited before user stop: {result:?}"),
      _ = tokio::time::sleep(Duration::from_millis(1250)) => {},
    }
    assert!(started.elapsed() > Duration::from_secs(1));
    stop_tx.send(()).unwrap();
    supervisor.await.unwrap();
    assert!(fs::read_to_string(dir.join("commands"))
      .unwrap()
      .contains("stop"));
    assert_test_pid_stopped(&dir.join("start.pid")).await;
    fs::remove_dir_all(dir).unwrap();
  }

  #[tokio::test]
  async fn startup_deadline_expiry_stops_the_session_and_reaps_the_cli_child() {
    let dir = env::temp_dir().join(format!("at-session-timeout-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let session = startup_test_session(&dir);
    let binary = fake_waydroid(
      &dir,
      r#"
      case "$1 $2" in
        "session start") echo $$ > "$HOME/start.pid"; exec sleep 30 ;;
        "prop get") echo 0 ;;
        "session stop") echo stop >> "$HOME/commands"; kill "$(cat "$HOME/start.pid")" 2>/dev/null || true ;;
      esac
    "#,
    );
    let started = tokio::time::Instant::now();
    let error = supervise_waydroid_session(
      supervised_test_command(&binary, &session),
      &session,
      &binary,
      &session.runtime_dir,
      Path::new(&session.runtime_dir),
      "wayland-test",
      Duration::from_millis(120),
      std::future::pending(),
    )
    .await
    .unwrap_err();
    assert!(error.contains("waydroid-startup-timeout"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(fs::read_to_string(dir.join("commands"))
      .unwrap()
      .contains("stop"));
    assert_test_pid_stopped(&dir.join("start.pid")).await;
    fs::remove_dir_all(dir).unwrap();
  }

  #[tokio::test]
  async fn session_exit_after_framework_ready_is_reported_and_cleaned_up() {
    let dir = env::temp_dir().join(format!("at-session-exit-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let session = startup_test_session(&dir);
    let binary = fake_waydroid(
      &dir,
      r#"
      case "$1 $2" in
        "session start") echo $$ > "$HOME/start.pid"; sleep 0.15; exit 7 ;;
        "prop get") echo 1 ;;
        "session stop") echo stop >> "$HOME/commands" ;;
      esac
    "#,
    );
    let error = supervise_waydroid_session(
      supervised_test_command(&binary, &session),
      &session,
      &binary,
      &session.runtime_dir,
      Path::new(&session.runtime_dir),
      "wayland-test",
      Duration::from_secs(2),
      std::future::pending(),
    )
    .await
    .unwrap_err();
    assert!(
      error.contains("Android framework 就绪后异常退出"),
      "{error}"
    );
    assert!(error.contains("exit status: 7"), "{error}");
    assert!(fs::read_to_string(dir.join("commands"))
      .unwrap()
      .contains("stop"));
    fs::remove_dir_all(dir).unwrap();
  }

  #[tokio::test]
  async fn user_stop_has_bounded_cleanup_for_the_long_lived_cli_child() {
    let dir = env::temp_dir().join(format!("at-session-stop-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let session = startup_test_session(&dir);
    let binary = fake_waydroid(
      &dir,
      r#"
      case "$1 $2" in
        "session start") echo $$ > "$HOME/start.pid"; exec sleep 30 ;;
        "prop get") touch "$HOME/framework-ready"; echo 1 ;;
        "session stop") echo stop >> "$HOME/commands"; kill "$(cat "$HOME/start.pid")" 2>/dev/null || true ;;
      esac
    "#,
    );
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let started = tokio::time::Instant::now();
    let supervisor = supervise_waydroid_session(
      supervised_test_command(&binary, &session),
      &session,
      &binary,
      &session.runtime_dir,
      Path::new(&session.runtime_dir),
      "wayland-test",
      Duration::from_secs(2),
      async move {
        let _ = stop_rx.await;
      },
    );
    tokio::pin!(supervisor);
    let framework_ready_file = dir.join("framework-ready");
    tokio::select! {
      result = &mut supervisor => panic!("supervisor exited before framework readiness: {result:?}"),
      _ = wait_for_test_file(&framework_ready_file) => {},
    }
    stop_tx.send(()).unwrap();
    supervisor.await.unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_test_pid_stopped(&dir.join("start.pid")).await;
    assert!(fs::read_to_string(dir.join("commands"))
      .unwrap()
      .contains("stop"));
    fs::remove_dir_all(dir).unwrap();
  }

  #[tokio::test]
  async fn multi_windows_waits_for_framework_and_can_take_more_than_fifteen_seconds() {
    let dir = env::temp_dir().join(format!("at-startup-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let session = startup_test_session(&dir);
    let binary = fake_waydroid(
      &dir,
      r#"
      echo "$*" >> "$HOME/commands"
      if [ "$3" = sys.boot_completed ]; then
        if [ -f "$HOME/boot-checked" ]; then echo 1;
        else touch "$HOME/boot-checked"; echo 0; fi
      elif [ "$2" = set ]; then
        exec sleep 16
      else echo true; fi
    "#,
    );
    let started = tokio::time::Instant::now();
    configure_multi_windows(&session, &binary, started + Duration::from_secs(35))
      .await
      .unwrap();
    assert!(started.elapsed() > WAYDROID_COMMAND_TIMEOUT);
    assert!(started.elapsed() < Duration::from_secs(35));
    let commands = fs::read_to_string(dir.join("commands")).unwrap();
    assert_eq!(
      commands.lines().collect::<Vec<_>>(),
      vec![
        "prop get sys.boot_completed",
        "prop get sys.boot_completed",
        "prop set persist.waydroid.multi_windows true",
        "prop get persist.waydroid.multi_windows",
      ]
    );
    fs::remove_dir_all(dir).unwrap();
  }

  #[tokio::test]
  async fn multi_windows_budget_exhaustion_preserves_diagnostics_and_cleans_a_hung_session() {
    let (provider, server) = mock_agentos_broker_for(
      EXTERNAL_WAYLAND_PROVIDER_ID,
      vec![BrokerReply::Json(json!({"stopped": true}))],
    )
    .await;
    let session = startup_test_session(&provider.data_dir);
    let binary = fake_waydroid(
      &provider.data_dir,
      r#"
      echo "$*" >> "$HOME/commands"
      if [ "$3" = sys.boot_completed ]; then echo 1;
      else exec sleep 30; fi
    "#,
    );
    let mut state = HostState {
      display_session_id: Some("private-wayland-test".into()),
      desktop_session_id: Some(session.session_id.clone()),
      ..HostState::default()
    };
    provider.save_state(&state).unwrap();
    let started = tokio::time::Instant::now();
    let error = configure_multi_windows(&session, &binary, started + Duration::from_secs(2))
      .await
      .unwrap_err();
    let HostProviderError::Agent(ref detail) = error else {
      panic!("structured startup error required")
    };
    assert_eq!(detail.code, "waydroid-startup-timeout");
    assert_eq!(
      detail.details.as_ref().unwrap()["phase"],
      "multi-windows-property-set"
    );
    assert!(started.elapsed() < Duration::from_secs(4));
    provider
      .cleanup_failed_start(&session, &binary, &mut state, Some("private-wayland-test"))
      .await;
    assert!(started.elapsed() < Duration::from_secs(13));
    assert!(state.display_session_id.is_none());
    assert!(state.desktop_session_id.is_none());
    assert!(provider.load_state().unwrap().display_session_id.is_none());
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "session.stop");
    assert_eq!(requests[0]["params"]["sessionId"], "private-wayland-test");
    let commands = fs::read_to_string(provider.data_dir.join("commands")).unwrap();
    assert_eq!(
      commands
        .lines()
        .filter(|line| *line == "session stop")
        .count(),
      1
    );
    assert!(!commands.contains("session start"));
    // Cleanup must not overwrite the original structured failure.
    assert!(
      matches!(classify_start_error(error), HostProviderError::Agent(ref e) if e.code == "waydroid-startup-timeout")
    );
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn expired_task_lookup_falls_back_to_authorization_status_and_preserves_retry_lineage() {
    let expired = json!({
      "taskId":"child-expired",
      "state":"expired",
      "retryOfTaskId":"original-denied",
      "reauthorizationRequestId":"reauth-confirmed"
    });
    let (provider, server) = mock_agentos_broker(vec![
      BrokerReply::Json(json!({"error": {
        "code":"authorization-expired",
        "message":"The authorization request has expired."
      }})),
      BrokerReply::Json(json!({"pending": [], "recent": [expired.clone()]})),
    ])
    .await;

    let task = provider.tracked_task("child-expired").await.unwrap();
    assert_eq!(task["state"], "expired");
    assert_eq!(task["retryOfTaskId"], "original-denied");
    assert_eq!(task["reauthorizationRequestId"], "reauth-confirmed");
    assert_eq!(task["trackingDiagnostic"]["source"], "authorization.status");
    assert_eq!(task["trackingDiagnostic"]["matchedTask"], true);
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["method"], "task.get");
    assert_eq!(requests[1]["method"], "authorization.status");

    let mut host_state = HostState::default();
    host_state.reauthorizations.push(ReauthorizationRecord {
      denied_task_id: "original-denied".into(),
      idempotency_key: "reauth-key-1".into(),
      target: "recovery".into(),
      request_id: Some("reauth-confirmed".into()),
      state: Some("confirmed".into()),
      response: Some(
        json!({"requestId":"reauth-confirmed", "state":"confirmed", "newTaskId":"child-expired"}),
      ),
      new_task_id: Some("child-expired".into()),
    });
    let view = recovery_task_view(&task, &recovery_capabilities()).unwrap();
    assert_eq!(view["state"], "authorization-expired");
    assert_eq!(view["canReapply"], true);
    let lineage = deployment_lineage(&host_state, &task).unwrap();
    assert_eq!(lineage["retryOfTaskId"], "original-denied");
    assert_eq!(lineage["reauthorizationState"], "confirmed");
    assert_eq!(lineage["newTaskId"], "child-expired");
  }

  #[tokio::test]
  async fn explicit_reapplication_after_expiration_rotates_key_and_tracks_new_task() {
    let identity = runtime_setup_request_identity().unwrap();
    let old_key = "expired-deployment-key";
    let (provider, server) = mock_agentos_broker(vec![
      BrokerReply::Json(json!({"error": {
        "code":"authorization-expired",
        "message":"The authorization request has expired."
      }})),
      BrokerReply::Json(
        json!({"pending": [], "recent": [{"taskId":"expired-task", "state":"expired"}]}),
      ),
      BrokerReply::Json(runtime_setup_capabilities()),
      BrokerReply::Json(json!({"taskId":"replacement-task", "state":"awaitingAuthorization"})),
    ])
    .await;
    provider
      .save_state(&HostState {
        deployment_task_id: Some("expired-task".into()),
        deployment_request_key: Some(old_key.into()),
        runtime_operation_version: identity.operation_version,
        deployment_request_binding: Some(DeploymentRequestBinding {
          request: identity.clone(),
          idempotency_key: old_key.into(),
        }),
        ..HostState::default()
      })
      .unwrap();

    let task = provider
      .plan_runtime_setup(&identity)
      .await
      .unwrap()
      .unwrap();
    assert_eq!(task["taskId"], "replacement-task");
    assert_eq!(task["state"], "awaitingAuthorization");
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0]["method"], "task.get");
    assert_eq!(requests[1]["method"], "authorization.status");
    assert_eq!(requests[2]["method"], "capabilities.get");
    assert_eq!(requests[3]["method"], "operation.plan");
    assert_eq!(requests[3]["params"]["operationId"], identity.operation_id);
    assert_eq!(
      requests[3]["params"]["operationVersion"],
      identity.operation_version
    );
    assert_eq!(requests[3]["params"]["parameters"], identity.parameters);
    let new_key = requests[3]["params"]["idempotencyKey"].as_str().unwrap();
    assert_ne!(new_key, old_key);
    let saved = provider.load_state().unwrap();
    assert_eq!(
      saved.deployment_task_id.as_deref(),
      Some("replacement-task")
    );
    assert_eq!(
      saved.deployment_request_binding.unwrap().idempotency_key,
      new_key
    );
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn explicit_recovery_after_reauthorized_child_expires_uses_a_new_plan_key() {
    let (provider, server) = mock_agentos_broker(vec![
      BrokerReply::Json(json!({"error": {
        "code":"authorization-expired",
        "message":"The authorization request has expired."
      }})),
      BrokerReply::Json(json!({"pending": [], "recent": [{
        "taskId":"expired-child",
        "state":"expired",
        "retryOfTaskId":"original-denied",
        "reauthorizationRequestId":"reauth-confirmed"
      }]})),
      BrokerReply::Json(json!({"taskId":"new-recovery-task", "state":"awaitingAuthorization"})),
    ])
    .await;
    provider
      .save_state(&HostState {
        recovery_task_id: Some("expired-child".into()),
        recovery_request_key: Some("expired-recovery-key".into()),
        ..HostState::default()
      })
      .unwrap();

    let task = provider
      .plan_recovery(&recovery_capabilities())
      .await
      .unwrap()
      .unwrap();
    assert_eq!(task["taskId"], "new-recovery-task");
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0]["method"], "task.get");
    assert_eq!(requests[1]["method"], "authorization.status");
    assert_eq!(requests[2]["method"], "operation.plan");
    assert_eq!(
      requests[2]["params"]["operationId"],
      "android-runtime-recover"
    );
    assert_eq!(requests[2]["params"]["operationVersion"], 1);
    assert_ne!(
      requests[2]["params"]["idempotencyKey"],
      "expired-recovery-key"
    );
    let saved = provider.load_state().unwrap();
    assert_eq!(saved.recovery_task_id.as_deref(), Some("new-recovery-task"));
    assert_ne!(
      saved.recovery_request_key.as_deref(),
      Some("expired-recovery-key")
    );
    fs::remove_dir_all(provider.data_dir).unwrap();
  }

  #[tokio::test]
  async fn terminal_reauthorization_is_restored_without_polling_or_reusing_the_old_task() {
    let (provider, server) = mock_agentos_broker(vec![]).await;
    let mut state = HostState::default();
    state.reauthorizations.push(ReauthorizationRecord {
      denied_task_id: "old-denied-task".into(),
      idempotency_key: "reauth-key-terminal".into(),
      target: "recovery".into(),
      request_id: Some("reauth-expired".into()),
      state: Some("confirmed".into()),
      response: Some(json!({
        "requestId":"reauth-expired",
        "deniedTaskId":"old-denied-task",
        "newTaskId":"new-expired-task",
        "state":"confirmed",
        "newTask":{"taskId":"new-expired-task", "state":"expired", "retryOfTaskId":"old-denied-task"}
      })),
      new_task_id: Some("new-expired-task".into()),
    });

    let response = provider
      .poll_reauthorization(&mut state)
      .await
      .unwrap()
      .unwrap();
    assert_eq!(response["state"], "confirmed");
    assert_eq!(response["newTaskId"], "new-expired-task");
    assert_eq!(state.reauthorizations[0].denied_task_id, "old-denied-task");
    assert_eq!(
      state.reauthorizations[0].new_task_id.as_deref(),
      Some("new-expired-task")
    );
    assert!(server.await.unwrap().is_empty());
    fs::remove_dir_all(provider.data_dir).unwrap();
  }
}
