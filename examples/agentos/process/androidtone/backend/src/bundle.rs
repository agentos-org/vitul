//! Read-only bundle inventory. Deployment remains a platform-authorized operation.
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
  collections::HashSet,
  fs,
  io::Read,
  path::{Component, Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
  schema_version: u32,
  id: String,
  architecture: String,
  host: Host,
  waydroid_version: String,
  android_version: String,
  variant: String,
  deployment: String,
  packages: Vec<Entry>,
  images: Vec<Entry>,
  supplementary: Vec<Entry>,
}
#[derive(Deserialize)]
struct Host {
  os: String,
  distribution: String,
  version: String,
}
#[derive(Deserialize)]
struct Entry {
  path: String,
  size: u64,
  sha256: String,
  #[serde(default)]
  package: String,
  #[serde(default)]
  architecture: String,
  #[serde(default)]
  kind: String,
}

fn root() -> Result<PathBuf, String> {
  std::env::current_exe()
    .map_err(|e| e.to_string())?
    .parent()
    .map(|p| p.join("runtime"))
    .ok_or_else(|| "Cannot locate application runtime payload".into())
}
fn safe_file(root: &Path, relative: &str) -> Result<PathBuf, String> {
  if relative.is_empty() {
    return Err("Empty bundle path".into());
  }
  let mut path = root.to_path_buf();
  for part in Path::new(relative).components() {
    let Component::Normal(name) = part else {
      return Err("Unsafe bundle path".into());
    };
    path.push(name);
    if fs::symlink_metadata(&path)
      .map_err(|e| e.to_string())?
      .file_type()
      .is_symlink()
    {
      return Err("Bundle symlinks are forbidden".into());
    }
  }
  if !path.is_file() {
    return Err("Bundle entry is not a regular file".into());
  }
  Ok(path)
}
fn inspect_at(root: &Path, hash: bool) -> Result<Value, String> {
  if fs::symlink_metadata(root)
    .map_err(|e| e.to_string())?
    .file_type()
    .is_symlink()
  {
    return Err("Bundle root symlink is forbidden".into());
  }
  let catalog_path = safe_file(root, "catalog.json")?;
  if fs::metadata(&catalog_path)
    .map_err(|e| e.to_string())?
    .len()
    > 4 * 1024 * 1024
  {
    return Err("Bundle catalog too large".into());
  }
  let catalog: Catalog =
    serde_json::from_slice(&fs::read(&catalog_path).map_err(|e| e.to_string())?)
      .map_err(|e| e.to_string())?;
  if catalog.schema_version != 1
    || catalog.id != "com.agentos.androidtone.runtime.noble-arm64"
    || catalog.architecture != "arm64"
    || catalog.host.os != "linux"
    || catalog.host.distribution != "ubuntu"
    || catalog.host.version != "24.04"
    || catalog.variant != "VANILLA"
    || catalog.deployment != "platform-authorized-offline-packages"
  {
    return Err("Unsupported runtime bundle profile".into());
  }
  if catalog.packages.is_empty()
    || catalog.supplementary.is_empty()
    || catalog.images.len() != 2
    || !catalog.packages.iter().any(|e| e.package == "waydroid")
    || !catalog.packages.iter().any(|e| e.package == "lxc")
    || !catalog
      .images
      .iter()
      .any(|e| e.kind == "system" && e.path == "images/system.img")
    || !catalog
      .images
      .iter()
      .any(|e| e.kind == "vendor" && e.path == "images/vendor.img")
  {
    return Err("Runtime bundle missing required components".into());
  }
  let mut seen = HashSet::new();
  let mut bytes = 0u64;
  for e in catalog
    .packages
    .iter()
    .chain(&catalog.images)
    .chain(&catalog.supplementary)
  {
    if !seen.insert(&e.path)
      || e.size == 0
      || e.sha256.len() != 64
      || !e.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
      return Err("Invalid or duplicate runtime entry".into());
    }
    if !e.package.is_empty()
      && (e.architecture != "arm64" && e.architecture != "all"
        || !e.path.starts_with("packages/")
        || !e.path.ends_with(".deb"))
    {
      return Err("Wrong package architecture or path".into());
    }
    let path = safe_file(root, &e.path)?;
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() != e.size {
      return Err(format!("Runtime size mismatch: {}", e.path));
    }
    if hash {
      let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
      let mut digest = Sha256::new();
      let mut buf = vec![0; 1024 * 1024];
      loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
          break;
        }
        digest.update(&buf[..n]);
      }
      if format!("{:x}", digest.finalize()) != e.sha256.to_lowercase() {
        return Err(format!("Runtime checksum mismatch: {}", e.path));
      }
    }
    bytes = bytes.checked_add(e.size).ok_or("Runtime size overflow")?;
  }
  Ok(
    json!({"included":true,"state":if hash {"verified"} else {"present"},
    "integrity":if hash {"sha256-verified"} else {"sizes-checked"},"id":catalog.id,
    "architecture":catalog.architecture,"host":{"distribution":catalog.host.distribution,"version":catalog.host.version},
    "waydroidVersion":catalog.waydroid_version,"androidVersion":catalog.android_version,"variant":catalog.variant,
    "packageCount":catalog.packages.len(),"imageCount":catalog.images.len(),"bytes":bytes,
    "deployment":"platform-dependency","requiresAgentOSBundledWaydroid":false}),
  )
}
pub fn verify() -> Result<Value, String> {
  inspect_at(&root()?, true)
}
pub fn status() -> Value {
  match root().and_then(|p| inspect_at(&p, false)) {
    Ok(v) => v,
    Err(e) => json!({"included":false,"state":"missing-or-invalid","message":e}),
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn rejects_escape_and_symlink() {
    let p = std::env::temp_dir().join(format!("androidtone-bundle-test-{}", std::process::id()));
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("file"), b"x").unwrap();
    std::os::unix::fs::symlink(p.join("file"), p.join("link")).unwrap();
    assert!(safe_file(&p, "../file").is_err());
    assert!(safe_file(&p, "/etc/passwd").is_err());
    assert!(safe_file(&p, "link").is_err());
    assert!(safe_file(&p, "file").is_ok());
    fs::remove_dir_all(p).unwrap();
  }
  #[test]
  fn detects_corruption_and_missing_images() {
    let p = std::env::temp_dir().join(format!(
      "androidtone-bundle-integrity-{}",
      std::process::id()
    ));
    fs::create_dir_all(p.join("packages")).unwrap();
    fs::create_dir_all(p.join("images")).unwrap();
    let hash = format!("{:x}", Sha256::digest(b"good"));
    let entry = |path: &str| json!({"path":path,"size":4,"sha256":hash});
    let mut waydroid = entry("packages/waydroid.deb");
    waydroid["package"] = json!("waydroid");
    waydroid["architecture"] = json!("all");
    let mut lxc = entry("packages/lxc.deb");
    lxc["package"] = json!("lxc");
    lxc["architecture"] = json!("arm64");
    let mut system = entry("images/system.img");
    system["kind"] = json!("system");
    let mut vendor = entry("images/vendor.img");
    vendor["kind"] = json!("vendor");
    for path in [
      "packages/waydroid.deb",
      "packages/lxc.deb",
      "images/system.img",
      "images/vendor.img",
      "NOTICE",
    ] {
      fs::write(p.join(path), b"good").unwrap();
    }
    let catalog = json!({"schemaVersion":1,"id":"com.agentos.androidtone.runtime.noble-arm64","architecture":"arm64","host":{"os":"linux","distribution":"ubuntu","version":"24.04"},"waydroidVersion":"test","androidVersion":"13","variant":"VANILLA","deployment":"platform-authorized-offline-packages","packages":[waydroid,lxc],"images":[system,vendor],"supplementary":[entry("NOTICE")]});
    fs::write(
      p.join("catalog.json"),
      serde_json::to_vec(&catalog).unwrap(),
    )
    .unwrap();
    assert_eq!(
      inspect_at(&p, true).unwrap()["integrity"],
      "sha256-verified"
    );
    fs::write(p.join("images/system.img"), b"evil").unwrap();
    assert!(inspect_at(&p, false).is_ok()); // metadata check explicitly does not claim SHA verification
    assert!(inspect_at(&p, true)
      .unwrap_err()
      .contains("checksum mismatch"));
    fs::remove_file(p.join("images/vendor.img")).unwrap();
    assert!(inspect_at(&p, false).is_err());
    fs::remove_dir_all(p).unwrap();
  }
}
