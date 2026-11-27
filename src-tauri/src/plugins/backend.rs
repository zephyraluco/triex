use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

use crate::config::AppPaths;
use crate::error::{AppError, AppResult};
use crate::plugins::manifest::{BackendKind, PluginBackendConfig};
use crate::state::AppState;

const DEFAULT_RPC_TIMEOUT_MS: u64 = 15_000;
const MAX_RPC_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_EXEC_TIMEOUT_MS: u64 = 10_000;
const MAX_EXEC_TIMEOUT_MS: u64 = 30_000;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecCommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

struct RunningBackend {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingMap,
    next_id: AtomicU64,
    idle_epoch: u64,
}

/// Manages long-lived plugin backend child processes communicating via newline-delimited
/// JSON-RPC 2.0 over stdio, with automatic Windows Job Object cleanup on host exit.
pub struct PluginBackendManager {
    backends: Mutex<HashMap<String, RunningBackend>>,
    idle_seq: AtomicU64,
    #[cfg(windows)]
    job_handle: Option<windows_job::JobGuard>,
}

impl Default for PluginBackendManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginBackendManager {
    pub fn new() -> Self {
        Self {
            backends: Mutex::new(HashMap::new()),
            idle_seq: AtomicU64::new(1),
            #[cfg(windows)]
            job_handle: windows_job::JobGuard::new(),
        }
    }

    /// Invokes a JSON-RPC 2.0 method on the plugin's managed backend process,
    /// lazily spawning the process if it is not already running.
    pub async fn call(
        &self,
        app: &AppHandle,
        plugin_id: &str,
        backend_cfg: &PluginBackendConfig,
        paths: &AppPaths,
        method: &str,
        params: Value,
        timeout_ms: Option<u64>,
    ) -> AppResult<Value> {
        if backend_cfg.kind != BackendKind::Process {
            return Err(AppError::Plugin(format!(
                "Unsupported backend kind for plugin '{plugin_id}'"
            )));
        }

        let (stdin, pending, req_id) =
            self.ensure_running(app, plugin_id, backend_cfg, paths)?;

        let (tx, rx) = oneshot::channel();
        {
            let mut map = pending
                .lock()
                .map_err(|_| AppError::Platform("Backend pending lock poisoned".into()))?;
            map.insert(req_id, tx);
        }

        let payload = json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&payload)?;
        line.push('\n');

        let write_result = {
            let mut guard = stdin
                .lock()
                .map_err(|_| AppError::Platform("Backend stdin lock poisoned".into()))?;
            guard.write_all(line.as_bytes()).and_then(|_| guard.flush())
        };

        if let Err(err) = write_result {
            if let Ok(mut map) = pending.lock() {
                map.remove(&req_id);
            }
            self.stop_plugin(plugin_id);
            return Err(AppError::Plugin(format!(
                "Failed to write RPC request to plugin '{plugin_id}' backend: {err}"
            )));
        }

        let wait_ms = timeout_ms
            .unwrap_or(DEFAULT_RPC_TIMEOUT_MS)
            .clamp(500, MAX_RPC_TIMEOUT_MS);

        match tokio::time::timeout(Duration::from_millis(wait_ms), rx).await {
            Ok(Ok(Ok(val))) => Ok(val),
            Ok(Ok(Err(rpc_err))) => Err(AppError::Plugin(rpc_err)),
            Ok(Err(_)) => Err(AppError::Plugin(format!(
                "Plugin '{plugin_id}' backend exited before responding"
            ))),
            Err(_) => {
                if let Ok(mut map) = pending.lock() {
                    map.remove(&req_id);
                }
                Err(AppError::Plugin(format!(
                    "Plugin '{plugin_id}' backend call '{method}' timed out after {wait_ms}ms"
                )))
            }
        }
    }

    /// Schedules idle shutdown for a plugin's backend after its UI is closed.
    pub fn schedule_idle_stop(&self, app: &AppHandle, plugin_id: &str, timeout_secs: u64) {
        let epoch = self.idle_seq.fetch_add(1, Ordering::Relaxed);
        let exists = {
            let Ok(mut map) = self.backends.lock() else {
                return;
            };
            if let Some(entry) = map.get_mut(plugin_id) {
                entry.idle_epoch = epoch;
                true
            } else {
                false
            }
        };
        if !exists {
            return;
        }

        let handle = app.clone();
        let pid = plugin_id.to_string();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(timeout_secs.max(5))).await;
            let state = handle.state::<AppState>();
            let is_active = state
                .active_plugin()
                .is_some_and(|session| session.plugin_id == pid);
            if is_active {
                return;
            }
            let should_stop = state
                .plugin_backends
                .backends
                .lock()
                .ok()
                .and_then(|map| map.get(&pid).map(|b| b.idle_epoch == epoch))
                .unwrap_or(false);
            if should_stop {
                state.plugin_backends.stop_plugin(&pid);
            }
        });
    }

    /// Gracefully stops and cleans up the backend process for a single plugin.
    pub fn stop_plugin(&self, plugin_id: &str) {
        let removed = self
            .backends
            .lock()
            .ok()
            .and_then(|mut map| map.remove(plugin_id));
        if let Some(backend) = removed {
            shutdown_running_backend(backend);
        }
    }

    /// Stops all running plugin backend processes (used during app exit / update install).
    pub fn stop_all(&self) {
        let drained: Vec<RunningBackend> = self
            .backends
            .lock()
            .ok()
            .map(|mut map| map.drain().map(|(_, b)| b).collect())
            .unwrap_or_default();
        for backend in drained {
            shutdown_running_backend(backend);
        }
    }

    fn ensure_running(
        &self,
        app: &AppHandle,
        plugin_id: &str,
        cfg: &PluginBackendConfig,
        paths: &AppPaths,
    ) -> AppResult<(Arc<Mutex<ChildStdin>>, PendingMap, u64)> {
        let mut map = self
            .backends
            .lock()
            .map_err(|_| AppError::Platform("Plugin backend map lock poisoned".into()))?;

        // Reuse existing backend if it has not exited.
        if let Some(existing) = map.get_mut(plugin_id) {
            match existing.child.try_wait() {
                Ok(None) => {
                    existing.idle_epoch = self.idle_seq.fetch_add(1, Ordering::Relaxed);
                    let id = existing.next_id.fetch_add(1, Ordering::Relaxed);
                    return Ok((
                        Arc::clone(&existing.stdin),
                        Arc::clone(&existing.pending),
                        id,
                    ));
                }
                _ => {
                    if let Some(dead) = map.remove(plugin_id) {
                        shutdown_running_backend(dead);
                    }
                }
            }
        }

        let plugin_root = paths.plugin_dir(plugin_id);
        let entry_path = plugin_root.join(&cfg.entry);
        if !AppPaths::ensure_within_dir(&plugin_root, &entry_path) || !entry_path.is_file() {
            return Err(AppError::Plugin(format!(
                "Backend executable not found or outside plugin directory: {}",
                cfg.entry
            )));
        }

        let data_dir = paths.plugin_data_root(plugin_id);
        std::fs::create_dir_all(&data_dir)?;

        let mut cmd = Command::new(&entry_path);
        cmd.args(&cfg.args)
            .current_dir(&plugin_root)
            .env("TRIEX_PLUGIN_ID", plugin_id)
            .env("TRIEX_PLUGIN_DIR", &plugin_root)
            .env("TRIEX_PLUGIN_DATA_DIR", &data_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|err| {
            AppError::Plugin(format!(
                "Failed to spawn backend for plugin '{plugin_id}': {err}"
            ))
        })?;

        #[cfg(windows)]
        if let Some(job) = &self.job_handle {
            job.assign_child(&child);
        }

        let stdin = child.stdin.take().ok_or_else(|| {
            AppError::Plugin(format!("Failed to attach stdin for plugin '{plugin_id}'"))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::Plugin(format!("Failed to attach stdout for plugin '{plugin_id}'"))
        })?;
        if let Some(stderr) = child.stderr.take() {
            let pid_log = plugin_id.to_string();
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        tracing::debug!("[plugin-backend:{pid_log}] {line}");
                    }
                }
            });
        }

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = Arc::clone(&pending);
        let reader_app = app.clone();
        let reader_pid = plugin_id.to_string();

        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(raw) = line else {
                    break;
                };
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    continue;
                }
                dispatch_rpc_line(trimmed, &reader_pending, |method, params| {
                    forward_backend_event(&reader_app, &reader_pid, method, params);
                });
            }

            // Fail all in-flight RPC calls when the backend stream closes.
            if let Ok(mut pending_map) = reader_pending.lock() {
                for (_, tx) in pending_map.drain() {
                    let _ = tx.send(Err("Plugin backend closed unexpectedly".to_string()));
                }
            }
        });

        let stdin = Arc::new(Mutex::new(stdin));
        let epoch = self.idle_seq.fetch_add(1, Ordering::Relaxed);
        let running = RunningBackend {
            child,
            stdin: Arc::clone(&stdin),
            pending: Arc::clone(&pending),
            next_id: AtomicU64::new(2),
            idle_epoch: epoch,
        };
        map.insert(plugin_id.to_string(), running);
        Ok((stdin, pending, 1))
    }
}

impl Drop for PluginBackendManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

fn shutdown_running_backend(mut backend: RunningBackend) {
    if let Ok(mut stdin) = backend.stdin.lock() {
        let _ = stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"shutdown\"}\n");
        let _ = stdin.flush();
    }
    if let Ok(mut pending) = backend.pending.lock() {
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err("Plugin backend stopped".to_string()));
        }
    }
    let _ = backend.child.kill();
    let _ = backend.child.wait();
}

fn dispatch_rpc_line<F>(line: &str, pending: &PendingMap, mut on_notification: F)
where
    F: FnMut(&str, &Value),
{
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        return;
    };

    if let Some(id) = msg.get("id").and_then(Value::as_u64) {
        let sender = pending.lock().ok().and_then(|mut map| map.remove(&id));
        if let Some(tx) = sender {
            if let Some(err_obj) = msg.get("error") {
                let err_msg = err_obj
                    .get("message")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| err_obj.to_string());
                let _ = tx.send(Err(err_msg));
            } else {
                let result = msg.get("result").cloned().unwrap_or(Value::Null);
                let _ = tx.send(Ok(result));
            }
        }
        return;
    }

    if let Some(method) = msg.get("method").and_then(Value::as_str) {
        let params = msg.get("params").unwrap_or(&Value::Null);
        on_notification(method, params);
    }
}

fn forward_backend_event(app: &AppHandle, plugin_id: &str, event_name: &str, payload: &Value) {
    let Ok(detail_json) = serde_json::to_string(&json!({
        "event": event_name,
        "payload": payload,
    })) else {
        return;
    };
    let script = format!(
        "window.dispatchEvent(new CustomEvent('triex:backend-event', {{ detail: {detail_json} }}));"
    );

    let state = app.state::<AppState>();
    if let Some(label) = state
        .active_plugin()
        .filter(|s| s.plugin_id == plugin_id)
        .map(|s| s.webview_label)
        && let Some(wv) = app.get_webview(&label)
    {
        let _ = wv.eval(&script);
    }

    let detached_label = format!("plugin-detached-{plugin_id}");
    if let Some(detached_win) = app.get_webview_window(&detached_label) {
        let _ = detached_win.eval(&script);
    }
}

/// Executes a one-shot external command on behalf of a plugin with `process` permission
/// (e.g. calling `es.exe` or a bundled CLI helper), hiding console windows and enforcing a timeout.
pub async fn exec_plugin_command(
    plugin_root: &Path,
    command: String,
    args: Vec<String>,
    timeout_ms: Option<u64>,
) -> AppResult<ExecCommandOutput> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("Command cannot be empty".into()));
    }

    let resolved_program = if trimmed.contains('/') || trimmed.contains('\\') {
        let candidate = plugin_root.join(trimmed);
        if candidate.is_file() && AppPaths::ensure_within_dir(plugin_root, &candidate) {
            candidate.to_string_lossy().to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        let bundled = plugin_root.join(trimmed);
        if bundled.is_file() && AppPaths::ensure_within_dir(plugin_root, &bundled) {
            bundled.to_string_lossy().to_string()
        } else {
            trimmed.to_string()
        }
    };

    let cwd = plugin_root.to_path_buf();
    let limit_ms = timeout_ms
        .unwrap_or(DEFAULT_EXEC_TIMEOUT_MS)
        .clamp(200, MAX_EXEC_TIMEOUT_MS);

    let result = tokio::task::spawn_blocking(move || -> AppResult<ExecCommandOutput> {
        let mut cmd = Command::new(&resolved_program);
        cmd.args(&args)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|err| {
            AppError::Plugin(format!("Failed to execute '{resolved_program}': {err}"))
        })?;

        let mut stdout = child.stdout.take().expect("stdout was piped");
        let mut stderr = child.stderr.take().expect("stderr was piped");
        std::thread::scope(|scope| {
            let stdout_reader = scope.spawn(move || {
                let mut output = Vec::new();
                stdout.read_to_end(&mut output)?;
                Ok::<_, std::io::Error>(output)
            });
            let stderr_reader = scope.spawn(move || {
                let mut output = Vec::new();
                stderr.read_to_end(&mut output)?;
                Ok::<_, std::io::Error>(output)
            });

            let deadline = std::time::Instant::now() + Duration::from_millis(limit_ms);
            let (exit_code, timed_out) = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break (status.code().unwrap_or(-1), false),
                    Ok(None) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Ok(None) => {
                        let _ = child.kill();
                        child.wait()?;
                        break (-1, true);
                    }
                    Err(err) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(AppError::Io(err));
                    }
                }
            };
            let stdout = stdout_reader
                .join()
                .map_err(|_| AppError::Platform("Command stdout reader panicked".into()))??;
            let stderr = stderr_reader
                .join()
                .map_err(|_| AppError::Platform("Command stderr reader panicked".into()))??;

            Ok(ExecCommandOutput {
                exit_code,
                stdout: String::from_utf8_lossy(&stdout).into_owned(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
                timed_out,
            })
        })
    })
    .await
    .map_err(|err| AppError::Platform(format!("Command task failed: {err}")))??;

    Ok(result)
}

#[cfg(windows)]
mod windows_job {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct JobObjectBasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct JobObjectExtendedLimitInformation {
        basic_limit_information: JobObjectBasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(
            lp_job_attributes: *mut c_void,
            lp_name: *const u16,
        ) -> *mut c_void;
        fn SetInformationJobObject(
            h_job: *mut c_void,
            job_object_information_class: u32,
            lp_job_object_information: *const c_void,
            cb_job_object_information_length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(h_job: *mut c_void, h_process: *mut c_void) -> i32;
        fn CloseHandle(h_object: *mut c_void) -> i32;
    }

    /// Ensures all assigned plugin child processes are terminated automatically by Windows
    /// if the Triex host exits or crashes.
    pub struct JobGuard {
        handle: *mut c_void,
    }

    unsafe impl Send for JobGuard {}
    unsafe impl Sync for JobGuard {}

    impl JobGuard {
        pub fn new() -> Option<Self> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
                if handle.is_null() {
                    return None;
                }
                let mut info = JobObjectExtendedLimitInformation::default();
                info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    (&raw const info).cast(),
                    std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
                );
                if ok == 0 {
                    CloseHandle(handle);
                    return None;
                }
                Some(Self { handle })
            }
        }

        pub fn assign_child(&self, child: &Child) {
            unsafe {
                let _ = AssignProcessToJobObject(self.handle, child.as_raw_handle());
            }
        }
    }

    impl Drop for JobGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_should_resolve_pending_rpc_response_and_error() {
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let (tx_ok, mut rx_ok) = oneshot::channel();
        let (tx_err, mut rx_err) = oneshot::channel();
        pending.lock().unwrap().insert(1, tx_ok);
        pending.lock().unwrap().insert(2, tx_err);

        dispatch_rpc_line(
            r#"{"jsonrpc":"2.0","id":1,"result":{"count":3}}"#,
            &pending,
            |_, _| panic!("should not treat response as notification"),
        );
        dispatch_rpc_line(
            r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Method not found"}}"#,
            &pending,
            |_, _| panic!("should not treat error as notification"),
        );

        assert_eq!(rx_ok.try_recv().unwrap().unwrap(), json!({"count": 3}));
        assert_eq!(
            rx_err.try_recv().unwrap().unwrap_err(),
            "Method not found"
        );
    }

    #[test]
    fn dispatch_should_forward_json_rpc_notification() {
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let mut captured = None;
        dispatch_rpc_line(
            r#"{"jsonrpc":"2.0","method":"progress","params":{"percent":80}}"#,
            &pending,
            |method, params| {
                captured = Some((method.to_string(), params.clone()));
            },
        );
        assert_eq!(
            captured,
            Some(("progress".to_string(), json!({"percent": 80})))
        );
    }
}
