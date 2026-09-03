use crate::{
    activity_for, emit_snapshot, repo_for_cwd, ConnectionInfo, ConnectionStatus, SharedState,
    Worker, WorkerStatus,
};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    io,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;
use tungstenite::{client, Error as WebSocketError, Message, WebSocket};

pub trait HostTransport: Send + Sync {
    fn run(&self, app: &AppHandle, state: &Arc<SharedState>) -> Result<(), String>;
}

pub struct LocalDaemonTransport {
    codex: PathBuf,
}

impl LocalDaemonTransport {
    pub fn new(codex: PathBuf) -> Self {
        Self { codex }
    }
}

impl HostTransport for LocalDaemonTransport {
    fn run(&self, app: &AppHandle, state: &Arc<SharedState>) -> Result<(), String> {
        let daemon = Command::new(&self.codex)
            .args(["app-server", "daemon", "start"])
            .output()
            .map_err(|error| format!("Could not start the Codex daemon: {error}"))?;
        if !daemon.status.success() {
            return Err(format!(
                "Codex daemon failed to start: {}",
                String::from_utf8_lossy(&daemon.stderr).trim()
            ));
        }

        let socket_path = daemon_socket(&self.codex)?;
        let stream = UnixStream::connect(&socket_path)
            .map_err(|error| format!("Could not connect to {}: {error}", socket_path.display()))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(150)))
            .map_err(|error| format!("Could not configure the Codex socket: {error}"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .map_err(|error| format!("Could not configure the Codex socket: {error}"))?;
        let (socket, _) = client("ws://localhost/", stream)
            .map_err(|error| format!("Codex WebSocket handshake failed: {error}"))?;

        monitor_socket(socket, app, state)
    }
}

pub fn spawn_monitor(app: AppHandle, state: Arc<SharedState>) {
    thread::spawn(move || {
        let mut backoff = Duration::from_secs(1);
        loop {
            update_connection(
                &app,
                &state,
                ConnectionStatus::Connecting,
                None,
                "Connecting to the Codex farm",
            );
            let codex = match resolve_codex() {
                Some(path) => path,
                None => {
                    update_connection(
                        &app,
                        &state,
                        ConnectionStatus::MissingCodex,
                        None,
                        "Codex CLI was not found in PATH. Install Codex, then restart Plow.",
                    );
                    thread::sleep(Duration::from_secs(10));
                    continue;
                }
            };

            let version = codex_version(&codex);
            *state.codex_path.lock().expect("codex path lock") = Some(codex.clone());
            let transport = LocalDaemonTransport::new(codex);
            if let Err(error) = transport.run(&app, &state) {
                update_connection(
                    &app,
                    &state,
                    ConnectionStatus::Disconnected,
                    version,
                    &error,
                );
            }
            thread::sleep(backoff);
            backoff = (backoff * 2).min(Duration::from_secs(20));
        }
    });
}

fn monitor_socket(
    mut socket: WebSocket<UnixStream>,
    app: &AppHandle,
    state: &Arc<SharedState>,
) -> Result<(), String> {
    write_message(
        &mut socket,
        &json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": { "name": "plow", "title": "Plow", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": { "experimentalApi": true, "requestAttestation": false }
            }
        }),
    )?;

    let mut initialized = false;
    let mut request_id = 2_u64;
    let mut next_poll = Instant::now();
    let mut repo_cache = HashMap::new();

    loop {
        if initialized && Instant::now() >= next_poll {
            write_message(
                &mut socket,
                &json!({
                    "method": "thread/list",
                    "id": request_id,
                    "params": {
                        "limit": 500,
                        "sortKey": "updated_at",
                        "sortDirection": "desc",
                        "archived": false,
                        "useStateDbOnly": true,
                        "sourceKinds": ["cli", "vscode", "exec", "appServer", "subAgent", "subAgentReview", "subAgentCompact", "subAgentThreadSpawn", "subAgentOther", "unknown"]
                    }
                }),
            )?;
            request_id += 1;
            next_poll = Instant::now() + Duration::from_secs(2);
        }

        match socket.read() {
            Ok(Message::Text(line)) => {
                let message: Value = serde_json::from_str(&line)
                    .map_err(|error| format!("Codex sent malformed JSON: {error}"))?;

                if message.get("id").and_then(Value::as_u64) == Some(1)
                    && message.get("result").is_some()
                {
                    write_message(
                        &mut socket,
                        &json!({ "method": "initialized", "params": {} }),
                    )?;
                    initialized = true;
                    update_connection(
                        app,
                        state,
                        ConnectionStatus::Connected,
                        codex_version_from_user_agent(&message),
                        "Watching the shared local Codex daemon",
                    );
                    next_poll = Instant::now();
                    continue;
                }

                if let Some(error) = message.get("error") {
                    let text = error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Codex protocol error");
                    if text.contains("method") || text.contains("unsupported") {
                        update_connection(app, state, ConnectionStatus::Incompatible, None, text);
                    }
                    continue;
                }

                if let Some(data) = message.pointer("/result/data").and_then(Value::as_array) {
                    reconcile_threads(data, state, &mut repo_cache);
                    emit_snapshot(app, state);
                    continue;
                }

                if let Some(method) = message.get("method").and_then(Value::as_str) {
                    handle_notification(method, message.get("params"), state);
                    emit_snapshot(app, state);
                }
            }
            Ok(Message::Close(_)) => {
                return Err("Codex closed the WebSocket connection".to_string())
            }
            Ok(_) => {}
            Err(WebSocketError::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => {
                return Err("Lost the Codex protocol stream".to_string());
            }
            Err(error) => return Err(format!("Codex WebSocket error: {error}")),
        }
    }
}

fn write_message(socket: &mut WebSocket<UnixStream>, message: &Value) -> Result<(), String> {
    socket
        .send(Message::Text(message.to_string().into()))
        .map_err(|error| format!("Could not write to Codex: {error}"))
}

fn daemon_socket(codex: &Path) -> Result<PathBuf, String> {
    let output = Command::new(codex)
        .args(["app-server", "daemon", "version"])
        .output()
        .map_err(|error| format!("Could not inspect the Codex daemon: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Could not inspect the Codex daemon: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Codex returned invalid daemon metadata: {error}"))?;
    metadata
        .get("socketPath")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "Codex daemon metadata did not include a socket path".to_string())
}

fn reconcile_threads(
    data: &[Value],
    state: &Arc<SharedState>,
    repo_cache: &mut HashMap<String, (String, String)>,
) {
    let now = unix_time();
    let dismissed = state
        .persisted
        .lock()
        .expect("persisted lock")
        .dismissed
        .clone();
    let mut snapshot = state.snapshot.lock().expect("snapshot lock");
    let previous: HashMap<String, Worker> = snapshot
        .workers
        .iter()
        .cloned()
        .map(|worker| (worker.id.clone(), worker))
        .collect();
    let mut next = Vec::new();
    let mut seen = HashSet::new();

    for thread in data {
        let Some(id) = thread.get("id").and_then(Value::as_str) else {
            continue;
        };
        seen.insert(id.to_string());
        let status_type = thread
            .pointer("/status/type")
            .and_then(Value::as_str)
            .unwrap_or("notLoaded");
        let active = status_type == "active";
        let system_error = status_type == "systemError";

        if active || system_error {
            next.push(normalize_worker(thread, previous.get(id), repo_cache, now));
        } else if let Some(old) = previous.get(id) {
            if matches!(
                old.status,
                WorkerStatus::Running | WorkerStatus::WaitingApproval | WorkerStatus::WaitingInput
            ) {
                let attention_id = format!(
                    "{}:{}",
                    id,
                    thread
                        .get("updatedAt")
                        .and_then(Value::as_i64)
                        .unwrap_or(now)
                );
                if !dismissed.contains(&attention_id) {
                    let mut completed = old.clone();
                    completed.status = WorkerStatus::Completed;
                    completed.attention_id = Some(attention_id);
                    completed.updated_at = now;
                    next.push(completed);
                }
            } else if old
                .attention_id
                .as_ref()
                .is_none_or(|key| !dismissed.contains(key))
            {
                next.push(old.clone());
            }
        }
    }

    for old in previous.values() {
        if !seen.contains(&old.id)
            && matches!(old.status, WorkerStatus::Completed | WorkerStatus::Failed)
            && old
                .attention_id
                .as_ref()
                .is_none_or(|key| !dismissed.contains(key))
        {
            next.push(old.clone());
        }
    }

    next.sort_by_key(|worker| std::cmp::Reverse(worker.updated_at));
    let attention: Vec<Worker> = next
        .iter()
        .filter(|worker| {
            matches!(
                worker.status,
                WorkerStatus::Completed | WorkerStatus::Failed
            )
        })
        .cloned()
        .collect();
    snapshot.workers = next;
    drop(snapshot);

    let changed = {
        let mut persisted = state.persisted.lock().expect("persisted lock");
        if persisted.attention == attention {
            false
        } else {
            persisted.attention = attention;
            true
        }
    };
    if changed {
        let _ = state.save();
    }
}

fn normalize_worker(
    thread: &Value,
    previous: Option<&Worker>,
    repo_cache: &mut HashMap<String, (String, String)>,
    now: i64,
) -> Worker {
    let id = thread
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let cwd = thread
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let (repo_name, repo_path) = repo_cache
        .entry(cwd.clone())
        .or_insert_with(|| repo_for_cwd(&cwd))
        .clone();
    let flags = thread
        .pointer("/status/activeFlags")
        .and_then(Value::as_array);
    let status_type = thread
        .pointer("/status/type")
        .and_then(Value::as_str)
        .unwrap_or("active");
    let status = if status_type == "systemError" {
        WorkerStatus::Failed
    } else if has_flag(flags, "waitingOnApproval") {
        WorkerStatus::WaitingApproval
    } else if has_flag(flags, "waitingOnUserInput") {
        WorkerStatus::WaitingInput
    } else {
        WorkerStatus::Running
    };
    let attention_id = match status {
        WorkerStatus::WaitingApproval => {
            transition_attention_id(previous, &status, &id, "approval", now)
        }
        WorkerStatus::WaitingInput => transition_attention_id(previous, &status, &id, "input", now),
        WorkerStatus::Failed => {
            transition_attention_id(previous, &status, &id, "system-error", now)
        }
        _ => None,
    };

    Worker {
        id: id.clone(),
        parent_id: thread
            .get("parentThreadId")
            .and_then(Value::as_str)
            .map(str::to_string),
        thread_name: thread
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| thread.get("preview").and_then(Value::as_str))
            .unwrap_or("Untitled Codex thread")
            .chars()
            .take(90)
            .collect(),
        repo_name,
        repo_path,
        cwd,
        branch: thread
            .pointer("/gitInfo/branch")
            .and_then(Value::as_str)
            .map(str::to_string),
        model: thread
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        source: source_label(thread.get("source")),
        status,
        activity: activity_for(&id),
        updated_at: thread
            .get("updatedAt")
            .and_then(Value::as_i64)
            .unwrap_or(now),
        started_at: previous.and_then(|worker| worker.started_at).or(Some(now)),
        attention_id,
    }
}

fn transition_attention_id(
    previous: Option<&Worker>,
    status: &WorkerStatus,
    thread_id: &str,
    kind: &str,
    now: i64,
) -> Option<String> {
    if let Some(previous) = previous {
        if std::mem::discriminant(&previous.status) == std::mem::discriminant(status) {
            if let Some(key) = &previous.attention_id {
                return Some(key.clone());
            }
        }
    }
    Some(format!("{thread_id}:{kind}:{now}"))
}

fn handle_notification(method: &str, params: Option<&Value>, state: &Arc<SharedState>) {
    let Some(params) = params else { return };
    if method != "turn/completed" {
        return;
    }
    let Some(thread_id) = params.get("threadId").and_then(Value::as_str) else {
        return;
    };
    let turn_id = params
        .pointer("/turn/id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let turn_status = params
        .pointer("/turn/status")
        .and_then(Value::as_str)
        .unwrap_or("completed");
    let mut snapshot = state.snapshot.lock().expect("snapshot lock");
    let Some(index) = snapshot
        .workers
        .iter()
        .position(|worker| worker.id == thread_id)
    else {
        return;
    };
    if turn_status == "interrupted" {
        snapshot.workers.remove(index);
        return;
    }
    let attention_id = format!("{thread_id}:{turn_id}");
    if state
        .persisted
        .lock()
        .expect("persisted lock")
        .dismissed
        .contains(&attention_id)
    {
        return;
    }
    let worker = &mut snapshot.workers[index];
    worker.status = if turn_status == "failed" {
        WorkerStatus::Failed
    } else {
        WorkerStatus::Completed
    };
    worker.attention_id = Some(attention_id);
    worker.updated_at = unix_time();
    let attention = worker.clone();
    drop(snapshot);
    let mut persisted = state.persisted.lock().expect("persisted lock");
    persisted.attention.retain(|worker| worker.id != thread_id);
    persisted.attention.push(attention);
    drop(persisted);
    let _ = state.save();
}

fn has_flag(flags: Option<&Vec<Value>>, target: &str) -> bool {
    flags.is_some_and(|values| values.iter().any(|value| value.as_str() == Some(target)))
}

fn source_label(source: Option<&Value>) -> String {
    match source {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Object(value)) if value.contains_key("subAgent") => "subAgent".to_string(),
        Some(_) => "other".to_string(),
        None => "unknown".to_string(),
    }
}

fn update_connection(
    app: &AppHandle,
    state: &Arc<SharedState>,
    status: ConnectionStatus,
    version: Option<String>,
    message: &str,
) {
    state.snapshot.lock().expect("snapshot lock").connection = ConnectionInfo {
        status,
        codex_version: version,
        message: message.to_string(),
    };
    emit_snapshot(app, state);
}

fn resolve_codex() -> Option<PathBuf> {
    std::env::var("PLOW_CODEX_PATH")
        .ok()
        .and_then(|path| crate::terminal::find_in_path(&path))
        .or_else(|| crate::terminal::find_in_path("codex"))
        .or_else(|| {
            std::env::var_os("HOME").and_then(|home| {
                [".local/bin/codex", ".cargo/bin/codex", ".codex/bin/codex"]
                    .into_iter()
                    .map(|suffix| PathBuf::from(&home).join(suffix))
                    .find(|candidate| candidate.is_file())
            })
        })
        .or_else(|| {
            [
                "/opt/homebrew/bin/codex",
                "/usr/local/bin/codex",
                "/usr/bin/codex",
            ]
            .into_iter()
            .map(PathBuf::from)
            .find(|candidate| candidate.is_file())
        })
}

fn codex_version(path: &Path) -> Option<String> {
    Command::new(path)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn codex_version_from_user_agent(message: &Value) -> Option<String> {
    message
        .pointer("/result/userAgent")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_attention_flags() {
        let flags = vec![Value::String("waitingOnApproval".to_string())];
        assert!(has_flag(Some(&flags), "waitingOnApproval"));
        assert!(!has_flag(Some(&flags), "waitingOnUserInput"));
    }

    #[test]
    fn labels_subagent_sources() {
        assert_eq!(source_label(Some(&json!({"subAgent": {}}))), "subAgent");
        assert_eq!(source_label(Some(&json!("cli"))), "cli");
    }
}
