mod api;
mod external_wayland;
mod installation;
mod model;
mod opentool;
mod pe_arch;
mod prefixes;
mod runtime;
mod shortcuts;
mod storage;
mod tray;

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

  let api = api::router(state.clone())
    .route("/health", get(api::health))
    .layer(CorsLayer::permissive());
  let app = build_app_router(open_tool.router(), api, public_dir);

  let address: SocketAddr = format!("{host}:{port}").parse()?;
  let listener = TcpListener::bind(address).await?;
  eprintln!("WinTone {SERVICE_VERSION} listening on {address}");
  tokio::spawn(tray::run());
  let startup_state = state.clone();
  tokio::spawn(async move {
    if let Err(error) = startup_state.runtime_manager.prepare().await {
      eprintln!(
        "WinTone could not prepare the Windows runtime at startup: {}",
        error.0
      );
    }
    installation::resume_waiting_tasks(startup_state).await;
  });
  axum::serve(listener, app).await?;
  Ok(())
}

fn build_app_router(open_tool: Router, api: Router, public_dir: PathBuf) -> Router {
  let static_files = ServeDir::new(public_dir).append_index_html_on_directories(true);
  open_tool.merge(
    Router::new()
      .nest(PLUGIN_ROUTE, api.clone())
      .nest_service(PLUGIN_ROUTE, static_files.clone())
      .merge(api)
      .fallback_service(static_files),
  )
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

#[cfg(test)]
mod tests {
  use super::*;
  use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
  };
  use http_body_util::BodyExt;
  use tower::ServiceExt;

  #[tokio::test]
  async fn frontend_entry_serves_index_assets_and_api() {
    let public_dir =
      std::env::temp_dir().join(format!("wintone-frontend-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(public_dir.join("assets")).unwrap();
    std::fs::write(
      public_dir.join("index.html"),
      "<title>WinTone</title><script type=\"module\" src=\"./assets/index.js\"></script><link rel=\"stylesheet\" href=\"./assets/index.css\">",
    )
    .unwrap();
    std::fs::write(
      public_dir.join("assets/index.js"),
      "console.log('wintone');",
    )
    .unwrap();
    std::fs::write(public_dir.join("assets/index.css"), "body { color: blue; }").unwrap();
    let api = Router::new().route("/api/status", get(|| async { "api-ready" }));
    let app = build_app_router(Router::new(), api, public_dir.clone());

    let response = app
      .clone()
      .oneshot(
        Request::builder()
          .uri("/apps/wintone/")
          .body(Body::empty())
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("<title>WinTone"));
    let javascript = html
      .split("src=\"./")
      .nth(1)
      .and_then(|asset| asset.split('\"').next())
      .expect("Vite should reference a JavaScript asset");
    let stylesheet = html
      .split("href=\"./")
      .nth(1)
      .and_then(|asset| asset.split('\"').next())
      .expect("Vite should reference a stylesheet asset");

    for asset in [javascript, stylesheet] {
      let response = app
        .clone()
        .oneshot(
          Request::builder()
            .uri(format!("/apps/wintone/{asset}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
      assert_eq!(response.status(), StatusCode::OK, "asset {asset}");
    }

    let response = app
      .oneshot(
        Request::builder()
          .uri("/apps/wintone/api/status")
          .body(Body::empty())
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"api-ready");

    std::fs::remove_dir_all(public_dir).unwrap();
  }
}
