mod api;
mod bundle;
mod model;
mod platform;
mod provider;
mod storage;

use std::{env, net::SocketAddr, path::PathBuf};

use axum::{
  routing::{get, get_service},
  Router,
};
use opentool::AndroidToneTool;
use tokio::net::TcpListener;
use tower_http::{
  cors::CorsLayer,
  services::{ServeDir, ServeFile},
};

mod opentool;

#[derive(Clone)]
pub struct AppState {
  pub repository: storage::Repository,
  pub provider: provider::ProviderClient,
  pub data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  match env::args().nth(1).as_deref() {
    Some("--verify-runtime") => {
      println!("{}", bundle::verify().map_err(std::io::Error::other)?);
      return Ok(());
    }
    Some("--agentos-host-provider") => return provider::run_host_provider().await,
    Some("--agentos-wayland-client") => {
      let context = env::args()
        .nth(2)
        .ok_or("AgentOS did not pass the trusted desktop-session context to the Wayland client.")?;
      return provider::run_wayland_client(&context).await;
    }
    _ => {}
  }
  let data_dir = PathBuf::from(env::var("DATA_DIR").unwrap_or_else(|_| "./data".into()));
  tokio::fs::create_dir_all(data_dir.join("imports")).await?;
  let repository = storage::Repository::open(data_dir.join("state.json")).await?;
  let provider = provider::ProviderClient::from_env();
  let state = AppState {
    repository: repository.clone(),
    provider: provider.clone(),
    data_dir,
  };
  let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
  let port = env::var("PORT")
    .unwrap_or_else(|_| "3000".into())
    .parse::<u16>()?;
  let open_tool = opentool::create_server(
    AndroidToneTool {
      provider,
      repository,
    },
    host.clone(),
    port,
  )
  .await?;
  let api = api::router(state)
    .route("/health", get(api::health))
    .layer(CorsLayer::permissive());
  let public_dir = env::var("PUBLIC_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|_| {
      env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("public")
    });
  let index = ServeFile::new(public_dir.join("index.html"));
  let static_files = ServeDir::new(public_dir).append_index_html_on_directories(true);
  let app = open_tool.router().merge(
    Router::new()
      // Axum 0.7 matches a nested root at the bare prefix; the Shell uses a trailing slash.
      .route("/apps/androidtone/", get_service(index.clone()))
      .nest(
        "/apps/androidtone",
        api
          .clone()
          .route("/", get_service(index))
          .fallback_service(static_files.clone()),
      )
      .merge(api)
      .fallback_service(static_files),
  );
  let address: SocketAddr = format!("{host}:{port}").parse()?;
  eprintln!("AndroidTone listening on {address}");
  axum::serve(TcpListener::bind(address).await?, app).await?;
  Ok(())
}
