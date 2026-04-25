use std::fs;

use super::models::Template;

pub fn load_templates() -> Vec<Template> {
    let mut path = dirs::config_dir().unwrap();
    path.push("WFAutoSplitter/templates.json");

    if !path.exists() {
        return vec![];
    }

    let content = fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str(&content).unwrap_or_default()
}
