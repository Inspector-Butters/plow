mod protocol;
mod remote;
mod shell;
mod terminal;

use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkerStatus {
    Running,
    WaitingApproval,
    WaitingInput,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FarmActivity {
    Plowing,
    Watering,
    Planting,
    Harvesting,
    Carrying,
    Digging,
    Raking,
    Repairing,
    Feeding,
    Chopping,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worker {
    pub id: String,
    #[serde(default)]
    pub thread_id: String,
    #[serde(default = "default_local_host_id")]
    pub host_id: String,
    #[serde(default = "default_local_host_label")]
    pub host_label: String,
    #[serde(default)]
    pub host_kind: HostKind,
    pub parent_id: Option<String>,
    #[serde(default = "default_worker_display_name")]
    pub display_name: String,
    pub thread_name: String,
    pub repo_name: String,
    pub repo_path: String,
    pub cwd: String,
    pub branch: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub context_tokens: Option<u64>,
    #[serde(default)]
    pub context_window: Option<u64>,
    pub source: String,
    pub status: WorkerStatus,
    pub activity: FarmActivity,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub attention_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitWindow {
    pub used_percent: f64,
    pub window_duration_mins: Option<u64>,
    pub resets_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitBucket {
    pub limit_id: String,
    pub limit_name: Option<String>,
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRateLimits {
    pub host_id: String,
    pub host_label: String,
    pub limits: Vec<RateLimitBucket>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    MissingCodex,
    Incompatible,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostKind {
    #[default]
    Local,
    Ssh,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub host_id: String,
    pub host_label: String,
    pub host_kind: HostKind,
    pub status: ConnectionStatus,
    pub codex_version: Option<String>,
    pub codex_path: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSnapshot {
    pub workers: Vec<Worker>,
    pub connections: Vec<ConnectionInfo>,
    pub rate_limits: Vec<HostRateLimits>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct PlowSettings {
    pub notify_when_unfocused: bool,
    pub keep_in_tray: bool,
    pub reduced_motion: bool,
    pub local_enabled: bool,
    pub codex_path: String,
    pub development_home: String,
    pub ssh_hosts: Vec<SshHostSettings>,
    pub view_mode: AgentViewMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct SshHostSettings {
    pub alias: String,
    pub codex_path: String,
    pub development_home: String,
    pub enabled: bool,
}

impl Default for SshHostSettings {
    fn default() -> Self {
        Self {
            alias: String::new(),
            codex_path: String::new(),
            development_home: String::new(),
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentViewMode {
    Field,
    Classic,
}

impl Default for PlowSettings {
    fn default() -> Self {
        Self {
            notify_when_unfocused: true,
            keep_in_tray: true,
            reduced_motion: false,
            local_enabled: true,
            codex_path: String::new(),
            development_home: String::new(),
            ssh_hosts: Vec::new(),
            view_mode: AgentViewMode::Field,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFolder {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    #[serde(default)]
    dismissed: HashSet<String>,
    #[serde(default)]
    settings: PlowSettings,
    #[serde(default)]
    attention: Vec<Worker>,
}

pub struct SharedState {
    snapshot: Mutex<MonitorSnapshot>,
    persisted: Mutex<PersistedState>,
    storage_path: PathBuf,
    codex_path: Mutex<Option<PathBuf>>,
    monitor_revision: AtomicU64,
}

impl SharedState {
    fn new(storage_path: PathBuf) -> Self {
        let mut persisted: PersistedState = fs::read(&storage_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        for worker in &mut persisted.attention {
            if worker.thread_id.is_empty() {
                worker.thread_id = worker.id.clone();
            }
        }
        let attention = persisted.attention.clone();
        Self {
            snapshot: Mutex::new(MonitorSnapshot {
                workers: attention,
                connections: Vec::new(),
                rate_limits: Vec::new(),
            }),
            persisted: Mutex::new(persisted),
            storage_path,
            codex_path: Mutex::new(None),
            monitor_revision: AtomicU64::new(0),
        }
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let bytes =
            serde_json::to_vec_pretty(&*self.persisted.lock().map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        fs::write(&self.storage_path, bytes).map_err(|error| error.to_string())
    }
}

#[tauri::command]
fn get_snapshot(state: State<'_, Arc<SharedState>>) -> Result<MonitorSnapshot, String> {
    Ok(state
        .snapshot
        .lock()
        .map_err(|error| error.to_string())?
        .clone())
}

#[tauri::command]
fn mark_reviewed(
    app: AppHandle,
    state: State<'_, Arc<SharedState>>,
    attention_id: String,
) -> Result<(), String> {
    if attention_id.len() > 180 || attention_id.is_empty() {
        return Err("Invalid attention id".to_string());
    }
    state
        .persisted
        .lock()
        .map_err(|error| error.to_string())?
        .dismissed
        .insert(attention_id.clone());
    state
        .snapshot
        .lock()
        .map_err(|error| error.to_string())?
        .workers
        .retain(|worker| worker.attention_id.as_deref() != Some(attention_id.as_str()));
    state
        .persisted
        .lock()
        .map_err(|error| error.to_string())?
        .attention
        .retain(|worker| worker.attention_id.as_deref() != Some(attention_id.as_str()));
    state.save()?;
    emit_snapshot(&app, state.inner());
    Ok(())
}

// Terminal automation can wait for macOS consent; keep the UI thread free.
#[tauri::command(async)]
fn open_thread(state: State<'_, Arc<SharedState>>, thread_id: String) -> Result<String, String> {
    let worker = state
        .snapshot
        .lock()
        .map_err(|error| error.to_string())?
        .workers
        .iter()
        .find(|worker| worker.id == thread_id)
        .cloned()
        .ok_or("That Codex thread is no longer available")?;
    match worker.host_kind {
        HostKind::Local => {
            let codex = state
                .codex_path
                .lock()
                .map_err(|error| error.to_string())?
                .clone()
                .ok_or("Codex is not connected on this computer")?;
            terminal::open_thread(&codex, &worker.thread_id, &worker.cwd)
        }
        HostKind::Ssh => {
            let host = configured_ssh_host(state.inner(), &worker.host_id)?;
            terminal::open_remote_thread(&host, &worker.thread_id, &worker.cwd)
        }
    }
}

#[tauri::command]
fn get_resume_command(
    state: State<'_, Arc<SharedState>>,
    worker_id: String,
) -> Result<String, String> {
    let worker = state
        .snapshot
        .lock()
        .map_err(|error| error.to_string())?
        .workers
        .iter()
        .find(|worker| worker.id == worker_id)
        .cloned()
        .ok_or("That Codex thread is no longer available")?;
    match worker.host_kind {
        HostKind::Local => terminal::resume_command(&worker.thread_id, &worker.cwd),
        HostKind::Ssh => {
            let host = configured_ssh_host(state.inner(), &worker.host_id)?;
            terminal::remote_resume_command(&host, &worker.thread_id, &worker.cwd)
        }
    }
}

fn list_project_folders(home: &Path) -> Result<Vec<ProjectFolder>, String> {
    let mut projects = fs::read_dir(home)
        .map_err(|error| format!("Could not read {}: {error}", home.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            (!name.starts_with('.') && path.is_dir()).then(|| ProjectFolder {
                name,
                path: path.to_string_lossy().into_owned(),
            })
        })
        .collect::<Vec<_>>();
    projects.sort_by_cached_key(|project| project.name.to_lowercase());
    Ok(projects)
}

fn list_remote_project_folders(host: &SshHostSettings) -> Result<Vec<ProjectFolder>, String> {
    if host.development_home.is_empty() {
        return Err(format!(
            "Set a development home folder for {} in Settings first",
            host.alias
        ));
    }
    remote::validate_remote_directory(
        &host.development_home,
        "the remote development home folder",
    )?;
    let argv = vec![
        "find".to_string(),
        host.development_home.clone(),
        "-mindepth".to_string(),
        "1".to_string(),
        "-maxdepth".to_string(),
        "1".to_string(),
        "-type".to_string(),
        "d".to_string(),
        "-print0".to_string(),
    ];
    let output = remote::run_argv(&host.alias, &argv)?;
    let stdout = remote::require_success(output, "list project folders", &host.alias)?;
    let mut projects = stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .filter_map(|path| {
            let path = String::from_utf8_lossy(path).into_owned();
            let name = Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())?
                .to_string();
            (!name.starts_with('.')).then_some(ProjectFolder { name, path })
        })
        .collect::<Vec<_>>();
    projects.sort_by_cached_key(|project| project.name.to_lowercase());
    Ok(projects)
}

fn validate_project_selection(home: &Path, value: &str) -> Result<PathBuf, String> {
    let home = fs::canonicalize(home)
        .map_err(|error| format!("Could not open the development home folder: {error}"))?;
    let selected = Path::new(value);
    if !selected.is_absolute() {
        return Err("The selected project path must be absolute".to_string());
    }
    let canonical_selected = terminal::validate_directory_path(value, "the selected project")?;
    if canonical_selected == home {
        return Ok(canonical_selected);
    }
    let parent = selected
        .parent()
        .ok_or("The selected project has no parent folder")?;
    let parent = fs::canonicalize(parent)
        .map_err(|error| format!("Could not open the project's parent folder: {error}"))?;
    if parent != home {
        return Err(
            "Choose the configured development home or a project directly inside it".to_string(),
        );
    }
    Ok(canonical_selected)
}

#[tauri::command]
fn list_projects(
    state: State<'_, Arc<SharedState>>,
    host_id: String,
) -> Result<Vec<ProjectFolder>, String> {
    let settings = state
        .persisted
        .lock()
        .map_err(|error| error.to_string())?
        .settings
        .development_home
        .clone();
    if host_id == "local" {
        if settings.is_empty() {
            return Err("Set a local development home folder in Settings first".to_string());
        }
        let home = terminal::validate_directory_path(&settings, "the development home folder")?;
        return list_project_folders(&home);
    }
    let host = configured_ssh_host(state.inner(), &host_id)?;
    list_remote_project_folders(&host)
}

#[tauri::command(async)]
fn start_agent(
    state: State<'_, Arc<SharedState>>,
    host_id: String,
    project_path: String,
) -> Result<String, String> {
    if host_id == "local" {
        let configured = state
            .persisted
            .lock()
            .map_err(|error| error.to_string())?
            .settings
            .development_home
            .clone();
        if configured.is_empty() {
            return Err("Set a local development home folder in Settings first".to_string());
        }
        let home = terminal::validate_directory_path(&configured, "the development home folder")?;
        let project = validate_project_selection(&home, &project_path)?;
        let codex = state
            .codex_path
            .lock()
            .map_err(|error| error.to_string())?
            .clone()
            .ok_or("Codex is not connected on this computer")?;
        return terminal::start_agent(&codex, &project);
    }
    let host = configured_ssh_host(state.inner(), &host_id)?;
    remote::validate_project_selection(&host.development_home, &project_path)?;
    let check = remote::run_argv(
        &host.alias,
        &["test".to_string(), "-d".to_string(), project_path.clone()],
    )?;
    remote::require_success(check, "open the selected project folder", &host.alias)?;
    terminal::start_remote_agent(&host, &project_path)
}

#[tauri::command]
fn list_ssh_hosts() -> Vec<String> {
    remote::discover_aliases()
}

fn configured_ssh_host(state: &Arc<SharedState>, host_id: &str) -> Result<SshHostSettings, String> {
    state
        .persisted
        .lock()
        .map_err(|error| error.to_string())?
        .settings
        .ssh_hosts
        .iter()
        .find(|host| host.enabled && remote::host_id(&host.alias) == host_id)
        .cloned()
        .ok_or_else(|| "That SSH host is no longer enabled".to_string())
}

#[tauri::command]
fn get_settings(state: State<'_, Arc<SharedState>>) -> Result<PlowSettings, String> {
    Ok(state
        .persisted
        .lock()
        .map_err(|error| error.to_string())?
        .settings
        .clone())
}

#[tauri::command]
fn update_settings(
    app: AppHandle,
    state: State<'_, Arc<SharedState>>,
    mut settings: PlowSettings,
) -> Result<(), String> {
    settings.codex_path = settings.codex_path.trim().to_string();
    settings.development_home = settings.development_home.trim().to_string();
    for host in &mut settings.ssh_hosts {
        host.alias = host.alias.trim().to_string();
        host.codex_path = host.codex_path.trim().to_string();
        host.development_home = host.development_home.trim().to_string();
    }
    if settings.ssh_hosts.len() > 16 {
        return Err("Plow supports up to 16 SSH hosts".to_string());
    }
    if settings.local_enabled && !settings.codex_path.is_empty() {
        terminal::validate_executable_path(&settings.codex_path)?;
    }
    if settings.local_enabled && !settings.development_home.is_empty() {
        terminal::validate_directory_path(
            &settings.development_home,
            "the development home folder",
        )?;
    }
    let mut aliases = HashSet::new();
    for host in &settings.ssh_hosts {
        remote::validate_host(host)?;
        if !aliases.insert(host.alias.to_lowercase()) {
            return Err(format!("SSH host {} is listed more than once", host.alias));
        }
    }
    if !settings.local_enabled && !settings.ssh_hosts.iter().any(|host| host.enabled) {
        return Err("Enable this computer or at least one SSH host".to_string());
    }

    let connections_changed = {
        let mut persisted = state.persisted.lock().map_err(|error| error.to_string())?;
        let changed = persisted.settings.local_enabled != settings.local_enabled
            || persisted.settings.codex_path != settings.codex_path
            || persisted.settings.ssh_hosts != settings.ssh_hosts;
        persisted.settings = settings;
        changed
    };
    if connections_changed {
        *state.codex_path.lock().map_err(|error| error.to_string())? = None;
        state.monitor_revision.fetch_add(1, Ordering::Relaxed);
        emit_snapshot(&app, state.inner());
    }
    state.save()
}

pub fn emit_snapshot(app: &AppHandle, state: &Arc<SharedState>) {
    let snapshot = state.snapshot.lock().expect("snapshot lock").clone();
    let attention = snapshot
        .workers
        .iter()
        .filter(|worker| !matches!(worker.status, WorkerStatus::Running))
        .count();
    if let Some(tray) = app.tray_by_id("main") {
        let tooltip = if attention == 0 {
            "Plow — all workers are busy".to_string()
        } else {
            format!("Plow — {attention} need attention")
        };
        let _ = tray.set_tooltip(Some(tooltip));
        let _ = tray.set_title(if attention == 0 {
            None
        } else {
            Some(attention.to_string())
        });
    }
    let _ = app.emit("monitor-snapshot", snapshot);
}

pub fn repo_for_cwd(cwd: &str) -> (String, String) {
    if cwd.is_empty() {
        return ("Local files".to_string(), String::new());
    }
    let root = Command::new("git")
        .args(["-C", cwd, "rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| cwd.to_string());
    let name = Path::new(&root)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Local files")
        .to_string();
    (name, root)
}

fn default_worker_display_name() -> String {
    "Codex".to_string()
}

fn default_local_host_id() -> String {
    "local".to_string()
}

fn default_local_host_label() -> String {
    "This computer".to_string()
}

pub fn display_name_for_cwd(cwd: &str, repo_name: &str) -> String {
    Path::new(cwd)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .or_else(|| (!repo_name.is_empty() && repo_name != "unknown").then_some(repo_name))
        .or_else(|| (!cwd.is_empty()).then_some(cwd))
        .unwrap_or("Codex")
        .to_string()
}

pub fn activity_for(id: &str) -> FarmActivity {
    let hash = id.bytes().fold(0_u64, |value, byte| {
        value.wrapping_mul(31).wrapping_add(byte as u64)
    });
    match hash % 10 {
        0 => FarmActivity::Plowing,
        1 => FarmActivity::Watering,
        2 => FarmActivity::Planting,
        3 => FarmActivity::Harvesting,
        4 => FarmActivity::Carrying,
        5 => FarmActivity::Digging,
        6 => FarmActivity::Raking,
        7 => FarmActivity::Repairing,
        8 => FarmActivity::Feeding,
        _ => FarmActivity::Chopping,
    }
}

#[cfg(test)]
mod display_name_tests {
    use super::*;

    #[test]
    fn names_workers_after_their_launch_folder() {
        assert_eq!(
            display_name_for_cwd("/home/alex/projects/plow", "repo"),
            "plow"
        );
        assert_eq!(display_name_for_cwd("/", "Local files"), "Local files");
        assert_eq!(display_name_for_cwd("", ""), "Codex");
    }

    #[test]
    fn loads_settings_saved_before_codex_path_was_added() {
        let settings: PlowSettings = serde_json::from_value(serde_json::json!({
            "notifyWhenUnfocused": false,
            "keepInTray": true,
            "reducedMotion": true
        }))
        .expect("legacy settings");
        assert_eq!(settings.codex_path, "");
        assert_eq!(settings.development_home, "");
        assert!(settings.local_enabled);
        assert!(settings.ssh_hosts.is_empty());
        assert!(matches!(settings.view_mode, AgentViewMode::Field));
        assert!(!settings.notify_when_unfocused);
        assert!(settings.reduced_motion);
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = Arc::new(SharedState::new(data_dir.join("plow-state.json")));
            app.manage(state.clone());

            let show = MenuItem::with_id(app, "show", "Show Plow", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::with_id("main")
                .menu(&menu)
                .tooltip("Plow — starting the farm");
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "show" => show_main_window(app),
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    show_main_window(tray.app_handle());
                }
            })
            .build(app)?;

            protocol::spawn_monitor(app.handle().clone(), state);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            mark_reviewed,
            open_thread,
            get_resume_command,
            list_projects,
            start_agent,
            list_ssh_hosts,
            get_settings,
            update_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running Plow");
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_assignment_is_stable() {
        let assignments = [
            ("d", FarmActivity::Plowing),
            ("e", FarmActivity::Watering),
            ("f", FarmActivity::Planting),
            ("g", FarmActivity::Harvesting),
            ("h", FarmActivity::Carrying),
            ("i", FarmActivity::Digging),
            ("j", FarmActivity::Raking),
            ("k", FarmActivity::Repairing),
            ("l", FarmActivity::Feeding),
            ("m", FarmActivity::Chopping),
        ];

        for (id, activity) in assignments {
            assert_eq!(activity_for(id), activity);
        }
    }

    #[test]
    fn cwd_fallback_has_a_readable_name() {
        let (name, root) = repo_for_cwd("/definitely/not/a/repository/plow");
        assert_eq!(name, "plow");
        assert_eq!(root, "/definitely/not/a/repository/plow");
    }

    #[test]
    fn project_listing_only_includes_visible_directories() {
        let home = std::env::temp_dir().join(format!("plow-projects-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(home.join("Zebra")).unwrap();
        fs::create_dir_all(home.join("apple")).unwrap();
        fs::create_dir_all(home.join(".hidden")).unwrap();
        fs::write(home.join("notes.txt"), b"not a project").unwrap();

        let projects = list_project_folders(&home).unwrap();
        assert_eq!(
            projects
                .iter()
                .map(|project| project.name.as_str())
                .collect::<Vec<_>>(),
            vec!["apple", "Zebra"]
        );
        assert!(validate_project_selection(&home, projects[0].path.as_str()).is_ok());
        assert!(validate_project_selection(&home, &home.to_string_lossy()).is_ok());
        assert!(validate_project_selection(&home, "/tmp").is_err());

        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn accepts_projects_below_a_symlinked_development_home() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!("plow-symlink-{}", uuid::Uuid::new_v4()));
        let home = root.join("real-home");
        let alias = root.join("home-alias");
        fs::create_dir_all(home.join("project")).unwrap();
        symlink(&home, &alias).unwrap();

        let selected = alias.join("project");
        assert!(validate_project_selection(&alias, &selected.to_string_lossy()).is_ok());

        fs::remove_dir_all(root).unwrap();
    }
}
