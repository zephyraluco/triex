use std::{
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, RwLock},
    time::{Duration, SystemTime},
};

use tokio::sync::Semaphore;
use walkdir::WalkDir;
use windows::{
    Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives},
    core::PCWSTR,
};

use crate::{
    Error, ItemKind, SearchItem, SearchPage, SearchRequest, Status,
    index::{self, Entry},
};

const DRIVE_FIXED: u32 = 3;
const REPARSE_POINT: u32 = 0x400;
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

type Snapshot = Arc<Vec<Entry>>;
type IndexState = RwLock<Result<Option<Snapshot>, Error>>;

pub(crate) struct Worker {
    index: Arc<IndexState>,
    started: OnceLock<bool>,
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
    fn start(&self) -> Result<(), Error> {
        if !*self.started.get_or_init(|| {
            let index = Arc::downgrade(&self.index);
            std::thread::Builder::new()
                .name("triex-file-index".into())
                .spawn(move || {
                    while index.strong_count() != 0 {
                        let scanned = scan();
                        let Some(index) = index.upgrade() else { break };
                        if let Ok(mut state) = index.write() {
                            match scanned {
                                Ok(entries) => *state = Ok(Some(Arc::new(entries))),
                                Err(error) if !matches!(*state, Ok(Some(_))) => *state = Err(error),
                                Err(_) => {}
                            }
                        }
                        drop(index);
                        std::thread::sleep(REFRESH_INTERVAL);
                    }
                })
                .is_ok()
        }) {
            return Err(Error::BackendUnavailable);
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<Option<Snapshot>, Error> {
        self.start()?;
        self.index
            .read()
            .map_err(|_| Error::BackendUnavailable)?
            .clone()
    }

    pub(crate) async fn status(&self) -> Result<Status, Error> {
        match self.snapshot() {
            Ok(Some(_)) => Ok(Status::Ready),
            Ok(None) => Ok(Status::Indexing),
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

fn scan() -> Result<Vec<Entry>, Error> {
    let drives = unsafe { GetLogicalDrives() };
    if drives == 0 {
        return Err(Error::BackendUnavailable);
    }
    let mut entries = Vec::new();
    for drive in 0..26 {
        if drives & (1 << drive) == 0 {
            continue;
        }
        let letter = b'A' + drive as u8;
        let wide = [u16::from(letter), b':' as u16, b'\\' as u16, 0];
        if unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) } != DRIVE_FIXED {
            continue;
        }
        let root = PathBuf::from(format!("{}:\\", char::from(letter)));
        scan_root(&root, &mut entries);
    }
    Ok(entries)
}

fn scan_root(root: &Path, entries: &mut Vec<Entry>) {
    for entry in WalkDir::new(root)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            // Junctions and other directory reparse points can cross volume boundaries.
            !entry.file_type().is_dir()
                || entry
                    .metadata()
                    .is_ok_and(|m| m.file_attributes() & REPARSE_POINT == 0)
        })
        .filter_map(Result::ok)
    {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Some(name) = entry.file_name().to_str() else {
            continue;
        };
        let Some(path) = entry.path().to_str() else {
            continue;
        };
        let kind = if metadata.is_dir() {
            ItemKind::Folder
        } else {
            ItemKind::File
        };
        let modified_at = metadata.modified().ok().and_then(|modified| {
            let elapsed = modified.duration_since(SystemTime::UNIX_EPOCH);
            let millis = match elapsed {
                Ok(duration) => i128::try_from(duration.as_millis()).ok()?,
                Err(before) => -i128::try_from(before.duration().as_millis()).ok()?,
            };
            i64::try_from(millis).ok()
        });
        entries.push(Entry::new(SearchItem {
            name: name.into(),
            path: path.into(),
            kind,
            size_bytes: (kind == ItemKind::File).then_some(metadata.len()),
            modified_at,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sort;

    #[test]
    fn scans_a_windows_directory_and_searches_without_an_external_process() {
        let fixture = tempfile::tempdir().unwrap();
        let folder = fixture.path().join("财务");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("年度报告.PDF"), b"test").unwrap();
        std::fs::write(folder.join("草稿.txt"), b"other").unwrap();

        let query = || SearchRequest {
            query: "ext:pdf 年度".into(),
            offset: 0,
            limit: Some(10),
            sort: Sort::NameAscending,
        };
        let mut entries = Vec::new();
        scan_root(fixture.path(), &mut entries);
        let page = index::search(&entries, query()).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].name, "年度报告.PDF");
        assert_eq!(page.items[0].size_bytes, Some(4));
        assert!(page.items[0].modified_at.is_some());

        std::fs::remove_file(folder.join("年度报告.PDF")).unwrap();
        entries.clear();
        scan_root(fixture.path(), &mut entries);
        assert_eq!(index::search(&entries, query()).unwrap().total, 0);
    }
}
