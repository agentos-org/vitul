use axum::{
  extract::State,
  http::{header, HeaderValue, Method, Request, StatusCode},
  middleware::{self, Next},
  response::{IntoResponse, Response},
  Json, Router,
};
use serde_json::{json, Value};
use std::{env, io::Write, sync::Arc};
use tokio::{net::TcpListener, sync::watch};
use tower_http::{
  cors::{AllowOrigin, CorsLayer},
  services::{ServeDir, ServeFile},
};
use vitul_backend::{create_server, VitulOpenToolServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
  let port = env::var("PORT")
    .unwrap_or_else(|_| "0".to_owned())
    .parse::<u16>()?;
  // OpenToolServer::router() deliberately leaves hosting and lifecycle to
  // the embedder. Share the same tool value and call stop() from both the
  // protocol endpoint and process shutdown so dispose runs exactly once.
  let server = Arc::new(create_server(std::env::args().skip(1), host.clone(), port).await?);

  let allowed_origins = [
    HeaderValue::from_static("vitul://localhost"),
    HeaderValue::from_static("http://vitul.localhost"),
    HeaderValue::from_static("https://vitul.localhost"),
  ];
  let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::list(allowed_origins))
    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
    .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

  let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
  let stop_state = Arc::new(StopState {
    server: Arc::clone(&server),
    shutdown: shutdown_tx.clone(),
  });
  let app: Router = server
    .router()
    .route_service("/", ServeFile::new("web/index.html"))
    .fallback_service(ServeDir::new("web").append_index_html_on_directories(true))
    .layer(middleware::from_fn_with_state(stop_state, intercept_stop))
    .layer(cors);
  let listener = TcpListener::bind((host.as_str(), port)).await?;
  let actual_port = listener.local_addr()?.port();
  println!("VITUL_OPENTOOL_READY={actual_port}");
  std::io::stdout().flush()?;

  let shutdown_server = Arc::clone(&server);
  let shutdown = async move {
    tokio::select! {
        _ = shutdown_rx.changed() => {},
        _ = wait_for_os_shutdown() => {
            let _ = shutdown_server.stop().await;
            shutdown_tx.send_replace(true);
        }
    }
  };
  axum::serve(listener, app)
    .with_graceful_shutdown(shutdown)
    .await?;
  Ok(())
}

struct StopState {
  server: Arc<VitulOpenToolServer>,
  shutdown: watch::Sender<bool>,
}

async fn intercept_stop(
  State(state): State<Arc<StopState>>,
  request: Request<axum::body::Body>,
  next: Next,
) -> Response {
  if request.method() == Method::POST && request.uri().path() == "/opentool/stop" {
    return match state.server.stop().await {
      Ok(()) => {
        state.shutdown.send_replace(true);
        Json(json!({"status": "stopped"})).into_response()
      }
      Err(error) => (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": error.to_string()})),
      )
        .into_response(),
    };
  }
  next.run(request).await
}

async fn wait_for_os_shutdown() {
  #[cfg(unix)]
  {
    use tokio::signal::unix::{signal, SignalKind};
    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = terminate.recv() => {},
    }
  }
  #[cfg(not(unix))]
  {
    let _ = tokio::signal::ctrl_c().await;
  }
}
