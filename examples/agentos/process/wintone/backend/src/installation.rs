use std::{
  collections::BTreeMap,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
  external_wayland::{CreateSession, ExternalSession, FileGrant, WindowHints},
  model::{InstallTask, InstalledApp},
  runtime::InstalledRuntime,
  shortcuts, AppState,
};
use serde_json::Value;
use uuid::Uuid;

/// Resolve a newly imported installer and launch its interactive setup in an
/// AgentOS-owned Wayland session.
pub async fn prepare_imported_task(state: AppState, task_id: String) {
  if !state.claim_install_task(&task_id) {
    return;
  }
  let _claim = ActiveTaskClaim {
    state: state.clone(),
    task_id: task_id.clone(),
  };
  prepare_imported_task_inner(state, task_id).await;
}

struct ActiveTaskClaim {
  state: AppState,
  task_id: String,
}

impl Drop for ActiveTaskClaim {
  fn drop(&mut self) {
    self.state.release_install_task(&self.task_id);
  }
}

async fn prepare_imported_task_inner(state: AppState, task_id: String) {
  let task = match task_by_id(&state, &task_id).await {
    Ok(Some(task)) => task,
    Ok(None) => return,
    Err(error) => {
      eprintln!("WinTone could not read install task {task_id}: {error}");
      return;
    }
  };
  if task.session_id.is_some() {
    return;
  }

  if !matches!(
    update(
      &state,
      &task_id,
      "preparing-runtime",
      "runtime",
      "Preparing the shared Wine/FEX runtime.",
    )
    .await,
    Ok(true)
  ) {
    return;
  }

  let runtime = match state.runtime_manager.prepare().await {
    Ok(runtime) => runtime,
    Err(error) => {
      let runtime_status = state.runtime_manager.status().await;
      if runtime_status.state == "catalog-unavailable" {
        let _ = update(
          &state,
          &task_id,
          "waiting-runtime",
          "runtime",
          &runtime_status.message,
        )
        .await;
      } else {
        let _ = update(&state, &task_id, "failed", "runtime", &error.0).await;
      }
      return;
    }
  };

  if !matches!(
    update(
      &state,
      &task_id,
      "preparing-prefix",
      "prefix",
      "Preparing a private Wine prefix for this installation.",
    )
    .await,
    Ok(true)
  ) {
    return;
  }

  match state.prefix_manager.prepare(&task_id, &runtime).await {
    Ok(_) => {}
    Err(error) => {
      let _ = update(&state, &task_id, "failed", "prefix", &error).await;
      return;
    }
  }
  let _ = update(
    &state,
    &task_id,
    "waiting-window-session",
    "window-session",
    "Waiting for the AgentOS Wayland session service.",
  )
  .await;
  launch_installer(&state, &task, &runtime).await;
}

async fn launch_installer(state: &AppState, task: &InstallTask, runtime: &InstalledRuntime) {
  if !state.external_wayland.is_configured() {
    let _ = update(
      state,
      &task.id,
      "waiting-window-session",
      "window-session",
      "This AgentOS process has no external Wayland session grant. Reinstall or update WinTone after granting the permission.",
    )
    .await;
    return;
  }
  let capabilities = match state.external_wayland.capabilities().await {
    Ok(capabilities) => capabilities,
    Err(error) => {
      let _ = update(
        state,
        &task.id,
        "waiting-window-session",
        "window-session",
        &error.message,
      )
      .await;
      return;
    }
  };
  if capabilities["available"] != true {
    let reasons = capabilities["unavailableReasons"]
      .as_array()
      .map(|values| {
        values
          .iter()
          .filter_map(Value::as_str)
          .collect::<Vec<_>>()
          .join(", ")
      })
      .filter(|value| !value.is_empty())
      .unwrap_or_else(|| "AgentOS Wayland sessions are unavailable.".to_owned());
    let _ = update(
      state,
      &task.id,
      "waiting-window-session",
      "window-session",
      &reasons,
    )
    .await;
    return;
  }

  let sessions = match state.external_wayland.list_sessions().await {
    Ok(sessions) => sessions,
    Err(error) => {
      let _ = update(
        state,
        &task.id,
        "waiting-window-session",
        "window-session",
        &error.message,
      )
      .await;
      return;
    }
  };
  let application_id = format!("wintone.install.{}", task.id);
  if let Some(session) = sessions
    .iter()
    .find(|session| session.application_id == application_id)
  {
    let active = !terminal(&session.state);
    let _ = state
      .repository
      .update_install_task_session(
        &task.id,
        Some(session.session_id.clone()),
        if active {
          "awaiting-user"
        } else {
          "starting-installer"
        },
        "installer",
        if active {
          "Recovered the existing AgentOS installer session."
        } else {
          "Recovered the completed AgentOS installer session."
        },
      )
      .await;
    return;
  }

  let installer_path = format!("/data/installers/{}.{}", task.id, task.extension);
  let installer_args = if task.extension.eq_ignore_ascii_case("msi") {
    vec!["msiexec".to_owned(), "/i".to_owned(), installer_path]
  } else {
    vec![installer_path]
  };
  let request = match make_session_request(
    state,
    runtime,
    application_id,
    &task.file_name,
    &task.id,
    installer_args,
  ) {
    Ok(request) => request,
    Err(error) => {
      let _ = update(state, &task.id, "failed", "launch", &error).await;
      return;
    }
  };
  let _ = update(
    state,
    &task.id,
    "starting-installer",
    "window-session",
    "Starting the Windows installer in an AgentOS Wayland session.",
  )
  .await;
  match state.external_wayland.create(&request).await {
    Ok(session) => {
      let _ = state
        .repository
        .update_install_task_session(
          &task.id,
          Some(session.session_id),
          "awaiting-user",
          "installer",
          "The installer is open. Finish or close its Windows setup window to continue.",
        )
        .await;
    }
    Err(error) => {
      if let Some(session_id) = error.session_id {
        let _ = state
          .repository
          .update_install_task_session(
            &task.id,
            Some(session_id),
            "starting-installer",
            "installer",
            &error.message,
          )
          .await;
      } else if error.status.is_some_and(|status| status.as_u16() < 500) {
        let _ = update(state, &task.id, "failed", "window-session", &error.message).await;
      } else {
        let _ = update(
          state,
          &task.id,
          "waiting-window-session",
          "window-session",
          &error.message,
        )
        .await;
      }
    }
  }
}

fn make_session_request(
  state: &AppState,
  runtime: &InstalledRuntime,
  application_id: String,
  title: &str,
  prefix_id: &str,
  mut program_args: Vec<String>,
) -> Result<CreateSession, String> {
  let (wine_program, wine_args) = state
    .runtime_manager
    .session_command(runtime, &runtime.wine_command)
    .map_err(|error| error.0)?;
  let mut environment = BTreeMap::from([
    (
      "WINEPREFIX".to_owned(),
      crate::prefixes::PrefixManager::session_path(prefix_id)?,
    ),
    ("WINEDEBUG".to_owned(), "-all".to_owned()),
  ]);
  let (executable, mut args) = if runtime.host_architecture == "aarch64" {
    let fex_command = runtime
      .fex_command
      .as_ref()
      .ok_or_else(|| "ARM64 runtime has no FEX command.".to_owned())?;
    let rootfs = runtime
      .fex_rootfs
      .as_deref()
      .ok_or_else(|| "ARM64 runtime has no FEX RootFS.".to_owned())?;
    let (fex_program, fex_args) = state
      .runtime_manager
      .session_command(runtime, fex_command)
      .map_err(|error| error.0)?;
    environment.insert(
      "FEX_ROOTFS".to_owned(),
      state
        .runtime_manager
        .session_data_path(runtime, rootfs)
        .map_err(|error| error.0)?,
    );
    let mut args = fex_args;
    args.push(wine_program);
    args.extend(wine_args);
    (fex_program, args)
  } else {
    (wine_program, wine_args)
  };
  args.append(&mut program_args);
  let prefix = crate::prefixes::PrefixManager::session_path(prefix_id)?;
  let title = title.chars().take(240).collect::<String>();
  Ok(CreateSession {
    version: 1,
    application_id,
    executable,
    args,
    working_directory: prefix,
    environment,
    files: vec![FileGrant {
      root: "data".to_owned(),
      access: "readWrite".to_owned(),
    }],
    permissions: vec!["network".to_owned()],
    window_hints: WindowHints {
      title: format!("WinTone · {title}"),
      width: 1100,
      height: 760,
    },
  })
}

pub async fn launch_installed_app(
  state: &AppState,
  app_id: &str,
) -> Result<ExternalSession, String> {
  let store = state.snapshot().await.map_err(|error| error.0)?;
  let app = store
    .installed_apps
    .iter()
    .find(|app| app.id == app_id)
    .cloned()
    .ok_or_else(|| "Installed application was not found.".to_owned())?;
  if app.session_id.is_some() {
    return Err("This application already has an active AgentOS session.".to_owned());
  }
  let runtime = state
    .runtime_manager
    .installed_runtime()
    .await
    .map_err(|error| error.0)?
    .ok_or_else(|| "The shared Wine/FEX runtime is not installed.".to_owned())?;
  if !app.runtime_version.is_empty() && app.runtime_version != runtime.version {
    return Err(format!(
      "This app uses Wine/FEX {}, but the active runtime is {}.",
      app.runtime_version, runtime.version
    ));
  }
  state.prefix_manager.path(&app.installation_id).await?;
  let request = make_session_request(
    state,
    &runtime,
    format!("wintone.app.{}", app.id),
    &app.name,
    &app.installation_id,
    vec![app.executable.clone()],
  )?;
  start_app_session(state, &app.id, "launch", request).await
}

pub async fn uninstall_installed_app(
  state: &AppState,
  app_id: &str,
) -> Result<ExternalSession, String> {
  let store = state.snapshot().await.map_err(|error| error.0)?;
  let app = store
    .installed_apps
    .iter()
    .find(|app| app.id == app_id)
    .cloned()
    .ok_or_else(|| "Installed application was not found.".to_owned())?;
  if app.session_id.is_some() {
    return Err("Close the running application before uninstalling it.".to_owned());
  }
  let uninstall_executable = app
    .uninstall_executable
    .clone()
    .ok_or_else(|| "This application did not register a supported uninstaller.".to_owned())?;
  let runtime = state
    .runtime_manager
    .installed_runtime()
    .await
    .map_err(|error| error.0)?
    .ok_or_else(|| "The shared Wine/FEX runtime is not installed.".to_owned())?;
  if !app.runtime_version.is_empty() && app.runtime_version != runtime.version {
    return Err(format!(
      "This app uses Wine/FEX {}, but the active runtime is {}.",
      app.runtime_version, runtime.version
    ));
  }
  state.prefix_manager.path(&app.installation_id).await?;
  let request = make_session_request(
    state,
    &runtime,
    format!("wintone.uninstall.{}", app.id),
    &format!("卸载 {}", app.name),
    &app.installation_id,
    std::iter::once(uninstall_executable)
      .chain(app.uninstall_args)
      .collect(),
  )?;
  start_app_session(state, app_id, "uninstall", request).await
}

async fn start_app_session(
  state: &AppState,
  app_id: &str,
  kind: &str,
  mut request: CreateSession,
) -> Result<ExternalSession, String> {
  if !state.claim_app_operation(app_id) {
    return Err("Another operation for this application is already starting.".to_owned());
  }
  let _claim = ActiveAppOperation {
    state: state.clone(),
    app_id: app_id.to_owned(),
  };
  let capabilities = state
    .external_wayland
    .capabilities()
    .await
    .map_err(|error| error.message)?;
  if capabilities["available"] != true {
    return Err("AgentOS Wayland sessions are currently unavailable.".to_owned());
  }
  let sessions = state
    .external_wayland
    .list_sessions()
    .await
    .map_err(|error| error.message)?;
  let application_prefix = if kind == "uninstall" {
    format!("wintone.uninstall.{app_id}")
  } else {
    format!("wintone.app.{app_id}")
  };
  if let Some(existing) = sessions
    .iter()
    .rev()
    .find(|session| {
      (session.application_id == application_prefix
        || session
          .application_id
          .starts_with(&format!("{application_prefix}.")))
        && !terminal(&session.state)
    })
    .cloned()
  {
    state
      .repository
      .update_app_session(
        app_id,
        Some(existing.session_id.clone()),
        Some(kind.to_owned()),
        None,
      )
      .await
      .map_err(|error| error.0)?;
    return Ok(existing);
  }
  request.application_id = format!("{application_prefix}.{}", Uuid::new_v4());
  let session = state
    .external_wayland
    .create(&request)
    .await
    .map_err(|error| error.message)?;
  if let Err(error) = state
    .repository
    .update_app_session(
      app_id,
      Some(session.session_id.clone()),
      Some(kind.to_owned()),
      None,
    )
    .await
  {
    let _ = state.external_wayland.terminate(&session.session_id).await;
    return Err(error.0);
  }
  Ok(session)
}

struct ActiveAppOperation {
  state: AppState,
  app_id: String,
}

impl Drop for ActiveAppOperation {
  fn drop(&mut self) {
    self.state.release_app_operation(&self.app_id);
  }
}

pub async fn resume_waiting_tasks(state: AppState) {
  match state.snapshot().await {
    Ok(store) => {
      for task in store.install_tasks.into_iter().filter(|task| {
        task.session_id.is_none()
          && matches!(
            task.status.as_str(),
            "waiting-runtime"
              | "preparing-runtime"
              | "preparing-prefix"
              | "waiting-window-session"
              | "starting-installer"
          )
      }) {
        let task_state = state.clone();
        tokio::spawn(async move {
          prepare_imported_task(task_state, task.id).await;
        });
      }
    }
    Err(error) => eprintln!("WinTone could not restore pending tasks: {}", error.0),
  }
}

pub async fn monitor_sessions(state: AppState) {
  let mut interval = tokio::time::interval(Duration::from_secs(4));
  loop {
    interval.tick().await;
    if !state.external_wayland.is_configured() {
      continue;
    }
    let sessions = match state.external_wayland.list_sessions().await {
      Ok(sessions) => sessions,
      Err(_) => continue,
    };
    let store = match state.snapshot().await {
      Ok(store) => store,
      Err(error) => {
        eprintln!("WinTone could not read session state: {}", error.0);
        continue;
      }
    };

    for task in store
      .install_tasks
      .iter()
      .filter(|task| task.session_id.is_some())
    {
      let Some(session_id) = task.session_id.as_deref() else {
        continue;
      };
      let session = sessions
        .iter()
        .find(|session| session.session_id == session_id);
      match session {
        Some(session) if terminal(&session.state) => {
          finish_install_task(&state, task, session).await;
        }
        None => {
          let lost = ExternalSession {
            session_id: session_id.to_owned(),
            application_id: format!("wintone.install.{}", task.id),
            state: "failed".to_owned(),
            exit_code: None,
            error: Some("The AgentOS session was lost after restart.".to_owned()),
            windows: Vec::new(),
          };
          finish_install_task(&state, task, &lost).await;
        }
        _ => {}
      }
    }

    for app in &store.installed_apps {
      let recovered = if app.session_id.is_none() {
        sessions.iter().rev().find(|session| {
          (session.application_id == format!("wintone.app.{}", app.id)
            || session
              .application_id
              .starts_with(&format!("wintone.app.{}.", app.id))
            || session.application_id == format!("wintone.uninstall.{}", app.id)
            || session
              .application_id
              .starts_with(&format!("wintone.uninstall.{}.", app.id)))
            && !terminal(&session.state)
        })
      } else {
        None
      };
      let recovered_kind = recovered.map(|session| {
        if session.application_id.starts_with("wintone.uninstall.") {
          "uninstall"
        } else {
          "launch"
        }
      });
      let session_id = app
        .session_id
        .as_deref()
        .or_else(|| recovered.map(|session| session.session_id.as_str()));
      let Some(session_id) = session_id else {
        continue;
      };
      if app.session_id.is_none() {
        if let Some(kind) = recovered_kind {
          let _ = state
            .repository
            .update_app_session(
              &app.id,
              Some(session_id.to_owned()),
              Some(kind.to_owned()),
              None,
            )
            .await;
        }
      }
      let session = sessions
        .iter()
        .find(|session| session.session_id == session_id);
      let session = session.cloned().unwrap_or_else(|| ExternalSession {
        session_id: session_id.to_owned(),
        application_id: format!("wintone.app.{}", app.id),
        state: "failed".to_owned(),
        exit_code: None,
        error: Some("The AgentOS session was lost after restart.".to_owned()),
        windows: Vec::new(),
      });
      if terminal(&session.state) {
        let mut app = app.clone();
        app.session_id = Some(session_id.to_owned());
        if app.session_kind.is_none() {
          app.session_kind = recovered_kind.map(str::to_owned);
        }
        finish_app_session(&state, &app, &session).await;
      }
    }

    resume_waiting_tasks(state.clone()).await;
  }
}

async fn finish_install_task(state: &AppState, task: &InstallTask, session: &ExternalSession) {
  if task.status == "completed" || task.status == "failed" {
    return;
  }
  let _ = state
    .repository
    .update_install_task_session(
      &task.id,
      task.session_id.clone(),
      "discovering-apps",
      "discovery",
      "Reading the Wine prefix to find installed Windows apps.",
    )
    .await;
  let prefix = match state.prefix_manager.path(&task.id).await {
    Ok(path) => path,
    Err(error) => {
      let _ = update(state, &task.id, "failed", "discovery", &error).await;
      return;
    }
  };
  let installation_id = task.id.clone();
  let discovery =
    tokio::task::spawn_blocking(move || shortcuts::discover(&prefix, &installation_id)).await;
  let discovered = match discovery {
    Ok(apps) => apps,
    Err(error) => {
      let _ = update(
        state,
        &task.id,
        "failed",
        "discovery",
        &format!("Could not inspect Wine registry: {error}"),
      )
      .await;
      return;
    }
  };
  if !discovered.is_empty() {
    let runtime_version = state
      .runtime_manager
      .installed_runtime()
      .await
      .ok()
      .flatten()
      .map(|runtime| runtime.version)
      .unwrap_or_default();
    let installed_at = now_millis();
    let apps = discovered
      .into_iter()
      .map(|app| InstalledApp {
        id: app.id,
        name: app.name,
        version: app.version,
        installation_id: task.id.clone(),
        executable: app.executable,
        installed_at,
        runtime_version: runtime_version.clone(),
        uninstall_executable: app.uninstall_executable,
        uninstall_args: app.uninstall_args,
        session_id: None,
        session_kind: None,
        last_error: None,
      })
      .collect::<Vec<_>>();
    match state
      .repository
      .replace_apps_for_installation(&task.id, apps.clone())
      .await
    {
      Ok(()) => {
        let message = format!(
          "Installation session ended; discovered {} application entry/entries.",
          apps.len()
        );
        let _ = state
          .repository
          .update_install_task_session(&task.id, None, "completed", "completed", &message)
          .await;
        remove_installer(state, task).await;
        return;
      }
      Err(error) => {
        let _ = update(state, &task.id, "failed", "discovery", &error.0).await;
        return;
      }
    }
  }

  let message = session.error.clone().unwrap_or_else(|| {
    "The installer session ended, but no launchable app was found in the Wine uninstall registry."
      .to_owned()
  });
  let _ = state
    .repository
    .update_install_task_session(&task.id, None, "failed", "discovery", &message)
    .await;
}

async fn finish_app_session(state: &AppState, app: &InstalledApp, session: &ExternalSession) {
  let succeeded = session.state == "exited" && session.exit_code == Some(0);
  if app.session_kind.as_deref() == Some("uninstall") {
    let still_registered = match state.prefix_manager.path(&app.installation_id).await {
      Ok(prefix) => {
        let installation_id = app.installation_id.clone();
        let app_id = app.id.clone();
        tokio::task::spawn_blocking(move || {
          shortcuts::discover(&prefix, &installation_id)
            .iter()
            .any(|entry| entry.id == app_id)
        })
        .await
        .ok()
      }
      Err(_) => Some(false),
    };
    if still_registered == Some(false) || (still_registered.is_none() && succeeded) {
      if let Err(error) = state.repository.remove_installed_app(&app.id).await {
        eprintln!(
          "WinTone could not remove app record {}: {}",
          app.id, error.0
        );
        return;
      }
      if let Ok(store) = state.snapshot().await {
        if !store
          .installed_apps
          .iter()
          .any(|installed| installed.installation_id == app.installation_id)
        {
          if let Err(error) = state.prefix_manager.remove(&app.installation_id).await {
            eprintln!(
              "WinTone could not remove prefix {}: {error}",
              app.installation_id
            );
          }
        }
      }
      return;
    }
    let _ = state
      .repository
      .update_app_session(
        &app.id,
        None,
        None,
        Some("The uninstall session ended, but the app remains in the Wine registry.".to_owned()),
      )
      .await;
    return;
  }
  let error = if succeeded {
    None
  } else {
    session.error.clone().or_else(|| {
      Some(format!(
        "Windows app session exited with code {:?}.",
        session.exit_code
      ))
    })
  };
  let _ = state
    .repository
    .update_app_session(&app.id, None, None, error)
    .await;
}

async fn remove_installer(state: &AppState, task: &InstallTask) {
  let path = state
    .installers_dir()
    .join(format!("{}.{}", task.id, task.extension));
  if let Err(error) = tokio::fs::remove_file(path).await {
    if error.kind() != std::io::ErrorKind::NotFound {
      eprintln!("WinTone could not remove installer {}: {error}", task.id);
    }
  }
}

fn terminal(state: &str) -> bool {
  matches!(state, "exited" | "failed" | "terminated")
}

async fn task_by_id(state: &AppState, task_id: &str) -> Result<Option<InstallTask>, String> {
  state
    .snapshot()
    .await
    .map(|store| {
      store
        .install_tasks
        .into_iter()
        .find(|task| task.id == task_id)
    })
    .map_err(|error| error.0)
}

async fn update(
  state: &AppState,
  task_id: &str,
  status: &str,
  phase: &str,
  message: &str,
) -> Result<bool, String> {
  state
    .repository
    .update_install_task(task_id, status, phase, message)
    .await
    .map_err(|error| error.0)
}

fn now_millis() -> u128 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis()
}
