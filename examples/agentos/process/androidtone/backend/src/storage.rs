use std::{io, path::PathBuf};

use tokio::sync::Mutex;

use crate::model::{InstallTask, Store};

#[derive(Clone)]
pub struct Repository {
  path: PathBuf,
  store: std::sync::Arc<Mutex<Store>>,
}

impl Repository {
  pub async fn open(path: PathBuf) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
    let store = match tokio::fs::read(&path).await {
      Ok(bytes) => serde_json::from_slice(&bytes)?,
      Err(error) if error.kind() == io::ErrorKind::NotFound => Store {
        schema_version: 1,
        ..Store::default()
      },
      Err(error) => return Err(error.into()),
    };
    Ok(Self {
      path,
      store: std::sync::Arc::new(Mutex::new(store)),
    })
  }

  pub async fn snapshot(&self) -> Store {
    self.store.lock().await.clone()
  }

  pub async fn insert(
    &self,
    task: InstallTask,
  ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut store = self.store.lock().await;
    store.install_tasks.push(task);
    if let Err(error) = self.persist(&store).await {
      store.install_tasks.pop();
      return Err(error);
    }
    Ok(())
  }

  pub async fn remove(
    &self,
    id: &str,
  ) -> Result<Option<InstallTask>, Box<dyn std::error::Error + Send + Sync>> {
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

  pub async fn update_status(
    &self,
    id: &str,
    status: &str,
    message: &str,
  ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut store = self.store.lock().await;
    if let Some(task) = store.install_tasks.iter_mut().find(|task| task.id == id) {
      task.status = status.to_owned();
      task.message = message.to_owned();
      self.persist(&store).await?;
    }
    Ok(())
  }

  async fn persist(&self, store: &Store) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bytes = serde_json::to_vec_pretty(store)?;
    let temporary = self.path.with_extension("json.tmp");
    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(temporary, &self.path).await?;
    Ok(())
  }
}
