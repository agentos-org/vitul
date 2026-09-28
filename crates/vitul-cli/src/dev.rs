use anyhow::{bail, Context, Result};
use std::{
  env,
  ffi::OsString,
  io::{BufRead, BufReader, Read, Write},
  net::TcpStream,
  path::Path,
  process::{Child, Command, Stdio},
  sync::{
    atomic::{AtomicU16, Ordering},
    mpsc::{self, Receiver, RecvTimeoutError},
    Arc, Mutex,
  },
  thread,
  time::{Duration, Instant},
};

const BACKEND_BINARY: &str = "vitul-backend";

pub fn run(args: Vec<OsString>) -> Result<()> {
  if super::option_value(&args, "--target", "-t")
    .is_some_and(|target| target.starts_with("agentos"))
  {
    bail!("AgentOS dev mode is planned after the AgentOS ARM64 build path");
  }

  let root = env::current_dir().context("failed to read current directory")?;
  let manifest = root.join("backend/Cargo.toml");
  if !manifest.is_file() {
    return super::run_tauri("dev", args, "vitul");
  }
  if args
    .iter()
    .any(|arg| matches!(arg.to_string_lossy().as_ref(), "-h" | "--help"))
  {
    return super::run_tauri("dev", args, "vitul");
  }
  let mut build = Command::new("cargo");
  build
    .args(["build", "--manifest-path"])
    .arg(&manifest)
    .current_dir(&root);
  run_status(&mut build, "building the OpenTool backend")?;

  let binary = backend_binary(&root);
  if !binary.is_file() {
    bail!("backend executable not found at {}", binary.display());
  }
  let (child, port_rx) = spawn_backend(&binary, &root)?;
  let child = Arc::new(Mutex::new(child));
  let active_port = Arc::new(AtomicU16::new(0));
  let _backend_guard = BackendGuard {
    child: Arc::clone(&child),
    active_port: Arc::clone(&active_port),
  };

  let port = wait_for_reported_port(&child, port_rx, Duration::from_secs(20))?;
  active_port.store(port, Ordering::Release);
  wait_until_ready(&child, port, Duration::from_secs(20))?;

  unsafe {
    env::set_var("VITUL_OPENTOOL_BASE", format!("http://127.0.0.1:{port}/"));
    env::set_var("VITUL_OPENTOOL_PORT", port.to_string());
  }
  let result = crate::run_tauri("dev", args, "vitul");
  unsafe {
    env::remove_var("VITUL_OPENTOOL_BASE");
    env::remove_var("VITUL_OPENTOOL_PORT");
  }
  result
}

struct BackendGuard {
  child: Arc<Mutex<Child>>,
  active_port: Arc<AtomicU16>,
}

impl Drop for BackendGuard {
  fn drop(&mut self) {
    stop_child(&self.child, &self.active_port);
  }
}

fn spawn_backend(binary: &Path, root: &Path) -> Result<(Child, Receiver<anyhow::Result<u16>>)> {
  let mut child = Command::new(binary)
    .env("HOST", "127.0.0.1")
    .env("PORT", "0")
    .current_dir(&root)
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .spawn()
    .with_context(|| format!("failed to launch backend at {}", binary.display()))?;
  let stdout = child
    .stdout
    .take()
    .context("OpenTool backend stdout was not captured")?;
  let (sender, receiver) = mpsc::channel();
  thread::spawn(move || {
    let mut reported_port = false;
    for line in BufReader::new(stdout).lines() {
      let Ok(line) = line else { break };
      if let Some(value) = line.strip_prefix("VITUL_OPENTOOL_READY=") {
        let result = value
          .parse::<u16>()
          .map_err(|_| anyhow::anyhow!("backend reported invalid port `{value}`"));
        reported_port = result.is_ok();
        let _ = sender.send(result);
      } else {
        eprintln!("[vitul backend] {line}");
      }
    }
    if !reported_port {
      let _ = sender.send(Err(anyhow::anyhow!(
        "OpenTool backend closed stdout before reporting its listening port"
      )));
    }
  });
  Ok((child, receiver))
}

fn wait_for_reported_port(
  child: &Arc<Mutex<Child>>,
  receiver: Receiver<anyhow::Result<u16>>,
  timeout: Duration,
) -> Result<u16> {
  let deadline = Instant::now() + timeout;
  loop {
    if let Some(status) = child
      .lock()
      .expect("backend child lock poisoned")
      .try_wait()?
    {
      bail!("OpenTool backend exited before becoming ready ({status})");
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
      bail!(
        "OpenTool backend did not report its listening port within {} seconds",
        timeout.as_secs()
      );
    }
    match receiver.recv_timeout(remaining.min(Duration::from_millis(150))) {
      Ok(Ok(port)) if port != 0 => return Ok(port),
      Ok(Ok(_)) => bail!("OpenTool backend reported port 0"),
      Ok(Err(error)) => return Err(error),
      Err(RecvTimeoutError::Disconnected) => {
        bail!("OpenTool backend exited before reporting its listening port")
      }
      Err(RecvTimeoutError::Timeout) => {}
    }
  }
}

fn backend_binary(root: &Path) -> std::path::PathBuf {
  let target_dir = env::var_os("CARGO_TARGET_DIR")
    .map(std::path::PathBuf::from)
    .unwrap_or_else(|| root.join("backend/target"));
  let mut path = target_dir.join("debug").join(BACKEND_BINARY);
  if cfg!(windows) {
    path.set_extension("exe");
  }
  path
}

fn wait_until_ready(child: &Arc<Mutex<Child>>, port: u16, timeout: Duration) -> Result<()> {
  let deadline = Instant::now() + timeout;
  while Instant::now() < deadline {
    if let Some(status) = child
      .lock()
      .expect("backend child lock poisoned")
      .try_wait()?
    {
      bail!("OpenTool backend exited before becoming ready ({status})");
    }
    if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
      stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();
      stream
        .write_all(
          b"GET /opentool/version HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
        )
        .ok();
      let mut response = String::new();
      stream.read_to_string(&mut response).ok();
      if response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200") {
        return Ok(());
      }
    }
    thread::sleep(Duration::from_millis(150));
  }
  bail!(
    "OpenTool backend did not become ready within {} seconds",
    timeout.as_secs()
  )
}

fn stop_child(child: &Arc<Mutex<Child>>, active_port: &Arc<AtomicU16>) {
  let port = active_port.load(Ordering::Acquire);
  if port != 0 {
    request_backend_stop(port);
  }

  let deadline = Instant::now() + Duration::from_secs(3);
  loop {
    let mut child = child.lock().expect("backend child lock poisoned");
    if child.try_wait().ok().flatten().is_some() {
      return;
    }
    if Instant::now() >= deadline {
      let _ = child.kill();
      let _ = child.wait();
      return;
    }
    drop(child);
    thread::sleep(Duration::from_millis(100));
  }
}

fn request_backend_stop(port: u16) {
  let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
  let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
    return;
  };
  let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
  let _ = stream.write_all(
    b"POST /opentool/stop HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
  );
  let mut response = String::new();
  let _ = stream.read_to_string(&mut response);
}

fn run_status(command: &mut Command, description: &str) -> Result<()> {
  let status = command
    .status()
    .with_context(|| format!("failed to start {description}"))?;
  if !status.success() {
    bail!("{description} failed with {status}");
  }
  Ok(())
}
