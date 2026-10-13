use serde_json::{Value, json};
use std::{
  fs,
  path::Path,
  process::{Command, Output},
};
use tempfile::TempDir;

const VITUL: &str = env!("CARGO_BIN_EXE_vitul");

fn project() -> (TempDir, Value) {
  let root = tempfile::tempdir().expect("create project directory");
  fs::create_dir_all(root.path().join("dist")).expect("create frontend artifact");
  fs::create_dir_all(root.path().join("service")).expect("create service artifact");
  fs::write(
    root.path().join("dist/index.html"),
    "<main>Generic App</main>",
  )
  .expect("write frontend");
  fs::write(
    root.path().join("service/server.mjs"),
    "export {}; // Generic process fixture\n",
  )
  .expect("write process entry");
  let config = json!({
    "schemaVersion": 2,
    "productName": "Generic Service",
    "identifier": "org.example.generic-service",
    "version": "1.0.0",
    "agentos": {
      "type": "process",
      "targets": ["agentos/arm64", "agentos/x64"],
      "build": {
        "steps": [],
        "artifacts": [
          { "from": "dist", "to": "web" },
          { "from": "service", "to": "service" }
        ]
      },
      "run": {
        "command": "node",
        "args": ["service/server.mjs"],
        "port": 18420,
        "healthCheck": "/health",
        "readyTimeout": 30,
        "autoStart": false,
        "frontendEntry": "/",
        "env": {}
      },
      "data": { "persistent": false }
    }
  });
  (root, config)
}

fn write_config(root: &Path, config: &Value) {
  fs::write(
    root.join("vitul.conf.json"),
    serde_json::to_vec_pretty(config).expect("encode config"),
  )
  .expect("write config");
}

fn invoke(root: &Path, args: &[&str]) -> Output {
  Command::new(VITUL)
    .args(args)
    .current_dir(root)
    .output()
    .expect("run the Vitul CLI")
}

fn assert_cli_success(output: Output, context: &str) {
  assert!(
    output.status.success(),
    "{context}\nstdout: {}\nstderr: {}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

fn package_manifest(root: &Path, architecture: &str) -> Value {
  let archive = root.join(format!(
    "dist/generic-service-1.0.0-agentos-{architecture}.agt"
  ));
  assert!(
    archive.is_file(),
    "missing generated package {}",
    archive.display()
  );
  let output = Command::new("tar")
    .args(["-xOzf"])
    .arg(archive)
    .arg("./app.json")
    .output()
    .expect("read app.json from the generated AGT");
  assert!(
    output.status.success(),
    "could not read packaged app.json: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  serde_json::from_slice(&output.stdout).expect("parse packaged app.json")
}

fn native_profile(config: &mut Value) -> &mut Value {
  config["agentos"]["execution"] = json!({
    "mode": "native",
    "native": {
      "version": 1,
      "identity": "desktop-user",
      "scope": "agentos-account"
    }
  });
  &mut config["agentos"]["execution"]["native"]
}

#[test]
fn official_build_embeds_the_complete_profile_in_arm64_and_x64_packages() {
  let (root, mut config) = project();
  config["agentos"]["data"]["persistent"] = json!(true);
  let native = native_profile(&mut config);
  native["lastWindowClose"] = json!("background");
  native["stopGracePeriodMs"] = json!(20000);
  write_config(root.path(), &config);

  let inspect = invoke(root.path(), &["agentos", "inspect"]);
  assert_cli_success(inspect, "agentos inspect should accept the native profile");

  assert_cli_success(
    invoke(root.path(), &["build", "--target", "agentos"]),
    "vitul build should create both AgentOS packages",
  );

  let expected_profile = json!({
    "version": 1,
    "identity": "desktop-user",
    "scope": "agentos-account",
    "lastWindowClose": "background",
    "stopGracePeriodMs": 20000
  });
  for architecture in ["arm64", "x64"] {
    let manifest = package_manifest(root.path(), architecture);
    assert_eq!(manifest["manifestVersion"], 1);
    assert_eq!(manifest["appType"], "process");
    assert_eq!(manifest["architecture"], architecture);
    assert_eq!(
      manifest["execution"],
      json!({ "mode": "native", "native": expected_profile })
    );
    assert_eq!(manifest["data"]["persistent"], true);
    assert_eq!(manifest["run"]["type"], "process");
    assert_eq!(manifest["run"]["autoStart"], false);
    assert_eq!(manifest["run"]["healthCheck"], "/health");
    assert_eq!(manifest["frontend"]["entry"], "/");
    assert_ne!(manifest["open"]["multiInstance"], true);
  }
}

#[test]
fn profile_defaults_are_written_and_legacy_native_config_is_rejected() {
  let (root, mut config) = project();
  config["agentos"]["targets"] = json!(["agentos/arm64"]);
  config["agentos"]["data"]["persistent"] = json!(true);
  native_profile(&mut config);
  write_config(root.path(), &config);
  assert_cli_success(
    invoke(root.path(), &["build", "--target", "agentos/arm64"]),
    "profile defaults should be accepted by the official build command",
  );
  let manifest = package_manifest(root.path(), "arm64");
  assert_eq!(
    manifest["execution"],
    json!({
      "mode": "native",
      "native": {
        "version": 1,
        "identity": "desktop-user",
        "scope": "agentos-account",
        "lastWindowClose": "stop",
        "stopGracePeriodMs": 5000
      }
    })
  );

  let (legacy_root, mut legacy) = project();
  legacy["agentos"]["execution"] = json!({ "mode": "native" });
  write_config(legacy_root.path(), &legacy);
  let validate = invoke(legacy_root.path(), &["agentos", "validate"]);
  assert!(
    !validate.status.success(),
    "mode-only native config must be rejected"
  );
  let build = invoke(legacy_root.path(), &["build", "--target", "agentos"]);
  assert!(
    !build.status.success(),
    "mode-only native config must not produce a legacy manifest"
  );
  assert!(
    !legacy_root
      .path()
      .join("dist/generic-service-1.0.0-agentos-arm64.agt")
      .exists(),
    "rejected legacy config must not leave an arm64 package"
  );
}

#[test]
fn container_process_and_remote_web_packages_keep_their_existing_manifests() {
  let (root, config) = project();
  write_config(root.path(), &config);
  assert_cli_success(
    invoke(root.path(), &["build", "--target", "agentos/arm64"]),
    "container process package should still build",
  );
  let manifest = package_manifest(root.path(), "arm64");
  assert_eq!(manifest["appType"], "process");
  assert!(manifest.get("execution").is_none());
  assert_eq!(manifest["run"]["autoStart"], false);

  let (remote_root, mut remote) = project();
  remote["productName"] = json!("Remote Web Fixture");
  remote["identifier"] = json!("org.example.remote-web-fixture");
  remote["agentos"]["type"] = json!("remote-web");
  remote["agentos"]["web"] = json!({ "url": "https://example.org/" });
  remote["agentos"].as_object_mut().unwrap().remove("run");
  remote["agentos"].as_object_mut().unwrap().remove("build");
  remote["agentos"].as_object_mut().unwrap().remove("data");
  write_config(remote_root.path(), &remote);
  assert_cli_success(
    invoke(remote_root.path(), &["build", "--target", "agentos"]),
    "remote-web package should still build",
  );
  let archive = remote_root
    .path()
    .join("dist/remote-web-fixture-1.0.0-agentos-any.agt");
  let output = Command::new("tar")
    .args(["-xOzf"])
    .arg(archive)
    .arg("./app.json")
    .output()
    .expect("read remote-web app.json");
  assert!(output.status.success());
  let manifest: Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(manifest["appType"], "url");
  assert_eq!(manifest["architecture"], "any");
  assert_eq!(manifest["open"]["url"], "https://example.org/");
  assert!(manifest.get("execution").is_none());
  assert!(manifest.get("run").is_none());
}

#[test]
fn invalid_native_profiles_and_incompatible_options_are_rejected() {
  let mut cases: Vec<(String, Box<dyn Fn(&mut Value)>)> = Vec::new();
  cases.push((
    "mode missing".to_owned(),
    Box::new(|config| {
      native_profile(config);
      config["agentos"]["execution"]
        .as_object_mut()
        .unwrap()
        .remove("mode");
    }),
  ));
  cases.push((
    "legacy mode without profile".to_owned(),
    Box::new(|config| {
      config["agentos"]["execution"] = json!({ "mode": "native" });
    }),
  ));
  cases.push((
    "container mode".to_owned(),
    Box::new(|config| {
      native_profile(config);
      config["agentos"]["execution"]["mode"] = json!("container");
    }),
  ));
  cases.push((
    "version 2".to_owned(),
    Box::new(|config| {
      native_profile(config)["version"] = json!(2);
    }),
  ));
  cases.push((
    "non-integer version".to_owned(),
    Box::new(|config| {
      native_profile(config)["version"] = json!(1.0);
    }),
  ));
  cases.push((
    "wrong identity".to_owned(),
    Box::new(|config| {
      native_profile(config)["identity"] = json!("root");
    }),
  ));
  cases.push((
    "wrong scope".to_owned(),
    Box::new(|config| {
      native_profile(config)["scope"] = json!("desktop-session");
    }),
  ));
  cases.push((
    "invalid close policy".to_owned(),
    Box::new(|config| {
      native_profile(config)["lastWindowClose"] = json!("quit");
    }),
  ));
  cases.push((
    "negative grace period".to_owned(),
    Box::new(|config| {
      native_profile(config)["stopGracePeriodMs"] = json!(-1);
    }),
  ));
  cases.push((
    "grace period too long".to_owned(),
    Box::new(|config| {
      native_profile(config)["stopGracePeriodMs"] = json!(30001);
    }),
  ));
  cases.push((
    "unknown native field".to_owned(),
    Box::new(|config| {
      native_profile(config)["tray"] = json!(true);
    }),
  ));
  cases.push((
    "non-persistent data".to_owned(),
    Box::new(|config| {
      config["agentos"]["data"]["persistent"] = json!(false);
      native_profile(config);
    }),
  ));
  cases.push((
    "autostart".to_owned(),
    Box::new(|config| {
      native_profile(config);
      config["agentos"]["run"]["autoStart"] = json!(true);
    }),
  ));
  cases.push((
    "multi-instance".to_owned(),
    Box::new(|config| {
      native_profile(config);
      config["agentos"]["open"]["multiInstance"] = json!(true);
    }),
  ));
  cases.push((
    "deployment".to_owned(),
    Box::new(|config| {
      native_profile(config);
      config["agentos"]["deployment"] = json!({ "protocolVersion": 1, "operations": [] });
    }),
  ));
  cases.push((
    "Host Providers".to_owned(),
    Box::new(|config| {
      native_profile(config);
      config["agentos"]["providers"] =
        json!([{ "id": "org.example.host", "methods": ["host.get"] }]);
    }),
  ));
  cases.push((
    "external Wayland".to_owned(),
    Box::new(|config| {
      native_profile(config);
      config["agentos"]["permissions"] = json!(["native.external-wayland"]);
    }),
  ));

  for reserved in [
    "HOME",
    "USER",
    "LOGNAME",
    "PATH",
    "TMPDIR",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "WAYLAND_SOCKET",
    "XAUTHORITY",
    "XDG_SESSION_ID",
    "AGENTOS_SESSION_ID",
    "AGENTOS_TRAY_SOCKET",
    "LD_PRELOAD",
    "LD_AUDIT",
  ] {
    let key = reserved.to_owned();
    let label = format!("reserved environment variable {reserved}");
    cases.push((
      label,
      Box::new(move |config| {
        native_profile(config);
        config["agentos"]["data"]["persistent"] = json!(true);
        config["agentos"]["run"]["env"]
          .as_object_mut()
          .unwrap()
          .insert(key.clone(), json!("override"));
      }),
    ));
  }

  for (name, mutate) in cases {
    let (root, mut config) = project();
    config["agentos"]["data"]["persistent"] = json!(true);
    mutate(&mut config);
    write_config(root.path(), &config);
    let output = invoke(root.path(), &["agentos", "validate"]);
    assert!(
      !output.status.success(),
      "{name} must be rejected\nstdout: {}\nstderr: {}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
  }

  let (remote_root, mut remote) = project();
  remote["agentos"]["type"] = json!("remote-web");
  remote["agentos"]["web"] = json!({ "url": "https://example.org/" });
  remote["agentos"].as_object_mut().unwrap().remove("run");
  remote["agentos"].as_object_mut().unwrap().remove("build");
  remote["agentos"]["data"]["persistent"] = json!(true);
  native_profile(&mut remote);
  write_config(remote_root.path(), &remote);
  let output = invoke(remote_root.path(), &["agentos", "validate"]);
  assert!(
    !output.status.success(),
    "native profile must reject remote-web apps"
  );
}

#[test]
fn grace_period_endpoints_are_valid() {
  for grace_period in [0, 30000] {
    let (root, mut config) = project();
    config["agentos"]["data"]["persistent"] = json!(true);
    native_profile(&mut config)["stopGracePeriodMs"] = json!(grace_period);
    write_config(root.path(), &config);
    assert_cli_success(
      invoke(root.path(), &["agentos", "validate"]),
      &format!("grace period {grace_period} should be valid"),
    );
  }
}
