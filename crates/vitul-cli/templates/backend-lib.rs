use async_trait::async_trait;
use opentool_rust::{CliArguments, OpenToolServer, Tool, ToolError};
use serde_json::{json, Value};

#[derive(Clone, Default)]
pub struct ExampleTool;

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
  "info": { "title": "Vitul App", "version": "0.1.0" },
  "functions": [{
    "name": "greet",
    "description": "Return a greeting for a name.",
    "parameters": [{ "name": "name", "type": "string", "required": true }],
    "returns": { "type": "object", "properties": { "message": { "type": "string" } } }
  }]
}"#;

pub type VitulOpenToolServer = OpenToolServer<ExampleTool>;

/// Creates an initialized OpenTool server that can be hosted over HTTP or
/// dispatched in-process through another transport such as native IPC.
pub async fn create_server(
  arguments: impl IntoIterator<Item = String>,
  host: String,
  port: u16,
) -> Result<VitulOpenToolServer, Box<dyn std::error::Error + Send + Sync>> {
  let tool = ExampleTool;
  let cli_arguments = CliArguments::new(arguments);
  let tool_args = tool.init_args(cli_arguments.parse()?)?;
  opentool_rust::schema::load_json(SPEC)?;
  tool.init(&tool_args).await?;

  Ok(
    OpenToolServer::builder(tool)
      .host(host)
      .port(port)
      .version(env!("CARGO_PKG_VERSION"))
      .spec_json(SPEC)
      .cli_arguments(CliArguments::new(std::iter::empty::<String>()))
      .build(),
  )
}
