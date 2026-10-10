mod api;
mod external_wayland;
mod installation;
mod model;
mod opentool;
mod prefixes;
mod runtime;
mod shortcuts;
mod storage;

use std::{
  collections::HashSet,
  env,
  net::SocketAddr,
  path::PathBuf,
  sync::{Arc, Mutex},
};

use axum::{routing::get, Router};
use opentool::WinToneTool;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::{model::Store, storage::StoreRepository};

const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");
const PLUGIN_ROUTE: &str = "/apps/wintone";

#[derive(Clone)]
pub struct AppState {
  pub repository: StoreRepository,
  pub runtime_manager: runtime::RuntimeManager,
  pub prefix_manager: prefixes::PrefixManager,
  pub external_wayland: external_wayland::ExternalWaylandClient,
  pub data_dir: PathBuf,
  pub active_install_tasks: Arc<Mutex<HashSet<String>>>,
  pub active_app_operations: Arc<Mutex<HashSet<String>>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let data_dir = PathBuf::from(env::var("DATA_DIR").unwrap_or_else(|_| "./data".into()));
  tokio::fs::create_dir_all(data_dir.join("installers")).await?;
  tokio::fs::create_dir_all(data_dir.join("runtimes")).await?;
  tokio::fs::create_dir_all(data_dir.join("prefixes")).await?;
  let repository = StoreRepository::open(data_dir.join("installers.json")).await?;
  let catalog_path = env::var_os("WINTONE_RUNTIME_CATALOG")
    .map(PathBuf::from)
    .or_else(|| {
      env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .map(|path| path.join("runtime-catalog.json"))
    })
    .unwrap_or_else(|| data_dir.join("runtime-catalog.json"));
  let state = AppState {
    repository,
    runtime_manager: runtime::RuntimeManager::new(data_dir.clone(), catalog_path),
    prefix_manager: prefixes::PrefixManager::new(data_dir.clone()),
    external_wayland: external_wayland::ExternalWaylandClient::from_env(),
    data_dir,
    active_install_tasks: Arc::new(Mutex::new(HashSet::new())),
    active_app_operations: Arc::new(Mutex::new(HashSet::new())),
  };
  installation::resume_waiting_tasks(state.clone()).await;
  tokio::spawn(installation::monitor_sessions(state.clone()));

  let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
  let port = env::var("PORT")
    .unwrap_or_else(|_| "3000".into())
    .parse::<u16>()?;
  let open_tool = opentool::create_server(
    WinToneTool {
      state: state.clone(),
    },
    host.clone(),
    port,
  )
  .await?;

  let public_dir = env::var("PUBLIC_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|_| {
      env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("public")
    });

  let api = api::router(state)
    .route("/health", get(api::health))
    .layer(CorsLayer::permissive());
  let static_files = ServeDir::new(public_dir).append_index_html_on_directories(true);
  let app = open_tool.router().merge(
    Router::new()
      .nest(
        PLUGIN_ROUTE,
        api.clone().fallback_service(static_files.clone()),
      )
      .merge(api)
      .fallback_service(static_files),
  );

  let address: SocketAddr = format!("{host}:{port}").parse()?;
  let listener = TcpListener::bind(address).await?;
  eprintln!("WinTone {SERVICE_VERSION} listening on {address}");
  axum::serve(listener, app).await?;
  Ok(())
}

impl AppState {
  pub async fn snapshot(&self) -> Result<Store, storage::StorageError> {
    self.repository.snapshot().await
  }

  pub fn installers_dir(&self) -> PathBuf {
    self.data_dir.join("installers")
  }

  pub fn claim_install_task(&self, task_id: &str) -> bool {
    self
      .active_install_tasks
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .insert(task_id.to_owned())
  }

  pub fn release_install_task(&self, task_id: &str) {
    self
      .active_install_tasks
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .remove(task_id);
  }

  pub fn install_task_is_active(&self, task_id: &str) -> bool {
    self
      .active_install_tasks
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .contains(task_id)
  }

  pub fn claim_app_operation(&self, app_id: &str) -> bool {
    self
      .active_app_operations
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .insert(app_id.to_owned())
  }

  pub fn release_app_operation(&self, app_id: &str) {
    self
      .active_app_operations
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .remove(app_id);
  }
}
