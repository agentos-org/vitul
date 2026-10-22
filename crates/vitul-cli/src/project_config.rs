use crate::agentos_config::Config;
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::{fs, path::Path};

const CONFIG_FILE: &str = "vitul.conf.json";
const PROJECT_SCHEMA_VERSION: u64 = 2;

pub fn is_unified(root: &Path) -> bool {
  read(root)
    .ok()
    .and_then(|config| config.get("schemaVersion").and_then(Value::as_u64))
    == Some(PROJECT_SCHEMA_VERSION)
}

pub fn write_runtime_config(root: &Path) -> Result<()> {
  let path = root.join(CONFIG_FILE);
  if !path.is_file() {
    return Ok(());
  }
  let config = read(root)?;
  if config.get("schemaVersion").and_then(Value::as_u64) != Some(PROJECT_SCHEMA_VERSION) {
    return Ok(());
  }
  let mut runtime = config;
  let object = runtime
    .as_object_mut()
    .context("Vitul config must be a JSON object")?;
  object.remove("schemaVersion");
  object.remove("backend");
  object.remove("agentos");
  object.remove("description");
  object.remove("nameZh");
  object.remove("descriptionZh");
  crate::macos_icon::prepare_projection(root, &mut runtime)?;
  add_runtime_path_prefixes(&mut runtime);

  let runtime_path = root.join("src-vitul/vitul.conf.json");
  if let Some(parent) = runtime_path.parent() {
    fs::create_dir_all(parent).with_context(|| format!("could not create {}", parent.display()))?;
  }
  fs::write(&runtime_path, serde_json::to_vec_pretty(&runtime)?).with_context(|| {
    format!(
      "could not write generated runtime config {}",
      runtime_path.display()
    )
  })
}

pub fn create_project_config(root: &Path) -> Result<()> {
  let runtime_path = root.join("src-vitul/vitul.conf.json");
  let mut config: Value = serde_json::from_slice(
    &fs::read(&runtime_path)
      .with_context(|| format!("could not read generated config {}", runtime_path.display()))?,
  )
  .context("could not parse generated desktop config")?;
  desktop_fields_from_runtime(&mut config);
  let object = config
    .as_object_mut()
    .context("generated desktop config must be a JSON object")?;
  object.insert("schemaVersion".into(), json!(PROJECT_SCHEMA_VERSION));
  object.insert(
    "backend".into(),
    json!({
      "type": "opentool",
      "manifest": "backend/Cargo.toml",
      "binary": "vitul-backend"
    }),
  );
  object.insert(
    "agentos".into(),
    json!({
      "type": "process",
      "targets": ["agentos/arm64", "agentos/x64"],
      "buildImage": "rust:1.90-bookworm",
      "permissions": ["native.window-control"],
      "open": {
        "mode": "window",
        "titleBar": "system",
        "defaultWidth": 960,
        "defaultHeight": 680,
        "minWidth": 520,
        "minHeight": 360
      },
      "rpc": { "basePath": "/opentool", "discovery": "static" },
      "run": {
        "command": "./bin/vitul-backend",
        "args": [],
        "port": 9627,
        "healthCheck": "/opentool/version",
        "readyTimeout": 60,
        "frontendEntry": "/",
        "env": { "HOST": "0.0.0.0", "PORT": "9627" }
      },
      "build": {
        "steps": [{ "command": "npm", "args": ["run", "build"] }],
        "artifacts": [
          { "from": "dist", "to": "web" },
          { "from": "agentos-build/bin/vitul-backend", "to": "bin/vitul-backend" }
        ]
      },
      "data": { "persistent": false }
    }),
  );
  let path = root.join(CONFIG_FILE);
  fs::write(&path, serde_json::to_vec_pretty(&config)?)
    .with_context(|| format!("could not write {}", path.display()))?;
  fs::remove_file(&runtime_path).with_context(|| {
    format!(
      "could not remove generated config {}",
      runtime_path.display()
    )
  })?;
  Ok(())
}

pub fn package_config(root: &Path) -> Result<Config> {
  let config = read(root)?;
  if config.get("schemaVersion").and_then(Value::as_u64) != Some(PROJECT_SCHEMA_VERSION) {
    bail!(
      "{} is not a unified Vitul project config",
      root.join(CONFIG_FILE).display()
    );
  }
  let agentos = config
    .get("agentos")
    .and_then(Value::as_object)
    .context("vitul.conf.json must define an `agentos` object")?;
  let config_object = config
    .as_object()
    .context("vitul.conf.json must be a JSON object")?;
  let empty = Map::new();
  let open = agentos
    .get("open")
    .and_then(Value::as_object)
    .unwrap_or(&empty);
  let rpc = agentos
    .get("rpc")
    .and_then(Value::as_object)
    .unwrap_or(&empty);
  let has_native_profile = agentos
    .get("execution")
    .and_then(Value::as_object)
    .and_then(|execution| execution.get("native"))
    .is_some_and(|native| !native.is_null());
  if has_native_profile {
    // `open.multiInstance` is not forwarded by the root-config projection;
    // reject it explicitly for native profile v1 rather than silently dropping it.
    if let Some(multi_instance) = open.get("multiInstance")
      && multi_instance != &Value::Bool(false)
    {
      bail!("agentos.execution.native does not support open.multiInstance true");
    }
  }
  let targets = agentos
    .get("targets")
    .cloned()
    .unwrap_or_else(|| json!(["agentos/arm64"]));
  let app_type = agentos
    .get("type")
    .cloned()
    .unwrap_or_else(|| json!("process"));
  let icon = app_icon(&config);
  let mut app = Map::new();
  app.insert("type".into(), app_type);
  insert_clone(config_object, "identifier", &mut app, "id");
  insert_clone(config_object, "productName", &mut app, "name");
  insert_clone(config_object, "version", &mut app, "version");
  insert_clone(agentos, "nameZh", &mut app, "name_zh");
  insert_clone(agentos, "description", &mut app, "description");
  insert_clone(agentos, "descriptionZh", &mut app, "description_zh");
  if let Some(icon) = icon {
    app.insert("icon".into(), icon);
  }

  let mut package_agentos = Map::new();
  copy_or_default(
    agentos,
    "permissions",
    &mut package_agentos,
    "permissions",
    json!([]),
  );
  copy_or_default(
    agentos,
    "providers",
    &mut package_agentos,
    "providers",
    json!([]),
  );
  if let Some(deployment) = agentos.get("deployment") {
    package_agentos.insert("deployment".into(), deployment.clone());
  }
  if let Some(network) = agentos.get("network") {
    package_agentos.insert("network".into(), network.clone());
  }
  if let Some(execution) = agentos.get("execution") {
    package_agentos.insert("execution".into(), execution.clone());
  }
  copy_or_default(
    &open,
    "mode",
    &mut package_agentos,
    "openMode",
    json!("window"),
  );
  copy_or_default(
    &open,
    "titleBar",
    &mut package_agentos,
    "titleBar",
    json!("system"),
  );
  copy_or_default(
    &open,
    "defaultWidth",
    &mut package_agentos,
    "defaultWidth",
    json!(960),
  );
  copy_or_default(
    &open,
    "defaultHeight",
    &mut package_agentos,
    "defaultHeight",
    json!(680),
  );
  copy_or_default(
    &open,
    "minWidth",
    &mut package_agentos,
    "minWidth",
    json!(520),
  );
  copy_or_default(
    &open,
    "minHeight",
    &mut package_agentos,
    "minHeight",
    json!(360),
  );
  copy_or_default(
    &rpc,
    "basePath",
    &mut package_agentos,
    "rpcBasePath",
    json!("/opentool"),
  );
  copy_or_default(
    &rpc,
    "discovery",
    &mut package_agentos,
    "rpcDiscovery",
    json!("static"),
  );

  // Keep the documented legacy alias usable, but never silently ignore a conflict.
  let mut run = agentos.get("run").cloned().unwrap_or_else(|| json!({}));
  if let Some(legacy_port) = agentos.get("port") {
    if !legacy_port
      .as_u64()
      .is_some_and(|port| (1..=65535).contains(&port))
    {
      bail!("agentos.port must be an integer from 1 to 65535; use agentos.run.port instead");
    }
    let run_object = run
      .as_object_mut()
      .context("agentos.run must be an object")?;
    if let Some(port) = run_object.get("port") {
      if port != legacy_port {
        bail!(
          "agentos.port conflicts with agentos.run.port; remove agentos.port and keep the intended port in agentos.run.port"
        );
      }
    } else {
      run_object.insert("port".into(), legacy_port.clone());
    }
    eprintln!(
      "Warning: agentos.port is deprecated; use agentos.run.port and remove agentos.port. The backend listener must use the same port."
    );
  }

  let package_config = json!({
    "schemaVersion": 1,
    "targets": targets,
    "app": Value::Object(app),
    "build": agentos.get("build").cloned().unwrap_or_else(|| json!({})),
    "run": run,
    "web": agentos.get("web").cloned().unwrap_or_else(|| json!({})),
    "agentos": Value::Object(package_agentos),
    "data": agentos.get("data").cloned().unwrap_or_else(|| json!({}))
  });
  let parsed: Config = serde_path_to_error::deserialize(package_config).map_err(|error| {
    anyhow::anyhow!(
      "invalid AgentOS section in vitul.conf.json at `{}`: {}",
      error.path(),
      error.inner()
    )
  })?;
  parsed.validate(root)?;
  Ok(parsed)
}

pub fn root_config(root: &Path) -> Result<Value> {
  read(root)
}

pub(crate) fn is_generated_backend_artifact(root: &Path, source: &Path) -> bool {
  let Ok(config) = read(root) else {
    return false;
  };
  if config.get("schemaVersion").and_then(Value::as_u64) != Some(PROJECT_SCHEMA_VERSION) {
    return false;
  }
  let Some(backend) = config.get("backend") else {
    return false;
  };
  if backend.get("type").and_then(Value::as_str) != Some("opentool") {
    return false;
  }
  let Some(binary) = backend.get("binary").and_then(Value::as_str) else {
    return false;
  };
  if binary.is_empty() || binary.contains('/') || binary.contains('\\') || binary.contains('\0') {
    return false;
  }
  source == Path::new("agentos-build").join("bin").join(binary)
}

fn read(root: &Path) -> Result<Value> {
  let path = root.join(CONFIG_FILE);
  let bytes = fs::read(&path).with_context(|| format!("could not read {}", path.display()))?;
  serde_json::from_slice(&bytes).with_context(|| format!("could not parse {}", path.display()))
}

fn app_icon(config: &Value) -> Option<Value> {
  config
    .get("bundle")?
    .get("icon")?
    .as_array()
    .and_then(|icons| {
      icons.iter().find(|icon| {
        icon.as_str().is_some_and(|path| {
          let lower = path.to_ascii_lowercase();
          lower.ends_with(".png") || lower.ends_with(".svg")
        })
      })
    })
    .cloned()
    .or_else(|| {
      config
        .get("bundle")?
        .get("icon")?
        .as_str()
        .filter(|path| {
          let lower = path.to_ascii_lowercase();
          lower.ends_with(".png") || lower.ends_with(".svg")
        })
        .map(|path| json!(path))
    })
}

fn insert_clone(
  source: &Map<String, Value>,
  from: &str,
  target: &mut Map<String, Value>,
  to: &str,
) {
  if let Some(value) = source.get(from) {
    target.insert(to.to_owned(), value.clone());
  }
}

fn copy_or_default(
  source: &Map<String, Value>,
  from: &str,
  target: &mut Map<String, Value>,
  to: &str,
  default: Value,
) {
  target.insert(to.to_owned(), source.get(from).cloned().unwrap_or(default));
}

fn add_runtime_path_prefixes(config: &mut Value) {
  for path in ["build.frontendDist", "build.additionalWatchFolders"] {
    map_config_path(config, path, |value| prefix_path(value));
  }
  for path in ["bundle.icon", "bundle.resources", "bundle.externalBin"] {
    map_config_path(config, path, |value| prefix_path(value));
  }
}

fn remove_runtime_path_prefixes(config: &mut Value) {
  for path in ["build.frontendDist", "build.additionalWatchFolders"] {
    map_config_path(config, path, |value| unprefix_path(value));
  }
  for path in ["bundle.icon", "bundle.resources", "bundle.externalBin"] {
    map_config_path(config, path, |value| unprefix_path(value));
  }
}

fn map_config_path(config: &mut Value, dotted: &str, map: impl Fn(&str) -> String + Copy) {
  let mut current = config;
  let mut parts = dotted.split('.').peekable();
  while let Some(part) = parts.next() {
    if parts.peek().is_none() {
      let Some(value) = current.get_mut(part) else {
        return;
      };
      map_path_value(value, map);
      return;
    }
    let Some(next) = current.get_mut(part) else {
      return;
    };
    current = next;
  }
}

fn map_path_value(value: &mut Value, map: impl Fn(&str) -> String + Copy) {
  match value {
    Value::String(path) if !path.starts_with("http://") && !path.starts_with("https://") => {
      *path = map(path);
    }
    Value::Array(paths) => {
      for path in paths {
        map_path_value(path, map);
      }
    }
    _ => {}
  }
}

fn prefix_path(value: &str) -> String {
  if Path::new(value).is_absolute() || value.is_empty() {
    value.to_owned()
  } else if let Some(path) = value.strip_prefix("src-vitul/") {
    path.to_owned()
  } else {
    format!("../{value}")
  }
}

fn unprefix_path(value: &str) -> String {
  if let Some(path) = value.strip_prefix("../") {
    path.to_owned()
  } else if Path::new(value).is_absolute() || value.is_empty() {
    value.to_owned()
  } else {
    format!("src-vitul/{value}")
  }
}

pub fn desktop_fields_from_runtime(runtime: &mut Value) {
  remove_runtime_path_prefixes(runtime);
}

#[cfg(test)]
mod tests {
  use super::{package_config, write_runtime_config};
  use serde_json::{Value, json};
  use std::{fs, path::Path};

  #[test]
  fn runtime_config_is_projected_from_the_root_config() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("src-vitul")).unwrap();
    fs::write(
      root.path().join("vitul.conf.json"),
      serde_json::to_vec(&json!({
        "schemaVersion": 2,
        "productName": "Time Memo",
        "identifier": "com.litevar.agentos.timeMemo",
        "build": { "frontendDist": "dist" },
        "bundle": { "icon": ["src-vitul/icons/icon.png", "src-vitul/icons/icon.icns"] },
        "backend": { "type": "opentool" },
        "agentos": { "targets": ["agentos/arm64"] }
      }))
      .unwrap(),
    )
    .unwrap();

    write_runtime_config(root.path()).unwrap();

    let runtime: Value =
      serde_json::from_slice(&fs::read(root.path().join("src-vitul/vitul.conf.json")).unwrap())
        .unwrap();
    assert_eq!(runtime["build"]["frontendDist"], "../dist");
    assert_eq!(runtime["bundle"]["icon"][0], "icons/icon.png");
    assert_eq!(runtime["bundle"]["icon"][1], "icons/icon.icns");
    assert_eq!(runtime["productName"], "Time Memo");
    assert_eq!(runtime["identifier"], "com.litevar.agentos.timeMemo");
    assert!(runtime.get("schemaVersion").is_none());
    assert!(runtime.get("backend").is_none());
    assert!(runtime.get("agentos").is_none());
  }

  #[test]
  fn auto_macos_icon_is_generated_and_projected_with_host_relative_paths() {
    let root = tempfile::tempdir().unwrap();
    let icons = root.path().join("src-vitul/icons");
    fs::create_dir_all(&icons).unwrap();
    fs::copy(
      Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/desktop-api/.icons/icon.png"),
      icons.join("icon.png"),
    )
    .unwrap();
    let config = json!({
      "schemaVersion": 2,
      "build": {},
      "bundle": {
        "icon": ["src-vitul/icons/icon.png"],
        "macosIcon": {
          "mode": "auto",
          "kind": "logo",
          "background": "#ffffff",
          "logoScale": 72
        }
      },
      "backend": { "type": "opentool" },
      "agentos": {}
    });
    fs::write(
      root.path().join("vitul.conf.json"),
      serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();

    write_runtime_config(root.path()).unwrap();

    let runtime: Value =
      serde_json::from_slice(&fs::read(root.path().join("src-vitul/vitul.conf.json")).unwrap())
        .unwrap();
    assert_eq!(runtime["bundle"]["icon"][0], "icons/icon.png");
    assert_eq!(runtime["bundle"]["icon"][1], "icons/macos/icon.icns");
    assert!(runtime["bundle"].get("macosIcon").is_none());
    assert!(
      root
        .path()
        .join("src-vitul/icons/macos/icon.icns")
        .is_file()
    );
  }

  #[test]
  fn unified_config_feeds_the_agentos_packager() {
    let root = tempfile::tempdir().unwrap();
    let config = json!({
      "schemaVersion": 2,
      "productName": "Demo",
      "version": "0.1.0",
      "identifier": "com.example.demo",
      "backend": { "type": "opentool", "manifest": "backend/Cargo.toml", "binary": "vitul-backend" },
      "agentos": {
        "type": "process",
        "targets": ["agentos/arm64", "agentos/x64"],
        "build": {
          "steps": [{ "command": "npm", "args": ["run", "build"] }],
          "artifacts": [
            { "from": "dist", "to": "web" },
            { "from": "agentos-build/bin/vitul-backend", "to": "bin/vitul-backend" }
          ]
        },
        "run": {
          "command": "./bin/vitul-backend",
          "port": 9627,
          "healthCheck": "/opentool/version"
        }
      }
    });
    fs::write(
      root.path().join("vitul.conf.json"),
      serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();

    let parsed = package_config(root.path()).unwrap();
    assert_eq!(parsed.app.name, "Demo");
    assert_eq!(parsed.app.id, "com.example.demo");
    assert_eq!(parsed.target_summary(), "agentos/arm64, agentos/x64");
    assert_eq!(parsed.run.command.as_deref(), Some("./bin/vitul-backend"));
  }

  #[test]
  fn unified_config_allows_the_backend_binary_generated_by_vitul() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("public")).unwrap();
    fs::write(root.path().join("start.sh"), "#!/bin/sh\n").unwrap();
    let config = json!({
      "schemaVersion": 2,
      "productName": "WinTone",
      "version": "0.1.0",
      "identifier": "com.agentos.wintone",
      "backend": {
        "type": "opentool",
        "manifest": "backend/Cargo.toml",
        "binary": "wintone-server"
      },
      "agentos": {
        "type": "process",
        "targets": ["agentos/arm64", "agentos/x64"],
        "build": {
          "artifacts": [
            { "from": "public", "to": "public" },
            { "from": "start.sh", "to": "start.sh" },
            {
              "from": "agentos-build/bin/wintone-server",
              "to": "wintone-server"
            }
          ]
        },
        "run": {
          "command": "./start.sh",
          "port": 3000,
          "healthCheck": "/health"
        }
      }
    });
    fs::write(
      root.path().join("vitul.conf.json"),
      serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();

    let parsed = package_config(root.path()).unwrap();
    assert_eq!(parsed.app.name, "WinTone");
    assert_eq!(parsed.run.command.as_deref(), Some("./start.sh"));
  }
}
