use std::{
    path::PathBuf,
    sync::{Arc, OnceLock, RwLock},
    time::Duration,
};

use crate::{
    Error, SearchPage, SearchRequest, Status,
    index::{self, Entry},
    service,
    snapshot::Snapshot as NativeSnapshot,
};
use tokio::sync::Semaphore;

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);

type Snapshot = Arc<Vec<Entry>>;
type IndexState = RwLock<Result<Option<Snapshot>, Error>>;

pub(crate) struct Worker {
    index: Arc<IndexState>,
    started: OnceLock<()>,
    searches: Arc<Semaphore>,
}

impl Default for Worker {
    fn default() -> Self {
        Self {
            index: Arc::new(RwLock::new(Ok(None))),
            started: OnceLock::new(),
            searches: Arc::new(Semaphore::new(2)),
        }
    }
}

impl Worker {
    fn start(&self) {
        self.started.get_or_init(|| {
            let index = Arc::downgrade(&self.index);
            tauri::async_runtime::spawn(async move {
                let path = snapshot_path();
                let mut cached = None;
                while let Some(index) = index.upgrade() {
                    let result = refresh(&path, &mut cached).await;
                    if let Ok(mut state) = index.write() {
                        *state = result.map(Some);
                    }
                    drop(index);
                    tokio::time::sleep(REFRESH_INTERVAL).await;
                }
            });
        });
    }

    fn snapshot(&self) -> Result<Option<Snapshot>, Error> {
        self.start();
        self.index
            .read()
            .map_err(|_| Error::BackendUnavailable)?
            .clone()
    }

    pub(crate) async fn status(&self) -> Result<Status, Error> {
        match self.snapshot() {
            Ok(Some(_)) => Ok(Status::Ready),
            Ok(None) => Ok(Status::Indexing),
            Err(Error::NotRunning) => Ok(Status::NotRunning),
            Err(Error::SnapshotMissing) => Ok(Status::SnapshotMissing),
            Err(Error::SnapshotUnsupported) => Ok(Status::SnapshotUnsupported),
            Err(Error::SnapshotStale) => Ok(Status::SnapshotStale),
            Err(Error::ServiceIncompatible) => Ok(Status::ServiceIncompatible),
            Err(_) => Ok(Status::BackendUnavailable),
        }
    }

    pub(crate) async fn search(&self, request: SearchRequest) -> Result<SearchPage, Error> {
        let snapshot = self.snapshot()?.ok_or(Error::Indexing)?;
        let permit = self
            .searches
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        let work = tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            index::search(&snapshot, request)
        });
        tokio::time::timeout(Duration::from_secs(3), work)
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::BackendUnavailable)?
    }
}

struct Cached {
    modified: std::time::SystemTime,
    len: u64,
    volumes: Vec<crate::snapshot::Volume>,
    entries: Snapshot,
}

fn snapshot_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TRIEX_EVERYTHING_DB") {
        return Some(path.into());
    }
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join("Everything")
            .join("Everything.db"),
    )
}

async fn refresh(path: &Option<PathBuf>, cached: &mut Option<Cached>) -> Result<Snapshot, Error> {
    let path = path.as_ref().ok_or(Error::SnapshotMissing)?;
    let metadata = std::fs::metadata(path).map_err(file_error)?;
    let modified = metadata.modified().map_err(file_error)?;
    let len = metadata.len();
    if cached
        .as_ref()
        .is_none_or(|cache| cache.modified != modified || cache.len != len)
    {
        // A new generation is published only after both the format and live service are validated.
        let path = path.to_owned();
        let parsed = tauri::async_runtime::spawn_blocking(move || {
            if len > 512 * 1024 * 1024 {
                return Err(Error::SnapshotUnsupported);
            }
            let bytes = std::fs::read(&path).map_err(file_error)?;
            let after = std::fs::metadata(&path).map_err(file_error)?;
            if after.len() != len || after.modified().map_err(file_error)? != modified {
                return Err(Error::Busy);
            }
            NativeSnapshot::decode(&bytes)
        })
        .await
        .map_err(|_| Error::BackendUnavailable)??;
        *cached = Some(Cached {
            modified,
            len,
            volumes: parsed.volumes,
            entries: Arc::new(parsed.entries),
        });
    }
    let cache = cached.as_ref().ok_or(Error::BackendUnavailable)?;
    service::verify_checkpoints(&cache.volumes).await?;
    Ok(cache.entries.clone())
}

fn file_error(error: std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        Error::SnapshotMissing
    } else {
        Error::BackendUnavailable
    }
}
