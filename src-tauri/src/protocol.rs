use crate::{
    activity_for, display_name_for_cwd, emit_snapshot, remote, repo_for_cwd, ConnectionInfo,
    ConnectionStatus, HostKind, PlowSettings, SharedState, SshHostSettings, Worker, WorkerStatus,
};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::Ordering, Arc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;
use tungstenite::{client, Error as WebSocketError, Message, WebSocket};

pub trait HostTransport: Send + Sync {
    fn host_id(&self) -> String;
    fn host_label(&self) -> String;
    fn host_kind(&self) -> HostKind;
    fn configured_codex(&self) -> Option<String>;
    fn monitor_revision(&self) -> u64;
    fn repo_for_cwd(&self, cwd: &str) -> (String, String);
    fn run(&self, app: &AppHandle, state: &Arc<SharedState>) -> Result<(), String>;
}

pub struct LocalDaemonTransport {
    codex: PathBuf,
    monitor_revision: u64,
}

impl LocalDaemonTransport {
    pub fn new(codex: PathBuf, monitor_revision: u64) -> Self {
        Self {
            codex,
            monitor_revision,
        }
    }
}

impl HostTransport for LocalDaemonTransport {
    fn host_id(&self) -> String {
        "local".to_string()
    }

    fn host_label(&self) -> String {
        "This computer".to_string()
    }

    fn host_kind(&self) -> HostKind {
        HostKind::Local
    }

    fn configured_codex(&self) -> Option<String> {
        Some(self.codex.to_string_lossy().into_owned())
    }

    fn monitor_revision(&self) -> u64 {
        self.monitor_revision
    }

    fn repo_for_cwd(&self, cwd: &str) -> (String, String) {
        repo_for_cwd(cwd)
    }

    fn run(&self, app: &AppHandle, state: &Arc<SharedState>) -> Result<(), String> {
        let daemon = Command::new(&self.codex)
            .args(["app-server", "daemon", "start"])
            .output()
            .map_err(|error| format!("Could not start the Codex daemon: {error}"))?;
        if !daemon.status.success() {
            let details = String::from_utf8_lossy(&daemon.stderr);
            if details.contains("managed standalone Codex install not found") {
                return Err(format!(
                    "Plow found Codex at {}, but that installation cannot start the managed daemon. Install the standalone Codex build, or select its executable in Settings. Codex said: {}",
                    self.codex.display(),
                    details.trim()
                ));
            }
            return Err(format!("Codex daemon failed to start: {}", details.trim()));
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

        monitor_socket(socket, app, state, self)
    }
}

pub struct SshDaemonTransport {
    host: SshHostSettings,
    monitor_revision: u64,
}

impl SshDaemonTransport {
    fn new(host: SshHostSettings, monitor_revision: u64) -> Self {
        Self {
            host,
            monitor_revision,
        }
    }
}

impl HostTransport for SshDaemonTransport {
    fn host_id(&self) -> String {
        remote::host_id(&self.host.alias)
    }

    fn host_label(&self) -> String {
        self.host.alias.clone()
    }

    fn host_kind(&self) -> HostKind {
        HostKind::Ssh
    }

    fn configured_codex(&self) -> Option<String> {
        Some(remote::remote_codex_command(&self.host.codex_path).to_string())
    }

    fn monitor_revision(&self) -> u64 {
        self.monitor_revision
    }

    fn repo_for_cwd(&self, cwd: &str) -> (String, String) {
        repo_name_and_path(cwd)
    }

    fn run(&self, app: &AppHandle, state: &Arc<SharedState>) -> Result<(), String> {
        let daemon = remote::run_codex(&self.host, &["app-server", "daemon", "start"])?;
        if !daemon.status.success() {
            let details = String::from_utf8_lossy(&daemon.stderr);
            let hint = if details.contains("managed standalone Codex install not found") {
                " Install the standalone Codex build on the remote host, or set its daemon-capable executable in Settings."
            } else {
                ""
            };
            return Err(format!(
                "Codex daemon failed to start on {}: {}{}",
                self.host.alias,
                details.trim(),
                hint
            ));
        }

        let stream = remote::spawn_codex_proxy(&self.host)?;
        let (mut socket, _) = client("ws://localhost/", stream)
            .map_err(|error| format!("Codex SSH WebSocket handshake failed: {error}"))?;
        socket
            .get_mut()
            .set_read_timeout(Duration::from_millis(150));
        monitor_socket(socket, app, state, self)
    }
}

pub fn spawn_monitor(app: AppHandle, state: Arc<SharedState>) {
    thread::spawn(move || {
        let mut observed_revision = u64::MAX;
        loop {
            let revision = state.monitor_revision.load(Ordering::Relaxed);
            if revision != observed_revision {
                let settings = state
                    .persisted
                    .lock()
                    .expect("persisted lock")
                    .settings
                    .clone();
                prepare_hosts(&app, &state, &settings);
                if settings.local_enabled {
                    let local_app = app.clone();
                    let local_state = state.clone();
                    let configured_path = settings.codex_path.clone();
                    thread::spawn(move || {
                        monitor_local(local_app, local_state, revision, configured_path)
                    });
                }
                for host in settings.ssh_hosts.into_iter().filter(|host| host.enabled) {
                    let remote_app = app.clone();
                    let remote_state = state.clone();
                    thread::spawn(move || monitor_remote(remote_app, remote_state, revision, host));
                }
                observed_revision = revision;
            }
            thread::sleep(Duration::from_millis(200));
        }
    });
}

fn prepare_hosts(app: &AppHandle, state: &Arc<SharedState>, settings: &PlowSettings) {
    let mut connections = Vec::new();
    let mut host_ids = HashSet::new();
    if settings.local_enabled {
        host_ids.insert("local".to_string());
        connections.push(ConnectionInfo {
            host_id: "local".to_string(),
            host_label: "This computer".to_string(),
            host_kind: HostKind::Local,
            status: ConnectionStatus::Connecting,
            codex_version: None,
            codex_path: None,
            message: "Connecting to the local Codex daemon".to_string(),
        });
    } else {
        *state.codex_path.lock().expect("codex path lock") = None;
    }
    for host in settings.ssh_hosts.iter().filter(|host| host.enabled) {
        let id = remote::host_id(&host.alias);
        host_ids.insert(id.clone());
        connections.push(ConnectionInfo {
            host_id: id,
            host_label: host.alias.clone(),
            host_kind: HostKind::Ssh,
            status: ConnectionStatus::Connecting,
            codex_version: None,
            codex_path: Some(remote::remote_codex_command(&host.codex_path).to_string()),
            message: format!("Connecting to {} over SSH", host.alias),
        });
    }
    {
        let mut snapshot = state.snapshot.lock().expect("snapshot lock");
        snapshot.workers.retain(|worker| {
            host_ids.contains(&worker.host_id)
                && matches!(
                    worker.status,
                    WorkerStatus::Completed | WorkerStatus::Failed
                )
        });
        snapshot.connections = connections;
    }
    {
        let mut persisted = state.persisted.lock().expect("persisted lock");
        persisted
            .attention
            .retain(|worker| host_ids.contains(&worker.host_id));
    }
    let _ = state.save();
    emit_snapshot(app, state);
}

fn monitor_local(app: AppHandle, state: Arc<SharedState>, revision: u64, configured_path: String) {
    let mut backoff = Duration::from_secs(1);
    while state.monitor_revision.load(Ordering::Relaxed) == revision {
        update_connection_fields(
            &app,
            &state,
            "local",
            "This computer",
            HostKind::Local,
            ConnectionStatus::Connecting,
            None,
            None,
            "Connecting to the local Codex daemon",
        );
        let codex = match resolve_codex(&configured_path) {
            Ok(Some(path)) => path,
            Ok(None) => {
                *state.codex_path.lock().expect("codex path lock") = None;
                update_connection_fields(
                    &app,
                    &state,
                    "local",
                    "This computer",
                    HostKind::Local,
                    ConnectionStatus::MissingCodex,
                    None,
                    None,
                    "Codex was not found. Open Settings to choose its executable, or install the standalone Codex build.",
                );
                if !wait_for_retry(&state, revision, Duration::from_secs(5)) {
                    break;
                }
                continue;
            }
            Err(error) => {
                *state.codex_path.lock().expect("codex path lock") = None;
                update_connection_fields(
                    &app,
                    &state,
                    "local",
                    "This computer",
                    HostKind::Local,
                    ConnectionStatus::MissingCodex,
                    None,
                    None,
                    &error,
                );
                if !wait_for_retry(&state, revision, Duration::from_secs(5)) {
                    break;
                }
                continue;
            }
        };
        let version = codex_version(&codex);
        *state.codex_path.lock().expect("codex path lock") = Some(codex.clone());
        let transport = LocalDaemonTransport::new(codex, revision);
        let result = transport.run(&app, &state);
        if state.monitor_revision.load(Ordering::Relaxed) != revision {
            break;
        }
        if let Err(error) = result {
            let status = if error.contains("cannot start the managed daemon") {
                ConnectionStatus::Incompatible
            } else {
                ConnectionStatus::Disconnected
            };
            update_connection(&app, &state, &transport, status, version, &error);
        }
        if !wait_for_retry(&state, revision, backoff) {
            break;
        }
        backoff = (backoff * 2).min(Duration::from_secs(5));
    }
}

fn monitor_remote(app: AppHandle, state: Arc<SharedState>, revision: u64, host: SshHostSettings) {
    let transport = SshDaemonTransport::new(host, revision);
    let mut backoff = Duration::from_secs(1);
    while state.monitor_revision.load(Ordering::Relaxed) == revision {
        update_connection(
            &app,
            &state,
            &transport,
            ConnectionStatus::Connecting,
            None,
            &format!("Connecting to {} over SSH", transport.host_label()),
        );
        let result = transport.run(&app, &state);
        if state.monitor_revision.load(Ordering::Relaxed) != revision {
            break;
        }
        if let Err(error) = result {
            let status = if error.contains("managed standalone Codex install not found") {
                ConnectionStatus::Incompatible
            } else {
                ConnectionStatus::Disconnected
            };
            update_connection(
                &app,
                &state,
                &transport,
                status,
                None,
                &format!("{error}. Confirm `ssh {}` works and Codex is installed and authenticated on that host.", transport.host_label()),
            );
        }
        if !wait_for_retry(&state, revision, backoff) {
            break;
        }
        backoff = (backoff * 2).min(Duration::from_secs(5));
    }
}

fn wait_for_retry(state: &Arc<SharedState>, revision: u64, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if state.monitor_revision.load(Ordering::Relaxed) != revision {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    true
}

fn monitor_socket<S: Read + Write>(
    mut socket: WebSocket<S>,
    app: &AppHandle,
    state: &Arc<SharedState>,
    transport: &dyn HostTransport,
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
        if state.monitor_revision.load(Ordering::Relaxed) != transport.monitor_revision() {
            return Ok(());
        }
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
                if state.monitor_revision.load(Ordering::Relaxed) != transport.monitor_revision() {
                    return Ok(());
                }
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
                        transport,
                        ConnectionStatus::Connected,
                        codex_version_from_user_agent(&message),
                        &if matches!(transport.host_kind(), HostKind::Ssh) {
                            format!("Watching {} over SSH", transport.host_label())
                        } else {
                            "Watching the shared local Codex daemon".to_string()
                        },
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
                        update_connection(
                            app,
                            state,
                            transport,
                            ConnectionStatus::Incompatible,
                            None,
                            text,
                        );
                    }
                    continue;
                }

                if let Some(data) = message.pointer("/result/data").and_then(Value::as_array) {
                    reconcile_threads(data, transport, state, &mut repo_cache);
                    emit_snapshot(app, state);
                    continue;
                }

                if let Some(method) = message.get("method").and_then(Value::as_str) {
                    handle_notification(method, message.get("params"), transport, state);
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

fn write_message<S: Read + Write>(
    socket: &mut WebSocket<S>,
    message: &Value,
) -> Result<(), String> {
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
    transport: &dyn HostTransport,
    state: &Arc<SharedState>,
    repo_cache: &mut HashMap<String, (String, String)>,
) {
    let host_id = transport.host_id();
    let now = unix_time();
    let dismissed = state
        .persisted
        .lock()
        .expect("persisted lock")
        .dismissed
        .clone();
    let previous: HashMap<String, Worker> = state
        .snapshot
        .lock()
        .expect("snapshot lock")
        .workers
        .iter()
        .filter(|worker| worker.host_id == host_id)
        .cloned()
        .map(|worker| (worker.id.clone(), worker))
        .collect();
    let mut next = Vec::new();
    let mut seen = HashSet::new();

    for thread in data {
        let Some(thread_id) = thread.get("id").and_then(Value::as_str) else {
            continue;
        };
        let id = worker_id(&host_id, thread_id);
        seen.insert(id.to_string());
        let status_type = thread
            .pointer("/status/type")
            .and_then(Value::as_str)
            .unwrap_or("notLoaded");
        let active = status_type == "active";
        let system_error = status_type == "systemError";

        if active || system_error {
            next.push(normalize_worker(
                thread,
                previous.get(&id),
                transport,
                repo_cache,
                now,
            ));
        } else if let Some(old) = previous.get(&id) {
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
    let mut snapshot = state.snapshot.lock().expect("snapshot lock");
    snapshot.workers.retain(|worker| worker.host_id != host_id);
    snapshot.workers.extend(next);
    snapshot
        .workers
        .sort_by_key(|worker| std::cmp::Reverse(worker.updated_at));
    drop(snapshot);

    let changed = {
        let mut persisted = state.persisted.lock().expect("persisted lock");
        let mut combined = persisted
            .attention
            .iter()
            .filter(|worker| worker.host_id != host_id)
            .cloned()
            .collect::<Vec<_>>();
        combined.extend(attention);
        combined.sort_by_key(|worker| std::cmp::Reverse(worker.updated_at));
        if persisted.attention == combined {
            false
        } else {
            persisted.attention = combined;
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
    transport: &dyn HostTransport,
    repo_cache: &mut HashMap<String, (String, String)>,
    now: i64,
) -> Worker {
    let thread_id = thread
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let host_id = transport.host_id();
    let id = worker_id(&host_id, &thread_id);
    let cwd = thread
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let (repo_name, repo_path) = repo_cache
        .entry(cwd.clone())
        .or_insert_with(|| transport.repo_for_cwd(&cwd))
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
        thread_id: thread_id.clone(),
        host_id: host_id.clone(),
        host_label: transport.host_label(),
        host_kind: transport.host_kind(),
        parent_id: thread
            .get("parentThreadId")
            .and_then(Value::as_str)
            .map(|parent| worker_id(&host_id, parent)),
        display_name: display_name_for_cwd(&cwd, &repo_name),
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

fn handle_notification(
    method: &str,
    params: Option<&Value>,
    transport: &dyn HostTransport,
    state: &Arc<SharedState>,
) {
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
        .position(|worker| worker.host_id == transport.host_id() && worker.thread_id == thread_id)
    else {
        return;
    };
    if turn_status == "interrupted" {
        snapshot.workers.remove(index);
        return;
    }
    let attention_id = format!("{}:{thread_id}:{turn_id}", transport.host_id());
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
    persisted
        .attention
        .retain(|worker| !(worker.host_id == transport.host_id() && worker.thread_id == thread_id));
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
    transport: &dyn HostTransport,
    status: ConnectionStatus,
    version: Option<String>,
    message: &str,
) {
    update_connection_fields(
        app,
        state,
        &transport.host_id(),
        &transport.host_label(),
        transport.host_kind(),
        status,
        version,
        transport.configured_codex(),
        message,
    );
}

#[allow(clippy::too_many_arguments)]
fn update_connection_fields(
    app: &AppHandle,
    state: &Arc<SharedState>,
    host_id: &str,
    host_label: &str,
    host_kind: HostKind,
    status: ConnectionStatus,
    version: Option<String>,
    codex_path: Option<String>,
    message: &str,
) {
    let connection = ConnectionInfo {
        host_id: host_id.to_string(),
        host_label: host_label.to_string(),
        host_kind,
        status,
        codex_version: version,
        codex_path,
        message: message.to_string(),
    };
    let mut snapshot = state.snapshot.lock().expect("snapshot lock");
    if let Some(existing) = snapshot
        .connections
        .iter_mut()
        .find(|connection| connection.host_id == host_id)
    {
        *existing = connection;
    } else {
        return;
    }
    drop(snapshot);
    emit_snapshot(app, state);
}

fn worker_id(host_id: &str, thread_id: &str) -> String {
    if host_id == "local" {
        thread_id.to_string()
    } else {
        format!("{host_id}:{thread_id}")
    }
}

fn repo_name_and_path(root: &str) -> (String, String) {
    if root.is_empty() {
        return ("Remote files".to_string(), String::new());
    }
    let name = Path::new(root)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Remote files")
        .to_string();
    (name, root.to_string())
}

fn resolve_codex(configured_path: &str) -> Result<Option<PathBuf>, String> {
    if !configured_path.is_empty() {
        return crate::terminal::validate_executable_path(configured_path)
            .map(Some)
            .map_err(|error| format!("Configured Codex path is unavailable: {error}"));
    }

    if let Ok(path) = std::env::var("PLOW_CODEX_PATH") {
        if !path.trim().is_empty() {
            return crate::terminal::validate_executable_path(path.trim())
                .map(Some)
                .map_err(|error| format!("PLOW_CODEX_PATH is unavailable: {error}"));
        }
    }

    let codex_home = std::env::var_os("CODEX_HOME").map(PathBuf::from);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let managed = managed_codex_location(codex_home.as_deref(), home.as_deref())
        .and_then(|path| crate::terminal::find_in_path(&path.to_string_lossy()));

    Ok(managed
        .or_else(|| crate::terminal::find_in_path("codex"))
        .or_else(|| {
            std::env::var_os("HOME").and_then(|home| {
                [".local/bin/codex", ".cargo/bin/codex", ".codex/bin/codex"]
                    .into_iter()
                    .map(|suffix| PathBuf::from(&home).join(suffix))
                    .find_map(|candidate| {
                        crate::terminal::find_in_path(&candidate.to_string_lossy())
                    })
            })
        })
        .or_else(|| {
            [
                "/opt/homebrew/bin/codex",
                "/usr/local/bin/codex",
                "/usr/bin/codex",
            ]
            .into_iter()
            .find_map(crate::terminal::find_in_path)
        }))
}

fn managed_codex_location(codex_home: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    codex_home
        .map(Path::to_path_buf)
        .or_else(|| home.map(|path| path.join(".codex")))
        .map(|path| path.join("packages/standalone/current/codex"))
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

    #[test]
    fn normalizes_the_launch_folder_as_the_worker_name() {
        let thread = json!({
            "id": "019f5ade-99ad-7ed1-b2f3-159136634cf7",
            "cwd": "/srv/projects/plow/frontend",
            "status": { "type": "active", "activeFlags": [] }
        });
        let transport = LocalDaemonTransport::new(PathBuf::from("/bin/true"), 0);
        let worker = normalize_worker(&thread, None, &transport, &mut HashMap::new(), 1);
        assert_eq!(worker.display_name, "frontend");
        assert_ne!(worker.repo_name, "unknown");
        assert_eq!(worker.thread_id, "019f5ade-99ad-7ed1-b2f3-159136634cf7");
        assert_eq!(worker.host_id, "local");
    }

    #[test]
    fn namespaces_remote_worker_ids() {
        assert_eq!(
            worker_id("ssh:devbox", "019f5ade-99ad-7ed1-b2f3-159136634cf7"),
            "ssh:devbox:019f5ade-99ad-7ed1-b2f3-159136634cf7"
        );
    }

    #[test]
    fn prefers_the_documented_managed_codex_location() {
        assert_eq!(
            managed_codex_location(
                Some(Path::new("/srv/custom-codex")),
                Some(Path::new("/home/farmer")),
            ),
            Some(PathBuf::from(
                "/srv/custom-codex/packages/standalone/current/codex"
            ))
        );
        assert_eq!(
            managed_codex_location(None, Some(Path::new("/home/farmer"))),
            Some(PathBuf::from(
                "/home/farmer/.codex/packages/standalone/current/codex"
            ))
        );
    }
}
