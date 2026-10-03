use std::{env, time::Duration};

use serde_json::{json, Value};
use tokio::{sync::mpsc, time};

use crate::{audio, start_track, AppState, Track};

const TRAY_ICON_PATH: &str = "/v1/icons/main";
const ICON_UPDATE_INTERVAL: Duration = Duration::from_secs(2);
const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub async fn run(state: AppState) {
  let socket_path = match env::var("AGENTOS_TRAY_SOCKET") {
    Ok(path) if !path.is_empty() => path,
    _ => {
      eprintln!("AgentOS tray socket is not configured; tray integration is disabled");
      return;
    }
  };

  let (event_sender, mut events) = mpsc::channel(16);
  tokio::spawn(poll_events(socket_path.clone(), event_sender));
  let icon_png_base64 = load_icon_png().await;

  let mut refresh = time::interval(ICON_UPDATE_INTERVAL);
  refresh.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
  let mut last_icon: Option<Value> = None;
  loop {
    tokio::select! {
      _ = refresh.tick() => {
        let icon = tray_icon(&state, icon_png_base64.as_deref()).await;
        if last_icon.as_ref() == Some(&icon) {
          continue;
        }
        let body = match serde_json::to_vec(&icon) {
          Ok(body) => body,
          Err(error) => {
            eprintln!("Could not serialize Toney Music tray state: {error}");
            continue;
          }
        };
        match audio::request_unix_http(
          &socket_path,
          "PUT",
          TRAY_ICON_PATH,
          &body,
          "application/json",
        ).await {
          Ok(_) => last_icon = Some(icon),
          Err(error) => eprintln!("Could not update Toney Music tray icon: {error}"),
        }
      }
      event = events.recv() => {
        match event {
          Some(event) => handle_event(&state, event).await,
          None => break,
        }
      }
    }
  }
}

async fn poll_events(socket_path: String, sender: mpsc::Sender<Value>) {
  loop {
    match audio::request_unix_http(
      &socket_path,
      "GET",
      "/v1/events?wait_ms=25000",
      &[],
      "application/json",
    )
    .await
    {
      Ok(response) => {
        if let Some(event) = response.get("event").filter(|event| event.is_object()) {
          if sender.send(event.clone()).await.is_err() {
            return;
          }
        }
      }
      Err(error) => {
        eprintln!("Toney Music tray event poll failed: {error}");
        time::sleep(Duration::from_secs(2)).await;
      }
    }
  }
}

async fn handle_event(state: &AppState, event: Value) {
  match event.get("type").and_then(Value::as_str) {
    // AgentOS Shell sends this when the tray popup opens; playback controls
    // are handled by explicit menu actions below.
    Some("click") => {}
    Some("menu") => match event.get("itemId").and_then(Value::as_str) {
      Some("toggle-playback") => {
        if let Err(error) = toggle_playback(state).await {
          eprintln!("Toney Music tray playback action failed: {error}");
        }
      }
      Some("stop-playback") => state.audio.stop().await,
      _ => {}
    },
    _ => {}
  }
}

async fn toggle_playback(state: &AppState) -> Result<(), String> {
  let snapshot = state.audio.snapshot().await;
  match snapshot.get("state").and_then(Value::as_str) {
    Some("playing") => state.audio.pause().await.map(|_| ()),
    Some("paused") => state.audio.resume().await.map(|_| ()),
    _ => {
      let current_id = snapshot.get("trackId").and_then(Value::as_str);
      let tracks = state.tracks.read().await;
      let track = current_id
        .and_then(|id| tracks.iter().find(|track| track.id == id))
        .cloned()
        .or_else(|| tracks.iter().min_by_key(|track| track.added_at).cloned())
        .ok_or_else(|| "the music library is empty".to_string())?;
      drop(tracks);

      let require_bit_perfect = *state.bit_perfect_enabled.read().await;
      let mode = if require_bit_perfect {
        "exclusive"
      } else {
        snapshot
          .get("mode")
          .and_then(Value::as_str)
          .filter(|mode| matches!(*mode, "exclusive" | "shared"))
          .unwrap_or("exclusive")
      };
      let device = snapshot
        .get("device")
        .and_then(Value::as_str)
        .filter(|device| !device.is_empty())
        .unwrap_or("default")
        .to_string();
      let position = if snapshot.get("state").and_then(Value::as_str) == Some("error") {
        snapshot
          .get("currentTime")
          .and_then(Value::as_f64)
          .unwrap_or(0.0)
      } else {
        0.0
      };
      start_track(state, track, position, mode, device, true)
        .await
        .map(|_| ())
    }
  }
}

async fn tray_icon(state: &AppState, icon_png_base64: Option<&str>) -> Value {
  let snapshot = state.audio.snapshot().await;
  let playback_state = snapshot
    .get("state")
    .and_then(Value::as_str)
    .unwrap_or("idle");
  let track_id = snapshot.get("trackId").and_then(Value::as_str);
  let track = if let Some(id) = track_id {
    state
      .tracks
      .read()
      .await
      .iter()
      .find(|track| track.id == id)
      .cloned()
  } else {
    None
  };
  let track_name = track.as_ref().map(track_label);
  let (status, status_zh) = match playback_state {
    "playing" => ("Playing", "正在播放"),
    "paused" => ("Paused", "已暂停"),
    "ended" => ("Finished", "已完成"),
    "error" => ("Playback error", "播放出错"),
    _ => ("Stopped", "已停止"),
  };
  let detail = track_name.as_deref().unwrap_or("No track selected");
  let detail_zh = track_name.as_deref().unwrap_or("未选择歌曲");
  let tooltip = shorten_utf16(&format!("Toney Music · {status}: {detail}"), 128);
  let status_label = shorten_utf16(&format!("{status}: {detail}"), 128);
  let status_label_zh = shorten_utf16(&format!("{status_zh}：{detail_zh}"), 128);
  let (toggle_label, toggle_label_zh) = if playback_state == "playing" {
    ("Pause", "暂停")
  } else {
    ("Play", "播放")
  };
  let has_tracks = !state.tracks.read().await.is_empty();
  let has_selected_track = track_id.is_some();

  let mut icon = json!({
    "tooltip": tooltip,
    "menu": [
      {
        "id": "current-track",
        "label": status_label,
        "localizedLabels": { "en-US": status_label, "zh-CN": status_label_zh },
        "enabled": false,
      },
      { "type": "separator" },
      {
        "id": "toggle-playback",
        "label": toggle_label,
        "localizedLabels": { "en-US": toggle_label, "zh-CN": toggle_label_zh },
        "enabled": has_tracks,
      },
      {
        "id": "stop-playback",
        "label": "Stop",
        "localizedLabels": { "en-US": "Stop", "zh-CN": "停止" },
        "enabled": has_selected_track,
      },
    ],
  });
  if let Some(image) = icon_png_base64 {
    icon["iconPngBase64"] = Value::String(image.to_string());
  }
  icon
}

async fn load_icon_png() -> Option<String> {
  let executable = match env::current_exe() {
    Ok(path) => path,
    Err(error) => {
      eprintln!("Could not locate Toney Music tray icon: {error}");
      return None;
    }
  };
  let icon_path = executable.parent()?.join("icon.png");
  match tokio::fs::read(&icon_path).await {
    Ok(bytes) => Some(encode_base64(&bytes)),
    Err(error) => {
      eprintln!(
        "Could not read Toney Music tray icon at {}: {error}",
        icon_path.display()
      );
      None
    }
  }
}

fn encode_base64(bytes: &[u8]) -> String {
  let mut encoded = Vec::with_capacity(bytes.len().div_ceil(3) * 4);
  for chunk in bytes.chunks(3) {
    let first = chunk[0];
    let second = chunk.get(1).copied().unwrap_or(0);
    let third = chunk.get(2).copied().unwrap_or(0);
    encoded.push(BASE64_TABLE[(first >> 2) as usize]);
    encoded.push(BASE64_TABLE[(((first & 0b11) << 4) | (second >> 4)) as usize]);
    if chunk.len() > 1 {
      encoded.push(BASE64_TABLE[(((second & 0b1111) << 2) | (third >> 6)) as usize]);
    } else {
      encoded.push(b'=');
    }
    if chunk.len() > 2 {
      encoded.push(BASE64_TABLE[(third & 0b0011_1111) as usize]);
    } else {
      encoded.push(b'=');
    }
  }
  String::from_utf8(encoded).expect("base64 output is valid UTF-8")
}

fn track_label(track: &Track) -> String {
  if track.artist.is_empty() {
    track.title.clone()
  } else {
    format!("{} — {}", track.title, track.artist)
  }
}

fn shorten_utf16(value: &str, max_units: usize) -> String {
  if max_units == 0 {
    return String::new();
  }
  let mut result = String::new();
  let mut used_units = 0;
  let mut truncated = false;
  for character in value.chars() {
    let units = character.len_utf16();
    if used_units + units > max_units.saturating_sub(1) {
      truncated = true;
      break;
    }
    result.push(character);
    used_units += units;
  }
  if truncated {
    result.push('…');
  }
  result
}
