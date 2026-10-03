use async_trait::async_trait;
use opentool_rust::{CliArguments, OpenToolServer, Tool, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
  env, fs,
  path::{Path, PathBuf},
  sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
  },
  time::{SystemTime, UNIX_EPOCH},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct FrameVideoTool {
  state: Arc<Mutex<LibraryState>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryState {
  file_path: PathBuf,
  videos: Vec<VideoRecord>,
}

#[derive(Deserialize)]
struct PersistedLibrary {
  videos: Vec<VideoRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoRecord {
  id: String,
  path: PathBuf,
  name: String,
  size: u64,
  added_at: u64,
  favorite: bool,
}

const SPEC: &str = r#"{
  "opentool": "1.0.0",
  "info": { "title": "Frame Video", "version": "0.2.0" },
  "functions": [
    {
      "name": "library.list",
      "description": "List videos saved in the Frame Video library.",
      "parameters": [],
      "returns": { "type": "object", "properties": { "videos": { "type": "array", "items": { "type": "object" } } } }
    },
    {
      "name": "library.add",
      "description": "Add a local video file to the Frame Video playlist.",
      "parameters": [{ "name": "path", "type": "string", "required": true }],
      "returns": { "type": "object", "properties": { "id": { "type": "string" }, "name": { "type": "string" }, "size": { "type": "number" } } }
    },
    {
      "name": "library.remove",
      "description": "Remove a video from the Frame Video playlist without deleting its source file.",
      "parameters": [{ "name": "id", "type": "string", "required": true }],
      "returns": { "type": "object", "properties": { "removed": { "type": "boolean" } } }
    },
    {
      "name": "library.setFavorite",
      "description": "Add or remove a video from the watch-later list.",
      "parameters": [
        { "name": "id", "type": "string", "required": true },
        { "name": "favorite", "type": "boolean", "required": true }
      ],
      "returns": { "type": "object", "properties": { "favorite": { "type": "boolean" } } }
    },
    {
      "name": "library.clear",
      "description": "Clear the Frame Video playlist without deleting source files.",
      "parameters": [],
      "returns": { "type": "object", "properties": { "cleared": { "type": "boolean" } } }
    }
  ]
}"#;

pub type VitulOpenToolServer = OpenToolServer<FrameVideoTool>;

impl FrameVideoTool {
  pub fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
    let file_path = data_directory()?.join("library.json");
    if let Some(parent) = file_path.parent() {
      fs::create_dir_all(parent)?;
    }

    let videos = if file_path.is_file() {
      let contents = fs::read(&file_path)?;
      serde_json::from_slice::<PersistedLibrary>(&contents)?.videos
    } else {
      Vec::new()
    };

    Ok(Self {
      state: Arc::new(Mutex::new(LibraryState { file_path, videos })),
    })
  }

  pub fn media_path(&self, id: &str) -> Option<PathBuf> {
    self
      .state
      .lock()
      .ok()?
      .videos
      .iter()
      .find(|video| video.id == id)
      .map(|video| video.path.clone())
  }

  fn list(&self) -> Result<Vec<Value>, ToolError> {
    let state = self
      .state
      .lock()
      .map_err(|_| ToolError::execution("video library state is unavailable"))?;
    Ok(state.videos.iter().map(video_view).collect())
  }

  fn add(&self, value: &Value) -> Result<Value, ToolError> {
    let raw_path = value
      .get("path")
      .and_then(Value::as_str)
      .filter(|path| !path.is_empty() && path.len() <= 4096)
      .ok_or_else(|| ToolError::invalid_params("path must be a non-empty file path"))?;
    let selected_path = Path::new(raw_path);
    if !selected_path.is_absolute() {
      return Err(ToolError::invalid_params("path must be absolute"));
    }
    let path = selected_path
      .canonicalize()
      .map_err(|error| ToolError::execution(format!("could not open selected file: {error}")))?;
    let metadata = fs::metadata(&path)
      .map_err(|error| ToolError::execution(format!("could not read selected file: {error}")))?;
    if !metadata.is_file() || !is_supported_video(&path) {
      return Err(ToolError::invalid_params("selected path is not a supported video file"));
    }

    let mut state = self
      .state
      .lock()
      .map_err(|_| ToolError::execution("video library state is unavailable"))?;
    if let Some(existing) = state.videos.iter().find(|video| video.path == path) {
      return Ok(video_view(existing));
    }
    if state.videos.len() >= 500 {
      return Err(ToolError::execution("the playlist can contain at most 500 videos"));
    }

    let name = path
      .file_name()
      .map(|name| name.to_string_lossy().into_owned())
      .filter(|name| !name.is_empty())
      .ok_or_else(|| ToolError::invalid_params("selected file has no name"))?;
    let record = VideoRecord {
      id: new_id(),
      path,
      name,
      size: metadata.len(),
      added_at: now_millis(),
      favorite: false,
    };
    state.videos.push(record.clone());
    persist(&state)?;
    Ok(video_view(&record))
  }

  fn remove(&self, id: &str) -> Result<bool, ToolError> {
    let mut state = self
      .state
      .lock()
      .map_err(|_| ToolError::execution("video library state is unavailable"))?;
    let previous_len = state.videos.len();
    state.videos.retain(|video| video.id != id);
    let removed = state.videos.len() != previous_len;
    if removed {
      persist(&state)?;
    }
    Ok(removed)
  }

  fn set_favorite(&self, id: &str, favorite: bool) -> Result<bool, ToolError> {
    let mut state = self
      .state
      .lock()
      .map_err(|_| ToolError::execution("video library state is unavailable"))?;
    let video = state
      .videos
      .iter_mut()
      .find(|video| video.id == id)
      .ok_or_else(|| ToolError::invalid_params("video id was not found"))?;
    video.favorite = favorite;
    persist(&state)?;
    Ok(favorite)
  }

  fn clear(&self) -> Result<(), ToolError> {
    let mut state = self
      .state
      .lock()
      .map_err(|_| ToolError::execution("video library state is unavailable"))?;
    state.videos.clear();
    persist(&state)
  }
}

#[async_trait]
impl Tool for FrameVideoTool {
  async fn call(&self, method: &str, params: Value) -> Result<Value, ToolError> {
    match method {
      "library.list" => Ok(json!({ "videos": self.list()? })),
      "library.add" => self.add(&params),
      "library.remove" => {
        let id = required_string(&params, "id")?;
        Ok(json!({ "removed": self.remove(id)? }))
      }
      "library.setFavorite" => {
        let id = required_string(&params, "id")?;
        let favorite = params
          .get("favorite")
          .and_then(Value::as_bool)
          .ok_or_else(|| ToolError::invalid_params("favorite must be a boolean"))?;
        Ok(json!({ "favorite": self.set_favorite(id, favorite)? }))
      }
      "library.clear" => {
        self.clear()?;
        Ok(json!({ "cleared": true }))
      }
      _ => Err(ToolError::method_not_found(method)),
    }
  }
}

pub async fn create_server(
  tool: FrameVideoTool,
  arguments: impl IntoIterator<Item = String>,
  host: String,
  port: u16,
) -> Result<VitulOpenToolServer, Box<dyn std::error::Error + Send + Sync>> {
  let cli_arguments = CliArguments::new(arguments);
  let tool_args = tool.init_args(cli_arguments.parse()?)?;
  opentool_rust::schema::load_json(SPEC)?;
  tool.init(&tool_args).await?;
  let max_request_body_bytes = if env::var_os("AGENTOS_APP_BUNDLE_ID").is_some() {
    1024 * 1024
  } else {
    opentool_rust::DEFAULT_MAX_REQUEST_BODY_BYTES
  };

  Ok(
    OpenToolServer::builder(tool)
      .host(host)
      .port(port)
      .version(env!("CARGO_PKG_VERSION"))
      .spec_json(SPEC)
      .cli_arguments(CliArguments::new(std::iter::empty::<String>()))
      .max_request_body_bytes(max_request_body_bytes)
      .build(),
  )
}

fn required_string<'a>(params: &'a Value, key: &str) -> Result<&'a str, ToolError> {
  params
    .get(key)
    .and_then(Value::as_str)
    .filter(|value| !value.is_empty() && value.len() <= 4096)
    .ok_or_else(|| ToolError::invalid_params(format!("{key} must be a non-empty string")))
}

fn video_view(video: &VideoRecord) -> Value {
  json!({
    "id": video.id,
    "key": video.id,
    "name": video.name,
    "size": video.size,
    "addedAt": video.added_at,
    "favorite": video.favorite
  })
}

fn is_supported_video(path: &Path) -> bool {
  matches!(
    path.extension()
      .and_then(|extension| extension.to_str())
      .unwrap_or_default()
      .to_ascii_lowercase()
      .as_str(),
    "mp4" | "webm" | "mov" | "mkv" | "avi" | "m4v" | "ogv"
  )
}

fn persist(state: &LibraryState) -> Result<(), ToolError> {
  let contents = serde_json::to_vec(&json!({ "videos": &state.videos }))
    .map_err(|error| ToolError::execution(format!("could not encode video library: {error}")))?;
  fs::write(&state.file_path, contents)
    .map_err(|error| ToolError::execution(format!("could not save video library: {error}")))
}

fn data_directory() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
  if let Some(path) = env::var_os("DATA_DIR").filter(|path| !path.is_empty()) {
    return Ok(PathBuf::from(path));
  }
  if cfg!(target_os = "macos") {
    let home = env::var_os("HOME").ok_or_else(|| {
      std::io::Error::new(std::io::ErrorKind::NotFound, "HOME is not set")
    })?;
    return Ok(PathBuf::from(home).join("Library/Application Support/Frame Video"));
  }
  if cfg!(target_os = "windows") {
    let base = env::var_os("LOCALAPPDATA")
      .or_else(|| env::var_os("USERPROFILE"))
      .ok_or_else(|| {
        std::io::Error::new(
          std::io::ErrorKind::NotFound,
          "neither LOCALAPPDATA nor USERPROFILE is set",
        )
      })?;
    return Ok(PathBuf::from(base).join("Frame Video"));
  }
  if let Some(path) = env::var_os("XDG_DATA_HOME").filter(|path| !path.is_empty()) {
    return Ok(PathBuf::from(path).join("frame-video"));
  }
  let home = env::var_os("HOME").ok_or_else(|| {
    std::io::Error::new(std::io::ErrorKind::NotFound, "HOME is not set")
  })?;
  Ok(PathBuf::from(home).join(".local/share/frame-video"))
}

fn new_id() -> String {
  let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_nanos();
  let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
  format!("{timestamp:x}-{sequence:x}")
}

fn now_millis() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis() as u64
}
