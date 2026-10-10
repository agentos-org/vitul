use async_trait::async_trait;
use opentool_rust::{CliArguments, OpenToolServer, Tool, ToolError};
use serde_json::{json, Value};

use crate::AppState;

const OPENTOOL_SPEC: &str = r#"{
  "opentool": "1.0.0",
  "info": { "title": "WinTone", "version": "0.1.0" },
  "functions": [
    {
      "name": "runtime.status",
      "description": "Get WinTone runtime and AgentOS window integration status.",
      "parameters": [],
      "returns": {
        "type": "object",
        "properties": {
          "state": { "type": "string" },
          "wineVersion": { "type": "string" },
          "fexVersion": { "type": "string" },
          "message": { "type": "string" }
        }
      }
    },
    {
      "name": "installations.list",
      "description": "List automatic Windows application installation tasks in WinTone.",
      "parameters": [],
      "returns": {
        "type": "object",
        "properties": {
          "installTasks": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "id": { "type": "string" },
                "fileName": { "type": "string" },
                "extension": { "type": "string" },
                "bytes": { "type": "number" },
                "sha256": { "type": "string" },
                "addedAt": { "type": "number" },
                "status": { "type": "string" },
                "phase": { "type": "string" },
                "message": { "type": "string" }
              }
            }
          }
        }
      }
    },
    {
      "name": "apps.list",
      "description": "List installed Windows applications managed by WinTone.",
      "parameters": [],
      "returns": {
        "type": "object",
        "properties": {
          "apps": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "version": { "type": "string" },
                "installedAt": { "type": "number" }
              }
            }
          }
        }
      }
    }
  ]
}"#;

#[derive(Clone)]
pub struct WinToneTool {
  pub state: AppState,
}

#[async_trait]
impl Tool for WinToneTool {
  async fn call(&self, method: &str, _params: Value) -> Result<Value, ToolError> {
    match method {
      "runtime.status" => Ok(
        serde_json::to_value(self.state.runtime_manager.status().await)
          .map_err(|error| ToolError::execution(error.to_string()))?,
      ),
      "installations.list" => self
        .state
        .snapshot()
        .await
        .map(|store| json!({ "installTasks": store.install_tasks }))
        .map_err(|error| ToolError::execution(error.0)),
      "apps.list" => self
        .state
        .snapshot()
        .await
        .map(|store| {
          json!({
            "apps": store.installed_apps.iter().map(|app| json!({
              "id": app.id,
              "name": app.name,
              "version": app.version,
              "installedAt": app.installed_at,
            })).collect::<Vec<_>>()
          })
        })
        .map_err(|error| ToolError::execution(error.0)),
      _ => Err(ToolError::method_not_found(method)),
    }
  }
}

pub async fn create_server(
  tool: WinToneTool,
  host: String,
  port: u16,
) -> Result<OpenToolServer<WinToneTool>, Box<dyn std::error::Error + Send + Sync>> {
  let arguments = CliArguments::new(std::iter::empty::<String>());
  let tool_arguments = tool.init_args(arguments.parse()?)?;
  opentool_rust::schema::load_json(OPENTOOL_SPEC)?;
  tool.init(&tool_arguments).await?;
  Ok(
    OpenToolServer::builder(tool)
      .host(host)
      .port(port)
      .version(env!("CARGO_PKG_VERSION"))
      .spec_json(OPENTOOL_SPEC)
      .cli_arguments(CliArguments::new(std::iter::empty::<String>()))
      .max_request_body_bytes(1024 * 1024)
      .build(),
  )
}
