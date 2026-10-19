use std::{env, time::Duration};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{client::conn::http1, Method, Request};
use hyper_util::rt::TokioIo;
use serde_json::json;
use tokio::net::UnixStream;

const ICON: &[u8] = include_bytes!("tray-icon.png");

/// The Shell supplies localized Open and Exit actions. Keep registration in
/// the backend so it does not depend on an open CEF page or a ready runtime.
pub async fn run() {
  let socket = match env::var("AGENTOS_TRAY_SOCKET") {
    Ok(value) if !value.is_empty() => value,
    _ => return,
  };
  let payload = serde_json::to_vec(&json!({
    "tooltip": "AndroidTone · 安卓通",
    "iconPngBase64": encode_base64(ICON),
    "menu": []
  }))
  .expect("tray payload is serializable");
  loop {
    // Re-register after each poll, including after a broker reconnect. Consume
    // click events even though the default menu actions are owned by Shell.
    let result = async {
      request(&socket, Method::PUT, "/v1/icons/main", payload.clone()).await?;
      request(&socket, Method::GET, "/v1/events?wait_ms=25000", Vec::new()).await
    }
    .await;
    if let Err(error) = result {
      eprintln!("AndroidTone tray IPC unavailable: {error}");
      tokio::time::sleep(Duration::from_secs(2)).await;
    }
  }
}

async fn request(
  socket: &str,
  method: Method,
  path: &str,
  body: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  tokio::time::timeout(Duration::from_secs(30), async {
    let stream = UnixStream::connect(socket).await?;
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await?;
    tokio::spawn(async move {
      let _ = connection.await;
    });
    let request = Request::builder()
      .method(method)
      .uri(path)
      .header("Host", "agentos")
      .header("Content-Type", "application/json")
      .body(Full::new(Bytes::from(body)))?;
    let response = sender.send_request(request).await?;
    let status = response.status();
    Limited::new(response.into_body(), 64 * 1024)
      .collect()
      .await?;
    if !status.is_success() {
      return Err(format!("AgentOS tray returned HTTP {status}").into());
    }
    Ok(())
  })
  .await?
}

fn encode_base64(bytes: &[u8]) -> String {
  const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
  for chunk in bytes.chunks(3) {
    let first = chunk[0];
    let second = chunk.get(1).copied().unwrap_or(0);
    let third = chunk.get(2).copied().unwrap_or(0);
    encoded.push(TABLE[(first >> 2) as usize] as char);
    encoded.push(TABLE[(((first & 3) << 4) | (second >> 4)) as usize] as char);
    encoded.push(if chunk.len() > 1 {
      TABLE[(((second & 15) << 2) | (third >> 6)) as usize] as char
    } else {
      '='
    });
    encoded.push(if chunk.len() > 2 {
      TABLE[(third & 63) as usize] as char
    } else {
      '='
    });
  }
  encoded
}
