use crate::templates::model::Template;
use std::fs;
use std::path::PathBuf;

const FILE_NAME: &str = "templates.json";

pub fn get_templates_path(app_dir: &PathBuf) -> PathBuf {
    let mut path = app_dir.clone();
    path.push(FILE_NAME);
    path
}

pub fn load_templates(app_dir: &PathBuf) -> Result<Vec<Template>, String> {
    let path = get_templates_path(app_dir);

    if !path.exists() {
        return Ok(Vec::new());
    }

    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let templates: Vec<Template> = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    Ok(templates)
}

pub fn save_templates(app_dir: &PathBuf, templates: &Vec<Template>) -> Result<(), String> {
    std::fs::create_dir_all(app_dir).map_err(|e| e.to_string())?;

    let path = get_templates_path(app_dir);

    let json = serde_json::to_string_pretty(templates).map_err(|e| e.to_string())?;

    std::fs::write(path, json).map_err(|e| e.to_string())?;

    Ok(())
}
