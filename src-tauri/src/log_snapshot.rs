use std::{env, path::PathBuf};
use tokio::fs;

#[derive(serde::Serialize)]
pub struct LogSnapshot {
    pub lines: Vec<String>,
    pub nickname: Option<String>,
}

pub async fn read_log_once(path: String) -> Result<LogSnapshot, String> {
    let expanded_path = if path.contains('%') {
        let mut result = path.clone();
        for (key, value) in env::vars() {
            let pattern = format!("%{}%", key);
            if result.contains(&pattern) {
                result = result.replace(&pattern, &value);
            }
        }
        result
    } else {
        path.clone()
    };

    let mut file_path = PathBuf::from(expanded_path);
    file_path.push("EE.log");

    if !file_path.exists() {
        return Err("Log file not found".into());
    }

    let content = fs::read_to_string(&file_path)
        .await
        .map_err(|e| e.to_string())?;

    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    let nickname = extract_nickname(&lines);

    Ok(LogSnapshot { lines, nickname })
}

fn extract_nickname(lines: &[String]) -> Option<String> {
    for line in lines.iter().rev() {
        if line.contains("Logged in ") {
            if let Some(nickname) = line.split("Logged in ").nth(1) {
                if let Some(end) = nickname.find('(') {
                    return Some(nickname[..end].trim().to_string());
                }
            }
        }
    }
    None
}
