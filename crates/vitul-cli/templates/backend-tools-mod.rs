mod greet;

pub use greet::GreetTool;

pub const SPEC: &str = r#"{
  "opentool": "1.0.0",
  "info": { "title": "Vitul App", "version": "0.1.0" },
  "functions": [{
    "name": "greet",
    "description": "Return a greeting for a name.",
    "parameters": [{ "name": "name", "type": "string", "required": true }],
    "returns": { "type": "object", "properties": { "message": { "type": "string" } } }
  }]
}"#;
