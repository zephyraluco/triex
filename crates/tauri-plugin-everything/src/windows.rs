use std::{path::Path, ptr, time::Duration};
use tokio::sync::{mpsc, oneshot};

use super::{Error, ItemKind, SearchItem, SearchPage, SearchRequest, Sort, Status};

const REQUEST_FULL_PATH: u32 = 0x4;
const REQUEST_SIZE: u32 = 0x10;
const REQUEST_DATE_MODIFIED: u32 = 0x40;
const ERROR_IPC: u32 = 2;

unsafe extern "system" {
    fn Everything_GetMajorVersion() -> u32;
    fn Everything_GetMinorVersion() -> u32;
    fn Everything_GetRevision() -> u32;
    fn Everything_IsDBLoaded() -> i32;
    fn Everything_GetLastError() -> u32;
    fn Everything_Reset();
    fn Everything_CleanUp();
    fn Everything_SetSearchW(query: *const u16);
    fn Everything_SetMax(max: u32);
    fn Everything_SetOffset(offset: u32);
    fn Everything_SetSort(sort: u32);
    fn Everything_SetRequestFlags(flags: u32);
    fn Everything_QueryW(wait: i32) -> i32;
    fn Everything_GetNumResults() -> u32;
    fn Everything_GetTotResults() -> u32;
    fn Everything_GetResultFullPathNameW(index: u32, buffer: *mut u16, capacity: u32) -> u32;
    fn Everything_IsFolderResult(index: u32) -> i32;
    fn Everything_GetResultSize(index: u32, size: *mut i64) -> i32;
    fn Everything_GetResultDateModified(index: u32, time: *mut FileTime) -> i32;
}

#[derive(Default)]
#[repr(C)]
struct FileTime {
    low: u32,
    high: u32,
}

impl FileTime {
    fn unix_millis(self) -> Option<i64> {
        let ticks = (u64::from(self.high) << 32) | u64::from(self.low);
        (ticks != 0).then(|| (ticks / 10_000) as i64 - 11_644_473_600_000)
    }
}

type Job = Box<dyn FnOnce() + Send>;

pub struct Worker(mpsc::Sender<Job>);

impl Worker {
    pub fn new() -> std::io::Result<Self> {
        let (sender, mut receiver) = mpsc::channel::<Job>(4);
        std::thread::Builder::new()
            .name("everything-sdk".into())
            .spawn(move || {
                while let Some(job) = receiver.blocking_recv() {
                    job();
                }
                // All SDK state is owned by this worker.
                unsafe { Everything_CleanUp() };
            })?;
        Ok(Self(sender))
    }

    async fn submit<T: Send + 'static>(
        &self,
        run: impl FnOnce() -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let (send, receive) = oneshot::channel();
        self.0
            .try_send(Box::new(move || {
                if !send.is_closed() {
                    let _ = send.send(run());
                }
            }))
            .map_err(|err| match err {
                mpsc::error::TrySendError::Full(_) => Error::Busy,
                mpsc::error::TrySendError::Closed(_) => Error::BackendUnavailable,
            })?;
        tokio::time::timeout(Duration::from_secs(3), receive)
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::BackendUnavailable)?
    }

    pub async fn status(&self) -> Result<Status, Error> {
        self.submit(|| Ok(status())).await
    }

    pub async fn search(&self, request: SearchRequest) -> Result<SearchPage, Error> {
        self.submit(move || search(request)).await
    }
}

fn status() -> Status {
    // The IPC API returns zero when it cannot reach the Everything search client.
    unsafe {
        let major = Everything_GetMajorVersion();
        if major == 0 {
            return if Everything_GetLastError() == ERROR_IPC {
                Status::NotRunning
            } else {
                Status::BackendUnavailable
            };
        }
        if (
            major,
            Everything_GetMinorVersion(),
            Everything_GetRevision(),
        ) < (1, 4, 1)
        {
            return Status::BackendUnavailable;
        }
        if Everything_IsDBLoaded() != 0 {
            Status::Ready
        } else if Everything_GetLastError() == ERROR_IPC {
            Status::NotRunning
        } else {
            Status::Indexing
        }
    }
}

fn search(request: SearchRequest) -> Result<SearchPage, Error> {
    match status() {
        Status::Ready => {}
        Status::NotRunning => return Err(Error::NotRunning),
        Status::Indexing => return Err(Error::Indexing),
        _ => return Err(Error::BackendUnavailable),
    }

    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            unsafe { Everything_Reset() };
        }
    }

    let _reset = Reset;
    let query: Vec<_> = request.query.encode_utf16().chain([0]).collect();
    let limit = request.limit.unwrap_or(50);
    // The SDK retains query/result state between calls. Only the worker accesses it.
    unsafe {
        Everything_SetSearchW(query.as_ptr());
        Everything_SetMax(limit);
        Everything_SetOffset(request.offset);
        Everything_SetSort(match request.sort {
            Sort::NameAscending => 1,
            Sort::NameDescending => 2,
            Sort::PathAscending => 3,
            Sort::PathDescending => 4,
            Sort::SizeAscending => 5,
            Sort::SizeDescending => 6,
            Sort::ModifiedAscending => 13,
            Sort::ModifiedDescending => 14,
        });
        Everything_SetRequestFlags(REQUEST_FULL_PATH | REQUEST_SIZE | REQUEST_DATE_MODIFIED);

        if Everything_QueryW(1) == 0 {
            return Err(if Everything_GetLastError() == ERROR_IPC {
                Error::NotRunning
            } else {
                Error::BackendUnavailable
            });
        }
        let count = Everything_GetNumResults().min(limit);
        let total = Everything_GetTotResults();
        let mut items = Vec::with_capacity(count as usize);
        for index in 0..count {
            let len = Everything_GetResultFullPathNameW(index, ptr::null_mut(), 0);
            if len == 0 || len >= 32_768 {
                continue;
            }
            let mut path = vec![0; len as usize + 1];
            if Everything_GetResultFullPathNameW(index, path.as_mut_ptr(), len + 1) != len {
                continue;
            }
            let Ok(path) = String::from_utf16(&path[..len as usize]) else {
                continue;
            };
            let name = Path::new(&path)
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or(&path)
                .to_owned();
            let is_folder = Everything_IsFolderResult(index) != 0;
            let mut size = 0;
            let mut modified = FileTime::default();
            items.push(SearchItem {
                name,
                path,
                kind: if is_folder {
                    ItemKind::Folder
                } else {
                    ItemKind::File
                },
                size_bytes: (!is_folder && Everything_GetResultSize(index, &mut size) != 0)
                    .then(|| u64::try_from(size).ok())
                    .flatten(),
                modified_at: (Everything_GetResultDateModified(index, &mut modified) != 0)
                    .then(|| modified.unix_millis())
                    .flatten(),
            });
        }
        let next = request.offset + count;
        Ok(SearchPage {
            items,
            total,
            next_offset: (count > 0 && next < total).then_some(next),
        })
    }
}
