use serde::{Deserialize, Serialize};
use tauri::{
    Manager, Runtime, State,
    plugin::{Builder, TauriPlugin},
};

#[cfg(windows)]
mod index;
#[cfg(windows)]
mod journal;
#[cfg(windows)]
mod service;
#[cfg(windows)]
mod snapshot;
#[cfg(windows)]
mod tree;
#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sort {
    #[default]
    NameAscending,
    NameDescending,
    PathAscending,
    PathDescending,
    SizeAscending,
    SizeDescending,
    ModifiedAscending,
    ModifiedDescending,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub offset: u32,
    pub limit: Option<u32>,
    #[serde(default)]
    pub sort: Sort,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub items: Vec<SearchItem>,
    pub total: u32,
    pub next_offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchItem {
    pub name: String,
    pub path: String,
    pub kind: ItemKind,
    pub size_bytes: Option<u64>,
    /// Unix timestamp in milliseconds.
    pub modified_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    File,
    Folder,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Ready,
    Indexing,
    NotRunning,
    SnapshotMissing,
    SnapshotUnsupported,
    SnapshotStale,
    ServiceIncompatible,
    UnsupportedPlatform,
    BackendUnavailable,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Error {
    InvalidRequest,
    NotRunning,
    SnapshotMissing,
    SnapshotUnsupported,
    SnapshotStale,
    ServiceIncompatible,
    Indexing,
    UnsupportedPlatform,
    BackendUnavailable,
    Busy,
    Timeout,
}

pub struct Everything {
    #[cfg(windows)]
    backend: windows::Worker,
}

impl Everything {
    pub async fn status(&self) -> Result<Status, Error> {
        #[cfg(windows)]
        return self.backend.status().await;
        #[cfg(not(windows))]
        Ok(Status::UnsupportedPlatform)
    }

    pub async fn search(&self, mut request: SearchRequest) -> Result<SearchPage, Error> {
        if request.query.len() > 4096 || request.query.contains('\0') {
            return Err(Error::InvalidRequest);
        }
        request.limit = Some(request.limit.unwrap_or(50).clamp(1, 200));
        if request.offset > u32::MAX - request.limit.unwrap_or_default() {
            return Err(Error::InvalidRequest);
        }
        #[cfg(windows)]
        {
            if request.query.trim().is_empty() {
                return Ok(SearchPage {
                    items: vec![],
                    total: 0,
                    next_offset: None,
                });
            }
            self.backend.search(request).await
        }
        #[cfg(not(windows))]
        Err(Error::UnsupportedPlatform)
    }
}

pub trait EverythingExt<R: Runtime> {
    fn everything(&self) -> State<'_, Everything>;
}

impl<R: Runtime, T: Manager<R>> EverythingExt<R> for T {
    fn everything(&self) -> State<'_, Everything> {
        self.state()
    }
}

mod commands {
    use super::*;

    #[tauri::command]
    pub async fn status(state: State<'_, Everything>) -> Result<Status, Error> {
        state.status().await
    }

    #[tauri::command]
    pub async fn search(
        state: State<'_, Everything>,
        request: SearchRequest,
    ) -> Result<SearchPage, Error> {
        state.search(request).await
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("everything")
        .setup(|app, _| {
            app.manage(Everything {
                #[cfg(windows)]
                backend: windows::Worker::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::status, commands::search])
        .build()
}
