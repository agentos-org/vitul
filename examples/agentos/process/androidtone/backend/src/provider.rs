use std::{
  env,
  fs::{self, File, OpenOptions},
  io::{Read, Write},
  os::fd::AsRawFd,
  os::unix::fs::{DirBuilderExt, PermissionsExt},
  path::{Path, PathBuf},
  process::Output,
  time::Duration,
};

use crate::platform::{
  kernel_observations, private_wayland_environment, AdministrativeOperation, SessionContext,
};
use bytes::Bytes;
use hyper::{body::Incoming, client::conn::http1, header::HOST, Method, Request};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::{net::UnixStream, process::Command};

const ANDROIDTONE_PROVIDER_ID: &str = "org.agentos.androidtone";
const EXTERNAL_WAYLAND_PROVIDER_ID: &str = "org.agentos.external-wayland";
const DESKTOP_SESSION_PROVIDER_ID: &str = "org.agentos.desktop-session";
const MAX_APK_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ProviderClient {
  socket: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ProviderError(pub String);

impl std::fmt::Display for ProviderError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(&self.0)
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
    SessionContext::from_provider(&value).map_err(ProviderError)
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
    self
      .request(
        Method::POST,
        &format!("/v1/providers/{provider_id}/invoke"),
        Some(body),
      )
      .await
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
    .map_err(|_| ProviderError("AgentOS Host Provider 请求超时".into()))?
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
      .ok_or_else(|| ProviderError("AgentOS 尚未向 AndroidTone 授予 Host Provider。".into()))?;
    let stream = UnixStream::connect(socket)
      .await
      .map_err(|error| ProviderError(format!("无法连接 AgentOS Host Provider：{error}")))?;
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
    decode(response).await
  }
}

async fn decode(response: hyper::Response<Incoming>) -> Result<Value, ProviderError> {
  let status = response.status();
  let body = http_body_util::BodyExt::collect(response.into_body())
    .await
    .map_err(protocol_error)?
    .to_bytes();
  let value: Value = serde_json::from_slice(&body).map_err(protocol_error)?;
  if !status.is_success() {
    return Err(ProviderError(
      value["error"]["message"]
        .as_str()
        .or(value["error"].as_str())
        .unwrap_or("AgentOS Host Provider 拒绝了请求")
        .to_owned(),
    ));
  }
  Ok(value)
}

fn protocol_error(error: impl std::fmt::Display) -> ProviderError {
  ProviderError(format!("AndroidTone Provider 协议错误：{error}"))
}

pub fn unavailable_status(error: Option<&ProviderError>) -> Value {
  json!({ "available": false, "installed": false, "running": false, "state": "provider-unavailable", "runtimeBundle": crate::bundle::status(), "canInstall": false, "message": error.map(|e| e.0.as_str()).unwrap_or("等待 AndroidTone Host Provider 授权") })
}

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
  install_tasks: Vec<InstallRecord>,
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

  async fn invoke(&self, method: &str, params: Value) -> Result<Value, String> {
    if method.starts_with("runtime.") {
      crate::platform::validate_runtime_params(&params)?;
    }
    match method {
      "runtime.status" => self.status().await,
      "runtime.install" => self.install_runtime().await,
      "runtime.initialize" => self.initialize_runtime().await,
      "runtime.start" => self.start_runtime().await,
      "runtime.stop" => self.stop_runtime().await,
      "applications.list" => self.applications().await,
      "applications.launch" => self.app_action("launch", &params).await,
      "applications.stop" => self.app_action("stop", &params).await,
      "applications.uninstall" => self.app_action("uninstall", &params).await,
      "installations.register" => self.register_installation(params).await,
      "installations.install" => self.install_apk(&params).await,
      "installations.list" => self.list_installations().await,
      "installations.remove" => self.remove_installation(&params).await,
      _ => Err("AndroidTone Host Provider does not register this method.".into()),
    }
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

  async fn status(&self) -> Result<Value, String> {
    let kernel = kernel_observations();
    let session = self.client.desktop_session().await.map_err(|error| error.0);
    let initialized = waydroid_initialized();
    let dependencies = json!({
      "administratorOperations": "platform-dependency",
      "trustedSessionContext": if session.is_ok() { "provided" } else { "platform-dependency" },
      "privateWaylandContext": "provided-on-session-start",
      "windowPresentation": "unverified"
    });
    let Some(binary) = waydroid_binary() else {
      return Ok(
        json!({"available": false, "installed": false, "initialized": false,
        "running": false, "state": "runtime-missing", "canInstall": false,
        "kernel": kernel, "platformDependencies": dependencies, "runtimeBundle": crate::bundle::status(),
        "message": "宿主尚未部署 Android 运行环境。当前 AgentOS 通用部署动作缺少 Binderfs 设备准备和 LXC 容器初始化与生命周期管理；APK 可先导入保存。"}),
      );
    };
    let session = match session {
      Ok(session) => session,
      Err(error) => {
        return Ok(
          json!({"available": false, "installed": true, "initialized": initialized,
        "running": false, "state": "platform-dependency", "canInstall": false,
        "canInitialize": false, "canStart": false, "kernel": kernel,
        "platformDependencies": dependencies, "runtimeBundle": crate::bundle::status(), "message": error}),
        );
      }
    };
    if !initialized {
      return Ok(
        json!({"available": false, "installed": true, "initialized": false,
        "running": false, "state": "uninitialized", "canInitialize": false,
        "kernel": kernel, "platformDependencies": dependencies, "runtimeBundle": crate::bundle::status(),
        "message": "Android 系统镜像尚未初始化；当前 AgentOS 通用部署动作尚不支持所需的受限容器初始化。"}),
      );
    }
    let output = run_user(&session, &binary, &["status".into()]).await;
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
    if session_running && state.display_session_id.is_none() {
      return Ok(
        json!({ "available": true, "installed": true, "initialized": true, "running": false, "state": "external-session", "kernel": kernel, "platformDependencies": dependencies, "runtimeBundle": crate::bundle::status(), "message": "Waydroid 报告已有未绑定当前平台显示会话的 session；AndroidTone 不接管该会话。" }),
      );
    }
    let running = session_running && container_running && state.display_session_id.is_some();
    let current_state = if running {
      "process-running"
    } else if container_running {
      "container-running"
    } else {
      "stopped"
    };
    Ok(
      json!({ "available": true, "installed": true, "initialized": true, "running": running, "windowVerification": {"mapping": "unknown", "flutterPresentation": "unknown", "inputFocus": "unknown", "scaling": "unknown", "multiWindow": "unknown", "gpuPresentation": "unknown", "exitCleanup": "unknown"}, "state": current_state, "displaySessionId": state.display_session_id, "kernel": kernel, "platformDependencies": dependencies, "runtimeBundle": crate::bundle::status(), "canStart": false, "canStop": state.display_session_id.is_some() || state.owns_container, "message": if running { "Waydroid 进程报告运行；窗口映射、Shell 呈现及输入尚未验证。" } else if container_running { "Waydroid 容器已运行，可以启动 AgentOS 显示会话。" } else { "Waydroid 已就绪，可以启动 Android 应用。" } }),
    )
  }

  async fn install_runtime(&self) -> Result<Value, String> {
    tokio::task::spawn_blocking(crate::bundle::verify)
      .await
      .map_err(|e| e.to_string())??;
    request_administrative_operation(AdministrativeOperation::InstallRuntime).await?;
    self.status().await
  }

  async fn initialize_runtime(&self) -> Result<Value, String> {
    tokio::task::spawn_blocking(crate::bundle::verify)
      .await
      .map_err(|e| e.to_string())??;
    request_administrative_operation(AdministrativeOperation::InitializeImages).await?;
    self.status().await
  }

  async fn start_runtime(&self) -> Result<Value, String> {
    let binary = waydroid_binary().ok_or("宿主尚未安装 Waydroid。")?;
    if !waydroid_initialized() {
      return Err("请先初始化 Waydroid Android 系统镜像。".into());
    }
    let desktop_session = self
      .client
      .desktop_session()
      .await
      .map_err(|error| error.0)?;
    // Private Wayland allocation and administrator operation contracts are platform dependencies.
    request_administrative_operation(AdministrativeOperation::StartContainer).await?;
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
        return self.status().await;
      }
      let _ = self
        .client
        .invoke_provider(
          EXTERNAL_WAYLAND_PROVIDER_ID,
          "session.stop",
          json!({ "sessionId": session_id }),
        )
        .await;
      self.save_state(&state)?;
    }
    if session_running {
      return Err(
        "Waydroid 报告已有未归属的 session；需要其所属会话退出，AndroidTone 不自行接管。".into(),
      );
    }
    if !container_running {
      let output =
        request_administrative_operation(AdministrativeOperation::StartContainer).await?;
      ensure_success(&output, "Waydroid 容器启动失败")?;
      state.owns_container = true;
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
    let display = self
      .client
      .invoke_provider(
        EXTERNAL_WAYLAND_PROVIDER_ID,
        "session.start",
        json!({"launch": launch}),
      )
      .await
      .map_err(|error| error.0)?;
    let session_id = display["sessionId"]
      .as_str()
      .ok_or("AgentOS did not create the Android display session.")?
      .to_owned();
    state.desktop_session_id = Some(desktop_session.session_id.clone());
    state.display_session_id = Some(session_id.clone());
    self.save_state(&state)?;
    self
      .wait_until_ready(&desktop_session, &binary, Duration::from_secs(60))
      .await?;
    let property = run_user(
      &desktop_session,
      &binary,
      &[
        "prop".into(),
        "set".into(),
        "persist.waydroid.multi_windows".into(),
        "true".into(),
      ],
    )
    .await?;
    ensure_success(&property, "Waydroid 多窗口设置失败")?;
    self
      .client
      .invoke_provider(
        EXTERNAL_WAYLAND_PROVIDER_ID,
        "session.prepare_restart",
        json!({"sessionId": session_id}),
      )
      .await
      .map_err(|error| error.0)?;
    let stop = run_user(
      &desktop_session,
      &binary,
      &["session".into(), "stop".into()],
    )
    .await?;
    ensure_success(&stop, "Waydroid 用户会话重启失败")?;
    self
      .client
      .invoke_provider(
        EXTERNAL_WAYLAND_PROVIDER_ID,
        "session.restart",
        json!({"sessionId": session_id}),
      )
      .await
      .map_err(|error| error.0)?;
    self
      .wait_until_ready(&desktop_session, &binary, Duration::from_secs(60))
      .await?;
    self.status().await
  }

  async fn stop_runtime(&self) -> Result<Value, String> {
    let desktop_session = self
      .client
      .desktop_session()
      .await
      .map_err(|error| error.0)?;
    let binary = waydroid_binary().ok_or("Waydroid is not installed.")?;
    let mut state = self.load_owned_state(&desktop_session)?;
    if let Some(session_id) = state.display_session_id.take() {
      let _ = run_user(
        &desktop_session,
        &binary,
        &["session".into(), "stop".into()],
      )
      .await;
      let _ = self
        .client
        .invoke_provider(
          EXTERNAL_WAYLAND_PROVIDER_ID,
          "session.stop",
          json!({"sessionId": session_id}),
        )
        .await;
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
    timeout: Duration,
  ) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut last_message = String::new();
    while tokio::time::Instant::now() < deadline {
      match run_user(desktop_session, binary, &["status".into()]).await {
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
      tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(if last_message.is_empty() {
      "Waydroid 在一分钟内没有就绪。".into()
    } else {
      format!("Waydroid 在一分钟内没有就绪：{last_message}")
    })
  }

  async fn applications(&self) -> Result<Value, String> {
    let binary = waydroid_binary().ok_or("Waydroid is not installed.")?;
    let desktop_session = self
      .client
      .desktop_session()
      .await
      .map_err(|error| error.0)?;
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
      .map_err(|error| error.0)?;
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
      .map_err(|error| error.0)?;
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
    self.save_state(&state)?;
    let task = state.install_tasks[index].clone();
    let stage_dir = env::temp_dir().join(format!("androidtone-apk-{}", uuid::Uuid::new_v4()));
    let install_result = async {
      let source = self.resolve_app_data_file(&format!("imports/{id}.apk"))?;
      verify_file(&source, task.bytes, &task.sha256)?;
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
      ensure_success(&output, "Waydroid APK 安装失败")
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
          record.message = "APK 安装完成。".into();
        }
        Err(error) => {
          record.status = "failed".into();
          record.message = error.clone();
        }
      }
    }
    self.save_state(&state)?;
    install_result?;
    Ok(json!({"id": id, "status": "installed", "message": "APK 安装完成。"}))
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
    Err(message) => {
      eprintln!("{}", message);
      Err(std::io::Error::other(message).into())
    }
  }
}

pub async fn run_wayland_client(
  desktop_session_json: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let session =
    SessionContext::from_launch_argument(desktop_session_json).map_err(std::io::Error::other)?;
  let (runtime_dir, display) =
    private_wayland_environment(&session).map_err(std::io::Error::other)?;
  let binary = waydroid_binary().ok_or("Waydroid is not installed.")?;
  let status = Command::new(binary)
    .args(["session", "start"])
    .env_clear()
    .envs(session.environment())
    .env("XDG_RUNTIME_DIR", runtime_dir)
    .env("WAYLAND_DISPLAY", display)
    .status()
    .await?;
  if !status.success() {
    return Err(format!("Waydroid user session exited with {status}.").into());
  }
  Ok(())
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
  Command::new(binary)
    .args(args)
    .env_clear()
    .envs(session.environment())
    .output()
    .await
    .map_err(|error| format!("Cannot run Waydroid: {error}"))
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
  let container = output
    .lines()
    .any(|line| line.trim().eq_ignore_ascii_case("Container: RUNNING"));
  let session = output
    .lines()
    .any(|line| line.trim().eq_ignore_ascii_case("Session: RUNNING"));
  (container, session)
}

fn waydroid_initialized() -> bool {
  Path::new("/var/lib/waydroid/images/system.img").is_file()
    && Path::new("/var/lib/waydroid/images/vendor.img").is_file()
}

fn valid_task_id(id: &str) -> bool {
  id.len() >= 16
    && id.len() <= 64
    && id.bytes().all(|byte| {
      byte.is_ascii_lowercase() && byte <= b'f' || byte.is_ascii_digit() || byte == b'-'
    })
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
