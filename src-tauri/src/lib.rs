// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

mod db;
mod hotkeys;
mod log_reader;
mod parser;
mod templates;

use db::database::init_db;
use db::runs;
use db::runs::{insert_run, Run};
use hotkeys::{parse_shortcut, HotkeyEntry, HotkeyTable};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_fs::init;

#[tauri::command]
fn save_run(run: Run) -> Result<i64, String> {
    let mut conn = init_db();
    insert_run(&mut conn, run).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_runs(template_id: Option<String>) -> Result<Vec<runs::Run>, String> {
    let mut conn = init_db();
    runs::get_runs(&mut conn, template_id.as_deref()).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_run(run_id: i64) -> Result<Option<runs::Run>, String> {
    let mut conn = init_db();
    runs::get_run_by_id(&mut conn, run_id).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_best_run(template_id: String) -> Result<Option<runs::Run>, String> {
    let mut conn = init_db();
    runs::get_best_run(&mut conn, &template_id).map_err(|e| e.to_string())
}
#[tauri::command]
fn delete_run(run_id: i64) -> Result<bool, String> {
    let mut conn = init_db();
    runs::delete_run(&mut conn, run_id).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_best_time(template_id: String) -> Result<Option<f64>, String> {
    let mut conn = init_db();
    runs::get_best_time(&mut conn, &template_id).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_best_splits(template_id: String) -> Result<Vec<runs::Split>, String> {
    let mut conn = init_db();
    runs::get_best_splits(&mut conn, &template_id).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_best_segments(template_id: String) -> Result<Vec<runs::Split>, String> {
    let mut conn = init_db();
    runs::get_best_segments(&mut conn, &template_id).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_template_summaries() -> Result<Vec<runs::TemplateSummary>, String> {
    let mut conn = init_db();
    runs::get_template_summaries(&mut conn).map_err(|e| e.to_string())
}
#[tauri::command]
fn get_runs_for_chart(template_id: String) -> Result<Vec<runs::RunChartPoint>, String> {
    let mut conn = init_db();
    runs::get_runs_for_chart(&mut conn, &template_id).map_err(|e| e.to_string())
}
#[tauri::command]
fn rename_template_runs(template_id: String, new_name: String) -> Result<(), String> {
    let mut conn = init_db();
    runs::rename_template_runs(&mut conn, &template_id, &new_name).map_err(|e| e.to_string())
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

#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    window: WindowSize,
    last_seen_version: Option<String>,
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
    pub custom_locales_dir: Option<String>,
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
    time_accuracy: Option<String>,
    time_gold: Option<String>,
    split_separators: bool,
    group_list: bool,
    sum_of_best: bool,
    fake_timer: bool,
    run_aborted: bool,
    sum_of_the_last: i32,
    toggle_visibility_key: String,
    drag_mode: bool,
    toggle_mode_key: String,
    run_reset_key: String,
}

struct SettingsState {
    inner: Mutex<AppSettings>,
}

struct HotkeyStore {
    table: HotkeyTable,
}

fn default_settings() -> AppSettings {
    AppSettings {
        window: WindowSize { x: 1200, y: 500, pos_x: 100, pos_y: 100 },
        last_seen_version: None,
        interface: InterfaceSettings {
            theme: Some("system".into()),
            language: "system".into(),
            path_log: "%LOCALAPPDATA%\\Warframe".into(),
            custom_locales_dir: None,
        },
        overlay: OverlaySettings {
            show: false,
            pos_x: 100,
            pos_y: 100,
            overlay_transparent: 50,
            run_name: true,
            show_splits: true,
            number_of_splits: 6,
            time_accuracy: Some("milliseconds".into()),
            time_gold: Some("segments".into()),
            split_separators: true,
            group_list: true,
            sum_of_best: true,
            fake_timer: true,
            run_aborted: true,
            sum_of_the_last: 0,
            toggle_visibility_key: "F7".into(),
            drag_mode: true,
            toggle_mode_key: "F5".into(),
            run_reset_key: "F4".into(),
        },
    }
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
fn get_settings(state: tauri::State<SettingsState>) -> AppSettings {
    state.inner.lock().unwrap().clone()
}

#[tauri::command]
fn get_default_settings() -> AppSettings {
    default_settings()
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

#[tauri::command]
fn register_shortcut_command(
    shortcut_key: String,
    shortcut_type: String,
    state: tauri::State<HotkeyStore>,
) -> Result<(), String> {
    let new_entry = parse_shortcut(&shortcut_key, &shortcut_type)
        .ok_or_else(|| format!("Unsupported key: '{}'", shortcut_key))?;
    let new_id = (new_entry.vk_code, new_entry.modifiers.clone());
    let mut table = state.table.lock().unwrap();

    if table.iter().any(|e| {
        (e.vk_code, e.modifiers.clone()) == new_id && e.shortcut_type != shortcut_type
    }) {
        return Err("Shortcut already used by another action".into());
    }

    if let Some(entry) = table.iter_mut().find(|e| e.shortcut_type == shortcut_type) {
        *entry = new_entry;
    } else {
        table.push(new_entry);
    }

    Ok(())
}

#[tauri::command]
fn unregister_shortcut_command(
    shortcut_type: String,
    state: tauri::State<HotkeyStore>,
) -> Result<(), String> {
    let mut table = state.table.lock().unwrap();
    table.retain(|e| e.shortcut_type != shortcut_type);
    Ok(())
}

#[tauri::command]
fn read_custom_locales(dir: String) -> Result<Vec<(String, String)>, String> {
    let path = std::path::Path::new(&dir);
    if !path.exists() || !path.is_dir() {
        return Ok(vec![]);
    }
    let mut result = vec![];
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_path = entry.path();
        if file_path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let lang_code = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if lang_code.is_empty() { continue; }
        let content = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
        serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|_| format!("{}.json содержит невалидный JSON", lang_code))?;
        result.push((lang_code, content));
    }
    Ok(result)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_settings = load_settings_from_file();

    let initial_table: Vec<HotkeyEntry> = [
        (&initial_settings.overlay.toggle_visibility_key, "toggle_visibility"),
        (&initial_settings.overlay.toggle_mode_key,       "toggle_mode"),
        (&initial_settings.overlay.run_reset_key,         "run_reset"),
    ]
    .iter()
    .filter_map(|(key, typ)| parse_shortcut(key, typ))
    .collect();

    let table: HotkeyTable = Arc::new(Mutex::new(initial_table));
    let table_for_hook = Arc::clone(&table);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(init())
        .manage(SettingsState {
            inner: Mutex::new(load_settings_from_file()),
        })
        .manage(HotkeyStore { table })
        .invoke_handler(tauri::generate_handler![
            greet,
            start_log_reading,
            get_settings,
            get_default_settings,
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
            rename_template_runs,
            force_run_reset,

            register_shortcut_command,
            unregister_shortcut_command,
            read_custom_locales,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    if let Some(overlay) =
                        window.app_handle().get_webview_window("overlay-window")
                    {
                        let _ = overlay.close();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    let app_handle = app.handle().clone();

    hotkeys::start_hook(table_for_hook, move |action| {
        let app = app_handle.clone();

        tauri::async_runtime::spawn(async move {
            match action.as_str() {
                "toggle_visibility" => {
                    let state = app.state::<SettingsState>();
                    let mut settings = state.inner.lock().unwrap();

                    settings.overlay.show = !settings.overlay.show;
                    save_settings_to_file(&settings);

                    if let Some(window) = app.get_webview_window("overlay-window") {
                        if settings.overlay.show {
                            let _ = window.show();
                        } else {
                            let _ = window.hide();
                        }
                    }

                    let _ = app.emit("settings-updated", settings.clone());
                }

                "toggle_mode" => {
                    let _ = app.emit("toggle-overlay-mode", ());
                }

                "run_reset" => {
                    let _ = app.emit("run-reset", ());
                    let _ = app.emit("force-run-reset", ());
                }

                _ => {}
            }
        });
    });

    app.run(|_, _| {});
}