use axum::{
  extract::State,
  http::{header, HeaderValue, Method, Request, StatusCode},
  middleware::{self, Next},
  response::{IntoResponse, Response},
  Json, Router,
};
use serde_json::json;
use std::{env, io::Write, sync::Arc};
use tokio::{net::TcpListener, sync::watch};
use tower_http::{
  cors::{AllowOrigin, CorsLayer},
  services::{ServeDir, ServeFile},
};
use vitul_backend::{create_server, VitulOpenToolServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
  let port = env::var("PORT")
    .unwrap_or_else(|_| "0".to_owned())
    .parse::<u16>()?;
  // OpenToolServer::router() deliberately leaves hosting and lifecycle to
  // the embedder. Share the same tool value and call stop() from both the
  // protocol endpoint and process shutdown so dispose runs exactly once.
  let server = Arc::new(create_server(std::env::args().skip(1), host.clone(), port).await?);

  let cors = cors_layer(env::var("VITUL_DEV_ORIGIN").ok().as_deref());

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

fn allowed_origins(dev_origin: Option<&str>) -> Vec<HeaderValue> {
  let mut origins = [
    "vitul://localhost",
    "http://vitul.localhost",
    "https://vitul.localhost",
  ]
  .into_iter()
  .map(HeaderValue::from_static)
  .collect::<Vec<_>>();

  #[cfg(debug_assertions)]
  if let Some(origin) = dev_origin.and_then(|origin| HeaderValue::from_str(origin).ok()) {
    if !origins.contains(&origin) {
      origins.push(origin);
    }
  }

  #[cfg(not(debug_assertions))]
  let _ = dev_origin;

  origins
}

fn cors_layer(dev_origin: Option<&str>) -> CorsLayer {
  CorsLayer::new()
    .allow_origin(AllowOrigin::list(allowed_origins(dev_origin)))
    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
    .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

#[cfg(test)]
mod tests {
  use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
  };
  use serde_json::json;
  use tower::ServiceExt;

  #[test]
  fn cors_allows_desktop_runtime_origins_and_debug_dev_origin() {
    let origins = super::allowed_origins(Some("http://localhost:1420"));
    let values = origins
      .iter()
      .map(|origin| origin.to_str().unwrap())
      .collect::<Vec<_>>();
    assert!(values.contains(&"vitul://localhost"));
    assert!(values.contains(&"http://vitul.localhost"));
    #[cfg(debug_assertions)]
    assert!(values.contains(&"http://localhost:1420"));
    #[cfg(not(debug_assertions))]
    assert!(!values.contains(&"http://localhost:1420"));
  }

  #[tokio::test]
  async fn development_opentool_rpc_and_cors_work_on_http() {
    let server = super::create_server(
      std::iter::empty(),
      "127.0.0.1".to_owned(),
      0,
    )
    .await
    .unwrap();
    let app = server
      .router()
      .layer(super::cors_layer(Some("http://localhost:1420")));

    let version = app
      .clone()
      .oneshot(
        Request::builder()
          .method(Method::GET)
          .uri("/opentool/version")
          .header(header::ORIGIN, "http://localhost:1420")
          .body(Body::empty())
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(version.status(), StatusCode::OK);
    assert_eq!(
      version
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .unwrap(),
      "http://localhost:1420"
    );
    let version_body = axum::body::to_bytes(version.into_body(), 1024)
      .await
      .unwrap();
    let version_json: serde_json::Value =
      serde_json::from_slice(&version_body).unwrap();
    assert_eq!(version_json["version"], "0.1.0");

    let greet = app
      .clone()
      .oneshot(
        Request::builder()
          .method(Method::POST)
          .uri("/opentool/call")
          .header(header::ORIGIN, "http://localhost:1420")
          .header(header::CONTENT_TYPE, "application/json")
          .body(Body::from(
            serde_json::to_vec(&json!({
              "jsonrpc": "2.0.0",
              "id": "smoke-1",
              "method": "greet",
              "params": { "name": "Codex" }
            }))
            .unwrap(),
          ))
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(greet.status(), StatusCode::OK);
    let greet_body = axum::body::to_bytes(greet.into_body(), 1024)
      .await
      .unwrap();
    let greet_json: serde_json::Value = serde_json::from_slice(&greet_body).unwrap();
    assert_eq!(greet_json["result"]["message"], "Hello, Codex!");

    let response = app
      .oneshot(
        Request::builder()
          .method(Method::OPTIONS)
          .uri("/opentool/call")
          .header(header::ORIGIN, "http://localhost:1420")
          .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
          .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
          .body(Body::empty())
          .unwrap(),
      )
      .await
      .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
      response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .unwrap(),
      "http://localhost:1420"
    );
  }
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
