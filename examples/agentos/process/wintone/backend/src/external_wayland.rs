use std::{env, path::PathBuf, time::Duration};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
  body::Incoming,
  client::conn::http1,
  header::{CONTENT_TYPE, HOST},
  Method, Request, StatusCode,
};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::UnixStream;

#[derive(Clone, Debug)]
pub struct ExternalWaylandClient {
  socket: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSession {
  pub session_id: String,
  pub application_id: String,
  pub state: String,
  pub exit_code: Option<i32>,
  pub error: Option<String>,
  #[serde(default)]
  pub windows: Vec<Value>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSession {
  pub version: u8,
  pub application_id: String,
  pub executable: String,
  pub args: Vec<String>,
  pub working_directory: String,
  pub environment: std::collections::BTreeMap<String, String>,
  pub files: Vec<FileGrant>,
  pub permissions: Vec<String>,
  pub window_hints: WindowHints,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileGrant {
  pub root: String,
  pub access: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct WindowHints {
  pub title: String,
  pub width: u32,
  pub height: u32,
}

#[derive(Debug)]
pub struct WaylandError {
  pub message: String,
  pub status: Option<StatusCode>,
  pub session_id: Option<String>,
}

impl std::fmt::Display for WaylandError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.message)
  }
}

impl std::error::Error for WaylandError {}

impl ExternalWaylandClient {
  pub fn from_env() -> Self {
    Self {
      socket: env::var_os("AGENTOS_EXTERNAL_WAYLAND_SOCKET").map(PathBuf::from),
    }
  }

  pub fn is_configured(&self) -> bool {
    self.socket.is_some()
  }

  pub async fn capabilities(&self) -> Result<Value, WaylandError> {
    self.request(Method::GET, "/v1/capabilities", None).await
  }

  pub async fn list_sessions(&self) -> Result<Vec<ExternalSession>, WaylandError> {
    let response = self.request(Method::GET, "/v1/sessions", None).await?;
    serde_json::from_value(response["sessions"].clone()).map_err(|error| WaylandError {
      message: format!("Invalid AgentOS session list: {error}"),
      status: None,
      session_id: None,
    })
  }

  pub async fn create(&self, request: &CreateSession) -> Result<ExternalSession, WaylandError> {
    let value = self
      .request(Method::POST, "/v1/sessions", Some(json!(request)))
      .await?;
    serde_json::from_value(value).map_err(|error| WaylandError {
      message: format!("Invalid AgentOS session response: {error}"),
      status: None,
      session_id: None,
    })
  }

  pub async fn close(&self, session_id: &str) -> Result<ExternalSession, WaylandError> {
    self.session_action(session_id, "close").await
  }

  pub async fn terminate(&self, session_id: &str) -> Result<ExternalSession, WaylandError> {
    self.session_action(session_id, "terminate").await
  }

  async fn session_action(
    &self,
    session_id: &str,
    action: &str,
  ) -> Result<ExternalSession, WaylandError> {
    let value = self
      .request(
        Method::POST,
        &format!("/v1/sessions/{session_id}/{action}"),
        Some(json!({})),
      )
      .await?;
    serde_json::from_value(value).map_err(|error| WaylandError {
      message: format!("Invalid AgentOS session response: {error}"),
      status: None,
      session_id: None,
    })
  }

  async fn request(
    &self,
    method: Method,
    path: &str,
    body: Option<Value>,
  ) -> Result<Value, WaylandError> {
    tokio::time::timeout(
      Duration::from_secs(5),
      self.request_inner(method, path, body),
    )
    .await
    .map_err(|_| WaylandError {
      message: "AgentOS Wayland API request timed out.".to_owned(),
      status: None,
      session_id: None,
    })?
  }

  async fn request_inner(
    &self,
    method: Method,
    path: &str,
    body: Option<Value>,
  ) -> Result<Value, WaylandError> {
    let socket = self.socket.as_ref().ok_or_else(|| WaylandError {
      message: "AgentOS external Wayland API is not enabled for this process App.".to_owned(),
      status: None,
      session_id: None,
    })?;
    let stream = UnixStream::connect(socket)
      .await
      .map_err(|error| WaylandError {
        message: format!("Cannot connect to AgentOS Wayland API: {error}"),
        status: None,
        session_id: None,
      })?;
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream))
      .await
      .map_err(protocol_error)?;
    tokio::spawn(async move {
      let _ = connection.await;
    });

    let body_bytes = match body {
      Some(value) => serde_json::to_vec(&value).map_err(|error| WaylandError {
        message: format!("Could not encode AgentOS request: {error}"),
        status: None,
        session_id: None,
      })?,
      None => Vec::new(),
    };
    let mut builder = Request::builder()
      .method(method)
      .uri(format!("http://agentos{path}"))
      .header(HOST, "agentos")
      .header("connection", "close");
    if !body_bytes.is_empty() {
      builder = builder.header(CONTENT_TYPE, "application/json");
    }
    let request = builder
      .body(Full::new(Bytes::from(body_bytes)))
      .map_err(protocol_error)?;
    let response = sender.send_request(request).await.map_err(protocol_error)?;
    decode_response(response).await
  }
}

async fn decode_response(response: hyper::Response<Incoming>) -> Result<Value, WaylandError> {
  let status = response.status();
  let bytes = response
    .into_body()
    .collect()
    .await
    .map_err(protocol_error)?
    .to_bytes();
  let value: Value = serde_json::from_slice(&bytes).map_err(|error| WaylandError {
    message: format!("AgentOS returned invalid JSON: {error}"),
    status: Some(status),
    session_id: None,
  })?;
  if !status.is_success() {
    let detail = &value["error"];
    return Err(WaylandError {
      message: detail["message"]
        .as_str()
        .or_else(|| value["error"].as_str())
        .unwrap_or("AgentOS rejected the Wayland session request.")
        .to_owned(),
      status: Some(status),
      session_id: value["sessionId"].as_str().map(str::to_owned),
    });
  }
  Ok(value)
}

fn protocol_error(error: impl std::fmt::Display) -> WaylandError {
  WaylandError {
    message: format!("AgentOS Wayland API request failed: {error}"),
    status: None,
    session_id: None,
  }
}
