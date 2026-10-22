use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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

fn package_file(root: &Path, architecture: &str, path: &str) -> Vec<u8> {
  let archive = root.join(format!(
    "dist/generic-service-1.0.0-agentos-{architecture}.agt"
  ));
  let output = Command::new("tar")
    .args(["-xOzf"])
    .arg(archive)
    .arg(format!("./{path}"))
    .output()
    .expect("read a file from the generated AGT");
  assert!(
    output.status.success(),
    "could not read packaged {path}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  output.stdout
}

fn sha256(bytes: &[u8]) -> String {
  format!("{:x}", Sha256::digest(bytes))
}

fn write_network_executable(root: &Path, path: &str, bytes: &[u8]) {
  let path = root.join(path);
  fs::create_dir_all(path.parent().unwrap()).expect("create executable directory");
  fs::write(&path, bytes).expect("write network executable");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
      .expect("make network executable executable");
  }
}

fn proxy_network_config() -> Value {
  json!({
    "protocolVersion": 1,
    "executables": [{ "id": "network-client", "path": "bin/network-client" }],
    "proxyProfiles": [{
      "id": "account-proxy",
      "executable": "network-client",
      "http": { "host": "proxy.example.invalid", "port": 8080 },
      "https": { "host": "proxy.example.invalid", "port": 8443 },
      "socks": { "host": "127.0.0.1", "port": 1080 },
      "bypass": ["localhost", "127.0.0.0/8", "::1"]
    }]
  })
}

fn tun_network_config() -> Value {
  json!({
    "protocolVersion": 1,
    "executables": [{ "id": "tunnel-kernel", "path": "network/tunnel-kernel" }],
    "tunProfiles": [{
      "id": "full-tunnel",
      "executable": "tunnel-kernel",
      "arguments": ["--config", "${config}"],
      "configuration": { "source": "runtime-snapshot", "maxBytes": 4194304 },
      "interfaceName": "app-tun0",
      "ipv4": true,
      "ipv6": true,
      "exitPolicy": "restore-previous"
    }]
  })
}

fn enable_native_network(config: &mut Value, permissions: &[&str], network: Value) {
  native_profile(config);
  config["agentos"]["data"]["persistent"] = json!(true);
  config["agentos"]["permissions"] = json!(permissions);
  config["agentos"]["network"] = network;
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

fn enable_native_system_deployment(config: &mut Value, methods: &[&str]) {
  native_profile(config);
  config["agentos"]["data"]["persistent"] = json!(true);
  config["agentos"]["permissions"] = json!(["native.providers", "native.system-deployment"]);
  config["agentos"]["providers"] = json!([
    { "id": "org.agentos.system-deployment", "methods": methods }
  ]);
  config["agentos"]["deployment"] = json!({
    "protocolVersion": 1,
    "operations": [{
      "id": "runtime-setup",
      "version": 1,
      "title": "Prepare runtime",
      "summary": "Prepare the example runtime.",
      "parameters": {},
      "actions": []
    }]
  });
}

#[test]
fn system_deployment_reauthorization_methods_are_optional_as_a_pair_and_preserved() {
  const REQUIRED_METHODS: &[&str] = &[
    "capabilities.get",
    "operation.plan",
    "authorization.status",
    "task.get",
    "task.cancel",
    "task.events",
    "resources.list",
  ];
  const OPTIONAL_METHODS: &[&str] = &[
    "authorization.reauthorize",
    "authorization.reauthorization.get",
  ];

  let (legacy_root, mut legacy) = project();
  enable_native_system_deployment(&mut legacy, REQUIRED_METHODS);
  write_config(legacy_root.path(), &legacy);
  assert_cli_success(
    invoke(legacy_root.path(), &["agentos", "validate"]),
    "the legacy system-deployment method list should remain valid",
  );
  assert_cli_success(
    invoke(legacy_root.path(), &["build", "--target", "agentos/arm64"]),
    "the legacy system-deployment method list should remain buildable",
  );
  let legacy_manifest = package_manifest(legacy_root.path(), "arm64");
  assert_eq!(
    legacy_manifest["providers"][0]["methods"],
    json!(REQUIRED_METHODS),
    "the legacy provider methods should be retained in app.json"
  );

  let (reauthorization_root, mut reauthorization) = project();
  let mut methods = REQUIRED_METHODS.to_vec();
  methods.extend_from_slice(OPTIONAL_METHODS);
  enable_native_system_deployment(&mut reauthorization, &methods);
  write_config(reauthorization_root.path(), &reauthorization);
  assert_cli_success(
    invoke(
      reauthorization_root.path(),
      &["build", "--target", "agentos/arm64"],
    ),
    "the paired system-deployment reauthorization methods should be accepted",
  );
  let reauthorization_manifest = package_manifest(reauthorization_root.path(), "arm64");
  assert_eq!(
    reauthorization_manifest["providers"][0]["methods"],
    json!(methods),
    "the optional provider methods should be retained in app.json"
  );

  for method in OPTIONAL_METHODS {
    let (root, mut config) = project();
    let mut methods = REQUIRED_METHODS.to_vec();
    methods.push(method);
    enable_native_system_deployment(&mut config, &methods);
    write_config(root.path(), &config);
    let output = invoke(root.path(), &["agentos", "validate"]);
    assert!(
      !output.status.success(),
      "declaring only {method} must be rejected\nstdout: {}\nstderr: {}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
  }

  let (unknown_root, mut unknown) = project();
  let mut methods = REQUIRED_METHODS.to_vec();
  methods.push("authorization.unknown");
  enable_native_system_deployment(&mut unknown, &methods);
  write_config(unknown_root.path(), &unknown);
  let output = invoke(unknown_root.path(), &["agentos", "validate"]);
  assert!(
    !output.status.success(),
    "unknown system-deployment methods must be rejected\nstdout: {}\nstderr: {}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn native_wayland_and_custom_providers_validate_and_survive_packaging() {
  for with_provider in [false, true] {
    let (root, mut config) = project();
    native_profile(&mut config);
    config["agentos"]["data"]["persistent"] = json!(true);
    config["agentos"]["permissions"] = json!(["native.external-wayland"]);
    if with_provider {
      config["agentos"]["permissions"] = json!(["native.external-wayland", "native.providers"]);
      config["agentos"]["providers"] = json!([
        { "id": "org.example.host", "methods": ["host.get"] },
        { "id": "org.agentos.external-wayland", "methods": [
          "session.start", "session.prepare_restart", "session.restart", "session.stop"
        ] }
      ]);
    }
    write_config(root.path(), &config);
    assert_cli_success(
      invoke(root.path(), &["agentos", "validate"]),
      "native Wayland config should validate",
    );
    assert_cli_success(
      invoke(root.path(), &["build", "--target", "agentos"]),
      "native Wayland config should build for both targets",
    );
    for architecture in ["arm64", "x64"] {
      let manifest = package_manifest(root.path(), architecture);
      assert_eq!(manifest["permissions"], config["agentos"]["permissions"]);
      if with_provider {
        assert_eq!(manifest["providers"], config["agentos"]["providers"]);
      } else {
        assert!(manifest.get("providers").is_none());
      }
    }
  }
}

#[test]
fn invalid_wayland_provider_declarations_are_rejected() {
  let methods = json!([
    "session.start",
    "session.prepare_restart",
    "session.restart",
    "session.stop"
  ]);
  for case in [
    "missing permission",
    "missing method",
    "unknown method",
    "executable",
  ] {
    let (root, mut config) = project();
    native_profile(&mut config);
    config["agentos"]["data"]["persistent"] = json!(true);
    config["agentos"]["permissions"] = json!(["native.providers", "native.external-wayland"]);
    config["agentos"]["providers"] =
      json!([{ "id": "org.agentos.external-wayland", "methods": methods }]);
    match case {
      "missing permission" => config["agentos"]["permissions"] = json!(["native.providers"]),
      "missing method" => {
        config["agentos"]["providers"][0]["methods"]
          .as_array_mut()
          .unwrap()
          .pop();
      }
      "unknown method" => {
        config["agentos"]["providers"][0]["methods"]
          .as_array_mut()
          .unwrap()
          .push(json!("session.unknown"));
      }
      "executable" => config["agentos"]["providers"][0]["executable"] = json!("service/server.mjs"),
      _ => unreachable!(),
    }
    write_config(root.path(), &config);
    let output = invoke(root.path(), &["agentos", "validate"]);
    assert!(!output.status.success(), "{case} must be rejected");
    assert!(String::from_utf8_lossy(&output.stderr).contains("org.agentos.external-wayland"));
  }
}

#[cfg(unix)]
#[test]
fn custom_provider_coexists_with_generated_network_provider() {
  let (root, mut config) = project();
  write_network_executable(root.path(), "network/client", b"network executable\n");
  config["agentos"]["build"]["artifacts"]
    .as_array_mut()
    .unwrap()
    .push(json!({ "from": "network/client", "to": "bin/network-client" }));
  enable_native_network(
    &mut config,
    &["native.providers", "native.network", "native.system-proxy"],
    proxy_network_config(),
  );
  let provider = json!({ "id": "org.example.host", "methods": ["host.get"] });
  config["agentos"]["providers"] = json!([provider]);
  write_config(root.path(), &config);
  assert_cli_success(
    invoke(root.path(), &["agentos", "validate"]),
    "custom and network providers should validate together",
  );
  assert_cli_success(
    invoke(root.path(), &["build", "--target", "agentos"]),
    "custom and network providers should build together",
  );
  for architecture in ["arm64", "x64"] {
    let manifest = package_manifest(root.path(), architecture);
    let providers = manifest["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 2);
    assert!(providers.contains(&provider));
    assert_eq!(
      providers
        .iter()
        .filter(|item| item["id"] == "org.agentos.network")
        .count(),
      1
    );
  }
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
    "AGENTOS_PROVIDER_SOCKET",
    "AGENTOS_APP_RUNTIME_INSTANCE_ID",
    "AGENTOS_TUN_CONFIG_PATH",
    "AGENTOS_TUN_INTERFACE",
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
    if let Some(reserved) = name.strip_prefix("reserved environment variable ") {
      assert!(
        String::from_utf8_lossy(&output.stderr).contains(&format!("run.env.{reserved}")),
        "{name} should identify its reserved field path\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
      );
    }
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

#[cfg(unix)]
#[test]
fn native_proxy_network_validates_and_emits_the_builtin_provider_and_package_digest() {
  let (root, mut config) = project();
  write_network_executable(root.path(), "network/network-client", b"proxy executable\n");
  config["agentos"]["build"]["artifacts"]
    .as_array_mut()
    .unwrap()
    .push(json!({ "from": "network", "to": "bin" }));
  enable_native_network(
    &mut config,
    &["native.providers", "native.network", "native.system-proxy"],
    proxy_network_config(),
  );
  write_config(root.path(), &config);
  assert_cli_success(
    invoke(root.path(), &["agentos", "validate"]),
    "native proxy network config should validate",
  );
  let inspect = invoke(root.path(), &["agentos", "inspect"]);
  assert!(
    inspect.status.success(),
    "inspect should display the network declaration\n{}",
    String::from_utf8_lossy(&inspect.stderr)
  );
  let inspect = String::from_utf8_lossy(&inspect.stdout);
  assert!(inspect.contains("Network protocol version: 1"));
  assert!(inspect.contains("Network profile types: proxy"));
  assert!(inspect.contains("Network proxy profiles: 1"));
  assert!(inspect.contains("native.system-proxy"));

  assert_cli_success(
    invoke(root.path(), &["build", "--target", "agentos/arm64"]),
    "native proxy network package should build",
  );
  let manifest = package_manifest(root.path(), "arm64");
  assert_eq!(manifest["network"]["protocolVersion"], 1);
  assert_eq!(
    manifest["network"]["executables"][0]["path"],
    "bin/network-client"
  );
  assert_eq!(
    manifest["network"]["executables"][0]["sha256"],
    sha256(&package_file(root.path(), "arm64", "bin/network-client"))
  );
  assert_eq!(manifest["providers"].as_array().unwrap().len(), 1);
  assert_eq!(manifest["providers"][0]["id"], "org.agentos.network");
  assert!(manifest["providers"][0].get("executable").is_none());
  assert_eq!(
    manifest["providers"][0]["methods"],
    json!([
      "capabilities.get",
      "authorization.request",
      "authorization.get",
      "authorization.cancel",
      "proxy.get",
      "proxy.enable",
      "proxy.disable",
      "tun.get",
      "tun.start",
      "tun.stop",
      "lease.list",
      "lease.release",
      "events.poll"
    ])
  );
  assert_eq!(
    manifest["permissions"],
    json!(["native.providers", "native.network", "native.system-proxy"])
  );
}

#[cfg(unix)]
#[test]
fn native_tun_manifest_emits_runtime_snapshot_without_packaging_configuration() {
  let (root, mut config) = project();
  config["agentos"]["targets"] = json!(["agentos/arm64"]);
  write_network_executable(root.path(), "network/tunnel-kernel", b"tunnel executable\n");
  config["agentos"]["build"]["artifacts"]
    .as_array_mut()
    .unwrap()
    .push(json!({ "from": "network/tunnel-kernel", "to": "network/tunnel-kernel" }));
  enable_native_network(
    &mut config,
    &["native.providers", "native.network", "native.tun"],
    tun_network_config(),
  );
  write_config(root.path(), &config);
  assert_cli_success(
    invoke(root.path(), &["agentos", "validate"]),
    "native TUN network config should validate",
  );
  let inspect = invoke(root.path(), &["agentos", "inspect"]);
  assert!(
    inspect.status.success(),
    "agentos inspect should show the TUN runtime snapshot declaration\n{}",
    String::from_utf8_lossy(&inspect.stderr)
  );
  let inspect = String::from_utf8_lossy(&inspect.stdout);
  assert!(inspect.contains("configuration runtime-snapshot up to 4194304 bytes"));
  assert_cli_success(
    invoke(root.path(), &["build", "--target", "agentos/arm64"]),
    "native TUN network package should build",
  );
  let manifest = package_manifest(root.path(), "arm64");
  assert_eq!(
    manifest["network"]["tunProfiles"][0]["exitPolicy"],
    "restore-previous"
  );
  assert_eq!(manifest["network"]["tunProfiles"][0]["ipv4"], true);
  assert_eq!(manifest["network"]["tunProfiles"][0]["ipv6"], true);
  assert_eq!(
    manifest["network"]["tunProfiles"][0]["configuration"],
    json!({ "source": "runtime-snapshot", "maxBytes": 4194304 })
  );
  assert!(manifest["network"]["tunProfiles"][0]["configuration"]["path"].is_null());
  assert!(manifest["network"]["tunProfiles"][0]["configuration"]["sha256"].is_null());
  assert!(
    manifest["network"]["tunProfiles"][0]
      .get("outerEndpoints")
      .is_none()
  );
  assert!(
    manifest["network"]["tunProfiles"][0]
      .get("dnsServers")
      .is_none()
  );
  assert_eq!(
    manifest["network"]["executables"][0]["sha256"],
    sha256(&package_file(root.path(), "arm64", "network/tunnel-kernel"))
  );
}

#[cfg(unix)]
#[test]
fn generic_native_network_example_passes_validate_and_build_for_both_targets() {
  let (root, _) = project();
  let example_path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../examples/agentos/native-network/vitul.conf.json.example");
  let example: Value = serde_json::from_slice(&fs::read(example_path).unwrap()).unwrap();
  write_network_executable(
    root.path(),
    "network/arm64/network-client",
    b"arm64 generic network client\n",
  );
  write_network_executable(
    root.path(),
    "network/x64/network-client",
    b"x64 generic network client\n",
  );
  write_network_executable(
    root.path(),
    "network/arm64/tunnel-kernel",
    b"arm64 generic tunnel kernel\n",
  );
  write_network_executable(
    root.path(),
    "network/x64/tunnel-kernel",
    b"x64 generic tunnel kernel\n",
  );
  write_config(root.path(), &example);
  assert_cli_success(
    invoke(root.path(), &["agentos", "validate"]),
    "the checked-in generic network example should validate",
  );
  assert_cli_success(
    invoke(root.path(), &["agentos", "inspect"]),
    "the checked-in generic network example should inspect",
  );
  assert_cli_success(
    invoke(root.path(), &["build", "--target", "agentos"]),
    "the checked-in generic network example should build for ARM64 and x64",
  );
  for architecture in ["arm64", "x64"] {
    let archive = root.path().join(format!(
      "dist/generic-native-network-app-1.0.0-agentos-{architecture}.agt"
    ));
    assert!(archive.is_file(), "missing {} package", architecture);
  }
}

#[cfg(unix)]
#[test]
fn arm64_and_x64_network_manifests_hash_their_own_final_executables() {
  let (root, mut config) = project();
  write_network_executable(
    root.path(),
    "network/arm/client",
    b"arm64 network executable\n",
  );
  write_network_executable(
    root.path(),
    "network/x64/client",
    b"x64 network executable\n",
  );
  config["agentos"]["build"]["artifacts"] = json!([
    { "from": "dist", "to": "web" },
    { "from": "service", "to": "service" },
    { "from": "network/arm/client", "to": "bin/network-client", "targets": ["agentos/arm64"] },
    { "from": "network/x64/client", "to": "bin/network-client", "targets": ["agentos/x64"] }
  ]);
  enable_native_network(
    &mut config,
    &["native.providers", "native.network", "native.system-proxy"],
    proxy_network_config(),
  );
  write_config(root.path(), &config);
  assert_cli_success(
    invoke(root.path(), &["build", "--target", "agentos"]),
    "both target-specific network packages should build",
  );
  let arm_manifest = package_manifest(root.path(), "arm64");
  let x64_manifest = package_manifest(root.path(), "x64");
  let arm_sha = sha256(&package_file(root.path(), "arm64", "bin/network-client"));
  let x64_sha = sha256(&package_file(root.path(), "x64", "bin/network-client"));
  assert_eq!(arm_manifest["network"]["executables"][0]["sha256"], arm_sha);
  assert_eq!(x64_manifest["network"]["executables"][0]["sha256"], x64_sha);
  assert_ne!(arm_sha, x64_sha);
}

#[test]
fn unsupported_network_app_types_providers_fields_and_permissions_are_rejected() {
  let invalid_cases: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
    (
      "container mode",
      Box::new(|config| {
        enable_native_network(
          config,
          &["native.providers", "native.network", "native.system-proxy"],
          proxy_network_config(),
        );
        config["agentos"]["execution"]["mode"] = json!("container");
      }),
    ),
    (
      "missing base network permissions",
      Box::new(|config| {
        enable_native_network(config, &["native.system-proxy"], proxy_network_config());
      }),
    ),
    (
      "proxy permission without proxy profile",
      Box::new(|config| {
        enable_native_network(
          config,
          &["native.providers", "native.network", "native.system-proxy"],
          json!({
            "protocolVersion": 1,
            "executables": [{ "id": "network-client", "path": "bin/network-client" }],
            "tunProfiles": [{
              "id": "full-tunnel", "executable": "network-client",
              "arguments": [], "configuration": { "path": "config/client.conf" },
              "interfaceName": "app-tun0",
              "outerEndpoints": [{ "address": "198.51.100.17", "port": 51820, "protocol": "udp" }],
              "ipv4": true, "ipv6": true, "dnsServers": ["1.1.1.1"]
            }]
          }),
        );
      }),
    ),
    (
      "manual built-in network provider",
      Box::new(|config| {
        enable_native_network(
          config,
          &["native.providers", "native.network", "native.system-proxy"],
          proxy_network_config(),
        );
        config["agentos"]["providers"] =
          json!([{ "id": "org.agentos.network", "methods": ["capabilities.get"] }]);
      }),
    ),
    (
      "unknown network field",
      Box::new(|config| {
        let mut network = proxy_network_config();
        network["unexpected"] = json!(true);
        enable_native_network(
          config,
          &["native.providers", "native.network", "native.system-proxy"],
          network,
        );
      }),
    ),
    (
      "HTTP endpoint missing HTTPS",
      Box::new(|config| {
        let mut network = proxy_network_config();
        network["proxyProfiles"][0]
          .as_object_mut()
          .unwrap()
          .remove("https");
        enable_native_network(
          config,
          &["native.providers", "native.network", "native.system-proxy"],
          network,
        );
      }),
    ),
  ];
  for (name, mutate) in invalid_cases {
    let (root, mut config) = project();
    mutate(&mut config);
    write_config(root.path(), &config);
    let output = invoke(root.path(), &["agentos", "validate"]);
    assert!(
      !output.status.success(),
      "{name} must be rejected\nstdout: {}\nstderr: {}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
    if name == "manual built-in network provider" {
      assert!(
        String::from_utf8_lossy(&output.stderr).contains("org.agentos.network"),
        "built-in Provider errors should identify the reserved provider: {}",
        String::from_utf8_lossy(&output.stderr)
      );
    }
  }

  let (remote_root, mut remote) = project();
  remote["agentos"]["type"] = json!("remote-web");
  remote["agentos"]["web"] = json!({ "url": "https://example.org/" });
  remote["agentos"].as_object_mut().unwrap().remove("run");
  remote["agentos"].as_object_mut().unwrap().remove("build");
  remote["agentos"]["data"]["persistent"] = json!(true);
  enable_native_network(
    &mut remote,
    &["native.providers", "native.network", "native.system-proxy"],
    proxy_network_config(),
  );
  write_config(remote_root.path(), &remote);
  let output = invoke(remote_root.path(), &["agentos", "validate"]);
  assert!(
    !output.status.success(),
    "remote-web network apps must be rejected"
  );

  let (missing_permission_root, mut missing_permission) = project();
  enable_native_network(
    &mut missing_permission,
    &["native.providers", "native.network"],
    proxy_network_config(),
  );
  write_config(missing_permission_root.path(), &missing_permission);
  let output = invoke(missing_permission_root.path(), &["agentos", "validate"]);
  assert!(
    !output.status.success(),
    "proxy profiles require native.system-proxy"
  );
}

#[test]
fn tun_manifest_v1_closed_fields_and_network_values_are_enforced() {
  let invalid_cases: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
    (
      "unsupported protocol version",
      Box::new(|network| network["protocolVersion"] = json!(2)),
    ),
    (
      "duplicate IDs",
      Box::new(|network| network["tunProfiles"][0]["id"] = json!("tunnel-kernel")),
    ),
    (
      "undeclared executable reference",
      Box::new(|network| network["tunProfiles"][0]["executable"] = json!("missing")),
    ),
    (
      "absolute package path",
      Box::new(|network| network["executables"][0]["path"] = json!("/usr/bin/client")),
    ),
    (
      "unsupported runtime snapshot source",
      Box::new(|network| {
        network["tunProfiles"][0]["configuration"]["source"] = json!("package-file")
      }),
    ),
    (
      "runtime snapshot below minimum",
      Box::new(|network| network["tunProfiles"][0]["configuration"]["maxBytes"] = json!(1023)),
    ),
    (
      "runtime snapshot above maximum",
      Box::new(|network| network["tunProfiles"][0]["configuration"]["maxBytes"] = json!(4194305)),
    ),
    (
      "non-integer runtime snapshot maximum",
      Box::new(|network| network["tunProfiles"][0]["configuration"]["maxBytes"] = json!(1024.5)),
    ),
    (
      "unknown configuration field",
      Box::new(|network| network["tunProfiles"][0]["configuration"]["unexpected"] = json!(true)),
    ),
    (
      "legacy static configuration path",
      Box::new(|network| {
        network["tunProfiles"][0]["configuration"]["path"] = json!("config/client.conf")
      }),
    ),
    (
      "legacy configuration digest",
      Box::new(|network| {
        network["tunProfiles"][0]["configuration"]["sha256"] = json!("0".repeat(64))
      }),
    ),
    (
      "legacy outer endpoints",
      Box::new(|network| {
        network["tunProfiles"][0]["outerEndpoints"] = json!([{
          "address": "198.51.100.17", "port": 443, "protocol": "udp"
        }])
      }),
    ),
    (
      "legacy DNS servers",
      Box::new(|network| network["tunProfiles"][0]["dnsServers"] = json!(["1.1.1.1"])),
    ),
    (
      "single stack TUN",
      Box::new(|network| network["tunProfiles"][0]["ipv6"] = json!(false)),
    ),
    (
      "single stack IPv4 disabled",
      Box::new(|network| network["tunProfiles"][0]["ipv4"] = json!(false)),
    ),
    (
      "unsupported argv placeholder",
      Box::new(|network| network["tunProfiles"][0]["arguments"][1] = json!("${command}")),
    ),
    (
      "legacy config placeholder",
      Box::new(|network| network["tunProfiles"][0]["arguments"][1] = json!("{config}")),
    ),
    (
      "missing config placeholder",
      Box::new(|network| {
        network["tunProfiles"][0]["arguments"] = json!(["--config", "client.conf"])
      }),
    ),
    (
      "host path in argv",
      Box::new(|network| network["tunProfiles"][0]["arguments"][1] = json!("/etc/agentos.conf")),
    ),
    (
      "Windows host path in argv",
      Box::new(|network| {
        network["tunProfiles"][0]["arguments"][1] = json!(r"C:\\AgentOS\\client.conf")
      }),
    ),
    (
      "path traversal in argv",
      Box::new(|network| network["tunProfiles"][0]["arguments"][0] = json!("../client.conf")),
    ),
    (
      "too many argv entries",
      Box::new(|network| {
        let mut arguments = vec!["--option"; 64];
        arguments.push("${config}");
        network["tunProfiles"][0]["arguments"] = json!(arguments);
      }),
    ),
    (
      "argv entry exceeds bound",
      Box::new(|network| {
        let mut argument = "a".repeat(4091);
        argument.push_str("${config}");
        network["tunProfiles"][0]["arguments"] = json!([argument]);
      }),
    ),
    (
      "invalid interface name",
      Box::new(|network| network["tunProfiles"][0]["interfaceName"] = json!("invalid/name")),
    ),
    (
      "interface name exceeds Linux bound",
      Box::new(|network| {
        network["tunProfiles"][0]["interfaceName"] = json!("interface-name-too-long")
      }),
    ),
    (
      "mutable exit policy",
      Box::new(|network| network["tunProfiles"][0]["exitPolicy"] = json!("leave-active")),
    ),
    (
      "unknown TUN field",
      Box::new(|network| {
        network["tunProfiles"][0]["systemdProperties"] = json!({ "User": "root" })
      }),
    ),
    (
      "unknown top-level network field",
      Box::new(|network| network["custom"] = json!({ "command": "ip route" })),
    ),
  ];
  for (name, mutate) in invalid_cases {
    let (root, mut config) = project();
    let mut network = tun_network_config();
    mutate(&mut network);
    enable_native_network(
      &mut config,
      &["native.providers", "native.network", "native.tun"],
      network,
    );
    write_config(root.path(), &config);
    let output = invoke(root.path(), &["agentos", "validate"]);
    assert!(
      !output.status.success(),
      "{name} must be rejected\nstdout: {}\nstderr: {}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
    if name == "unknown top-level network field" {
      assert!(
        String::from_utf8_lossy(&output.stderr).contains("agentos.network.custom"),
        "closed-schema errors should identify the field path: {}",
        String::from_utf8_lossy(&output.stderr)
      );
    }
  }
}

#[cfg(unix)]
#[test]
fn missing_network_files_and_non_executable_binaries_fail_builds() {
  for missing in [true, false] {
    let (root, mut config) = project();
    fs::create_dir_all(root.path().join("network")).unwrap();
    if !missing {
      let binary = root.path().join("network/client");
      fs::write(&binary, b"not executable\n").unwrap();
      use std::os::unix::fs::PermissionsExt;
      fs::set_permissions(&binary, fs::Permissions::from_mode(0o644)).unwrap();
      config["agentos"]["build"]["artifacts"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "from": "network", "to": "bin" }));
    }
    enable_native_network(
      &mut config,
      &["native.providers", "native.network", "native.system-proxy"],
      proxy_network_config(),
    );
    write_config(root.path(), &config);
    let build = invoke(root.path(), &["build", "--target", "agentos/arm64"]);
    assert!(
      !build.status.success(),
      "missing={missing} should fail\nstdout: {}\nstderr: {}",
      String::from_utf8_lossy(&build.stdout),
      String::from_utf8_lossy(&build.stderr)
    );
  }
}

#[test]
fn legacy_port_alias_is_used_in_inspection_and_packaging() {
  let (root, mut config) = project();
  config["agentos"]["run"]
    .as_object_mut()
    .unwrap()
    .remove("port");
  config["agentos"]["port"] = json!(12345);
  write_config(root.path(), &config);
  let inspect = invoke(root.path(), &["agentos", "inspect"]);
  assert!(inspect.status.success());
  assert!(String::from_utf8_lossy(&inspect.stdout).contains("Backend port: 12345"));
  assert!(String::from_utf8_lossy(&inspect.stderr).contains("agentos.port is deprecated"));
  assert_cli_success(
    invoke(root.path(), &["build", "--target", "agentos/arm64"]),
    "legacy port should build",
  );
  assert_eq!(package_manifest(root.path(), "arm64")["run"]["port"], 12345);
}

#[test]
fn conflicting_and_invalid_legacy_ports_fail_before_packaging() {
  for port in [
    json!(12345),
    json!(0),
    json!(65536),
    json!("18420"),
    Value::Null,
  ] {
    let (root, mut config) = project();
    config["agentos"]["port"] = port;
    write_config(root.path(), &config);
    for args in [
      vec!["agentos", "validate"],
      vec!["agentos", "inspect"],
      vec!["build", "--target", "agentos/arm64"],
    ] {
      let output = invoke(root.path(), &args);
      assert!(!output.status.success());
      assert!(String::from_utf8_lossy(&output.stderr).contains("agentos.port"));
    }
  }
  let (root, mut config) = project();
  config["agentos"]["port"] = config["agentos"]["run"]["port"].clone();
  write_config(root.path(), &config);
  assert_cli_success(
    invoke(root.path(), &["agentos", "validate"]),
    "matching aliases remain compatible",
  );
}

#[test]
fn inspect_reports_effective_configuration_without_claiming_runtime_verification() {
  let (root, mut config) = project();
  config["agentos"]["permissions"] = json!(["native.clipboard"]);
  write_config(root.path(), &config);
  let output = invoke(root.path(), &["agentos", "inspect"]);
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  for expected in [
    "Backend port: 18420",
    "Auto start: false",
    "Permissions: native.clipboard",
    "Execution mode: container",
    "Persistent data: false",
    "Runtime behavior: not verified",
  ] {
    assert!(stdout.contains(expected), "missing {expected} in {stdout}");
  }
}
