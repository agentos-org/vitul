use std::{env, time::Duration};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{client::conn::http1, Method, Request};
use hyper_util::rt::TokioIo;
use serde_json::json;
use tokio::net::UnixStream;

const ICON: &[u8] = include_bytes!("../../icon.png");

/// Register the app with AgentOS Shell independently of the frontend window.
/// The Shell provides localized Open and Exit actions for process apps.
pub async fn run() {
  let socket = match env::var("AGENTOS_TRAY_SOCKET") {
    Ok(value) if !value.is_empty() => value,
    _ => {
      eprintln!("AgentOS tray socket is not configured; tray integration is disabled");
      return;
    }
  };
  let payload = serde_json::to_vec(&json!({
    "tooltip": "温易通 · Windows 应用管理",
    "iconPngBase64": encode_base64(ICON),
    "menu": []
  }))
  .expect("tray payload is serializable");

  loop {
    let result = async {
      request(&socket, Method::PUT, "/v1/icons/main", payload.clone()).await?;
      // Consume Shell events so they do not build up while the window is closed.
      request(&socket, Method::GET, "/v1/events?wait_ms=25000", Vec::new()).await
    }
    .await;
    if let Err(error) = result {
      eprintln!("WinTone tray IPC unavailable: {error}");
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn embeds_a_png_icon_for_agentos_shell() {
    assert_eq!(&ICON[..8], b"\x89PNG\r\n\x1a\n");
    let payload: serde_json::Value = serde_json::from_slice(
      &serde_json::to_vec(&json!({
        "tooltip": "温易通 · Windows 应用管理",
        "iconPngBase64": encode_base64(ICON),
        "menu": []
      }))
      .unwrap(),
    )
    .unwrap();
    assert!(payload["iconPngBase64"]
      .as_str()
      .unwrap()
      .starts_with("iVBOR"));
  }

  #[test]
  fn base64_encoder_matches_rfc_4648_vectors() {
    assert_eq!(encode_base64(b""), "");
    assert_eq!(encode_base64(b"f"), "Zg==");
    assert_eq!(encode_base64(b"fo"), "Zm8=");
    assert_eq!(encode_base64(b"foo"), "Zm9v");
  }
}
