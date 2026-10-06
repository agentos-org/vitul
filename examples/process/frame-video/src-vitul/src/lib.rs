#[tauri::command]
fn opentool_endpoint() -> Result<String, String> {
  std::env::var("VITUL_OPENTOOL_BASE")
    .map_err(|_| "OpenTool backend was not started by `vitul dev`".to_owned())
}

#[tauri::command]
fn opentool_transport() -> &'static str {
  #[cfg(mobile)]
  {
    "ipc"
  }
  #[cfg(not(mobile))]
  {
    "http"
  }
}

#[cfg(mobile)]
struct OpenToolIpcState {
  server: tokio::sync::OnceCell<std::sync::Arc<frame_video_backend::VitulOpenToolServer>>,
}

#[cfg(mobile)]
impl Default for OpenToolIpcState {
  fn default() -> Self {
    Self {
      server: tokio::sync::OnceCell::new(),
    }
  }
}

#[cfg(mobile)]
async fn open_tool_server(
  state: &OpenToolIpcState,
) -> Result<std::sync::Arc<frame_video_backend::VitulOpenToolServer>, String> {
  let server = state
    .server
    .get_or_try_init(|| async {
      let tool = frame_video_backend::FrameVideoTool::load()
        .map_err(|error| error.to_string())?;
      frame_video_backend::create_server(tool, std::iter::empty::<String>(), "127.0.0.1".to_owned(), 0)
        .await
        .map(std::sync::Arc::new)
        .map_err(|error| error.to_string())
    })
    .await?;
  Ok(std::sync::Arc::clone(server))
}

#[cfg(not(mobile))]
#[tauri::command]
async fn pick_video_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
  use tauri_plugin_dialog::DialogExt;

  let paths = app
    .dialog()
    .file()
    .add_filter("Video files", &["mp4", "webm", "mov", "mkv", "avi", "m4v", "ogv"])
    .blocking_pick_files()
    .unwrap_or_default();
  paths
    .into_iter()
    .map(|path| {
      path
        .into_path()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| error.to_string())
    })
    .collect()
}

#[cfg(mobile)]
#[tauri::command(rename_all = "camelCase")]
async fn opentool_ipc(
  operation: String,
  request: Option<serde_json::Value>,
  on_event: Option<tauri::ipc::Channel<serde_json::Value>>,
  state: tauri::State<'_, OpenToolIpcState>,
) -> Result<serde_json::Value, String> {
  use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request},
  };
  use tower::ServiceExt;

  const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

  let (method, path, is_stream) = match operation.as_str() {
    "version" => (Method::GET, "/opentool/version", false),
    "load" => (Method::GET, "/opentool/load", false),
    "call" => (Method::POST, "/opentool/call", false),
    "streamCall" => (Method::POST, "/opentool/streamCall", true),
    _ => return Err(format!("Unsupported OpenTool operation: {operation}")),
  };

  if matches!(operation.as_str(), "call" | "streamCall") && request.is_none() {
    return Err("OpenTool call requires a JSON-RPC request".to_owned());
  }
  if is_stream && on_event.is_none() {
    return Err("OpenTool streamCall requires an IPC event channel".to_owned());
  }

  let body = match request {
    Some(request) => serde_json::to_vec(&request).map_err(|error| error.to_string())?,
    None => Vec::new(),
  };
  let server = open_tool_server(&state).await?;
  let request = Request::builder()
    .method(method)
    .uri(path)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(body))
    .map_err(|error| error.to_string())?;
  // Reuse the OpenTool HTTP router as an in-process Tower service. No socket
  // is bound and no HTTP request is sent over the Tauri IPC transport.
  let response = server
    .router()
    .oneshot(request)
    .await
    .map_err(|never: std::convert::Infallible| match never {})?;

  if !response.status().is_success() {
    return Err(format!(
      "OpenTool request failed with status {}",
      response.status()
    ));
  }

  if is_stream {
    let channel = on_event.expect("stream channel validated above");
    forward_open_tool_sse(response.into_body(), channel).await?;
    return Ok(serde_json::Value::Null);
  }

  let bytes = to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
    .await
    .map_err(|error| format!("failed to read OpenTool response: {error}"))?;
  serde_json::from_slice(&bytes).map_err(|error| format!("OpenTool returned invalid JSON: {error}"))
}

#[cfg(mobile)]
async fn forward_open_tool_sse(
  body: axum::body::Body,
  channel: tauri::ipc::Channel<serde_json::Value>,
) -> Result<(), String> {
  use futures_util::StreamExt;

  let mut stream = body.into_data_stream();
  let mut pending = Vec::new();
  let mut saw_done = false;
  while let Some(chunk) = stream.next().await {
    let chunk = chunk.map_err(|error| format!("OpenTool stream failed: {error}"))?;
    pending.extend_from_slice(&chunk);
    while let Some((end, delimiter_len)) = find_sse_frame_end(&pending) {
      let frame = pending.drain(..end).collect::<Vec<_>>();
      pending.drain(..delimiter_len);
      saw_done |= send_open_tool_sse_frame(&frame, &channel)?.as_deref() == Some("DONE");
    }
  }
  if !pending.is_empty() {
    saw_done |= send_open_tool_sse_frame(&pending, &channel)?.as_deref() == Some("DONE");
  }
  if !saw_done {
    channel
      .send(serde_json::json!({ "event": "DONE", "data": {} }))
      .map_err(|error| format!("failed to forward OpenTool stream completion: {error}"))?;
  }
  Ok(())
}

#[cfg(mobile)]
fn find_sse_frame_end(buffer: &[u8]) -> Option<(usize, usize)> {
  let lf = buffer.windows(2).position(|window| window == b"\n\n");
  let crlf = buffer
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .map(|position| (position, 4));
  match (lf, crlf) {
    (Some(lf), Some((crlf, crlf_len))) if crlf < lf => Some((crlf, crlf_len)),
    (Some(lf), _) => Some((lf, 2)),
    (None, value) => value,
  }
}

#[cfg(mobile)]
fn send_open_tool_sse_frame(
  frame: &[u8],
  channel: &tauri::ipc::Channel<serde_json::Value>,
) -> Result<Option<String>, String> {
  let text = std::str::from_utf8(frame).map_err(|error| error.to_string())?;
  let mut event = "message";
  let mut data = Vec::new();
  for line in text.lines() {
    let line = line.strip_suffix('\r').unwrap_or(line);
    if let Some(value) = line.strip_prefix("event:") {
      event = value.trim();
    } else if let Some(value) = line.strip_prefix("data:") {
      data.push(value.strip_prefix(' ').unwrap_or(value));
    }
  }
  if data.is_empty() {
    return Ok(None);
  }
  let data = serde_json::from_str::<serde_json::Value>(&data.join("\n"))
    .map_err(|error| format!("OpenTool stream returned invalid JSON: {error}"))?;
  channel
    .send(serde_json::json!({ "event": event, "data": data }))
    .map_err(|error| format!("failed to forward OpenTool stream event: {error}"))?;
  Ok(Some(event.to_owned()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let builder = tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init());

  #[cfg(mobile)]
  let builder = builder.manage(OpenToolIpcState::default());

  #[cfg(mobile)]
  let builder = builder.invoke_handler(tauri::generate_handler![
    opentool_endpoint,
    opentool_transport,
    opentool_ipc
  ]);

  #[cfg(not(mobile))]
  let builder = builder.invoke_handler(tauri::generate_handler![
    opentool_endpoint,
    opentool_transport,
    pick_video_files
  ]);

  let app = builder
    .build(tauri::generate_context!())
    .expect("error while building Vitul application");
  app.run(|app_handle, event| {
    #[cfg(mobile)]
    if matches!(event, tauri::RunEvent::Exit) {
      use tauri::Manager;

      if let Some(state) = app_handle.try_state::<OpenToolIpcState>() {
        if let Some(server) = state.server.get() {
          let _ = tauri::async_runtime::block_on(server.stop());
        }
      }
    }
    #[cfg(not(mobile))]
    let _ = (app_handle, event);
  });
}
