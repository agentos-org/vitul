use std::{
  fs::File,
  io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
  path::{Path, PathBuf},
  process::Command,
  time::Instant,
};

use ape_decoder::ApeDecoder;
use claxon::FlacReader;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use symphonia::{
  core::{
    audio::{Audio, AudioBytes, Channels, GenericAudioBufferRef, Position},
    codecs::audio::AudioDecoderOptions,
    errors::Error as SymphoniaError,
    formats::{probe::Hint, FormatOptions, TrackType},
    io::MediaSourceStream,
    meta::MetadataOptions,
  },
  default::{get_codecs, get_probe},
};
use symphonia_common::apple::audio::alac::MagicCookie;
use tokio::{
  fs as async_fs,
  io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
  net::UnixStream,
  sync::Mutex,
  task::JoinHandle,
};
use wavpack::WavpackReader;

const PCM_CHUNK_SECONDS: usize = 1;
const MAX_PCM_CHUNK_BYTES: usize = 1 << 20;
const PCM_CACHE_VERSION: u32 = 3;

#[derive(Clone)]
pub struct AudioManager {
  inner: std::sync::Arc<Mutex<Runtime>>,
}

struct Runtime {
  generation: u64,
  track_id: Option<String>,
  state: String,
  position: f64,
  duration: f64,
  started_at: Option<Instant>,
  session_id: Option<String>,
  task: Option<JoinHandle<()>>,
  mode: String,
  device: String,
  info: Option<PcmInfo>,
  error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PcmInfo {
  sample_rate: u32,
  channels: u16,
  bits_per_sample: u16,
  sample_format: String,
  broker_format: String,
  channel_map: String,
  bytes_per_sample: u16,
  frame_bytes: u32,
  duration: f64,
  decoder: String,
  #[serde(default)]
  cache_version: u32,
  #[serde(default)]
  decode_warning: Option<String>,
  #[serde(default)]
  decoder_preference: String,
  source_lossless: bool,
  exact_decode: bool,
  source_size: u64,
  source_modified: u64,
}

struct DecodedTrack {
  path: PathBuf,
  info: PcmInfo,
}

impl AudioManager {
  pub fn new(_cache_dir: PathBuf) -> Self {
    Self {
      inner: std::sync::Arc::new(Mutex::new(Runtime {
        generation: 0,
        track_id: None,
        state: "idle".to_string(),
        position: 0.0,
        duration: 0.0,
        started_at: None,
        session_id: None,
        task: None,
        mode: "exclusive".to_string(),
        device: String::new(),
        info: None,
        error: None,
      })),
    }
  }

  pub async fn current_track_id(&self) -> Option<String> {
    self.inner.lock().await.track_id.clone()
  }

  pub async fn snapshot(&self) -> Value {
    let runtime = self.inner.lock().await;
    let position = current_position(&runtime);
    let info = runtime.info.as_ref();
    let (bit_perfect_output, reason) = if runtime.mode == "shared" {
      (
        Value::Bool(false),
        "Shared PipeWire output may mix or convert the stream.",
      )
    } else {
      (
        Value::Null,
        "Exclusive ALSA output is unverified beyond the host hardware interface.",
      )
    };
    json!({
        "state": runtime.state,
        "trackId": runtime.track_id,
        "currentTime": position,
        "duration": runtime.duration,
        "mode": runtime.mode,
        "device": runtime.device,
        "sampleRate": info.map(|value| value.sample_rate),
        "channels": info.map(|value| value.channels),
        "bitsPerSample": info.map(|value| value.bits_per_sample),
        "sampleFormat": info.map(|value| value.sample_format.as_str()),
        "decoder": info.map(|value| value.decoder.as_str()),
        "decodeWarning": info.and_then(|value| value.decode_warning.as_deref()),
        "sourceLossless": info.map(|value| value.source_lossless),
        "exactDecode": info.map(|value| value.exact_decode),
        "bitPerfectDecode": info.map(|value| value.source_lossless && value.exact_decode),
        "bitPerfectOutput": bit_perfect_output,
        "bitPerfectReason": reason,
        "error": runtime.error,
    })
  }

  pub async fn capabilities(&self) -> Value {
    match broker_request("GET", "/v1/devices", &[], "application/json").await {
      Ok(devices) => {
        let formats = broker_request("GET", "/v1/formats", &[], "application/json")
          .await
          .unwrap_or_else(|_| json!({}));
        json!({ "available": true, "devices": devices.get("devices").cloned().unwrap_or(devices), "formats": formats })
      }
      Err(error) => json!({ "available": false, "devices": [], "error": error }),
    }
  }

  pub async fn check_bit_perfect_output(
    &self,
    cache_identity: String,
    source: PathBuf,
    cache_dir: PathBuf,
    ffmpeg: String,
    extension: String,
    decoder_preference: String,
    device: String,
  ) -> Result<Value, String> {
    let id = cache_identity;
    let ext = extension.clone();
    let preference = decoder_preference.clone();
    let decoded = tokio::task::spawn_blocking(move || {
      decode_track(&source, &cache_dir, &ffmpeg, &id, &ext, &preference)
    })
    .await
    .map_err(|error| format!("decoder task failed: {error}"))??;

    if !decoded.info.source_lossless {
      return Err("Bit-perfect mode requires a lossless source.".to_string());
    }
    if !decoded.info.exact_decode {
      let detail = decoded
        .info
        .decode_warning
        .as_deref()
        .unwrap_or("The decoder did not preserve the source PCM samples exactly.");
      return Err(format!(
        "Bit-perfect check failed: {} is not an exact decode. {detail}",
        decoded.info.decoder
      ));
    }

    let target_device = if device.is_empty() || device == "default" {
      first_exclusive_device().await?
    } else {
      device
    };
    let is_hw_device = target_device
      .strip_prefix("hw:")
      .and_then(|parts| parts.split_once(','))
      .is_some_and(|(card, device)| {
        !card.is_empty()
          && !device.is_empty()
          && card.chars().all(|value| value.is_ascii_digit())
          && device.chars().all(|value| value.is_ascii_digit())
      });
    if !is_hw_device {
      return Err("Bit-perfect mode requires an exclusive ALSA hardware device.".to_string());
    }

    let (active, active_compatible) = {
      let runtime = self.inner.lock().await;
      let active =
        matches!(runtime.state.as_str(), "playing" | "paused") && runtime.session_id.is_some();
      let active_compatible = active
        && runtime.mode == "exclusive"
        && runtime.device == target_device
        && runtime.info.as_ref().is_some_and(|info| {
          info.source_lossless
            && info.exact_decode
            && info.sample_rate == decoded.info.sample_rate
            && info.channels == decoded.info.channels
            && info.bits_per_sample == decoded.info.bits_per_sample
            && info.broker_format == decoded.info.broker_format
            && info.channel_map == decoded.info.channel_map
        });
      (active, active_compatible)
    };
    if active && !active_compatible {
      return Err(
        "Stop the current playback before checking a different exclusive format or device."
          .to_string(),
      );
    }

    if !active_compatible {
      let create_body = broker_session_body(&decoded.info, "exclusive", &target_device)?;
      let response =
        broker_request("POST", "/v1/sessions", &create_body, "application/json").await?;
      let session_id = response
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| "AgentOS audio broker did not return sessionId".to_string())?;
      broker_request(
        "DELETE",
        &format!("/v1/sessions/{session_id}"),
        &[],
        "application/json",
      )
      .await?;
    }

    Ok(json!({
      "device": target_device,
      "sampleRate": decoded.info.sample_rate,
      "channels": decoded.info.channels,
      "bitsPerSample": decoded.info.bits_per_sample,
      "sampleFormat": decoded.info.sample_format,
      "decoder": decoded.info.decoder,
      "sourceLossless": decoded.info.source_lossless,
      "exactDecode": decoded.info.exact_decode,
      "deviceAcceptedFormat": true,
    }))
  }

  pub async fn play(
    &self,
    track_id: String,
    cache_identity: String,
    source: PathBuf,
    cache_dir: PathBuf,
    ffmpeg: String,
    extension: String,
    decoder_preference: String,
    cue_start_frame: Option<u64>,
    cue_end_frame: Option<u64>,
    position: f64,
    mode: &str,
    device: String,
    autoplay: bool,
    require_bit_perfect: bool,
  ) -> Result<Value, String> {
    if !matches!(mode, "exclusive" | "shared") {
      return Err("audio output mode must be shared or exclusive".to_string());
    }
    let id = cache_identity;
    let ext = extension.clone();
    let preference = decoder_preference.clone();
    let decoded = tokio::task::spawn_blocking(move || {
      decode_track(&source, &cache_dir, &ffmpeg, &id, &ext, &preference)
    })
    .await
    .map_err(|error| format!("decoder task failed: {error}"))??;
    let source_frame_count = std::fs::metadata(&decoded.path)
      .map_err(|error| format!("read decoded PCM size: {error}"))?
      .len()
      / decoded.info.frame_bytes as u64;
    let cd_frame_to_sample = |frame: u64| {
      ((frame as u128 * decoded.info.sample_rate as u128) / 75).min(u64::MAX as u128) as u64
    };
    let segment_start_frame = cue_start_frame.map(cd_frame_to_sample).unwrap_or(0);
    let segment_end_frame = cue_end_frame
      .map(cd_frame_to_sample)
      .unwrap_or(source_frame_count)
      .min(source_frame_count);
    if segment_start_frame >= source_frame_count || segment_end_frame <= segment_start_frame {
      return Err("CUE track boundaries fall outside the decoded audio source.".to_string());
    }
    let mut playback_info = decoded.info.clone();
    playback_info.duration =
      (segment_end_frame - segment_start_frame) as f64 / playback_info.sample_rate as f64;
    if require_bit_perfect {
      if mode != "exclusive" {
        return Err("Bit-perfect mode requires exclusive ALSA output.".to_string());
      }
      if !decoded.info.source_lossless || !decoded.info.exact_decode {
        let detail = decoded
          .info
          .decode_warning
          .as_deref()
          .unwrap_or("The decoder did not preserve the source PCM samples exactly.");
        return Err(format!(
          "Bit-perfect playback is unavailable: {} is not an exact lossless decode. {detail}",
          decoded.info.decoder
        ));
      }
    }
    self.stop().await;
    let mut target_device = device;
    if mode == "exclusive" && target_device == "default" {
      target_device = first_exclusive_device().await?;
    }
    if mode == "shared" {
      target_device = "default".to_string();
    }
    let create_body = broker_session_body(&playback_info, mode, &target_device)?;
    let response = broker_request("POST", "/v1/sessions", &create_body, "application/json").await?;
    let session_id = response
      .get("sessionId")
      .and_then(Value::as_str)
      .ok_or_else(|| "AgentOS audio broker did not return sessionId".to_string())?
      .to_string();
    if !autoplay {
      let _ = broker_request(
        "POST",
        &format!("/v1/sessions/{session_id}/pause"),
        &[],
        "application/json",
      )
      .await;
    }

    {
      let mut runtime = self.inner.lock().await;
      runtime.generation = runtime.generation.wrapping_add(1);
      runtime.track_id = Some(track_id);
      runtime.state = if autoplay { "playing" } else { "paused" }.to_string();
      runtime.position = position.max(0.0).min(playback_info.duration);
      runtime.duration = playback_info.duration;
      runtime.started_at = if autoplay { Some(Instant::now()) } else { None };
      runtime.session_id = Some(session_id.clone());
      runtime.mode = mode.to_string();
      runtime.device = target_device;
      runtime.info = Some(decoded.info.clone());
      runtime.error = None;
      let generation = runtime.generation;
      let manager = self.clone();
      let task = tokio::spawn(async move {
        manager
          .stream_track(
            generation,
            session_id,
            decoded.path,
            playback_info,
            segment_start_frame,
            segment_end_frame,
            position,
          )
          .await;
      });
      runtime.task = Some(task);
    }
    Ok(self.snapshot().await)
  }

  pub async fn pause(&self) -> Result<Value, String> {
    let (session_id, position) = {
      let runtime = self.inner.lock().await;
      let session_id = runtime
        .session_id
        .clone()
        .ok_or_else(|| "no active audio session".to_string())?;
      (session_id, current_position(&runtime))
    };
    let _ = broker_request(
      "POST",
      &format!("/v1/sessions/{session_id}/pause"),
      &[],
      "application/json",
    )
    .await?;
    let mut runtime = self.inner.lock().await;
    runtime.position = position;
    runtime.started_at = None;
    runtime.state = "paused".to_string();
    drop(runtime);
    Ok(self.snapshot().await)
  }

  pub async fn resume(&self) -> Result<Value, String> {
    let session_id = self
      .inner
      .lock()
      .await
      .session_id
      .clone()
      .ok_or_else(|| "no active audio session".to_string())?;
    let _ = broker_request(
      "POST",
      &format!("/v1/sessions/{session_id}/resume"),
      &[],
      "application/json",
    )
    .await?;
    let mut runtime = self.inner.lock().await;
    runtime.started_at = Some(Instant::now());
    runtime.state = "playing".to_string();
    drop(runtime);
    Ok(self.snapshot().await)
  }

  pub async fn stop(&self) {
    let (session_id, task) = {
      let mut runtime = self.inner.lock().await;
      runtime.generation = runtime.generation.wrapping_add(1);
      runtime.started_at = None;
      runtime.state = "idle".to_string();
      runtime.track_id = None;
      runtime.position = 0.0;
      runtime.duration = 0.0;
      runtime.info = None;
      runtime.error = None;
      (runtime.session_id.take(), runtime.task.take())
    };
    if let Some(task) = task {
      task.abort();
    }
    if let Some(session_id) = session_id {
      let _ = broker_request(
        "DELETE",
        &format!("/v1/sessions/{session_id}"),
        &[],
        "application/json",
      )
      .await;
    }
  }

  async fn stream_track(
    &self,
    generation: u64,
    session_id: String,
    path: PathBuf,
    info: PcmInfo,
    segment_start_frame: u64,
    segment_end_frame: u64,
    requested_position: f64,
  ) {
    let mut file = match async_fs::File::open(&path).await {
      Ok(file) => file,
      Err(error) => {
        self.finish_with_error(generation, error.to_string()).await;
        return;
      }
    };
    let relative_start_frame =
      (requested_position.max(0.0).min(info.duration) * info.sample_rate as f64).floor() as u64;
    let absolute_start_frame = segment_start_frame.saturating_add(relative_start_frame);
    if file
      .seek(SeekFrom::Start(
        absolute_start_frame.saturating_mul(info.frame_bytes as u64),
      ))
      .await
      .is_err()
    {
      self
        .finish_with_error(generation, "could not seek decoded PCM".to_string())
        .await;
      return;
    }
    let mut chunk_size =
      info.frame_bytes as usize * (info.sample_rate as usize).max(1) * PCM_CHUNK_SECONDS;
    chunk_size = chunk_size
      .min(MAX_PCM_CHUNK_BYTES)
      .max(info.frame_bytes as usize);
    chunk_size -= chunk_size % (info.frame_bytes as usize);
    let mut buffer = vec![0u8; chunk_size];
    let mut committed_position = relative_start_frame as f64 / info.sample_rate as f64;
    loop {
      if !self.generation_is_current(generation).await {
        return;
      }
      let current_source_frame = segment_start_frame
        .saturating_add((committed_position * info.sample_rate as f64).floor() as u64);
      let remaining_frames = segment_end_frame.saturating_sub(current_source_frame);
      if remaining_frames == 0 {
        break;
      }
      let bytes_to_read = (remaining_frames.saturating_mul(info.frame_bytes as u64))
        .min(buffer.len() as u64) as usize;
      let read = match file.read(&mut buffer[..bytes_to_read]).await {
        Ok(0) => break,
        Ok(read) => read,
        Err(error) => {
          self.finish_with_error(generation, error.to_string()).await;
          return;
        }
      };
      if read % info.frame_bytes as usize != 0 {
        self
          .finish_with_error(
            generation,
            "decoded PCM cache is not frame aligned".to_string(),
          )
          .await;
        return;
      }
      if let Err(error) = broker_request(
        "POST",
        &format!("/v1/sessions/{session_id}/pcm"),
        &buffer[..read],
        "application/octet-stream",
      )
      .await
      {
        self.finish_with_error(generation, error).await;
        return;
      }
      let frames = read / info.frame_bytes as usize;
      committed_position =
        (committed_position + frames as f64 / info.sample_rate as f64).min(info.duration);
      let mut runtime = self.inner.lock().await;
      if runtime.generation != generation {
        return;
      }
      if runtime.state == "playing" {
        runtime.position = committed_position;
        runtime.started_at = Some(Instant::now());
      }
    }
    let _ = broker_request(
      "DELETE",
      &format!("/v1/sessions/{session_id}?drain=true"),
      &[],
      "application/json",
    )
    .await;
    let mut runtime = self.inner.lock().await;
    if runtime.generation == generation {
      runtime.state = "ended".to_string();
      runtime.position = info.duration;
      runtime.started_at = None;
      runtime.session_id = None;
      runtime.task = None;
    }
  }

  async fn generation_is_current(&self, generation: u64) -> bool {
    self.inner.lock().await.generation == generation
  }

  async fn finish_with_error(&self, generation: u64, error: String) {
    let session_id = {
      let mut runtime = self.inner.lock().await;
      if runtime.generation != generation {
        return;
      }
      runtime.state = "error".to_string();
      runtime.started_at = None;
      runtime.error = Some(error);
      runtime.session_id.take()
    };
    if let Some(session_id) = session_id {
      let _ = broker_request(
        "DELETE",
        &format!("/v1/sessions/{session_id}"),
        &[],
        "application/json",
      )
      .await;
    }
  }
}

fn current_position(runtime: &Runtime) -> f64 {
  let position = if runtime.state == "playing" {
    runtime.position
      + runtime
        .started_at
        .map(|started| started.elapsed().as_secs_f64())
        .unwrap_or(0.0)
  } else {
    runtime.position
  };
  position.min(runtime.duration).max(0.0)
}

fn broker_session_body(info: &PcmInfo, mode: &str, device: &str) -> Result<Vec<u8>, String> {
  serde_json::to_vec(&json!({
    "mode": mode,
    "format": &info.broker_format,
    "sampleRate": info.sample_rate,
    "channels": info.channels,
    "channelMap": &info.channel_map,
    "device": device,
  }))
  .map_err(|error| error.to_string())
}

async fn first_exclusive_device() -> Result<String, String> {
  let response = broker_request("GET", "/v1/devices", &[], "application/json").await?;
  response
    .get("devices")
    .and_then(Value::as_array)
    .and_then(|devices| {
      devices
        .iter()
        .find(|device| device.get("mode").and_then(Value::as_str) == Some("exclusive"))
    })
    .and_then(|device| device.get("id").and_then(Value::as_str))
    .map(str::to_string)
    .ok_or_else(|| "AgentOS did not report an exclusive ALSA playback device".to_string())
}

async fn broker_request(
  method: &str,
  path: &str,
  body: &[u8],
  content_type: &str,
) -> Result<Value, String> {
  let socket_path =
    std::env::var("AGENTOS_AUDIO_SOCKET").unwrap_or_else(|_| "/agentos-audio.sock".to_string());
  request_unix_http(&socket_path, method, path, body, content_type).await
}

pub(crate) async fn request_unix_http(
  socket_path: &str,
  method: &str,
  path: &str,
  body: &[u8],
  content_type: &str,
) -> Result<Value, String> {
  let mut stream = UnixStream::connect(socket_path)
    .await
    .map_err(|error| format!("AgentOS socket unavailable at {socket_path}: {error}"))?;
  let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
  stream
    .write_all(request.as_bytes())
    .await
    .map_err(|error| error.to_string())?;
  if !body.is_empty() {
    stream
      .write_all(body)
      .await
      .map_err(|error| error.to_string())?;
  }
  stream.shutdown().await.map_err(|error| error.to_string())?;
  let mut response = Vec::new();
  stream
    .read_to_end(&mut response)
    .await
    .map_err(|error| error.to_string())?;
  let header_end = response
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .ok_or_else(|| "invalid response from AgentOS broker".to_string())?;
  let header = String::from_utf8_lossy(&response[..header_end]);
  let status = header
    .lines()
    .next()
    .and_then(|line| line.split_whitespace().nth(1))
    .and_then(|value| value.parse::<u16>().ok())
    .unwrap_or(500);
  let response_body = &response[header_end + 4..];
  let chunked = header.lines().any(|line| {
    line
      .split_once(':')
      .map(|(name, value)| {
        name.eq_ignore_ascii_case("transfer-encoding")
          && value.to_ascii_lowercase().contains("chunked")
      })
      .unwrap_or(false)
  });
  let decoded_body = if chunked {
    decode_chunked(response_body)?
  } else {
    response_body.to_vec()
  };
  let payload: Value = if decoded_body.is_empty() {
    json!({})
  } else {
    serde_json::from_slice(&decoded_body).unwrap_or_else(|_| json!({}))
  };
  if status >= 400 {
    let message = payload
      .get("message")
      .and_then(Value::as_str)
      .or_else(|| payload.get("error").and_then(Value::as_str))
      .unwrap_or("AgentOS broker request failed");
    return Err(format!("AgentOS broker ({status}): {message}"));
  }
  Ok(payload)
}

fn decode_chunked(mut body: &[u8]) -> Result<Vec<u8>, String> {
  let mut decoded = Vec::new();
  loop {
    let line_end = body
      .windows(2)
      .position(|window| window == b"\r\n")
      .ok_or_else(|| "invalid chunked AgentOS broker response".to_string())?;
    let size_text = std::str::from_utf8(&body[..line_end]).map_err(|error| error.to_string())?;
    let size = usize::from_str_radix(size_text.split(';').next().unwrap_or_default().trim(), 16)
      .map_err(|error| format!("invalid chunk size: {error}"))?;
    body = &body[line_end + 2..];
    if size == 0 {
      break;
    }
    if body.len() < size + 2 || &body[size..size + 2] != b"\r\n" {
      return Err("truncated chunked AgentOS broker response".to_string());
    }
    decoded.extend_from_slice(&body[..size]);
    body = &body[size + 2..];
  }
  Ok(decoded)
}

fn decode_track(
  source: &Path,
  cache_dir: &Path,
  ffmpeg: &str,
  id: &str,
  extension: &str,
  decoder_preference: &str,
) -> Result<DecodedTrack, String> {
  let source_meta = std::fs::metadata(source).map_err(|error| format!("read source: {error}"))?;
  let source_size = source_meta.len();
  let source_modified = source_meta
    .modified()
    .ok()
    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|duration| duration.as_secs())
    .unwrap_or(0);
  let cache_path = cache_dir.join(format!("{id}.pcm"));
  let info_path = cache_dir.join(format!("{id}.json"));
  if let Ok(data) = std::fs::read(&info_path) {
    if let Ok(info) = serde_json::from_slice::<PcmInfo>(&data) {
      if info.source_size == source_size
        && info.source_modified == source_modified
        && info.decoder_preference == decoder_preference
        && info.cache_version == PCM_CACHE_VERSION
        && !(info.decoder.starts_with("FFmpeg fallback") && info.decode_warning.is_none())
        && cache_path.is_file()
      {
        return Ok(DecodedTrack {
          path: cache_path,
          info,
        });
      }
    }
  }
  let _ = std::fs::remove_file(&cache_path);
  let _ = std::fs::remove_file(&info_path);
  let tmp_path = cache_dir.join(format!("{id}.{}.tmp", uuid::Uuid::new_v4().simple()));
  let info = if decoder_preference == "ffmpeg" {
    if ffmpeg.is_empty() || !Path::new(ffmpeg).is_file() {
      return Err(
        "FFmpeg was selected for this format, but the bundled FFmpeg is unavailable".to_string(),
      );
    }
    decode_ffmpeg(ffmpeg, source, &tmp_path, extension)
      .map_err(|error| format!("forced FFmpeg decode failed ({error})"))?
  } else {
    let engine_result = match extension {
      "ape" => decode_ape(source, &tmp_path),
      "wv" => decode_wavpack(source, &tmp_path),
      "flac" => decode_flac(source, &tmp_path),
      "wav" | "wave" | "aiff" | "aif" | "mp3" | "aac" | "m4a" | "ogg" | "oga" | "alac" => {
        decode_symphonia(source, &tmp_path, extension)
      }
      _ => Err(format!("no native decoder for .{extension}")),
    };
    match engine_result {
      Ok(info) => info,
      Err(native_error) => {
        let _ = std::fs::remove_file(&tmp_path);
        if ffmpeg.is_empty() || !Path::new(ffmpeg).is_file() {
          return Err(format!(
            "native decode failed ({native_error}); FFmpeg fallback is unavailable"
          ));
        }
        let mut fallback =
          decode_ffmpeg(ffmpeg, source, &tmp_path, extension).map_err(|error| {
            format!("native decode failed ({native_error}); FFmpeg fallback failed ({error})")
          })?;
        fallback.decode_warning = Some(format!("Native decoder failed: {native_error}"));
        fallback
      }
    }
  };
  let mut info = info;
  info.cache_version = PCM_CACHE_VERSION;
  info.decoder_preference = decoder_preference.to_string();
  if decoder_preference == "ffmpeg" {
    info.decoder = "FFmpeg (forced, s32le)".to_string();
  }
  info.source_size = source_size;
  info.source_modified = source_modified;
  let tmp_size = std::fs::metadata(&tmp_path)
    .map_err(|error| error.to_string())?
    .len();
  if tmp_size == 0 || tmp_size % info.frame_bytes as u64 != 0 {
    let _ = std::fs::remove_file(&tmp_path);
    return Err("decoder returned empty or frame-misaligned PCM".to_string());
  }
  info.duration = (tmp_size / info.frame_bytes as u64) as f64 / info.sample_rate as f64;
  std::fs::rename(&tmp_path, &cache_path).map_err(|error| error.to_string())?;
  std::fs::write(
    &info_path,
    serde_json::to_vec(&info).map_err(|error| error.to_string())?,
  )
  .map_err(|error| error.to_string())?;
  Ok(DecodedTrack {
    path: cache_path,
    info,
  })
}

fn decode_flac(source: &Path, output: &Path) -> Result<PcmInfo, String> {
  let mut reader = FlacReader::open(source).map_err(|error| format!("open FLAC: {error}"))?;
  let stream = reader.streaminfo();
  let sample_rate = stream.sample_rate;
  let channels = u16::try_from(stream.channels).map_err(|_| "invalid FLAC channel count")?;
  let bits = u16::try_from(stream.bits_per_sample).map_err(|_| "invalid FLAC sample depth")?;
  if sample_rate == 0 || !(1..=8).contains(&channels) || !(4..=32).contains(&bits) {
    return Err(format!(
      "unsupported FLAC format: {sample_rate} Hz, {channels} channels, {bits} bits"
    ));
  }
  let frames = stream
    .samples
    .filter(|samples| *samples > 0)
    .ok_or_else(|| "FLAC STREAMINFO has no total sample count".to_string())?;
  let interleaved_samples = frames
    .checked_mul(u64::from(channels))
    .ok_or_else(|| "FLAC sample count overflow".to_string())?;
  let (container_bits, broker_format, bytes_per_sample) = match bits {
    1..=8 => (8u16, "u8", 1u16),
    9..=16 => (16, "s16le", 2),
    17..=24 => (24, "s24_3le", 3),
    _ => (32, "s32le", 4),
  };
  let sample_format = if bits == container_bits {
    format!("Signed {bits}-bit PCM (Claxon FLAC)")
  } else {
    format!("Signed {bits}-bit PCM in {container_bits}-bit container (Claxon FLAC)")
  };
  let channel_map = channel_map(channels)?;
  let frame_bytes = bytes_per_sample as u32 * channels as u32;
  let mut writer = BufWriter::new(File::create(output).map_err(|error| error.to_string())?);
  let mut chunk = Vec::with_capacity(MAX_PCM_CHUNK_BYTES + frame_bytes as usize);
  let mut samples = reader.samples();
  let shift = u32::from(container_bits - bits);

  // Decode exactly the frame count declared by FLAC STREAMINFO. These files
  // contain trailing non-frame bytes after all declared samples; asking the
  // decoder for one more frame turns that harmless tail into an EOF error.
  for sample_index in 0..interleaved_samples {
    let sample = samples
      .next()
      .ok_or_else(|| format!("FLAC ended at sample {sample_index} of {interleaved_samples}"))?
      .map_err(|error| format!("FLAC decode failed at sample {sample_index}: {error}"))?;
    let scaled = i64::from(sample) << shift;
    match container_bits {
      8 => {
        let sample = i8::try_from(scaled)
          .map_err(|_| "FLAC sample exceeds its declared bit depth".to_string())?;
        chunk.push((sample as u8) ^ 0x80);
      }
      16 => {
        let sample = i16::try_from(scaled)
          .map_err(|_| "FLAC sample exceeds its declared bit depth".to_string())?;
        chunk.extend_from_slice(&sample.to_le_bytes());
      }
      24 => {
        let sample = i32::try_from(scaled)
          .map_err(|_| "FLAC sample exceeds its declared bit depth".to_string())?;
        chunk.extend_from_slice(&sample.to_le_bytes()[..3]);
      }
      32 => {
        let sample = i32::try_from(scaled)
          .map_err(|_| "FLAC sample exceeds its declared bit depth".to_string())?;
        chunk.extend_from_slice(&sample.to_le_bytes());
      }
      _ => unreachable!(),
    }
    if chunk.len() >= MAX_PCM_CHUNK_BYTES && (sample_index + 1) % u64::from(channels) == 0 {
      writer
        .write_all(&chunk)
        .map_err(|error| error.to_string())?;
      chunk.clear();
    }
  }
  if chunk.len() % frame_bytes as usize != 0 {
    return Err("FLAC decoder produced a partial PCM frame".to_string());
  }
  writer
    .write_all(&chunk)
    .map_err(|error| error.to_string())?;
  writer.flush().map_err(|error| error.to_string())?;

  Ok(PcmInfo {
    sample_rate,
    channels,
    bits_per_sample: bits,
    sample_format,
    broker_format: broker_format.to_string(),
    channel_map,
    bytes_per_sample,
    frame_bytes,
    duration: frames as f64 / sample_rate as f64,
    decoder: "Claxon (Rust FLAC)".to_string(),
    cache_version: 0,
    decode_warning: None,
    decoder_preference: String::new(),
    source_lossless: true,
    exact_decode: true,
    source_size: 0,
    source_modified: 0,
  })
}

fn decode_symphonia(source: &Path, output: &Path, extension: &str) -> Result<PcmInfo, String> {
  let file = File::open(source).map_err(|error| error.to_string())?;
  let stream = MediaSourceStream::new(Box::new(file), Default::default());
  let mut hint = Hint::new();
  hint.with_extension(extension);
  let mut format = get_probe()
    .probe(
      &hint,
      stream,
      FormatOptions::default(),
      MetadataOptions::default(),
    )
    .map_err(|error| format!("probe failed: {error}"))?;
  let track = format
    .default_track(TrackType::Audio)
    .ok_or_else(|| "no audio track".to_string())?;
  let track_id = track.id;
  let params = track
    .codec_params
    .as_ref()
    .and_then(|params| params.audio())
    .cloned()
    .ok_or_else(|| "audio codec parameters are missing".to_string())?;
  let mut bits = params
    .bits_per_sample
    .map(|value| value as u16)
    .unwrap_or(0);
  let codec_id = params.codec;
  if codec_id == symphonia::core::codecs::audio::well_known::CODEC_ID_ALAC && bits == 0 {
    bits = params
      .extra_data
      .as_deref()
      .and_then(|extra_data| MagicCookie::read(extra_data).ok())
      .map(|cookie| u16::from(cookie.bit_depth))
      .ok_or_else(|| "ALAC bit depth is missing from its codec configuration".to_string())?;
  }
  let decoder = get_codecs()
    .make_audio_decoder(&params, &AudioDecoderOptions::default())
    .map_err(|error| format!("codec unsupported: {error}"))?;
  let mut decoder = decoder;
  let file = File::create(output).map_err(|error| error.to_string())?;
  let mut writer = BufWriter::new(file);
  let mut first_spec: Option<PcmInfo> = None;
  let mut total_frames = 0u64;
  loop {
    let packet = match format.next_packet() {
      Ok(Some(packet)) => packet,
      Ok(None) => break,
      Err(SymphoniaError::ResetRequired) => {
        return Err("stream requires a decoder reset; use FFmpeg fallback".to_string())
      }
      Err(error) => return Err(format!("demux error: {error}")),
    };
    if packet.track_id != track_id {
      continue;
    }
    let decoded = decoder
      .decode(&packet)
      .map_err(|error| format!("decode error: {error}"))?;
    let spec = decoded.spec();
    let sample_rate = spec.rate();
    let channels = u16::try_from(spec.channels().count()).map_err(|_| "too many audio channels")?;
    let channel_map = channel_map_from_spec(spec.channels())?;
    let (bytes, sample_format, broker_format, sample_bits, bytes_per_sample) =
      encode_symphonia(&decoded, bits)?;
    if bytes.is_empty() {
      continue;
    }
    let frame_bytes = bytes_per_sample as u32 * channels as u32;
    if bytes.len() % frame_bytes as usize != 0 {
      return Err("decoder produced a partial PCM frame".to_string());
    }
    if let Some(info) = &first_spec {
      if info.sample_rate != sample_rate
        || info.channels != channels
        || info.broker_format != broker_format
        || info.channel_map != channel_map
      {
        return Err(
          "audio format changes within the track are not supported for exclusive output"
            .to_string(),
        );
      }
    } else {
      let exact_decode = sample_format != "64-bit float converted to 32-bit float";
      first_spec = Some(PcmInfo {
        sample_rate,
        channels,
        bits_per_sample: sample_bits,
        sample_format,
        broker_format,
        channel_map,
        bytes_per_sample,
        frame_bytes,
        duration: 0.0,
        decoder: "Symphonia (Rust)".to_string(),
        cache_version: 0,
        decode_warning: None,
        decoder_preference: String::new(),
        source_lossless: is_symphonia_lossless(codec_id),
        exact_decode,
        source_size: 0,
        source_modified: 0,
      });
    }
    writer
      .write_all(&bytes)
      .map_err(|error| error.to_string())?;
    total_frames += (bytes.len() / frame_bytes as usize) as u64;
  }
  writer.flush().map_err(|error| error.to_string())?;
  let mut info = first_spec.ok_or_else(|| "decoder produced no PCM frames".to_string())?;
  info.duration = total_frames as f64 / info.sample_rate as f64;
  Ok(info)
}

fn encode_symphonia(
  buffer: &GenericAudioBufferRef<'_>,
  declared_bits: u16,
) -> Result<(Vec<u8>, String, String, u16, u16), String> {
  let mut out = Vec::new();
  let result = match buffer {
    GenericAudioBufferRef::U8(buffer) => {
      buffer.copy_bytes_to_vec_interleaved(&mut out);
      ("Unsigned 8-bit PCM", "u8", 8, 1)
    }
    GenericAudioBufferRef::S8(buffer) => {
      let mut samples: Vec<i8> = Vec::new();
      buffer.copy_to_vec_interleaved(&mut samples);
      out.extend(samples.into_iter().map(|sample| (sample as u8) ^ 0x80));
      ("Signed 8-bit PCM", "u8", 8, 1)
    }
    GenericAudioBufferRef::S16(buffer) => {
      buffer.copy_bytes_to_vec_interleaved(&mut out);
      ("Signed 16-bit PCM", "s16le", 16, 2)
    }
    GenericAudioBufferRef::S24(buffer) => {
      buffer.copy_bytes_to_vec_interleaved(&mut out);
      ("Signed 24-bit PCM", "s24_3le", 24, 3)
    }
    GenericAudioBufferRef::S32(buffer) => {
      let bit_depth = if (1..=32).contains(&declared_bits) {
        declared_bits
      } else {
        32
      };
      let mut samples: Vec<i32> = Vec::new();
      buffer.copy_to_vec_interleaved(&mut samples);
      match bit_depth {
        8 => {
          for sample in samples {
            out.push((((sample >> 24) as i8) as u8) ^ 0x80);
          }
          ("Signed 8-bit PCM in i32 decoder output", "u8", 8, 1)
        }
        16 => {
          for sample in samples {
            out.extend_from_slice(&((sample >> 16) as i16).to_le_bytes());
          }
          ("Signed 16-bit PCM in i32 decoder output", "s16le", 16, 2)
        }
        24 => {
          for sample in samples {
            pack_s24(sample >> 8, &mut out);
          }
          ("Signed 24-bit PCM in i32 decoder output", "s24_3le", 24, 3)
        }
        32 => {
          for sample in samples {
            out.extend_from_slice(&sample.to_le_bytes());
          }
          ("Signed 32-bit PCM", "s32le", 32, 4)
        }
        _ => return Err(format!("unsupported integer sample depth: {bit_depth}")),
      }
    }
    GenericAudioBufferRef::F32(buffer) => {
      buffer.copy_bytes_to_vec_interleaved(&mut out);
      ("32-bit float PCM", "f32le", 32, 4)
    }
    GenericAudioBufferRef::F64(buffer) => {
      let mut samples: Vec<f64> = Vec::new();
      buffer.copy_to_vec_interleaved(&mut samples);
      for sample in samples {
        out.extend_from_slice(&(sample as f32).to_le_bytes());
      }
      ("64-bit float converted to 32-bit float", "f32le", 32, 4)
    }
    GenericAudioBufferRef::U16(buffer) => {
      let mut samples: Vec<u16> = Vec::new();
      buffer.copy_to_vec_interleaved(&mut samples);
      for sample in samples {
        out.extend_from_slice(&(sample.wrapping_sub(0x8000) as i16).to_le_bytes());
      }
      ("Unsigned 16-bit converted to signed PCM", "s16le", 16, 2)
    }
    GenericAudioBufferRef::U24(buffer) => {
      buffer.copy_bytes_to_vec_interleaved(&mut out);
      for sample in out.chunks_exact_mut(3) {
        sample[2] ^= 0x80;
      }
      ("Unsigned 24-bit converted to signed PCM", "s24_3le", 24, 3)
    }
    GenericAudioBufferRef::U32(buffer) => {
      let mut samples: Vec<u32> = Vec::new();
      buffer.copy_to_vec_interleaved(&mut samples);
      for sample in samples {
        out.extend_from_slice(&((sample ^ 0x8000_0000) as i32).to_le_bytes());
      }
      ("Unsigned 32-bit converted to signed PCM", "s32le", 32, 4)
    }
  };
  if matches!(buffer, GenericAudioBufferRef::F64(_)) {
    return Ok((
      out,
      result.0.to_string(),
      result.1.to_string(),
      result.2,
      result.3,
    ));
  }
  if declared_bits != 0
    && declared_bits != result.2
    && matches!(
      buffer,
      GenericAudioBufferRef::S16(_) | GenericAudioBufferRef::S24(_) | GenericAudioBufferRef::F32(_)
    )
  {
    // The decoded typed output is authoritative when a codec advertises a storage precision.
  }
  Ok((
    out,
    result.0.to_string(),
    result.1.to_string(),
    result.2,
    result.3,
  ))
}

fn decode_ape(source: &Path, output: &Path) -> Result<PcmInfo, String> {
  let file = File::open(source).map_err(|error| error.to_string())?;
  let mut decoder = ApeDecoder::new(BufReader::new(file)).map_err(|error| error.to_string())?;
  let info = decoder.info();
  let sample_rate = info.sample_rate;
  let channels = u16::try_from(info.channels).map_err(|_| "unsupported APE channel count")?;
  let bits = u16::try_from(info.bits_per_sample).map_err(|_| "unsupported APE bit depth")?;
  let channel_map = match channels {
    1 => "MONO".to_string(),
    2 => "FL,FR".to_string(),
    _ => {
      return Err(
        "APE multichannel layout is not identified by the decoder; use FFmpeg fallback".to_string(),
      )
    }
  };
  let (format, broker_format, bytes_per_sample) = match bits {
    16 => ("Signed 16-bit PCM", "s16le", 2),
    24 => ("Signed 24-bit PCM", "s24_3le", 3),
    32 => ("Signed 32-bit PCM", "s32le", 4),
    _ => {
      return Err(format!(
        "APE {bits}-bit PCM is not supported by the AgentOS bit-exact output formats"
      ))
    }
  };
  let mut writer = BufWriter::new(File::create(output).map_err(|error| error.to_string())?);
  for frame in decoder.frames() {
    let pcm = frame.map_err(|error| error.to_string())?;
    writer.write_all(&pcm).map_err(|error| error.to_string())?;
  }
  writer.flush().map_err(|error| error.to_string())?;
  Ok(PcmInfo {
    sample_rate,
    channels,
    bits_per_sample: bits,
    sample_format: format.to_string(),
    broker_format: broker_format.to_string(),
    channel_map,
    bytes_per_sample,
    frame_bytes: bytes_per_sample as u32 * channels as u32,
    duration: 0.0,
    decoder: "ape-decoder (Rust)".to_string(),
    cache_version: 0,
    decode_warning: None,
    decoder_preference: String::new(),
    source_lossless: true,
    exact_decode: true,
    source_size: 0,
    source_modified: 0,
  })
}

fn decode_wavpack(source: &Path, output: &Path) -> Result<PcmInfo, String> {
  let mut reader = WavpackReader::open(source)
    .map_err(|error| error.to_string())?
    .build()
    .map_err(|error| error.to_string())?;
  let channels = u16::try_from(
    reader
      .get_num_channels()
      .map_err(|error| error.to_string())?,
  )
  .map_err(|_| "invalid WavPack channels")?;
  let sample_rate = reader
    .get_sample_rate()
    .map_err(|error| error.to_string())?;
  let bits = u16::try_from(
    reader
      .get_bits_per_sample()
      .map_err(|error| error.to_string())?,
  )
  .map_err(|_| "invalid WavPack bit depth")?;
  let total_samples = u64::try_from(
    reader
      .get_num_samples()
      .map_err(|error| error.to_string())?,
  )
  .map_err(|_| "invalid WavPack sample count")?;
  let channel_mask = u32::try_from(
    reader
      .get_channel_mask()
      .map_err(|error| error.to_string())?,
  )
  .map_err(|_| "invalid WavPack channel mask")?;
  let channel_map = channel_map_from_wave_mask(channel_mask, channels)?;
  let (format, broker_format, bytes_per_sample) = match bits {
    8 => ("Signed 8-bit PCM", "u8", 1),
    16 => ("Signed 16-bit PCM", "s16le", 2),
    24 => ("Signed 24-bit PCM", "s24_3le", 3),
    32 => ("Signed 32-bit PCM", "s32le", 4),
    _ => {
      return Err(format!(
        "WavPack {bits}-bit output is not supported without resampling or quantization"
      ))
    }
  };
  let mut writer = BufWriter::new(File::create(output).map_err(|error| error.to_string())?);
  let mut samples = vec![0i32; 8192 * channels as usize];
  loop {
    let frames = reader
      .unpack_samples(&mut samples)
      .map_err(|error| error.to_string())? as usize;
    if frames == 0 {
      break;
    }
    for sample in &samples[..frames * channels as usize] {
      match bits {
        8 => {
          let byte = ((*sample as i8) as u8) ^ 0x80;
          writer
            .write_all(&[byte])
            .map_err(|error| error.to_string())?;
        }
        16 => writer
          .write_all(&(*sample as i16).to_le_bytes())
          .map_err(|error| error.to_string())?,
        24 => {
          let bytes = sample.to_le_bytes();
          writer
            .write_all(&bytes[..3])
            .map_err(|error| error.to_string())?;
        }
        32 => writer
          .write_all(&sample.to_le_bytes())
          .map_err(|error| error.to_string())?,
        _ => unreachable!(),
      }
    }
  }
  writer.flush().map_err(|error| error.to_string())?;
  let lossy_blocks = reader
    .get_lossy_blocks()
    .map_err(|error| error.to_string())?;
  Ok(PcmInfo {
    sample_rate,
    channels,
    bits_per_sample: bits,
    sample_format: format.to_string(),
    broker_format: broker_format.to_string(),
    channel_map,
    bytes_per_sample,
    frame_bytes: bytes_per_sample as u32 * channels as u32,
    duration: total_samples as f64 / sample_rate as f64,
    decoder: "libwavpack 5.6".to_string(),
    cache_version: 0,
    decode_warning: None,
    decoder_preference: String::new(),
    source_lossless: lossy_blocks == 0,
    exact_decode: lossy_blocks == 0,
    source_size: 0,
    source_modified: 0,
  })
}

fn decode_ffmpeg(
  ffmpeg: &str,
  source: &Path,
  output: &Path,
  extension: &str,
) -> Result<PcmInfo, String> {
  let wav_path = output.with_extension("fallback.wav");
  let result = Command::new(ffmpeg)
    .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
    .arg(source)
    .args(["-map", "0:a:0", "-vn", "-c:a", "pcm_s32le", "-f", "wav"])
    .arg(&wav_path)
    .output()
    .map_err(|error| error.to_string())?;
  if !result.status.success() {
    let _ = std::fs::remove_file(&wav_path);
    return Err(String::from_utf8_lossy(&result.stderr).trim().to_string());
  }
  let mut wav = File::open(&wav_path).map_err(|error| error.to_string())?;
  let (sample_rate, channels, data_offset, data_len) = read_wave_header(&mut wav)?;
  if channels == 0 || channels > 8 {
    let _ = std::fs::remove_file(&wav_path);
    return Err(format!(
      "FFmpeg output has unsupported {channels}-channel layout"
    ));
  }
  let channel_map = channel_map(channels)?;
  wav
    .seek(SeekFrom::Start(data_offset))
    .map_err(|error| error.to_string())?;
  let mut output_file = BufWriter::new(File::create(output).map_err(|error| error.to_string())?);
  let mut remaining = data_len;
  let mut buffer = vec![0u8; 1024 * 1024];
  while remaining > 0 {
    let wanted = remaining.min(buffer.len() as u64) as usize;
    let count = wav
      .read(&mut buffer[..wanted])
      .map_err(|error| error.to_string())?;
    if count == 0 {
      return Err("truncated FFmpeg WAV output".to_string());
    }
    output_file
      .write_all(&buffer[..count])
      .map_err(|error| error.to_string())?;
    remaining -= count as u64;
  }
  output_file.flush().map_err(|error| error.to_string())?;
  let _ = std::fs::remove_file(&wav_path);
  let frame_bytes = channels as u32 * 4;
  Ok(PcmInfo {
    sample_rate,
    channels,
    bits_per_sample: 32,
    sample_format: "Signed 32-bit PCM (FFmpeg)".to_string(),
    broker_format: "s32le".to_string(),
    channel_map,
    bytes_per_sample: 4,
    frame_bytes,
    duration: data_len as f64 / frame_bytes as f64 / sample_rate as f64,
    decoder: "FFmpeg fallback (s32le)".to_string(),
    cache_version: 0,
    decode_warning: None,
    decoder_preference: String::new(),
    source_lossless: matches!(
      extension,
      "flac" | "wav" | "wave" | "aiff" | "aif" | "alac" | "ape" | "wv"
    ),
    exact_decode: false,
    source_size: 0,
    source_modified: 0,
  })
}

fn read_wave_header(file: &mut File) -> Result<(u32, u16, u64, u64), String> {
  let file_len = file.metadata().map_err(|error| error.to_string())?.len();
  let mut header = [0u8; 12];
  file
    .read_exact(&mut header)
    .map_err(|error| error.to_string())?;
  let is_riff = &header[..4] == b"RIFF" || &header[..4] == b"RF64";
  if !is_riff || &header[8..12] != b"WAVE" {
    return Err("FFmpeg did not produce a RIFF/WAVE stream".to_string());
  }
  let mut sample_rate = None;
  let mut channels = None;
  let mut data_offset = None;
  let mut data_size = None;
  loop {
    let mut chunk = [0u8; 8];
    if file.read_exact(&mut chunk).is_err() {
      break;
    }
    let chunk_size = u32::from_le_bytes(chunk[4..8].try_into().unwrap()) as u64;
    let start = file.stream_position().map_err(|error| error.to_string())?;
    match &chunk[..4] {
      b"fmt " => {
        let mut format = vec![0u8; chunk_size.min(64) as usize];
        file
          .read_exact(&mut format)
          .map_err(|error| error.to_string())?;
        if format.len() < 16 {
          return Err("invalid FFmpeg WAV fmt chunk".to_string());
        }
        let tag = u16::from_le_bytes([format[0], format[1]]);
        let rate = u32::from_le_bytes(format[4..8].try_into().unwrap());
        let count = u16::from_le_bytes([format[2], format[3]]);
        let bits = u16::from_le_bytes([format[14], format[15]]);
        if !matches!(tag, 1 | 0xfffe) || bits != 32 {
          return Err("FFmpeg WAV fallback is not 32-bit integer PCM".to_string());
        }
        sample_rate = Some(rate);
        channels = Some(count);
        file
          .seek(SeekFrom::Start(start + chunk_size + (chunk_size & 1)))
          .map_err(|error| error.to_string())?;
      }
      b"ds64" => {
        let mut ds64 = vec![0u8; chunk_size.min(64) as usize];
        file
          .read_exact(&mut ds64)
          .map_err(|error| error.to_string())?;
        if ds64.len() >= 24 {
          data_size = Some(u64::from_le_bytes(ds64[8..16].try_into().unwrap()));
        }
        file
          .seek(SeekFrom::Start(start + chunk_size + (chunk_size & 1)))
          .map_err(|error| error.to_string())?;
      }
      b"data" => {
        data_offset = Some(start);
        data_size = if chunk_size == u32::MAX as u64 {
          data_size.or_else(|| file_len.checked_sub(start))
        } else {
          Some(chunk_size)
        };
        break;
      }
      _ => {
        file
          .seek(SeekFrom::Start(start + chunk_size + (chunk_size & 1)))
          .map_err(|error| error.to_string())?;
      }
    }
  }
  Ok((
    sample_rate.ok_or_else(|| "WAV sample rate is missing".to_string())?,
    channels.ok_or_else(|| "WAV channel count is missing".to_string())?,
    data_offset.ok_or_else(|| "WAV data chunk is missing".to_string())?,
    data_size.ok_or_else(|| "WAV data length is missing".to_string())?,
  ))
}

fn is_symphonia_lossless(codec_id: symphonia::core::codecs::audio::AudioCodecId) -> bool {
  use symphonia::core::codecs::audio::well_known::*;
  matches!(
    codec_id,
    CODEC_ID_FLAC
      | CODEC_ID_ALAC
      | CODEC_ID_PCM_S8
      | CODEC_ID_PCM_S16LE
      | CODEC_ID_PCM_S16BE
      | CODEC_ID_PCM_S24LE
      | CODEC_ID_PCM_S24BE
      | CODEC_ID_PCM_S32LE
      | CODEC_ID_PCM_S32BE
      | CODEC_ID_PCM_U8
      | CODEC_ID_PCM_U16LE
      | CODEC_ID_PCM_U16BE
      | CODEC_ID_PCM_U24LE
      | CODEC_ID_PCM_U24BE
      | CODEC_ID_PCM_U32LE
      | CODEC_ID_PCM_U32BE
      | CODEC_ID_PCM_F32LE
      | CODEC_ID_PCM_F32BE
      | CODEC_ID_PCM_F64LE
      | CODEC_ID_PCM_F64BE
  )
}

fn channel_map_from_spec(channels: &Channels) -> Result<String, String> {
  match channels {
    Channels::Positioned(positions) => {
      let names = positions
        .iter()
        .map(|position| match position {
          Position::FRONT_LEFT => Some("FL"),
          Position::FRONT_RIGHT => Some("FR"),
          Position::FRONT_CENTER => Some("FC"),
          Position::LFE1 => Some("LFE"),
          Position::REAR_LEFT => Some("RL"),
          Position::REAR_RIGHT => Some("RR"),
          Position::REAR_CENTER => Some("RC"),
          Position::SIDE_LEFT => Some("SL"),
          Position::SIDE_RIGHT => Some("SR"),
          _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| format!("unsupported positioned channel layout: {channels}"))?;
      if names.is_empty() || names.len() > 8 {
        return Err(format!("unsupported channel count: {}", names.len()));
      }
      if names.as_slice() == ["FC"] {
        return Ok("MONO".to_string());
      }
      Ok(names.join(","))
    }
    Channels::Discrete(1) => Ok("MONO".to_string()),
    Channels::Discrete(2) => Ok("FL,FR".to_string()),
    Channels::Discrete(count) => Err(format!(
      "a {count}-channel discrete layout has no speaker map"
    )),
    _ => Err(format!("unsupported channel layout: {channels}")),
  }
}

fn channel_map_from_wave_mask(mask: u32, channels: u16) -> Result<String, String> {
  if mask == 0 {
    return match channels {
      1 => Ok("MONO".to_string()),
      2 => Ok("FL,FR".to_string()),
      _ => Err("WavPack multichannel audio has no channel mask".to_string()),
    };
  }
  let names = (0..32)
    .filter(|bit| mask & (1 << bit) != 0)
    .map(|bit| match bit {
      0 => Some("FL"),
      1 => Some("FR"),
      2 => Some("FC"),
      3 => Some("LFE"),
      4 => Some("RL"),
      5 => Some("RR"),
      6 => Some("FLC"),
      7 => Some("FRC"),
      8 => Some("RC"),
      9 => Some("SL"),
      10 => Some("SR"),
      _ => None,
    })
    .collect::<Option<Vec<_>>>()
    .ok_or_else(|| format!("unsupported WavPack channel mask 0x{mask:x}"))?;
  if names.len() != channels as usize || names.is_empty() || names.len() > 8 {
    return Err(format!(
      "WavPack channel mask does not match its {channels} channels"
    ));
  }
  if channels == 1 && names.as_slice() == ["FC"] {
    return Ok("MONO".to_string());
  }
  Ok(names.join(","))
}

fn channel_map(channels: u16) -> Result<String, String> {
  match channels {
    1 => Ok("MONO".to_string()),
    2 => Ok("FL,FR".to_string()),
    3 => Ok("FL,FR,FC".to_string()),
    4 => Ok("FL,FR,RL,RR".to_string()),
    5 => Ok("FL,FR,FC,RL,RR".to_string()),
    6 => Ok("FL,FR,FC,LFE,RL,RR".to_string()),
    7 => Ok("FL,FR,FC,LFE,RC,SL,SR".to_string()),
    8 => Ok("FL,FR,FC,LFE,RL,RR,SL,SR".to_string()),
    _ => Err(format!(
      "AgentOS audio output does not support {channels} channels"
    )),
  }
}

fn pack_s24(value: i32, output: &mut Vec<u8>) {
  let bytes = value.to_le_bytes();
  output.extend_from_slice(&bytes[..3]);
}
