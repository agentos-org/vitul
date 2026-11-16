use serde_json::{Value, json};

// These examples embed their OpenTool documents in Rust source. Inspect the
// documents they actually serve rather than a separate copy of each schema.
fn spec_from_source(source: &str, constant: &str) -> Value {
  let marker = format!("const {constant}: &str = r#\"");
  let json = source
    .split_once(&marker)
    .expect("OpenTool spec constant")
    .1
    .split_once("\"#;")
    .expect("end of OpenTool spec")
    .0;
  serde_json::from_str(json).expect("valid OpenTool JSON")
}

fn assert_schema(schema: &Value) {
  let object = schema.as_object().expect("schema must be an object");
  match object.get("type").and_then(Value::as_str) {
    Some("object") => {
      let properties = object
        .get("properties")
        .and_then(Value::as_object)
        .expect("object schema must describe its properties");
      for property in properties.values() {
        assert_schema(property);
      }
    }
    Some("array") => assert_schema(
      object
        .get("items")
        .expect("array schema must describe its items"),
    ),
    Some("string" | "number" | "integer" | "boolean") => {}
    _ => panic!("unsupported or missing schema type: {schema}"),
  }
}

fn assert_open_tool_parameter_and_return_schemas(spec: &Value) {
  assert_eq!(spec["opentool"], "1.0.0");
  assert!(spec["info"]["title"].is_string());
  assert!(spec["info"]["version"].is_string());
  let functions = spec["functions"].as_array().expect("functions array");
  assert!(!functions.is_empty());
  for function in functions {
    assert!(function["name"].is_string());
    assert!(function["description"].is_string());
    for parameter in function["parameters"].as_array().expect("parameters array") {
      assert!(parameter["name"].is_string());
      assert!(parameter["required"].is_boolean());
      assert!(parameter.get("type").is_none());
      assert_schema(&parameter["schema"]);
    }
    assert!(function.get("returns").is_none());
    let result = &function["return"];
    assert!(result["name"].is_string());
    assert_schema(&result["schema"]);
  }
}

#[test]
fn maintained_examples_expose_opentool_parameter_and_return_schemas() {
  let starter = spec_from_source(
    include_str!("../../../examples/starter/vitul-demo/backend/src/lib.rs"),
    "SPEC",
  );
  let fixture = spec_from_source(
    include_str!("../../../tests/fixtures/desktop-api/backend/src/main.rs"),
    "SPEC",
  );
  let video = spec_from_source(
    include_str!("../../../examples/agentos/process/frame-video/backend/src/lib.rs"),
    "SPEC",
  );
  let music = spec_from_source(
    include_str!("../../../examples/agentos/process/toney-music/backend/src/main.rs"),
    "OPENTOOL_SPEC",
  );

  for spec in [&starter, &fixture, &video, &music] {
    assert_open_tool_parameter_and_return_schemas(spec);
  }

  for spec in [&starter, &fixture] {
    assert_eq!(
      spec["functions"][0]["parameters"],
      json!([{ "name": "name", "schema": { "type": "string" }, "required": true }])
    );
    assert_eq!(
      spec["functions"][0]["return"]["schema"]["properties"]["message"]["type"],
      "string"
    );
  }
  assert_eq!(
    video["functions"][3]["parameters"][1]["schema"]["type"],
    "boolean"
  );
  assert_eq!(video["functions"][3]["parameters"][1]["required"], true);
  assert_eq!(
    music["functions"][0]["parameters"][0]["schema"]["type"],
    "string"
  );
  assert_eq!(music["functions"][0]["parameters"][0]["required"], false);
}
