// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

mod db;
mod log_reader;
mod log_snapshot;
mod parser;
mod templates;

use db::database::init_db;
use db::runs;
use db::runs::{insert_run, Run};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_fs::init;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[tauri::command]
fn save_run(run: Run) -> Result<i64, String> {
    let mut conn = init_db();
    insert_run(&mut conn, run).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_runs(
    nickname: String,
    template_id: Option<String>,
) -> Result<Vec<runs::Run>, String> {
    let mut conn = init_db();
    runs::get_runs(&mut conn, &nickname, template_id.as_deref())
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn get_run(run_id: i64) -> Result<Option<runs::Run>, String> {
    let mut conn = init_db();
    runs::get_run_by_id(&mut conn, run_id).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_best_run(
    nickname: String,
    template_id: String,
) -> Result<Option<runs::Run>, String> {
    let mut conn = init_db();
    runs::get_best_run(&mut conn, &nickname, &template_id)
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn delete_run(run_id: i64) -> Result<bool, String> {
    let mut conn = init_db();
    runs::delete_run(&mut conn, run_id).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_best_time(
    nickname: String,
    template_id: String,
) -> Result<Option<f64>, String> {
    let mut conn = init_db();
    runs::get_best_time(&mut conn, &nickname, &template_id)
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn get_best_splits(
    nickname: String,
    template_id: String,
) -> Result<Vec<runs::Split>, String> {
    let mut conn = init_db();
    runs::get_best_splits(&mut conn, &nickname, &template_id)
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn get_best_segments(
    nickname: String,
    template_id: String,
) -> Result<Vec<runs::Split>, String> {
    let mut conn = init_db();
    runs::get_best_segments(&mut conn, &nickname, &template_id)
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn get_template_summaries(nickname: String) -> Result<Vec<runs::TemplateSummary>, String> {
    let mut conn = init_db();
    runs::get_template_summaries(&mut conn, &nickname).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_runs_for_chart(
    nickname: String,
    template_id: String,
) -> Result<Vec<runs::RunChartPoint>, String> {
    let mut conn = init_db();
    runs::get_runs_for_chart(&mut conn, &nickname, &template_id)
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn sync_runs_to_template(
    nickname: String,
    from_template_id: String,
    to_template_id: String,
    to_template_name: String,
) -> Result<(), String> {
    let mut conn = init_db();
    runs::sync_runs_to_template(
        &mut conn,
        &nickname,
        &from_template_id,
        &to_template_id,
        &to_template_name,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn force_run_reset(app: tauri::AppHandle) {
    let _ = app.emit("force-run-reset", ());
}

#[tauri::command]
async fn start_log_reading(app: tauri::AppHandle, path: String) {
    if let Err(e) = log_reader::start_log_reader(app, path).await {
        println!("Watcher error: {:?}", e);
    }
}
#[tauri::command]
async fn read_log_snapshot(path: String) -> Result<log_snapshot::LogSnapshot, String> {
    log_snapshot::read_log_once(path).await
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    window: WindowSize,
    interface: InterfaceSettings,
    overlay: OverlaySettings,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WindowSize {
    x: i32,
    y: i32,
    pos_x: i32,
    pos_y: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InterfaceSettings {
    theme: Option<String>,
    language: String,
    path_log: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OverlaySettings {
    show: bool,
    pos_x: i32,
    pos_y: i32,
    overlay_transparent: i32,
    run_name: bool,
    show_splits: bool,
    number_of_splits: i32,
    split_separators: bool,
    sum_of_best: bool,
    fake_timer: bool,
    run_aborted: bool,
    sum_of_the_last: i32,
    time_accuracy: Option<String>,
    netracell_tip: bool,
    toggle_visibility_key: String,
    drag_mode: bool,
    toggle_mode_key: String,
    run_reset_key: String,
}

struct SettingsState {
    inner: Mutex<AppSettings>,
}
struct ShortcutStateStore {
    shortcuts: Mutex<Vec<(String, String)>>,
}

fn default_settings() -> AppSettings {
    AppSettings {
        window: WindowSize {
            x: 1200,
            y: 500,
            pos_x: 100,
            pos_y: 100,
        },
        interface: InterfaceSettings {
            theme: Some("system".into()),
            language: "system".into(),
            path_log: "%LOCALAPPDATA%\\Warframe".into(),
        },
        overlay: OverlaySettings {
            show: false,
            pos_x: 100,
            pos_y: 100,
            overlay_transparent: 50,
            run_name: true,
            show_splits: true,
            number_of_splits: 0,
            split_separators: true,
            sum_of_best: true,
            fake_timer: true,
            run_aborted: true,
            sum_of_the_last: 0,
            time_accuracy: Some("milliseconds".into()),
            netracell_tip: true,
            toggle_visibility_key: "F6".into(),
            drag_mode: true,
            toggle_mode_key: "F4".into(),
            run_reset_key: "F3".into(),
        },
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<SettingsState>) -> AppSettings {
    state.inner.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(
    new_settings: AppSettings,
    state: tauri::State<SettingsState>,
    app: tauri::AppHandle,
) {
    let mut settings = state.inner.lock().unwrap();
    *settings = new_settings.clone();

    save_settings_to_file(&new_settings);

    if let Some(window) = app.get_webview_window("overlay-window") {
        if new_settings.overlay.show {
            let _ = window.show();
        } else {
            let _ = window.hide();
        }
    }

    app.emit("settings-updated", new_settings).unwrap();
}

use std::fs;
use std::path::PathBuf;

fn get_settings_path() -> PathBuf {
    let mut path = dirs::config_dir().expect("No config dir");
    path.push("WFAutoSplitter");
    fs::create_dir_all(&path).unwrap();
    path.push("settings.json");
    path
}

fn save_settings_to_file(settings: &AppSettings) {
    let path = get_settings_path();

    let json = serde_json::to_string_pretty(settings).unwrap();
    fs::write(path, json).unwrap();
}

fn load_settings_from_file() -> AppSettings {
    let path = get_settings_path();

    if !path.exists() {
        return default_settings();
    }

    let content = fs::read_to_string(path).unwrap_or_default();

    serde_json::from_str(&content).unwrap_or_else(|_| default_settings())
}

#[tauri::command]
fn register_shortcut_command(
    app: tauri::AppHandle,
    shortcut_key: String,
    shortcut_type: String,
    state: tauri::State<ShortcutStateStore>,
) -> Result<(), String>{
    let mut shortcuts = state.shortcuts.lock().unwrap();

    if shortcuts.iter().any(|(key, typ)| key == &shortcut_key && typ != &shortcut_type) {
        return Err("Shortcut already used by another action".into());
    }

    if let Some((key, _)) = shortcuts.iter_mut().find(|(_, t)| t == &shortcut_type) {
        *key = shortcut_key.clone();
    } else {
        shortcuts.push((shortcut_key.clone(), shortcut_type.clone()));
    }

    use tauri_plugin_global_shortcut::Shortcut;

    app.global_shortcut().unregister_all().ok();

    for (key, _) in shortcuts.iter() {
        let shortcut: Shortcut = key.parse().unwrap();

        app.global_shortcut()
            .register(shortcut)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn unregister_shortcut_command(app: tauri::AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }

                    let pressed_key = shortcut.to_string();

                    let state = app.state::<ShortcutStateStore>();
                    let shortcuts = state.shortcuts.lock().unwrap();

                    let action: Option<String> = shortcuts
                        .iter()
                        .find(|(key, _): &&(String, String)| key == &pressed_key)
                        .map(|(_, typ)| typ.clone());

                    drop(shortcuts);

                    let Some(action) = action else { return };

                    let app_clone = app.clone();

                    tauri::async_runtime::spawn(async move {
                        match action.as_str() {
                            "toggle_visibility" => {
                                let state = app_clone.state::<SettingsState>();
                                let mut settings = state.inner.lock().unwrap();

                                settings.overlay.show = !settings.overlay.show;

                                save_settings_to_file(&settings);

                                if let Some(window) = app_clone.get_webview_window("overlay-window") {
                                    if settings.overlay.show {
                                        let _ = window.show();
                                    } else {
                                        let _ = window.hide();
                                    }
                                }

                                let _ = app_clone.emit("settings-updated", settings.clone());
                            }

                            "toggle_mode" => {
                                let _ = app_clone.emit("toggle-overlay-mode", ());
                            }

                            "run_reset" => {
                                let _ = app_clone.emit("run-reset", ());
                                let _ = app_clone.emit("force-run-reset", ());
                            }

                            _ => {}
                        }
                    });
                })
                .build()
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(init())
        .manage(SettingsState {
            inner: Mutex::new(load_settings_from_file()),
        })
        .manage(ShortcutStateStore {
            shortcuts: Mutex::new(Vec::new()),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            start_log_reading,
            read_log_snapshot,
            get_settings,
            set_settings,

            templates::commands::import_default_templates,
            templates::commands::get_templates,
            templates::commands::create_template,
            templates::commands::update_template,
            templates::commands::delete_template,
            templates::commands::get_default_templates,
            templates::commands::import_default_template,

            save_run,
            get_runs,
            get_run,
            get_best_run,
            delete_run,
            get_best_time,
            get_best_splits,
            get_best_segments,
            get_template_summaries,
            get_runs_for_chart,
            sync_runs_to_template,
            force_run_reset,

            register_shortcut_command,
            unregister_shortcut_command,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    if let Some(test_window) =
                        window.app_handle().get_webview_window("overlay-window")
                    {
                        let _ = test_window.close();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
