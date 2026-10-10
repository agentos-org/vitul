use std::{fs, path::Path};

fn main() {
  let vitul_config = Path::new("vitul.conf.json");
  let desktop_config = Path::new("tauri.conf.json");
  println!("cargo:rerun-if-changed={}", vitul_config.display());
  fs::copy(vitul_config, desktop_config).unwrap_or_else(|error| {
    panic!(
      "failed to prepare the generated Vitul runtime config from {}: {error}",
      vitul_config.display()
    )
  });
  vitul_build::build()
}
