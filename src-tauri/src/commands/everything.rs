use std::{
    path::{Component, Path, Prefix},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Manager, Webview};
use tauri_plugin_everything::{EverythingExt, SearchItem, SearchRequest, Status};
use tauri_plugin_opener::OpenerExt;

use crate::{
    error::{AppError, AppResult},
    plugins::{EVERYTHING_ID, permission::require_permission},
    state::AppState,
};

pub const EMBEDDED_PREFIX: &str = "triex-builtin-everything:";
pub const DETACHED_LABEL: &str = "triex-builtin-everything-detached";
pub const ENTRY: &str = "everything.html";
const RESULT_TTL: Duration = Duration::from_secs(300);

#[derive(Default)]
pub struct Results {
    embedded: Mutex<ResultPage>,
    detached: Mutex<ResultPage>,
    next_generation: AtomicU64,
}

#[derive(Default)]
struct ResultPage {
    sequence: u64,
    generation: u64,
    expires_at: Option<Instant>,
    paths: Vec<String>,
}

impl ResultPage {
    fn resolve(&self, id: &str, now: Instant) -> Result<&str, Error> {
        let (generation, index) = id.split_once(':').ok_or(Error::StaleResult)?;
        let generation = generation.parse::<u64>().map_err(|_| Error::StaleResult)?;
        let index = index.parse::<usize>().map_err(|_| Error::StaleResult)?;
        if self.generation != generation || self.expires_at.is_none_or(|expires| now >= expires) {
            return Err(Error::StaleResult);
        }
        self.paths
            .get(index)
            .map(String::as_str)
            .ok_or(Error::StaleResult)
    }
}

impl Results {
    fn page(&self, label: &str) -> &Mutex<ResultPage> {
        if label == DETACHED_LABEL {
            &self.detached
        } else {
            &self.embedded
        }
    }
}

pub fn clear_results(app: &AppHandle, label: &str) {
    if let Ok(mut page) = app.state::<Results>().page(label).lock() {
        *page = ResultPage::default();
    }
}

pub fn close_detached(app: &AppHandle) {
    clear_results(app, DETACHED_LABEL);
    if let Some(window) = app.get_webview_window(DETACHED_LABEL) {
        let _ = window.destroy();
    }
}

pub fn is_builtin_url(app: &AppHandle, url: &tauri::Url) -> bool {
    if url.path() != "/everything.html" || url.query().is_some() {
        return false;
    }
    if tauri::is_dev() {
        return app.config().build.dev_url.as_ref().is_some_and(|dev| {
            url.scheme() == dev.scheme()
                && url.host_str() == dev.host_str()
                && url.port_or_known_default() == dev.port_or_known_default()
        });
    }
    matches!(
        (url.scheme(), url.host_str()),
        ("http", Some("tauri.localhost")) | ("tauri", Some("localhost"))
    ) && url.port().is_none()
}

#[derive(Debug, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Error {
    PermissionDenied,
    BackendUnavailable,
    StaleResult,
    FileUnavailable,
    OpenFailed,
    #[serde(untagged)]
    Search(tauri_plugin_everything::Error),
}

impl From<tauri_plugin_everything::Error> for Error {
    fn from(error: tauri_plugin_everything::Error) -> Self {
        Self::Search(error)
    }
}

fn authorize(webview: &Webview, app: &AppHandle) -> Result<(), Error> {
    let state = app.state::<AppState>();
    let label = webview.label();
    let owns_session = label == DETACHED_LABEL
        || (label.starts_with(EMBEDDED_PREFIX)
            && state.active_plugin().is_some_and(|session| {
                session.plugin_id == EVERYTHING_ID && session.webview_label == label
            }));
    if !owns_session || !webview.url().is_ok_and(|url| is_builtin_url(app, &url)) {
        return Err(Error::PermissionDenied);
    }
    require_permission(&state.db, EVERYTHING_ID, "everything").map_err(|_| Error::PermissionDenied)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    rows: Vec<Row>,
    total: u32,
    next_offset: Option<u32>,
}

#[derive(Serialize)]
struct Row {
    id: String,
    #[serde(flatten)]
    item: SearchItem,
}

#[tauri::command]
pub async fn everything_status(webview: Webview, app: AppHandle) -> Result<Status, Error> {
    authorize(&webview, &app)?;
    Ok(app.everything().status().await?)
}

#[tauri::command]
pub fn everything_clear(sequence: u64, webview: Webview, app: AppHandle) -> Result<(), Error> {
    authorize(&webview, &app)?;
    let results = app.state::<Results>();
    let mut page = results
        .page(webview.label())
        .lock()
        .map_err(|_| Error::BackendUnavailable)?;
    if sequence <= page.sequence {
        return Err(Error::StaleResult);
    }
    *page = ResultPage {
        sequence,
        ..Default::default()
    };
    Ok(())
}

#[tauri::command]
pub async fn everything_search(
    request: SearchRequest,
    sequence: u64,
    webview: Webview,
    app: AppHandle,
) -> Result<Page, Error> {
    authorize(&webview, &app)?;
    let results = app.state::<Results>();
    let page = results.page(webview.label());
    let generation = results.next_generation.fetch_add(1, Ordering::Relaxed) + 1;
    {
        let mut page = page.lock().map_err(|_| Error::BackendUnavailable)?;
        if page.sequence != sequence || page.generation != 0 {
            return Err(Error::StaleResult);
        }
        page.generation = generation;
    }
    let found = app.everything().search(request).await?;
    authorize(&webview, &app)?;

    let mut page = page.lock().map_err(|_| Error::BackendUnavailable)?;
    if page.generation != generation {
        return Err(Error::StaleResult);
    }
    page.expires_at = Some(Instant::now() + RESULT_TTL);
    page.paths = found.items.iter().map(|item| item.path.clone()).collect();
    let rows = found
        .items
        .into_iter()
        .enumerate()
        .map(|(index, item)| Row {
            id: format!("{generation}:{index}"),
            item,
        })
        .collect();
    Ok(Page {
        rows,
        total: found.total,
        next_offset: found.next_offset,
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Open,
    Reveal,
}

fn existing_path(path: &str) -> AppResult<()> {
    let path = Path::new(path);
    let filesystem_path = matches!(path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(),
            Prefix::Disk(_) | Prefix::VerbatimDisk(_) | Prefix::UNC(..) | Prefix::VerbatimUNC(..)));
    if !filesystem_path || !path.is_absolute() || path.as_os_str().to_string_lossy().contains('\0')
    {
        return Err(AppError::InvalidInput(
            "Expected an absolute filesystem path".into(),
        ));
    }
    std::fs::metadata(path)?;
    Ok(())
}

#[tauri::command]
pub async fn everything_open_result(
    id: String,
    action: Action,
    webview: Webview,
    app: AppHandle,
) -> Result<(), Error> {
    authorize(&webview, &app)?;
    let path = {
        let state = app.state::<Results>();
        let page = state
            .page(webview.label())
            .lock()
            .map_err(|_| Error::BackendUnavailable)?;
        page.resolve(&id, Instant::now())?.to_owned()
    };
    tauri::async_runtime::spawn_blocking(move || {
        existing_path(&path).map_err(|_| Error::FileUnavailable)?;
        match action {
            Action::Open => app.opener().open_path(path, None::<&str>),
            Action::Reveal => app.opener().reveal_item_in_dir(path),
        }
        .map_err(|_| Error::OpenFailed)
    })
    .await
    .map_err(|_| Error::BackendUnavailable)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_ids_only_resolve_for_the_current_page_and_lifetime() {
        let now = Instant::now();
        let page = ResultPage {
            sequence: 1,
            generation: 9,
            expires_at: Some(now + RESULT_TTL),
            paths: vec![r"C:\文档\报告.pdf".into()],
        };

        assert_eq!(page.resolve("9:0", now).unwrap(), r"C:\文档\报告.pdf");
        assert!(page.resolve("8:0", now).is_err());
        assert!(page.resolve("9:1", now).is_err());
        assert!(page.resolve(r"C:\文档\报告.pdf", now).is_err());
        assert!(page.resolve("9:0", now + RESULT_TTL).is_err());
        assert!(ResultPage::default().resolve("9:0", now).is_err());
    }
}
