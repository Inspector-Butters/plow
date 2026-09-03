mod protocol;
mod terminal;

use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worker {
    pub id: String,
    pub parent_id: Option<String>,
    pub thread_name: String,
    pub repo_name: String,
    pub repo_path: String,
    pub cwd: String,
    pub branch: Option<String>,
    pub model: Option<String>,
    pub source: String,
    pub status: WorkerStatus,
    pub activity: FarmActivity,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub attention_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    MissingCodex,
    Incompatible,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub status: ConnectionStatus,
    pub codex_version: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSnapshot {
    pub workers: Vec<Worker>,
    pub connection: ConnectionInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlowSettings {
    pub notify_when_unfocused: bool,
    pub keep_in_tray: bool,
    pub reduced_motion: bool,
}

impl Default for PlowSettings {
    fn default() -> Self {
        Self {
            notify_when_unfocused: true,
            keep_in_tray: true,
            reduced_motion: false,
        }
    }
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
}

impl SharedState {
    fn new(storage_path: PathBuf) -> Self {
        let persisted: PersistedState = fs::read(&storage_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let attention = persisted.attention.clone();
        Self {
            snapshot: Mutex::new(MonitorSnapshot {
                workers: attention,
                connection: ConnectionInfo {
                    status: ConnectionStatus::Connecting,
                    codex_version: None,
                    message: "Starting the local monitor".to_string(),
                },
            }),
            persisted: Mutex::new(persisted),
            storage_path,
            codex_path: Mutex::new(None),
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

#[tauri::command]
fn open_thread(state: State<'_, Arc<SharedState>>, thread_id: String) -> Result<String, String> {
    let codex = state
        .codex_path
        .lock()
        .map_err(|error| error.to_string())?
        .clone()
        .ok_or("Codex is not connected")?;
    terminal::open_thread(&codex, &thread_id)
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
    state: State<'_, Arc<SharedState>>,
    settings: PlowSettings,
) -> Result<(), String> {
    state
        .persisted
        .lock()
        .map_err(|error| error.to_string())?
        .settings = settings;
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
        return ("unknown".to_string(), "unknown".to_string());
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
        .unwrap_or("unknown")
        .to_string();
    (name, root)
}

pub fn activity_for(id: &str) -> FarmActivity {
    let hash = id.bytes().fold(0_u64, |value, byte| {
        value.wrapping_mul(31).wrapping_add(byte as u64)
    });
    match hash % 5 {
        0 => FarmActivity::Plowing,
        1 => FarmActivity::Watering,
        2 => FarmActivity::Planting,
        3 => FarmActivity::Harvesting,
        _ => FarmActivity::Carrying,
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
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
        assert!(matches!(
            activity_for("thread-a"),
            FarmActivity::Plowing
                | FarmActivity::Watering
                | FarmActivity::Planting
                | FarmActivity::Harvesting
                | FarmActivity::Carrying
        ));
        assert_eq!(
            serde_json::to_string(&activity_for("thread-a")).unwrap(),
            serde_json::to_string(&activity_for("thread-a")).unwrap()
        );
    }

    #[test]
    fn cwd_fallback_has_a_readable_name() {
        let (name, root) = repo_for_cwd("/definitely/not/a/repository/plow");
        assert_eq!(name, "plow");
        assert_eq!(root, "/definitely/not/a/repository/plow");
    }
}
