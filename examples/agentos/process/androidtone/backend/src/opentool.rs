use async_trait::async_trait;
use opentool_rust::{CliArguments, OpenToolServer, Tool, ToolError};
use serde_json::{json, Value};

use crate::provider::ProviderClient;

const OPENTOOL_SPEC: &str = r#"{
  "opentool": "1.0.0",
  "info": { "title": "AndroidTone", "version": "0.1.6" },
  "functions": [
    {"name":"runtime.status","description":"Get Waydroid installation, kernel, initialization, and session status.","parameters":[],"returns":{"type":"object","properties":{"state":{"type":"string"},"message":{"type":"string"}}}},
    {"name":"runtime.install","description":"Install the bundled Android runtime through a declared AgentOS system deployment operation when the complete platform contract is available.","parameters":[],"returns":{"type":"object"}},
    {"name":"runtime.initialize","description":"Initialize the bundled Android system images through a declared AgentOS system deployment operation when available.","parameters":[],"returns":{"type":"object"}},
    {"name":"runtime.recover","description":"Request AgentOS Shell authorization to prepare Binder and restart the fixed Waydroid service without installing packages.","parameters":[],"returns":{"type":"object"}},
    {"name":"runtime.start","description":"Start the Android runtime and request its private AgentOS Wayland session when the platform contract is available.","parameters":[],"returns":{"type":"object"}},
    {"name":"runtime.stop","description":"Stop the AndroidTone Waydroid display session.","parameters":[],"returns":{"type":"object"}},
    {"name":"apps.list","description":"List installed Android applications managed by AndroidTone.","parameters":[],"returns":{"type":"object","properties":{"apps":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"name":{"type":"string"},"version":{"type":"string"}}}}}}},
    {"name":"installations.list","description":"List imported APK installation tasks.","parameters":[],"returns":{"type":"object","properties":{"installTasks":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"fileName":{"type":"string"},"status":{"type":"string"},"message":{"type":"string"}}}}}}}
  ]
}"#;

#[derive(Clone)]
pub struct AndroidToneTool {
  pub provider: ProviderClient,
  pub repository: crate::storage::Repository,
}

#[async_trait]
impl Tool for AndroidToneTool {
  async fn call(&self, method: &str, _params: Value) -> Result<Value, ToolError> {
    match method {
      "runtime.status" => Ok(match self.provider.status().await {
        Ok(value) => value,
        Err(error) => crate::provider::unavailable_status(Some(&error)),
      }),
      "apps.list" => Ok(match self.provider.apps().await {
        Ok(value) => value,
        Err(error) => json!({ "available": false, "error": error.message, "apps": [] }),
      }),
      "installations.list" => {
        let local = self.repository.snapshot().await.install_tasks;
        Ok(match self.provider.install_tasks().await {
          Ok(value) => {
            let mut merged = local
              .into_iter()
              .map(|task| {
                (
                  task.id.clone(),
                  serde_json::to_value(task).unwrap_or(Value::Null),
                )
              })
              .collect::<std::collections::BTreeMap<_, _>>();
            if let Some(remote_tasks) = value["installTasks"].as_array() {
              for task in remote_tasks {
                if let Some(id) = task["id"].as_str() {
                  merged.insert(id.to_owned(), task.clone());
                }
              }
            }
            json!({ "installTasks": merged.into_values().collect::<Vec<_>>() })
          }
          Err(_) => json!({ "installTasks": local }),
        })
      }
      "runtime.install" | "runtime.initialize" | "runtime.start" | "runtime.recover"
      | "runtime.stop" => self
        .provider
        .invoke(method, json!({}))
        .await
        .map_err(|error| ToolError::execution(error.message)),
      _ => Err(ToolError::method_not_found(method)),
    }
  }
}

pub async fn create_server(
  tool: AndroidToneTool,
  host: String,
  port: u16,
) -> Result<OpenToolServer<AndroidToneTool>, Box<dyn std::error::Error + Send + Sync>> {
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

#[cfg(test)]
mod tests {
  use super::OPENTOOL_SPEC;

  #[test]
  fn reauthorization_is_not_exposed_as_an_opentool_action() {
    assert!(!OPENTOOL_SPEC.contains("runtime.reauthorize"));
    assert!(!OPENTOOL_SPEC.contains("authorization.reauthorize"));
  }
}
