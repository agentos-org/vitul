use std::{io, path::PathBuf};

use tokio::sync::Mutex;

use crate::model::Store;

#[derive(Clone)]
pub struct StoreRepository {
  path: PathBuf,
  store: std::sync::Arc<Mutex<Store>>,
}

#[derive(Debug)]
pub struct StorageError(pub String);

impl std::fmt::Display for StorageError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(&self.0)
  }
}

impl std::error::Error for StorageError {}

impl From<io::Error> for StorageError {
  fn from(error: io::Error) -> Self {
    Self(error.to_string())
  }
}

impl From<serde_json::Error> for StorageError {
  fn from(error: serde_json::Error) -> Self {
    Self(error.to_string())
  }
}

impl StoreRepository {
  pub async fn open(path: PathBuf) -> Result<Self, StorageError> {
    let mut store = match tokio::fs::read(&path).await {
      Ok(bytes) => serde_json::from_slice::<Store>(&bytes)?,
      Err(error) if error.kind() == io::ErrorKind::NotFound => Store {
        schema_version: 2,
        ..Store::default()
      },
      Err(error) => return Err(error.into()),
    };
    let needs_migration = store.schema_version < 2;
    if needs_migration {
      store.schema_version = 2;
    }
    let repository = Self {
      path,
      store: std::sync::Arc::new(Mutex::new(store)),
    };
    if needs_migration {
      let snapshot = repository.snapshot().await?;
      repository.persist(&snapshot).await?;
    }
    Ok(repository)
  }

  pub async fn snapshot(&self) -> Result<Store, StorageError> {
    Ok(self.store.lock().await.clone())
  }

  pub async fn insert_install_task(
    &self,
    task: crate::model::InstallTask,
  ) -> Result<(), StorageError> {
    let mut store = self.store.lock().await;
    store.install_tasks.push(task);
    if let Err(error) = self.persist(&store).await {
      store.install_tasks.pop();
      return Err(error);
    }
    Ok(())
  }

  pub async fn remove_install_task(
    &self,
    id: &str,
  ) -> Result<Option<crate::model::InstallTask>, StorageError> {
    let mut store = self.store.lock().await;
    let Some(index) = store.install_tasks.iter().position(|task| task.id == id) else {
      return Ok(None);
    };
    let removed = store.install_tasks.remove(index);
    if let Err(error) = self.persist(&store).await {
      store.install_tasks.insert(index, removed.clone());
      return Err(error);
    }
    Ok(Some(removed))
  }

  pub async fn update_install_task(
    &self,
    id: &str,
    status: &str,
    phase: &str,
    message: &str,
  ) -> Result<bool, StorageError> {
    let mut store = self.store.lock().await;
    let Some(task) = store.install_tasks.iter_mut().find(|task| task.id == id) else {
      return Ok(false);
    };
    let previous = task.clone();
    task.status = status.to_owned();
    task.phase = phase.to_owned();
    task.message = message.to_owned();
    if let Err(error) = self.persist(&store).await {
      if let Some(task) = store.install_tasks.iter_mut().find(|task| task.id == id) {
        *task = previous;
      }
      return Err(error);
    }
    Ok(true)
  }

  pub async fn update_install_task_session(
    &self,
    id: &str,
    session_id: Option<String>,
    status: &str,
    phase: &str,
    message: &str,
  ) -> Result<bool, StorageError> {
    let mut store = self.store.lock().await;
    let Some(task) = store.install_tasks.iter_mut().find(|task| task.id == id) else {
      return Ok(false);
    };
    let previous = task.clone();
    task.session_id = session_id;
    task.status = status.to_owned();
    task.phase = phase.to_owned();
    task.message = message.to_owned();
    if let Err(error) = self.persist(&store).await {
      if let Some(task) = store.install_tasks.iter_mut().find(|task| task.id == id) {
        *task = previous;
      }
      return Err(error);
    }
    Ok(true)
  }

  pub async fn replace_apps_for_installation(
    &self,
    installation_id: &str,
    apps: Vec<crate::model::InstalledApp>,
  ) -> Result<(), StorageError> {
    let mut store = self.store.lock().await;
    let previous = store.installed_apps.clone();
    store
      .installed_apps
      .retain(|app| app.installation_id != installation_id);
    store.installed_apps.extend(apps);
    if let Err(error) = self.persist(&store).await {
      store.installed_apps = previous;
      return Err(error);
    }
    Ok(())
  }

  pub async fn update_app_session(
    &self,
    id: &str,
    session_id: Option<String>,
    session_kind: Option<String>,
    last_error: Option<String>,
  ) -> Result<bool, StorageError> {
    let mut store = self.store.lock().await;
    let Some(app) = store.installed_apps.iter_mut().find(|app| app.id == id) else {
      return Ok(false);
    };
    let previous = app.clone();
    app.session_id = session_id;
    app.session_kind = session_kind;
    app.last_error = last_error;
    if let Err(error) = self.persist(&store).await {
      if let Some(app) = store.installed_apps.iter_mut().find(|app| app.id == id) {
        *app = previous;
      }
      return Err(error);
    }
    Ok(true)
  }

  pub async fn remove_installed_app(
    &self,
    id: &str,
  ) -> Result<Option<crate::model::InstalledApp>, StorageError> {
    let mut store = self.store.lock().await;
    let Some(index) = store.installed_apps.iter().position(|app| app.id == id) else {
      return Ok(None);
    };
    let removed = store.installed_apps.remove(index);
    if let Err(error) = self.persist(&store).await {
      store.installed_apps.insert(index, removed.clone());
      return Err(error);
    }
    Ok(Some(removed))
  }

  async fn persist(&self, store: &Store) -> Result<(), StorageError> {
    let temporary = self.path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(store)?;
    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(temporary, &self.path).await?;
    Ok(())
  }
}
