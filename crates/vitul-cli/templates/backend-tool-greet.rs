use async_trait::async_trait;
use opentool_rust::{Tool, ToolError};
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub struct GreetTool;

#[async_trait]
impl Tool for GreetTool {
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
