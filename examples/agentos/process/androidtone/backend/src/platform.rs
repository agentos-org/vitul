//! Host observations and trusted platform context. Privileged actions go through AgentOS.
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
  StopContainer,
  StopApplication,
}
impl AdministrativeOperation {
  pub fn platform_dependency(&self) -> String {
    let operation = match self {
      Self::StopContainer => "stop-container",
      Self::StopApplication => "stop-application",
    };
    format!("platform-dependency: AndroidTone 的 {operation} 尚无完整、可审查的 AgentOS 通用部署操作声明；当前不会从 no-new-privs Provider 直接提权，也不会提交部分宿主变更。")
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

/// Return the PulseAudio runtime directory only when the trusted desktop
/// session owns the expected Unix socket at `pulse/native`.
pub fn pulse_runtime_path(session: &SessionContext) -> Result<PathBuf, String> {
  let runtime_dir = Path::new(&session.runtime_dir);
  if !runtime_dir.is_absolute() {
    return Err("platform-dependency: AgentOS 可信会话的 runtime 目录无效。".into());
  }
  let pulse_dir = runtime_dir.join("pulse");
  let socket_path = pulse_dir.join("native");
  let metadata = fs::symlink_metadata(&socket_path).map_err(|error| {
    if error.kind() == std::io::ErrorKind::NotFound {
      format!(
        "platform-dependency: 当前桌面会话缺少 PulseAudio Unix socket（{}）；请先启动桌面音频会话后重试。",
        socket_path.display()
      )
    } else {
      format!(
        "platform-dependency: 无法检查当前桌面会话的 PulseAudio socket（{}）。",
        socket_path.display()
      )
    }
  })?;
  if !metadata.file_type().is_socket() {
    return Err(format!(
      "platform-dependency: 当前桌面会话的 PulseAudio 路径不是 Unix socket（{}）。",
      socket_path.display()
    ));
  }
  if metadata.uid() != session.uid {
    return Err(format!(
      "platform-dependency: 当前桌面会话的 PulseAudio socket 所有者不匹配（{}）。",
      socket_path.display()
    ));
  }
  Ok(pulse_dir)
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
  use std::os::unix::net::UnixListener;
  use uuid::Uuid;

  fn test_runtime_dir() -> PathBuf {
    let path = PathBuf::from(format!("/tmp/at-pulse-{}", Uuid::new_v4()));
    fs::create_dir_all(path.join("pulse")).unwrap();
    path
  }

  fn test_session(runtime_dir: &Path, uid: u32) -> SessionContext {
    SessionContext {
      session_id: "test-session".into(),
      uid,
      gid: unsafe { libc::getegid() },
      home: std::env::temp_dir().display().to_string(),
      runtime_dir: runtime_dir.display().to_string(),
      dbus_session_bus_address: String::new(),
    }
  }

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
    for op in [AdministrativeOperation::StopContainer] {
      assert!(op.platform_dependency().starts_with("platform-dependency:"));
      assert!(op
        .platform_dependency()
        .contains("完整、可审查的 AgentOS 通用部署操作声明"));
      assert!(op.platform_dependency().contains("不会提交部分宿主变更"));
    }
  }
  #[test]
  fn desktop_session_requires_the_platform_to_mark_it_available() {
    let value = json!({"available": false, "state": "unavailable", "reason": "no-active-session"});
    assert!(SessionContext::from_provider(&value).is_err());
  }

  #[test]
  fn pulse_runtime_path_accepts_session_owned_unix_socket() {
    let runtime_dir = test_runtime_dir();
    let socket_path = runtime_dir.join("pulse/native");
    let _socket = UnixListener::bind(&socket_path).unwrap();
    let session = test_session(&runtime_dir, unsafe { libc::geteuid() });

    assert_eq!(
      pulse_runtime_path(&session).unwrap(),
      runtime_dir.join("pulse")
    );
    fs::remove_dir_all(runtime_dir).unwrap();
  }

  #[test]
  fn pulse_runtime_path_reports_missing_socket_immediately() {
    let runtime_dir = test_runtime_dir();
    let session = test_session(&runtime_dir, unsafe { libc::geteuid() });

    let error = pulse_runtime_path(&session).unwrap_err();
    assert!(error.contains("缺少 PulseAudio Unix socket"));
    assert!(error.contains("pulse/native"));
    fs::remove_dir_all(runtime_dir).unwrap();
  }

  #[test]
  fn pulse_runtime_path_rejects_socket_owned_by_another_uid() {
    let runtime_dir = test_runtime_dir();
    let _socket = UnixListener::bind(runtime_dir.join("pulse/native")).unwrap();
    let owner_uid = unsafe { libc::geteuid() };
    let session = test_session(&runtime_dir, owner_uid.wrapping_add(1));

    let error = pulse_runtime_path(&session).unwrap_err();
    assert!(error.contains("PulseAudio socket 所有者不匹配"));
    fs::remove_dir_all(runtime_dir).unwrap();
  }
}
