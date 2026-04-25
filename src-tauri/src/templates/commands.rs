use std::collections::HashSet;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::templates::model::{Step, Template};
use crate::templates::storage::{load_templates, save_templates};

fn get_first_mission_code(t: &Template) -> Option<String> {
    t.groups.get(0).and_then(|g| {
        g.steps.iter().find_map(|step| {
            if let Step::StartMission { mission_code, .. } = step {
                mission_code.clone()
            } else {
                None
            }
        })
    })
}

#[tauri::command]
pub fn import_default_templates(app: AppHandle) -> Result<(), String> {
    use std::fs;
    use std::path::PathBuf;

    let app_dir = app.path().app_data_dir().unwrap();
    let mut current_templates = load_templates(&app_dir)?;

    let default_path: PathBuf = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("default_templates.json")
    } else {
        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_dir = exe_path.parent().ok_or("Cannot get exe dir")?;
        exe_dir.join("resources").join("default_templates.json")
    };

    if !default_path.exists() {
        return Err(format!(
            "Default templates file not found at: {:?}",
            default_path
        ));
    }

    let data = fs::read_to_string(&default_path).map_err(|e| e.to_string())?;

    let default_templates: Vec<Template> = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    let existing_ids: HashSet<String> = current_templates.iter().map(|t| t.id.clone()).collect();
    let existing_keys: HashSet<(String, Option<String>)> = current_templates
        .iter()
        .map(|t| (t.name.clone(), get_first_mission_code(t)))
        .collect();

    for template in default_templates {
        let key = (template.name.clone(), get_first_mission_code(&template));
        if !existing_ids.contains(&template.id) && !existing_keys.contains(&key) {
            let mut new_template = template;
            new_template.is_active = false;
            current_templates.push(new_template);
        }
    }

    save_templates(&app_dir, &current_templates)
}

#[tauri::command]
pub fn get_templates(app: AppHandle) -> Result<Vec<Template>, String> {
    let app_dir = app.path().app_data_dir().unwrap();
    load_templates(&app_dir)
}

#[tauri::command]
pub fn create_template(app: AppHandle, mut template: Template) -> Result<(), String> {
    let app_dir = app.path().app_data_dir().unwrap();
    let mut templates = load_templates(&app_dir)?;

    if templates.iter().any(|t| t.name == template.name) {
        return Err("Template name must be unique".into());
    }

    template.id = Uuid::new_v4().to_string();
    template.is_active = false;

    templates.push(template);

    save_templates(&app_dir, &templates)
}

#[tauri::command]
pub fn update_template(app: AppHandle, template: Template) -> Result<(), String> {
    let app_dir = app.path().app_data_dir().unwrap();
    let mut templates = load_templates(&app_dir)?;

    let pos = templates
        .iter()
        .position(|t| t.id == template.id)
        .ok_or("Template not found")?;

    templates[pos] = template;

    save_templates(&app_dir, &templates)
}

#[tauri::command]
pub fn delete_template(app: AppHandle, template_id: String) -> Result<(), String> {
    let app_dir = app.path().app_data_dir().unwrap();
    let mut templates = load_templates(&app_dir)?;

    templates.retain(|t| t.id != template_id);

    save_templates(&app_dir, &templates)
}

#[tauri::command]
pub fn get_default_templates(app: AppHandle) -> Result<Vec<Template>, String> {
    use std::fs;
    use std::path::PathBuf;

    let default_path: PathBuf = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("default_templates.json")
    } else {
        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_dir = exe_path.parent().ok_or("Cannot get exe dir")?;
        exe_dir.join("resources").join("default_templates.json")
    };


    if !default_path.exists() {
        return Err(format!(
            "Default templates file not found at: {:?}",
            default_path
        ));
    }

    let data = fs::read_to_string(&default_path).map_err(|e| e.to_string())?;

    let templates: Vec<Template> = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    Ok(templates)
}

#[tauri::command]
pub fn import_default_template(app: AppHandle, template: Template) -> Result<(), String> {
    let app_dir = app.path().app_data_dir().unwrap();
    let mut templates = load_templates(&app_dir)?;

    let key = (template.name.clone(), get_first_mission_code(&template));
    let exists_by_id = templates.iter().any(|t| t.id == template.id);
    let exists_by_key = templates.iter().any(|t| (t.name.clone(), get_first_mission_code(t)) == key);

    if exists_by_id || exists_by_key {
        return Ok(());
    }

    let mut new_template = template;
    new_template.is_active = false;

    templates.push(new_template);

    save_templates(&app_dir, &templates)
}
