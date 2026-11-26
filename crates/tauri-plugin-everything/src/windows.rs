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
    snapshot: NativeSnapshot,
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
    let expected = match identity(path) {
        Ok(identity) => Some(identity),
        Err(Error::SnapshotMissing) => None,
        Err(error) => return Err(error),
    };
    if cached
        .as_ref()
        .is_none_or(|cache| expected != Some((cache.modified, cache.len)))
    {
        let snapshot = if let Some(identity) = expected {
            let path = path.to_owned();
            tauri::async_runtime::spawn_blocking(move || load(&path, identity))
                .await
                .map_err(|_| Error::BackendUnavailable)??
        } else {
            service::rebuild().await?
        };
        *cached = Some(
            match synchronize(path, expected, snapshot, expected.is_none()).await {
                Err(Error::SnapshotStale) if expected.is_some() => {
                    synchronize(path, expected, service::rebuild().await?, true).await?
                }
                result => result?,
            },
        );
    } else {
        let mut pending = Vec::new();
        for volume in &cached
            .as_ref()
            .ok_or(Error::BackendUnavailable)?
            .snapshot
            .volumes
        {
            match service::updates(volume).await {
                Ok(batch) => pending.push(batch),
                Err(Error::SnapshotStale) => {
                    let rebuilt = service::rebuild().await?;
                    *cached = Some(synchronize(path, expected, rebuilt, true).await?);
                    return Ok(cached
                        .as_ref()
                        .ok_or(Error::BackendUnavailable)?
                        .entries
                        .clone());
                }
                Err(error) => return Err(error),
            }
        }
        if pending
            .iter()
            .zip(
                &cached
                    .as_ref()
                    .ok_or(Error::BackendUnavailable)?
                    .snapshot
                    .volumes,
            )
            .any(|((next, _), volume)| *next != volume.start_usn)
        {
            let mut snapshot = cached.take().ok_or(Error::BackendUnavailable)?.snapshot;
            let persist = has_disk_changes(path, &snapshot, &pending)?;
            match apply_changes(&mut snapshot, pending) {
                Ok(()) => *cached = Some(publish(path, expected, snapshot, persist).await?),
                Err(Error::SnapshotStale) => {
                    let rebuilt = service::rebuild().await?;
                    *cached = Some(synchronize(path, expected, rebuilt, true).await?);
                }
                Err(error) => return Err(error),
            }
        }
    }
    Ok(cached
        .as_ref()
        .ok_or(Error::BackendUnavailable)?
        .entries
        .clone())
}

type Identity = (std::time::SystemTime, u64);

fn identity(path: &std::path::Path) -> Result<Identity, Error> {
    let metadata = std::fs::metadata(path).map_err(file_error)?;
    Ok((metadata.modified().map_err(file_error)?, metadata.len()))
}

fn load(path: &std::path::Path, expected: Identity) -> Result<NativeSnapshot, Error> {
    if expected.1 > 512 * 1024 * 1024 {
        return Err(Error::SnapshotUnsupported);
    }
    let bytes = std::fs::read(path).map_err(file_error)?;
    if identity(path)? != expected {
        return Err(Error::Busy);
    }
    NativeSnapshot::decode(&bytes)
}

async fn synchronize(
    path: &std::path::Path,
    expected: Option<Identity>,
    mut snapshot: NativeSnapshot,
    force_save: bool,
) -> Result<Cached, Error> {
    let mut pending = Vec::new();
    for volume in &snapshot.volumes {
        pending.push(service::updates(volume).await?);
    }
    if !force_save
        && expected.is_some()
        && pending
            .iter()
            .zip(&snapshot.volumes)
            .all(|((next, _), volume)| *next == volume.start_usn)
    {
        let (modified, len) = expected.ok_or(Error::BackendUnavailable)?;
        return Ok(Cached {
            modified,
            len,
            entries: Arc::new(snapshot.entries()?),
            snapshot,
        });
    }
    let persist = force_save || expected.is_none() || has_disk_changes(path, &snapshot, &pending)?;
    apply_changes(&mut snapshot, pending)?;
    publish(path, expected, snapshot, persist).await
}

fn has_disk_changes(
    path: &std::path::Path,
    snapshot: &NativeSnapshot,
    pending: &[(u64, Vec<crate::journal::Change>)],
) -> Result<bool, Error> {
    let Some(parent) = path.parent() else {
        return Err(Error::BackendUnavailable);
    };
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(Error::BackendUnavailable)?;
    let backup = path.with_extension("db.triex-backup");
    let backup_name = backup
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(Error::BackendUnavailable)?;
    let parent = parent.to_string_lossy();
    for ((_, changes), (volume, tree)) in pending
        .iter()
        .zip(snapshot.volumes.iter().zip(&snapshot.trees))
    {
        if changes.is_empty() {
            continue;
        }
        if changes.iter().any(|change| {
            !change.name.eq_ignore_ascii_case(filename)
                && !change.name.eq_ignore_ascii_case(backup_name)
                && !change.name.starts_with(".triex-everything-")
        }) {
            return Ok(true);
        }
        let paths = tree.paths(&volume.drive)?;
        if changes.iter().any(|change| {
            paths
                .get(&change.parent)
                .is_none_or(|directory| !directory.eq_ignore_ascii_case(&parent))
        }) {
            return Ok(true);
        }
    }
    // Saving creates USN records for the database, backup and staging file.
    // Consume those records in memory without starting another save.
    Ok(false)
}

fn apply_changes(
    snapshot: &mut NativeSnapshot,
    pending: Vec<(u64, Vec<crate::journal::Change>)>,
) -> Result<(), Error> {
    for ((volume, tree), (next, changes)) in snapshot
        .volumes
        .iter_mut()
        .zip(&mut snapshot.trees)
        .zip(pending)
    {
        if !changes.is_empty() {
            tree.replay(volume, changes)?;
        }
        volume.start_usn = next;
    }
    Ok(())
}

async fn publish(
    path: &std::path::Path,
    expected: Option<Identity>,
    snapshot: NativeSnapshot,
    persist: bool,
) -> Result<Cached, Error> {
    let path = path.to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let entries = Arc::new(snapshot.entries()?);
        let (modified, len) = if persist {
            save(&path, expected, &snapshot)?;
            identity(&path)?
        } else {
            expected.ok_or(Error::BackendUnavailable)?
        };
        Ok(Cached {
            modified,
            len,
            entries,
            snapshot,
        })
    })
    .await
    .map_err(|_| Error::BackendUnavailable)?
}

fn save(
    path: &std::path::Path,
    expected: Option<Identity>,
    snapshot: &NativeSnapshot,
) -> Result<(), Error> {
    use std::io::{BufWriter, Write};
    let parent = path.parent().ok_or(Error::BackendUnavailable)?;
    std::fs::create_dir_all(parent).map_err(file_error)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".triex-everything-")
        .tempfile_in(parent)
        .map_err(file_error)?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        snapshot.write_to(&mut writer).map_err(file_error)?;
        writer.flush().map_err(file_error)?;
    }
    temporary.as_file().sync_all().map_err(file_error)?;
    if let Some(expected) = expected {
        if identity(path)? != expected {
            return Err(Error::Busy);
        }
        let backup = path.with_extension("db.triex-backup");
        if !backup.exists() {
            let mut copy = tempfile::Builder::new()
                .prefix(".triex-everything-")
                .tempfile_in(parent)
                .map_err(file_error)?;
            let mut source = std::fs::File::open(path).map_err(file_error)?;
            std::io::copy(&mut source, copy.as_file_mut()).map_err(file_error)?;
            copy.as_file().sync_all().map_err(file_error)?;
            match copy.persist_noclobber(&backup) {
                Ok(_) => {}
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(file_error(error.error)),
            }
        }
        if identity(path)? != expected {
            return Err(Error::Busy);
        }
        temporary.persist(path).map_err(|e| file_error(e.error))?;
    } else {
        temporary
            .persist_noclobber(path)
            .map_err(|e| file_error(e.error))?;
    }
    Ok(())
}

fn file_error(error: std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        Error::SnapshotMissing
    } else {
        Error::BackendUnavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        snapshot::Volume,
        tree::{Folder, Tree},
    };

    #[test]
    fn creates_without_clobbering_and_backs_up_before_replacing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Everything.db");
        let volume = Volume {
            guid: r"\\?\Volume{11111111-1111-1111-1111-111111111111}".into(),
            drive: "C:".into(),
            journal_id: 1,
            start_usn: 7,
        };
        let mut tree = Tree::default();
        tree.folders.insert(
            5,
            Folder {
                parent: None,
                name: "C:".into(),
                modified: 0,
            },
        );
        let mut snapshot = NativeSnapshot {
            volumes: vec![volume],
            trees: vec![tree],
        };
        save(&path, None, &snapshot).unwrap();
        let old = std::fs::read(&path).unwrap();
        assert!(save(&path, None, &snapshot).is_err());
        let expected = identity(&path).unwrap();
        snapshot.volumes[0].start_usn = 19;
        save(&path, Some(expected), &snapshot).unwrap();
        let backup = std::fs::read(path.with_extension("db.triex-backup")).unwrap();
        assert_eq!(old, backup);
        let result = NativeSnapshot::decode(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(result.volumes[0].start_usn, 19);
    }
}
