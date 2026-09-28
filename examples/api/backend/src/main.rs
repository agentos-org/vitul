use async_trait::async_trait;
use axum::{
  extract::State,
  http::{header, HeaderValue, Method, Request, StatusCode},
  middleware::{self, Next},
  response::{IntoResponse, Response},
  Json, Router,
};
use opentool_rust::{CliArguments, OpenToolServer, Tool, ToolError};
use serde_json::{json, Value};
use std::{env, io::Write, sync::Arc};
use tokio::{net::TcpListener, sync::watch};
use tower_http::{
  cors::{AllowOrigin, CorsLayer},
  services::{ServeDir, ServeFile},
};

#[derive(Clone, Default)]
struct ExampleTool;

#[async_trait]
impl Tool for ExampleTool {
  async fn call(&self, method: &str, params: Value) -> Result<Value, ToolError> {
    match method {
      "greet" => {
        let name = params
          .get("name")
          .and_then(Value::as_str)
          .unwrap_or("world");
        Ok(json!({ "message": format!("Hello, {name}!" ) }))
      }
      _ => Err(ToolError::method_not_found(method)),
    }
  }
}

const SPEC: &str = r#"{
  "opentool": "1.0.0",
  "info": { "title": "Vitul API Example", "version": "0.1.0" },
  "functions": [{
    "name": "greet",
    "description": "Return a greeting for a name.",
    "parameters": [{ "name": "name", "type": "string", "required": true }],
    "returns": { "type": "object", "properties": { "message": { "type": "string" } } }
  }]
}"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
  let port = env::var("PORT")
    .unwrap_or_else(|_| "0".to_owned())
    .parse::<u16>()?;
  let tool = ExampleTool;
  let cli_arguments = CliArguments::from_env();
  let tool_args = tool.init_args(cli_arguments.parse()?)?;
  tool.init(&tool_args).await?;

  // OpenToolServer::router() deliberately leaves hosting and lifecycle to
  // the embedder. Share the same tool value and call stop() from both the
  // protocol endpoint and process shutdown so dispose runs exactly once.
  let server = Arc::new(
    OpenToolServer::builder(tool.clone())
      .host(host.clone())
      .port(port)
      .version(env!("CARGO_PKG_VERSION"))
      .spec_json(SPEC)
      .cli_arguments(CliArguments::new(std::iter::empty::<String>()))
      .build(),
  );

  let allowed_origins = [
    HeaderValue::from_static("vitul://localhost"),
    HeaderValue::from_static("http://vitul.localhost"),
    HeaderValue::from_static("https://vitul.localhost"),
    HeaderValue::from_static("http://localhost:1420"),
    HeaderValue::from_static("http://127.0.0.1:1420"),
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
  server: Arc<OpenToolServer<ExampleTool>>,
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
