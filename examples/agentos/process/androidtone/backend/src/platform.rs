//! Read-only observations and proposed trusted platform context. No host provisioning.
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
  collections::BTreeMap,
  env, fs,
  os::unix::fs::{FileTypeExt, MetadataExt},
  path::{Path, PathBuf},
};

#[cfg(target_os = "linux")]
use std::os::unix::io::FromRawFd;

pub enum AdministrativeOperation {
  InstallRuntime,
  InitializeImages,
  StartContainer,
  StopContainer,
  StopApplication,
}
impl AdministrativeOperation {
  pub fn platform_dependency(&self) -> String {
    let operation = match self {
      Self::InstallRuntime => "install-runtime",
      Self::InitializeImages => "initialize-images",
      Self::StartContainer => "start-container",
      Self::StopContainer => "stop-container",
      Self::StopApplication => "stop-application",
    };
    format!("platform-dependency: AgentOS 当前的通用系统部署动作尚未覆盖 Android 运行环境的 {operation}：需要受限的 Binderfs 设备准备以及 LXC 容器初始化和生命周期管理。普通 Provider 保持 no-new-privs，不直接提权。")
  }
}

pub fn validate_runtime_params(params: &Value) -> Result<(), String> {
  if !params.as_object().is_some_and(|object| object.is_empty()) {
    return Err("Runtime operations accept only an empty parameter object; arbitrary commands or arguments are denied.".into());
  }
  Ok(())
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContext {
  pub session_id: String,
  pub uid: u32,
  pub gid: u32,
  pub home: String,
  pub runtime_dir: String,
  pub dbus_session_bus_address: String,
}
impl SessionContext {
  pub fn from_provider(value: &Value) -> Result<Self, String> {
    if value.get("available").and_then(Value::as_bool) != Some(true) {
      let state = value
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("unavailable");
      let reason = value
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("trusted-session-unavailable");
      return Err(format!(
        "platform-dependency: AgentOS 未提供可用的可信桌面会话（{state}: {reason}）。"
      ));
    }
    let context = Self {
      session_id: value
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or("platform-dependency: AgentOS 桌面会话缺少 sessionId。")?
        .to_owned(),
      uid: value
        .get("uid")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or("platform-dependency: AgentOS 桌面会话缺少有效 UID。")?,
      gid: value
        .get("gid")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or("platform-dependency: AgentOS 桌面会话缺少有效 GID。")?,
      home: value
        .get("home")
        .and_then(Value::as_str)
        .ok_or("platform-dependency: AgentOS 桌面会话缺少 home。")?
        .to_owned(),
      runtime_dir: value
        .get("runtimeDir")
        .and_then(Value::as_str)
        .ok_or("platform-dependency: AgentOS 桌面会话缺少 runtimeDir。")?
        .to_owned(),
      dbus_session_bus_address: value
        .get("dbusSessionBusAddress")
        .and_then(Value::as_str)
        .ok_or("platform-dependency: AgentOS 桌面会话缺少已验证的 D-Bus 地址。")?
        .to_owned(),
    };
    context.validate()?;
    Ok(context)
  }

  pub fn from_launch_argument(value: &str) -> Result<Self, String> {
    let context: Self = serde_json::from_str(value)
      .map_err(|_| "platform-dependency: AgentOS 私有 Wayland 客户端会话参数无效。".to_owned())?;
    context.validate()?;
    Ok(context)
  }

  pub fn environment(&self) -> BTreeMap<String, String> {
    BTreeMap::from([
      (
        "PATH".into(),
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into(),
      ),
      ("HOME".into(), self.home.clone()),
      ("XDG_RUNTIME_DIR".into(), self.runtime_dir.clone()),
      (
        "DBUS_SESSION_BUS_ADDRESS".into(),
        self.dbus_session_bus_address.clone(),
      ),
      ("LANG".into(), "C.UTF-8".into()),
    ])
  }
  pub fn validate(&self) -> Result<(), String> {
    if self.session_id.is_empty()
      || self.session_id.len() > 256
      || self.session_id.chars().any(char::is_control)
      || self.uid != unsafe { libc::geteuid() }
      || self.gid != unsafe { libc::getegid() }
      || !Path::new(&self.home).is_absolute()
      || !Path::new(&self.home).is_dir()
    {
      return Err("platform-dependency: AgentOS 桌面会话身份或 home 无效。".into());
    }
    check_runtime(&self.runtime_dir, self.uid)?;
    let bus = self
      .dbus_session_bus_address
      .strip_prefix("unix:path=")
      .filter(|path| !path.contains([',', ';']))
      .ok_or("platform-dependency: 需要平台提供已验证的用户 D-Bus socket 地址。")?;
    check_socket(Path::new(bus), self.uid)
  }
}

/// The external-wayland host client receives this private session environment
/// directly from AgentOS. Validate those assigned values instead of expecting
/// a second, undocumented JSON context variable.
pub fn private_wayland_environment(session: &SessionContext) -> Result<(String, String), String> {
  let runtime_dir = env::var("XDG_RUNTIME_DIR")
    .map_err(|_| "platform-dependency: AgentOS 未分配私有 Wayland runtime 目录。".to_owned())?;
  let display = env::var("WAYLAND_DISPLAY")
    .map_err(|_| "platform-dependency: AgentOS 未分配私有 Wayland display。".to_owned())?;
  if display.is_empty()
    || display.len() > 128
    || display.contains('/')
    || display == "."
    || display == ".."
  {
    return Err("platform-dependency: AgentOS 私有 Wayland display 名称无效。".into());
  }
  check_runtime(&runtime_dir, session.uid)?;
  check_socket(&Path::new(&runtime_dir).join(&display), session.uid)?;
  Ok((runtime_dir, display))
}
fn check_runtime(path: &str, uid: u32) -> Result<(), String> {
  let metadata =
    fs::metadata(path).map_err(|_| "platform-dependency: 平台 runtime 目录不存在。".to_owned())?;
  if !Path::new(path).is_absolute() || !metadata.is_dir() || metadata.uid() != uid {
    return Err("platform-dependency: 平台 runtime 目录与会话用户不匹配。".into());
  }
  Ok(())
}
fn check_socket(path: &Path, uid: u32) -> Result<(), String> {
  let metadata =
    fs::metadata(path).map_err(|_| "platform-dependency: 平台会话 socket 不存在。".to_owned())?;
  if !path.is_absolute() || !metadata.file_type().is_socket() || metadata.uid() != uid {
    return Err("platform-dependency: 平台会话 socket 类型或所有者不匹配。".into());
  }
  Ok(())
}

fn config_flag(config: &str, key: &str) -> &'static str {
  if config.lines().any(|line| line == format!("{key}=y")) {
    "built-in"
  } else if config.lines().any(|line| line == format!("{key}=m")) {
    "module"
  } else if config
    .lines()
    .any(|line| line == format!("# {key} is not set") || line == format!("{key}=n"))
  {
    "disabled"
  } else {
    "unknown"
  }
}
fn binder_mounts(mountinfo: &str) -> Vec<String> {
  mountinfo
    .lines()
    .filter_map(|line| {
      let (left, right) = line.split_once(" - ")?;
      if !matches!(right.split_whitespace().next(), Some("binder" | "binderfs")) {
        return None;
      }
      left
        .split_whitespace()
        .nth(4)
        .map(|p| p.replace("\\040", " ").replace("\\134", "\\"))
    })
    .collect()
}
pub fn kernel_observations() -> Value {
  let release = fs::read_to_string("/proc/sys/kernel/osrelease")
    .unwrap_or_default()
    .trim()
    .to_owned();
  let config_path = format!("/boot/config-{release}");
  let config = fs::read_to_string(&config_path).ok().or_else(|| {
    if !Path::new("/proc/config.gz").is_file() {
      return None;
    }
    let output = std::process::Command::new("gzip")
      .args(["-cd", "/proc/config.gz"])
      .output()
      .ok()?;
    if output.status.success() {
      String::from_utf8(output.stdout).ok()
    } else {
      None
    }
  });
  let text = config.as_deref().unwrap_or("");
  let binder = config_flag(text, "CONFIG_ANDROID_BINDER_IPC");
  let modinfo = ["/sbin/modinfo", "/usr/sbin/modinfo"]
    .into_iter()
    .find(|p| Path::new(p).is_file());
  let module = modinfo.and_then(|tool| {
    std::process::Command::new(tool)
      .args(["-k", &release, "-F", "filename", "binder_linux"])
      .output()
      .ok()
  });
  let module_installed = if binder == "built-in" {
    Some(true)
  } else {
    module
      .as_ref()
      .and_then(|out| out.status.success().then_some(true))
  };
  let module_path = module
    .filter(|out| out.status.success())
    .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned());
  let loaded = Path::new("/sys/module/binder_linux").exists();
  let mountinfo = fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
  let mounts = binder_mounts(&mountinfo);
  let mut candidates: Vec<PathBuf> = ["/dev/binder", "/dev/hwbinder", "/dev/vndbinder"]
    .into_iter()
    .map(PathBuf::from)
    .collect();
  for mount in &mounts {
    if let Ok(entries) = fs::read_dir(mount) {
      candidates.extend(
        entries
          .flatten()
          .filter(|e| e.file_name() != "binder-control")
          .map(|e| e.path()),
      );
    }
  }
  let devices: Vec<Value> = candidates.into_iter().filter_map(|path| {
    let metadata = fs::metadata(&path).ok()?;
    metadata.file_type().is_char_device().then(|| json!({"path": path, "uid": metadata.uid(), "gid": metadata.gid(), "mode": metadata.mode() & 0o777}))
  }).collect();
  json!({"source": "read-only-provider-namespace", "kernelRelease": release,
    "configuration": {"binderIpc": binder, "binderfs": config_flag(text, "CONFIG_ANDROID_BINDERFS"), "memfd": config_flag(text, "CONFIG_MEMFD_CREATE"), "readable": config.is_some()},
    "moduleInstallation": {"installed": module_installed, "path": module_path},
    "moduleLoading": {"observed": loaded}, "binderfsMounting": {"state": if mounts.is_empty() { "not-observed" } else { "mounted" }, "paths": mounts},
    "deviceAllocation": {"state": if devices.is_empty() { "not-observed" } else { "observed" }, "devices": devices, "scope": "observed paths; allocation ownership requires platform evidence"},
    "memfdProbe": memfd_probe(), "supportVerdict": "configuration-and-runtime-are-separate"})
}
#[cfg(target_os = "linux")]
fn memfd_probe() -> Value {
  let name = b"androidtone-readonly-probe\0";
  let fd = unsafe { libc::memfd_create(name.as_ptr().cast(), libc::MFD_CLOEXEC) };
  if fd < 0 {
    json!({"available": false, "error": std::io::Error::last_os_error().to_string()})
  } else {
    drop(unsafe { fs::File::from_raw_fd(fd) });
    json!({"available": true})
  }
}
#[cfg(not(target_os = "linux"))]
fn memfd_probe() -> Value {
  json!({"available": null, "reason": "not-linux"})
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn administrator_parameters_do_not_accept_commands() {
    assert!(validate_runtime_params(&json!({})).is_ok());
    assert!(
      validate_runtime_params(&json!({"executable": "/bin/sh", "args": ["-c", "id"]})).is_err()
    );
    assert!(validate_runtime_params(&Value::Null).is_err());
  }
  #[test]
  fn module_config_is_support_even_without_device_or_mount() {
    assert_eq!(
      config_flag("CONFIG_ANDROID_BINDER_IPC=m\n", "CONFIG_ANDROID_BINDER_IPC"),
      "module"
    );
    assert!(binder_mounts("").is_empty());
    assert_eq!(config_flag("", "CONFIG_ANDROID_BINDER_IPC"), "unknown");
  }
  #[test]
  fn mountinfo_detects_binder_separately() {
    assert_eq!(
      binder_mounts("11 10 0:9 / /dev/binderfs rw - binder none rw"),
      vec!["/dev/binderfs"]
    );
    assert!(binder_mounts("11 10 0:9 / /dev rw - tmpfs none rw").is_empty());
  }
  #[test]
  fn admin_operations_fail_closed_and_are_scoped() {
    for op in [
      AdministrativeOperation::InstallRuntime,
      AdministrativeOperation::InitializeImages,
      AdministrativeOperation::StartContainer,
      AdministrativeOperation::StopContainer,
    ] {
      assert!(op.platform_dependency().starts_with("platform-dependency:"));
      assert!(op.platform_dependency().contains("通用系统部署动作"));
    }
  }
  #[test]
  fn desktop_session_requires_the_platform_to_mark_it_available() {
    let value = json!({"available": false, "state": "unavailable", "reason": "no-active-session"});
    assert!(SessionContext::from_provider(&value).is_err());
  }
}
