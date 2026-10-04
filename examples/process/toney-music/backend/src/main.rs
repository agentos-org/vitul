mod audio;
mod cue;
mod metadata;
mod tray;

use std::{
  net::SocketAddr,
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
  sync::Arc,
  time::SystemTime,
};

use axum::{
  extract::{DefaultBodyLimit, Multipart, Path as AxumPath, State},
  http::{header, Method, StatusCode},
  response::{IntoResponse, Response},
  routing::{delete, get, post},
  Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::{fs, io::AsyncWriteExt, net::TcpListener, sync::RwLock};
use tower_http::{
  cors::{Any, CorsLayer},
  services::ServeDir,
};
use uuid::Uuid;

const SERVICE_NAME: &str = "toney-musicd";
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");
const PLUGIN_ROUTE: &str = "/apps/toney-music";
const MAX_UPLOAD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = (8 * 1024 * 1024 * 1024usize) + (32 * 1024 * 1024);
const SUPPORTED_EXTENSIONS: &[&str] = &[
  "flac", "wav", "wave", "aiff", "aif", "mp3", "aac", "m4a", "ogg", "oga", "opus", "dsf", "dff",
  "ape", "wv", "alac",
];

#[derive(Clone)]
struct AppState {
  data_dir: PathBuf,
  library_dir: PathBuf,
  cache_dir: PathBuf,
  store_path: PathBuf,
  ffmpeg: String,
  decoder_preferences: Arc<RwLock<DecoderPreferences>>,
  bit_perfect_enabled: Arc<RwLock<bool>>,
  tray_locale: Arc<RwLock<String>>,
  tracks: Arc<RwLock<Vec<Track>>>,
  audio: Arc<audio::AudioManager>,
  ui_last_seen_ms: Arc<AtomicU64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum DecoderPreference {
  Auto,
  Ffmpeg,
}

impl DecoderPreference {
  fn as_str(self) -> &'static str {
    match self {
      Self::Auto => "auto",
      Self::Ffmpeg => "ffmpeg",
    }
  }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct DecoderPreferences {
  flac: DecoderPreference,
  wav: DecoderPreference,
  aiff: DecoderPreference,
  alac: DecoderPreference,
  wv: DecoderPreference,
  ape: DecoderPreference,
  other: DecoderPreference,
}

impl Default for DecoderPreferences {
  fn default() -> Self {
    Self {
      flac: DecoderPreference::Auto,
      wav: DecoderPreference::Auto,
      aiff: DecoderPreference::Auto,
      alac: DecoderPreference::Auto,
      wv: DecoderPreference::Auto,
      ape: DecoderPreference::Auto,
      other: DecoderPreference::Auto,
    }
  }
}

impl DecoderPreferences {
  fn for_extension(&self, extension: &str) -> DecoderPreference {
    match extension {
      "flac" => self.flac,
      "wav" | "wave" => self.wav,
      "aiff" | "aif" => self.aiff,
      "alac" | "m4a" => self.alac,
      "wv" => self.wv,
      "ape" => self.ape,
      _ => self.other,
    }
  }

  fn set(&mut self, format: DecoderFormat, preference: DecoderPreference) {
    match format {
      DecoderFormat::Flac => self.flac = preference,
      DecoderFormat::Wav => self.wav = preference,
      DecoderFormat::Aiff => self.aiff = preference,
      DecoderFormat::Alac => self.alac = preference,
      DecoderFormat::Wv => self.wv = preference,
      DecoderFormat::Ape => self.ape = preference,
      DecoderFormat::Other => self.other = preference,
    }
  }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DecoderFormat {
  Flac,
  Wav,
  Aiff,
  Alac,
  Wv,
  Ape,
  Other,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Track {
  id: String,
  key: String,
  #[serde(default)]
  source_key: String,
  file_name: String,
  stored_name: String,
  title: String,
  artist: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  album: String,
  extension: String,
  size: u64,
  #[serde(default)]
  duration: f64,
  tone: u8,
  added_at: u128,
  #[serde(default)]
  metadata_version: u8,
  #[serde(default)]
  cue_start_frame: Option<u64>,
  #[serde(default)]
  cue_end_frame: Option<u64>,
  #[serde(default)]
  track_number: Option<u32>,
}

#[derive(Serialize, Deserialize)]
struct LibraryFile {
  version: u8,
  tracks: Vec<Track>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicTrack<'a> {
  id: &'a str,
  key: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  track_number: Option<u32>,
  file_name: &'a str,
  title: &'a str,
  artist: &'a str,
  #[serde(skip_serializing_if = "str::is_empty")]
  album: &'a str,
  extension: &'a str,
  size: u64,
  duration: f64,
  tone: u8,
  added_at: u128,
}

#[derive(Debug)]
struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    (self.0, Json(json!({ "error": self.1 }))).into_response()
  }
}

impl From<std::io::Error> for ApiError {
  fn from(error: std::io::Error) -> Self {
    Self(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayRequest {
  track_id: String,
  #[serde(default)]
  position: f64,
  #[serde(default = "default_audio_mode")]
  mode: String,
  #[serde(default)]
  device: String,
  #[serde(default = "default_true")]
  autoplay: bool,
}

#[derive(Deserialize)]
struct SeekRequest {
  position: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecoderPreferenceRequest {
  format: DecoderFormat,
  preference: DecoderPreference,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct BitPerfectSettings {
  enabled: bool,
}

impl Default for BitPerfectSettings {
  fn default() -> Self {
    Self { enabled: false }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitPerfectSettingsRequest {
  enabled: bool,
  track_id: Option<String>,
  device: Option<String>,
}

#[derive(Deserialize)]
struct TrayLocaleRequest {
  locale: String,
}

fn default_audio_mode() -> String {
  "exclusive".to_string()
}
fn default_true() -> bool {
  true
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let data_dir = PathBuf::from(env_value("DATA_DIR", "./data"));
  let public_dir = PathBuf::from(env_value("PUBLIC_DIR", "./public"));
  let host = env_value("HOST", "0.0.0.0");
  let port: u16 = env_value("PORT", "3000").parse().unwrap_or(3000);
  let ffmpeg = std::env::var("FFMPEG_BIN").unwrap_or_default();
  let library_dir = data_dir.join("library");
  let cache_dir = data_dir.join("decoded-cache");
  fs::create_dir_all(&library_dir).await?;
  fs::create_dir_all(&cache_dir).await?;

  let store_path = data_dir.join("library.json");
  let mut tracks = match fs::read(&store_path).await {
    Ok(data) => serde_json::from_slice::<LibraryFile>(&data)?.tracks,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
    Err(error) => return Err(error.into()),
  };
  let stale_tracks = tracks
    .iter()
    .enumerate()
    .filter(|(_, track)| track.metadata_version < 2)
    .map(|(index, track)| {
      (
        index,
        library_dir.join(&track.stored_name),
        track.extension.clone(),
        track.file_name.clone(),
      )
    })
    .collect::<Vec<_>>();
  let mut tracks_metadata_updated = false;
  if !stale_tracks.is_empty() {
    let ffmpeg_for_metadata = ffmpeg.clone();
    let updates = tokio::task::spawn_blocking(move || {
      stale_tracks
        .into_iter()
        .map(|(index, path, extension, file_name)| {
          let metadata = metadata::read(&path, &extension, &ffmpeg_for_metadata);
          (index, file_name, metadata)
        })
        .collect::<Vec<_>>()
    })
    .await?;
    for (index, file_name, metadata) in updates {
      let track = &mut tracks[index];
      let (fallback_title, fallback_artist) = parse_track_name(&file_name);
      track.title = metadata.title.unwrap_or(fallback_title);
      track.artist = metadata.artist.unwrap_or(fallback_artist);
      track.album = metadata.album.unwrap_or_default();
      if metadata.duration > 0.0 {
        track.duration = metadata.duration;
      }
      track.metadata_version = 2;
    }
    tracks_metadata_updated = true;
  }
  let decoder_preferences_path = data_dir.join("decoder-settings.json");
  let decoder_preferences = match fs::read(&decoder_preferences_path).await {
    Ok(data) => serde_json::from_slice::<DecoderPreferences>(&data).unwrap_or_default(),
    Err(_) => DecoderPreferences::default(),
  };
  let bit_perfect_settings_path = data_dir.join("bit-perfect-settings.json");
  let bit_perfect_settings = match fs::read(&bit_perfect_settings_path).await {
    Ok(data) => serde_json::from_slice::<BitPerfectSettings>(&data).unwrap_or_default(),
    Err(_) => BitPerfectSettings::default(),
  };
  let tray_locale = fs::read_to_string(data_dir.join("tray-locale.txt"))
    .await
    .unwrap_or_default();
  let state = AppState {
    data_dir,
    library_dir,
    cache_dir: cache_dir.clone(),
    store_path,
    ffmpeg,
    decoder_preferences: Arc::new(RwLock::new(decoder_preferences)),
    bit_perfect_enabled: Arc::new(RwLock::new(bit_perfect_settings.enabled)),
    tray_locale: Arc::new(RwLock::new(
      if tray_locale.trim() == "zh" {
        "zh"
      } else {
        "en"
      }
      .to_string(),
    )),
    tracks: Arc::new(RwLock::new(tracks)),
    audio: Arc::new(audio::AudioManager::new(cache_dir)),
    ui_last_seen_ms: Arc::new(AtomicU64::new(0)),
  };
  if tracks_metadata_updated {
    let tracks = state.tracks.read().await;
    save_library(&state, &tracks).await?;
  }
  let tray_state = state.clone();

  let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods([
      Method::GET,
      Method::HEAD,
      Method::POST,
      Method::DELETE,
      Method::OPTIONS,
    ])
    .allow_headers([header::CONTENT_TYPE, header::RANGE]);

  let routes = Router::new()
    .route("/health", get(health))
    .route("/api/status", get(status))
    .route("/api/tray/locale", post(update_tray_locale))
    .route(
      "/api/decoder-settings",
      get(get_decoder_settings).post(update_decoder_settings),
    )
    .route(
      "/api/bit-perfect",
      get(get_bit_perfect_settings).post(update_bit_perfect_settings),
    )
    .route("/api/tracks", get(list_tracks).post(upload_track))
    .route("/api/tracks/:id", delete(delete_track))
    .route("/api/playback", get(playback_status).post(play_track))
    .route("/api/client-heartbeat", post(client_heartbeat))
    .route("/api/playback/pause", post(pause_playback))
    .route("/api/playback/resume", post(resume_playback))
    .route("/api/playback/stop", post(stop_playback))
    .route("/api/playback/seek", post(seek_playback))
    .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
    .layer(cors)
    .with_state(state.clone());
  let app = Router::new()
    .nest(
      PLUGIN_ROUTE,
      routes
        .clone()
        .fallback_service(ServeDir::new(public_dir.clone()).append_index_html_on_directories(true)),
    )
    .merge(routes)
    .fallback_service(ServeDir::new(public_dir).append_index_html_on_directories(true));

  let address: SocketAddr = format!("{host}:{port}").parse()?;
  let listener = TcpListener::bind(address).await?;
  eprintln!("{SERVICE_NAME} {SERVICE_VERSION} listening on {address}");
  tokio::spawn(tray::run(tray_state));
  tokio::spawn(auto_advance_cue_tracks(state.clone()));
  axum::serve(listener, app).await?;
  Ok(())
}

async fn health() -> Json<Value> {
  Json(json!({ "status": "ok", "service": SERVICE_NAME, "version": SERVICE_VERSION }))
}

async fn update_tray_locale(
  State(state): State<AppState>,
  Json(request): Json<TrayLocaleRequest>,
) -> Result<StatusCode, ApiError> {
  if !matches!(request.locale.as_str(), "zh" | "en") {
    return Err(ApiError(
      StatusCode::BAD_REQUEST,
      "unsupported tray locale".to_string(),
    ));
  }
  fs::write(state.data_dir.join("tray-locale.txt"), &request.locale).await?;
  *state.tray_locale.write().await = request.locale;
  Ok(StatusCode::NO_CONTENT)
}

async fn status(State(state): State<AppState>) -> Json<Value> {
  let tracks = state.tracks.read().await.len();
  let audio = state.audio.capabilities().await;
  let ffmpeg_available = !state.ffmpeg.is_empty() && Path::new(&state.ffmpeg).is_file();
  Json(json!({
      "service": SERVICE_NAME,
      "version": SERVICE_VERSION,
      "tracks": tracks,
      "decoder": {
          "available": true,
          "engine": "claxon-flac-rust + symphonia-rust + ape-decoder + libwavpack",
          "path": if ffmpeg_available { "FFmpeg fallback bundled" } else { "FFmpeg fallback unavailable" },
          "formats": ["flac", "wav", "aiff", "alac", "ape", "wv", "mp3", "aac", "vorbis", "opus", "dsd"],
          "engines": [
              { "formats": ["flac"], "engine": "Claxon (Rust FLAC)" },
              { "formats": ["wav", "wave", "aiff", "aif", "alac", "m4a", "mp3", "aac", "ogg", "oga"], "engine": "Symphonia (Rust)" },
              { "formats": ["ape"], "engine": "ape-decoder (Rust)" },
              { "formats": ["wv"], "engine": "libwavpack 5.6" },
              { "formats": ["opus", "dsf", "dff", "other"], "engine": "FFmpeg fallback" }
          ],
          "ffmpegAvailable": ffmpeg_available
      },
      "audio": audio
  }))
}

async fn get_decoder_settings(State(state): State<AppState>) -> Json<DecoderPreferences> {
  Json(state.decoder_preferences.read().await.clone())
}

async fn update_decoder_settings(
  State(state): State<AppState>,
  Json(request): Json<DecoderPreferenceRequest>,
) -> Result<Json<DecoderPreferences>, ApiError> {
  let mut preferences = state.decoder_preferences.write().await;
  let mut updated = preferences.clone();
  updated.set(request.format, request.preference);
  save_decoder_preferences(&state.data_dir, &updated)
    .await
    .map_err(|error| {
      ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("save decoder settings: {error}"),
      )
    })?;
  *preferences = updated.clone();
  Ok(Json(updated))
}

async fn get_bit_perfect_settings(State(state): State<AppState>) -> Json<Value> {
  Json(json!({
    "enabled": *state.bit_perfect_enabled.read().await,
  }))
}

async fn update_bit_perfect_settings(
  State(state): State<AppState>,
  Json(request): Json<BitPerfectSettingsRequest>,
) -> Result<Json<Value>, ApiError> {
  let preflight = if request.enabled {
    let track_id = if let Some(track_id) = request.track_id.clone() {
      Some(track_id)
    } else if let Some(track_id) = state.audio.current_track_id().await {
      Some(track_id)
    } else {
      state
        .tracks
        .read()
        .await
        .iter()
        .min_by_key(|track| track.added_at)
        .map(|track| track.id.clone())
    };
    if let Some(track_id) = track_id {
      let track = state
        .tracks
        .read()
        .await
        .iter()
        .find(|track| track.id == track_id)
        .cloned()
        .ok_or_else(|| {
          ApiError(
            StatusCode::NOT_FOUND,
            "The selected song is no longer in the library.".to_string(),
          )
        })?;
      let decoder_preference = state
        .decoder_preferences
        .read()
        .await
        .for_extension(&track.extension)
        .as_str()
        .to_string();
      Some(
        state
          .audio
          .check_bit_perfect_output(
            cache_identity(&track),
            state.library_dir.join(&track.stored_name),
            state.cache_dir.clone(),
            state.ffmpeg.clone(),
            track.extension,
            decoder_preference,
            request.device.unwrap_or_default(),
          )
          .await
          .map_err(|error| ApiError(StatusCode::UNPROCESSABLE_ENTITY, error))?,
      )
    } else {
      let capabilities = state.audio.capabilities().await;
      let requested_device = request
        .device
        .as_deref()
        .filter(|device| !device.is_empty());
      let exclusive_device = capabilities
        .get("devices")
        .and_then(Value::as_array)
        .and_then(|devices| {
          devices.iter().find(|device| {
            device.get("mode").and_then(Value::as_str) == Some("exclusive")
              && device.get("available").and_then(Value::as_bool) != Some(false)
              && requested_device.is_none_or(|requested| {
                requested == "default"
                  || device.get("id").and_then(Value::as_str) == Some(requested)
              })
          })
        });
      let device_id = exclusive_device
        .and_then(|device| device.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
          ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "AgentOS did not provide an available exclusive ALSA device.".to_string(),
          )
        })?;
      Some(json!({ "deferred": true, "device": device_id }))
    }
  } else {
    None
  };

  let settings = BitPerfectSettings {
    enabled: request.enabled,
  };
  save_bit_perfect_settings(&state.data_dir, settings)
    .await
    .map_err(|error| {
      ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("save bit-perfect setting: {error}"),
      )
    })?;
  *state.bit_perfect_enabled.write().await = settings.enabled;
  Ok(Json(json!({
    "enabled": settings.enabled,
    "preflight": preflight,
  })))
}

async fn list_tracks(State(state): State<AppState>) -> Json<Value> {
  let mut tracks = state.tracks.read().await.clone();
  tracks.sort_by_key(|track| track.added_at);
  Json(json!({ "tracks": tracks.iter().map(public_track).collect::<Vec<_>>() }))
}

async fn upload_track(
  State(state): State<AppState>,
  mut multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), ApiError> {
  let mut audio_upload: Option<(String, String, PathBuf, u64, String)> = None;
  let mut cue_upload: Option<(String, Vec<u8>)> = None;
  while let Some(field) = multipart
    .next_field()
    .await
    .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?
  {
    let field_name = field.name().unwrap_or_default().to_string();
    let original_name = field.file_name().map(str::to_string);
    if field_name == "cue" {
      if cue_upload.is_some() {
        return Err(ApiError(
          StatusCode::BAD_REQUEST,
          "only one CUE sheet can be imported at a time".to_string(),
        ));
      }
      let cue_name = original_name
        .as_deref()
        .and_then(|name| Path::new(name).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("album.cue")
        .to_string();
      let mut bytes = Vec::new();
      let mut field = field;
      while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))?
      {
        if bytes.len().saturating_add(chunk.len()) > 2 * 1024 * 1024 {
          return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "CUE file is larger than 2 MiB".to_string(),
          ));
        }
        bytes.extend_from_slice(&chunk);
      }
      cue_upload = Some((cue_name, bytes));
      continue;
    }
    if field_name != "file" {
      continue;
    }
    if audio_upload.is_some() {
      return Err(ApiError(
        StatusCode::BAD_REQUEST,
        "import one audio source at a time".to_string(),
      ));
    }
    let file_name = original_name
      .as_deref()
      .and_then(|name| Path::new(name).file_name())
      .and_then(|name| name.to_str())
      .unwrap_or("audio")
      .to_string();
    let extension = Path::new(&file_name)
      .extension()
      .and_then(|ext| ext.to_str())
      .unwrap_or("")
      .to_ascii_lowercase();
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
      return Err(ApiError(
        StatusCode::BAD_REQUEST,
        format!("unsupported audio format: {extension}"),
      ));
    }
    let upload_id = Uuid::new_v4().simple().to_string();
    let tmp_path = state.library_dir.join(format!(".{upload_id}.upload"));
    let mut file = fs::File::create(&tmp_path).await?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut field = field;
    while let Some(chunk) = field
      .chunk()
      .await
      .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?
    {
      size += chunk.len() as u64;
      if size > MAX_UPLOAD_BYTES {
        let _ = fs::remove_file(&tmp_path).await;
        return Err(ApiError(
          StatusCode::PAYLOAD_TOO_LARGE,
          "audio file is larger than 8 GiB".to_string(),
        ));
      }
      hasher.update(&chunk);
      file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);
    if size == 0 {
      let _ = fs::remove_file(&tmp_path).await;
      return Err(ApiError(
        StatusCode::BAD_REQUEST,
        "audio file is empty".to_string(),
      ));
    }
    let key = format!("{}:{size}", hex_digest(hasher.finalize().as_slice()));
    audio_upload = Some((file_name, extension, tmp_path, size, key));
  }
  let Some((file_name, extension, tmp_path, size, source_key)) = audio_upload else {
    return Err(ApiError(
      StatusCode::BAD_REQUEST,
      "missing audio file field".to_string(),
    ));
  };
  let cue_sheet = if let Some((cue_name, bytes)) = cue_upload {
    if !cue_name.to_ascii_lowercase().ends_with(".cue") {
      let _ = fs::remove_file(&tmp_path).await;
      return Err(ApiError(
        StatusCode::BAD_REQUEST,
        "the accompanying file must use the .cue extension".to_string(),
      ));
    }
    match cue::parse(&bytes, &file_name) {
      Ok(sheet) => Some(sheet),
      Err(error) => {
        let _ = fs::remove_file(&tmp_path).await;
        return Err(ApiError(StatusCode::BAD_REQUEST, error));
      }
    }
  } else {
    None
  };

  let metadata_path = tmp_path.clone();
  let metadata_extension = extension.clone();
  let ffmpeg = state.ffmpeg.clone();
  let metadata = tokio::task::spawn_blocking(move || {
    metadata::read(&metadata_path, &metadata_extension, &ffmpeg)
  })
  .await
  .unwrap_or_default();
  let mut tracks = state.tracks.write().await;

  if cue_sheet.is_none() {
    if let Some(existing) = tracks
      .iter()
      .find(|track| track.source_key == source_key || track.key == source_key)
    {
      let existing = existing.clone();
      drop(tracks);
      let _ = fs::remove_file(&tmp_path).await;
      return Ok((
        StatusCode::CREATED,
        Json(json!({
          "track": public_track(&existing),
          "tracks": [public_track(&existing)],
          "duplicate": true,
        })),
      ));
    }
  }

  let existing_source = tracks
    .iter()
    .find(|track| track.source_key == source_key || track.key == source_key)
    .map(|track| track.stored_name.clone());
  let source_already_stored = existing_source.is_some();
  let source_id = Uuid::new_v4().simple().to_string();
  let stored_name = existing_source.unwrap_or_else(|| format!("{source_id}.{extension}"));
  let final_path = state.library_dir.join(&stored_name);
  if final_path.exists() {
    let _ = fs::remove_file(&tmp_path).await;
  } else if let Err(error) = fs::rename(&tmp_path, &final_path).await {
    let _ = fs::remove_file(&tmp_path).await;
    return Err(ApiError(
      StatusCode::INTERNAL_SERVER_ERROR,
      format!("store audio source: {error}"),
    ));
  }

  let (fallback_title, fallback_artist) = parse_track_name(&file_name);
  let added_at = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis();
  let previous_tracks = tracks.clone();
  let mut added = Vec::new();
  let mut duplicate = true;
  if let Some(sheet) = cue_sheet {
    // Replace a previously imported whole-disc entry with its CUE-defined songs.
    let current_id = state.audio.current_track_id().await;
    let removed_current = current_id.is_some_and(|id| {
      tracks.iter().any(|track| {
        track.id == id
          && (track.source_key == source_key
            || (track.source_key.is_empty() && track.key == source_key))
          && track.track_number.is_none()
      })
    });
    tracks.retain(|track| {
      !((track.source_key == source_key
        || (track.source_key.is_empty() && track.key == source_key))
        && track.track_number.is_none())
    });
    if removed_current {
      state.audio.stop().await;
    }
    for (index, cue_track) in sheet.tracks.iter().enumerate() {
      let end_frame = sheet.tracks.get(index + 1).map(|next| next.start_frame);
      let key = format!(
        "{source_key}#cue:{}:{}",
        cue_track.number, cue_track.start_frame
      );
      if let Some(existing) = tracks.iter().find(|track| track.key == key) {
        added.push(existing.clone());
        continue;
      }
      let id = Uuid::new_v4().simple().to_string();
      let source_duration = if let Some(end) = end_frame {
        (end.saturating_sub(cue_track.start_frame)) as f64 / 75.0
      } else {
        (metadata.duration - cue_track.start_frame as f64 / 75.0).max(0.0)
      };
      let track = Track {
        id,
        key: key.clone(),
        source_key: source_key.clone(),
        file_name: file_name.clone(),
        stored_name: stored_name.clone(),
        title: cue_track
          .title
          .clone()
          .unwrap_or_else(|| format!("Track {:02}", cue_track.number)),
        artist: cue_track
          .performer
          .clone()
          .or_else(|| sheet.performer.clone())
          .or_else(|| metadata.artist.clone())
          .unwrap_or_else(|| fallback_artist.clone()),
        album: sheet
          .album
          .clone()
          .or_else(|| metadata.album.clone())
          .unwrap_or_else(|| {
            Path::new(&file_name)
              .file_stem()
              .and_then(|name| name.to_str())
              .unwrap_or("Local album")
              .to_string()
          }),
        extension: extension.clone(),
        size,
        duration: source_duration,
        tone: hasher_tone(&key),
        added_at: added_at + index as u128,
        metadata_version: 2,
        cue_start_frame: Some(cue_track.start_frame),
        cue_end_frame: end_frame,
        track_number: Some(cue_track.number),
      };
      tracks.push(track.clone());
      added.push(track);
      duplicate = false;
    }
  } else {
    let track = Track {
      id: source_id,
      key: source_key.clone(),
      source_key: source_key.clone(),
      file_name,
      stored_name,
      title: metadata.title.unwrap_or(fallback_title),
      artist: metadata.artist.unwrap_or(fallback_artist),
      album: metadata.album.unwrap_or_default(),
      extension,
      size,
      duration: metadata.duration,
      tone: hasher_tone(&source_key),
      added_at,
      metadata_version: 2,
      cue_start_frame: None,
      cue_end_frame: None,
      track_number: None,
    };
    tracks.push(track.clone());
    added.push(track);
    duplicate = false;
  }

  if let Err(error) = save_library(&state, &tracks).await {
    *tracks = previous_tracks;
    if !source_already_stored {
      let _ = fs::remove_file(final_path).await;
    }
    return Err(ApiError(
      StatusCode::INTERNAL_SERVER_ERROR,
      format!("save library: {error}"),
    ));
  }
  let public_tracks = added.iter().map(public_track).collect::<Vec<_>>();
  let first_track = added.first().map(public_track);
  Ok((
    StatusCode::CREATED,
    Json(json!({
      "track": first_track,
      "tracks": public_tracks,
      "duplicate": duplicate,
    })),
  ))
}

async fn delete_track(
  State(state): State<AppState>,
  AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
  let current_id = state.audio.current_track_id().await;
  if current_id.as_deref() == Some(&id) {
    state.audio.stop().await;
  }
  let mut tracks = state.tracks.write().await;
  let index = tracks
    .iter()
    .position(|track| track.id == id)
    .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "track not found".to_string()))?;
  let track = tracks.remove(index);
  let source_still_used = tracks
    .iter()
    .any(|item| item.stored_name == track.stored_name);
  let source_key_still_used = !track.source_key.is_empty()
    && tracks
      .iter()
      .any(|item| item.source_key == track.source_key);
  if !source_still_used {
    let _ = fs::remove_file(state.library_dir.join(&track.stored_name)).await;
  }
  if !source_key_still_used {
    let cache_id = cache_identity(&track);
    let _ = fs::remove_file(state.cache_dir.join(format!("{cache_id}.pcm"))).await;
    let _ = fs::remove_file(state.cache_dir.join(format!("{cache_id}.json"))).await;
  }
  save_library(&state, &tracks).await.map_err(|e| {
    ApiError(
      StatusCode::INTERNAL_SERVER_ERROR,
      format!("save library: {e}"),
    )
  })?;
  Ok(StatusCode::NO_CONTENT)
}

async fn playback_status(State(state): State<AppState>) -> Json<Value> {
  let mut snapshot = state.audio.snapshot().await;
  let track_id = snapshot.get("trackId").and_then(Value::as_str);
  let auto_advance_cue = if let Some(track_id) = track_id {
    let tracks = state.tracks.read().await;
    tracks
      .iter()
      .find(|track| track.id == track_id)
      .and_then(|track| {
        track.track_number.map(|track_number| {
          tracks.iter().any(|candidate| {
            candidate.source_key == track.source_key
              && candidate
                .track_number
                .is_some_and(|number| number > track_number)
          })
        })
      })
      .unwrap_or(false)
  } else {
    false
  };
  if let Some(fields) = snapshot.as_object_mut() {
    fields.insert("autoAdvanceCue".to_string(), Value::Bool(auto_advance_cue));
  }
  Json(snapshot)
}

async fn client_heartbeat(State(state): State<AppState>) -> StatusCode {
  let now = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis() as u64;
  state.ui_last_seen_ms.store(now, Ordering::Relaxed);
  StatusCode::NO_CONTENT
}

async fn auto_advance_cue_tracks(state: AppState) {
  let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
  interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
  let mut attempted_from_track: Option<String> = None;
  loop {
    interval.tick().await;
    let snapshot = state.audio.snapshot().await;
    if snapshot.get("state").and_then(Value::as_str) != Some("ended") {
      attempted_from_track = None;
      continue;
    }
    let now = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .unwrap_or_default()
      .as_millis() as u64;
    let last_seen = state.ui_last_seen_ms.load(Ordering::Relaxed);
    if last_seen > 0 && now.saturating_sub(last_seen) < 5_000 {
      continue;
    }
    let Some(current_id) = snapshot.get("trackId").and_then(Value::as_str) else {
      continue;
    };
    if attempted_from_track.as_deref() == Some(current_id) {
      continue;
    }
    let next_track = {
      let tracks = state.tracks.read().await;
      let Some(current) = tracks.iter().find(|track| track.id == current_id) else {
        continue;
      };
      let Some(track_number) = current.track_number else {
        continue;
      };
      tracks
        .iter()
        .filter(|candidate| {
          candidate.source_key == current.source_key
            && candidate
              .track_number
              .is_some_and(|number| number > track_number)
        })
        .min_by_key(|candidate| candidate.track_number)
        .cloned()
    };
    let Some(next_track) = next_track else {
      continue;
    };
    attempted_from_track = Some(current_id.to_string());
    let require_bit_perfect = *state.bit_perfect_enabled.read().await;
    let mode = if require_bit_perfect {
      "exclusive".to_string()
    } else {
      snapshot
        .get("mode")
        .and_then(Value::as_str)
        .filter(|mode| matches!(*mode, "exclusive" | "shared"))
        .unwrap_or("exclusive")
        .to_string()
    };
    let device = snapshot
      .get("device")
      .and_then(Value::as_str)
      .filter(|device| !device.is_empty())
      .unwrap_or("default")
      .to_string();
    if let Err(error) = start_track(&state, next_track, 0.0, &mode, device, true).await {
      eprintln!("Toney Music could not continue the CUE album in tray mode: {error}");
    }
  }
}

async fn play_track(
  State(state): State<AppState>,
  Json(request): Json<PlayRequest>,
) -> Result<Json<Value>, ApiError> {
  let track = state
    .tracks
    .read()
    .await
    .iter()
    .find(|track| track.id == request.track_id)
    .cloned()
    .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "track not found".to_string()))?;
  let device = if request.device.is_empty() {
    "default".to_string()
  } else {
    request.device
  };
  let response = start_track(
    &state,
    track,
    request.position,
    &request.mode,
    device,
    request.autoplay,
  )
  .await
  .map_err(|error| ApiError(StatusCode::SERVICE_UNAVAILABLE, error))?;
  Ok(Json(response))
}

async fn start_track(
  state: &AppState,
  track: Track,
  position: f64,
  mode: &str,
  device: String,
  autoplay: bool,
) -> Result<Value, String> {
  let require_bit_perfect = *state.bit_perfect_enabled.read().await;
  if require_bit_perfect && mode != "exclusive" {
    return Err("Bit-perfect mode requires exclusive ALSA output.".to_string());
  }
  let decoder_preference = state
    .decoder_preferences
    .read()
    .await
    .for_extension(&track.extension)
    .as_str()
    .to_string();
  let response = state
    .audio
    .play(
      track.id.clone(),
      cache_identity(&track),
      state.library_dir.join(&track.stored_name),
      state.cache_dir.clone(),
      state.ffmpeg.clone(),
      track.extension.clone(),
      decoder_preference,
      track.cue_start_frame,
      track.cue_end_frame,
      position,
      mode,
      device,
      autoplay,
      require_bit_perfect,
    )
    .await?;
  if let Some(duration) = response.get("duration").and_then(Value::as_f64) {
    let mut tracks = state.tracks.write().await;
    if let Some(stored) = tracks.iter_mut().find(|item| item.id == track.id) {
      stored.duration = duration;
      let _ = save_library(state, &tracks).await;
    }
  }
  Ok(response)
}

async fn pause_playback(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  state
    .audio
    .pause()
    .await
    .map(Json)
    .map_err(|e| ApiError(StatusCode::SERVICE_UNAVAILABLE, e))
}

async fn resume_playback(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
  state
    .audio
    .resume()
    .await
    .map(Json)
    .map_err(|e| ApiError(StatusCode::SERVICE_UNAVAILABLE, e))
}

async fn stop_playback(State(state): State<AppState>) -> Json<Value> {
  state.audio.stop().await;
  Json(state.audio.snapshot().await)
}

async fn seek_playback(
  State(state): State<AppState>,
  Json(request): Json<SeekRequest>,
) -> Result<Json<Value>, ApiError> {
  let track_id = state
    .audio
    .current_track_id()
    .await
    .ok_or_else(|| ApiError(StatusCode::CONFLICT, "no track is selected".to_string()))?;
  let track = state
    .tracks
    .read()
    .await
    .iter()
    .find(|track| track.id == track_id)
    .cloned()
    .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "track not found".to_string()))?;
  let snapshot = state.audio.snapshot().await;
  let mode = snapshot
    .get("mode")
    .and_then(Value::as_str)
    .unwrap_or("exclusive")
    .to_string();
  let device = snapshot
    .get("device")
    .and_then(Value::as_str)
    .unwrap_or("default")
    .to_string();
  let was_playing = snapshot.get("state").and_then(Value::as_str) != Some("paused");
  let response = start_track(&state, track, request.position, &mode, device, was_playing)
    .await
    .map_err(|error| ApiError(StatusCode::SERVICE_UNAVAILABLE, error))?;
  Ok(Json(response))
}

fn public_track(track: &Track) -> PublicTrack<'_> {
  PublicTrack {
    id: &track.id,
    key: &track.key,
    track_number: track.track_number,
    file_name: &track.file_name,
    title: &track.title,
    artist: &track.artist,
    album: &track.album,
    extension: &track.extension,
    size: track.size,
    duration: track.duration,
    tone: track.tone,
    added_at: track.added_at,
  }
}

fn cache_identity(track: &Track) -> String {
  if track.source_key.is_empty() {
    track.id.clone()
  } else {
    format!("source-{}", hex_digest(track.source_key.as_bytes()))
  }
}

async fn save_library(state: &AppState, tracks: &[Track]) -> Result<(), std::io::Error> {
  let mut ordered = tracks.to_vec();
  ordered.sort_by_key(|track| track.added_at);
  let data = serde_json::to_vec_pretty(&LibraryFile {
    version: 1,
    tracks: ordered,
  })
  .map_err(std::io::Error::other)?;
  let tmp = state
    .data_dir
    .join(format!("library-{}.json", Uuid::new_v4().simple()));
  fs::write(&tmp, data).await?;
  fs::rename(tmp, &state.store_path).await
}

async fn save_decoder_preferences(
  data_dir: &Path,
  preferences: &DecoderPreferences,
) -> Result<(), std::io::Error> {
  let data = serde_json::to_vec_pretty(preferences).map_err(std::io::Error::other)?;
  let tmp = data_dir.join(format!("decoder-settings-{}.json", Uuid::new_v4().simple()));
  fs::write(&tmp, data).await?;
  fs::rename(tmp, data_dir.join("decoder-settings.json")).await
}

async fn save_bit_perfect_settings(
  data_dir: &Path,
  settings: BitPerfectSettings,
) -> Result<(), std::io::Error> {
  let data = serde_json::to_vec_pretty(&settings).map_err(std::io::Error::other)?;
  let tmp = data_dir.join(format!(
    "bit-perfect-settings-{}.json",
    Uuid::new_v4().simple()
  ));
  fs::write(&tmp, data).await?;
  fs::rename(tmp, data_dir.join("bit-perfect-settings.json")).await
}

fn parse_track_name(file_name: &str) -> (String, String) {
  let base = Path::new(file_name)
    .file_stem()
    .and_then(|v| v.to_str())
    .unwrap_or(file_name);
  for separator in [" – ", " — ", " - "] {
    if let Some((artist, title)) = base.split_once(separator) {
      if !title.trim().is_empty() {
        return (
          title.trim().to_string(),
          if artist.trim().is_empty() {
            "Local file".to_string()
          } else {
            artist.trim().to_string()
          },
        );
      }
    }
  }
  (base.to_string(), "Local file".to_string())
}

fn hasher_tone(value: &str) -> u8 {
  let digest = Sha256::digest(value.as_bytes());
  digest[0] % 5
}

fn hex_digest(digest: &[u8]) -> String {
  digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn env_value(name: &str, fallback: &str) -> String {
  std::env::var(name)
    .ok()
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| fallback.to_string())
}
