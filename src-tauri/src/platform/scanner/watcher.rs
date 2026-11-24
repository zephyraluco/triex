use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

/// Watches Start Menu and custom application directories, firing `on_change` with debouncing
/// when applications are installed or removed.
pub fn spawn_directory_watcher<F>(dirs: Vec<PathBuf>, on_change: F) -> Option<RecommendedWatcher>
where
    F: Fn() + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        Config::default(),
    )
    .ok()?;

    let mut watched_any = false;
    for dir in dirs {
        if dir.is_dir() && watcher.watch(&dir, RecursiveMode::Recursive).is_ok() {
            watched_any = true;
        }
    }

    if !watched_any {
        return None;
    }

    std::thread::Builder::new()
        .name("triex-dir-watcher".to_string())
        .spawn(move || {
            let debounce = Duration::from_secs(2);
            let mut pending_since: Option<Instant> = None;

            loop {
                match rx.recv_timeout(Duration::from_millis(400)) {
                    Ok(Ok(_event)) => {
                        pending_since = Some(Instant::now());
                    }
                    Ok(Err(err)) => {
                        tracing::debug!("Directory watcher error: {err}");
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(since) = pending_since
                            && since.elapsed() >= debounce
                        {
                            pending_since = None;
                            on_change();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .ok();

    Some(watcher)
}
