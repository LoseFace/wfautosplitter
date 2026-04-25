use dirs;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::env;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{fs::File, io::AsyncSeekExt, sync::Mutex};

use tauri::{AppHandle, Emitter, Listener};

use crate::parser::LogParser;

use crossbeam_channel::unbounded;
use crate::parser::overlay_bridge::OverlayBridge;

pub async fn start_log_reader(app: AppHandle, path: String) -> notify::Result<()> {
    let (tx, rx) = unbounded();
    let snapshot_nickname = crate::log_snapshot::read_log_once(path.clone())
        .await
        .ok()
        .and_then(|s| s.nickname);

    let parser = LogParser::new()
        .with_nickname(snapshot_nickname)
        .with_event_sender(tx);
    let bridge = OverlayBridge::new(rx, app.clone());
    bridge.start();

    let parser = Arc::new(Mutex::new(parser));

    let mut templates_path = dirs::config_dir().unwrap();
    templates_path.push("WFAutoSplitter/templates.json");
    let templates_dir = templates_path.parent().unwrap().to_path_buf();

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

    let directory = file_path.parent().unwrap().to_path_buf();
    let file_name = file_path.file_name().unwrap().to_owned();

    let offset = Arc::new(Mutex::new(0u64));

    if file_path.exists() {
        if let Ok(metadata) = std::fs::metadata(&file_path) {
            let file_size = metadata.len();
            let mut off = offset.lock().await;
            *off = file_size;
        }
    }

    let tail_path = file_path.clone();
    let tail_offset = offset.clone();
    let tail_app = app.clone();
    let parser_tail = parser.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            read_new_lines(
                &tail_app,
                &tail_path,
                tail_offset.clone(),
                parser_tail.clone(),
                &tail_app,
            )
            .await;
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    });

    let offset_clone = offset.clone();

    let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let relevant = event
                    .paths
                    .iter()
                    .any(|p| p.file_name() == Some(&file_name));

                if !relevant {
                    return;
                }

                match event.kind {
                    EventKind::Remove(_) | EventKind::Create(_) => {
                        let offset_inner = offset_clone.clone();

                        tauri::async_runtime::spawn(async move {
                            let mut off = offset_inner.lock().await;
                            if *off != 0 {
                                *off = 0;
                                // println!("File recreated → offset reset");
                            }
                        });
                    }
                    _ => {}
                }
            }
        },
        Config::default(),
    )?;

    watcher.watch(&directory, RecursiveMode::NonRecursive)?;

    let parser_clone = parser.clone();
    let mut templates_watcher: RecommendedWatcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let changed = event.paths.iter().any(|p| {
                    p.file_name()
                        .map(|f| f == "templates.json")
                        .unwrap_or(false)
                });

                if !changed {
                    return;
                }

                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    let parser_inner = parser_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        let mut p = parser_inner.lock().await;
                        p.reload_templates();
                    });
                }
            }
        },
        Config::default(),
    )?;
    templates_watcher.watch(&templates_dir, RecursiveMode::NonRecursive)?;

    let parser_reset = parser.clone();
    let unlisten_reset = app.listen("force-run-reset", move |_| {
        let parser_inner = parser_reset.clone();
        tauri::async_runtime::spawn(async move {
            let mut p = parser_inner.lock().await;
            p.reset_active_run("RUN RESET (manual)");
        });
    });
    let _ = unlisten_reset;

    std::mem::forget(watcher);
    std::mem::forget(templates_watcher);
    Ok(())
}

use tokio::io::AsyncReadExt;

async fn read_new_lines(
    app: &AppHandle,
    path: &PathBuf,
    offset: Arc<Mutex<u64>>,
    parser: Arc<Mutex<LogParser>>,
    event_app: &AppHandle,
) {
    if !Path::new(path).exists() {
        let mut off = offset.lock().await;
        *off = 0;
        return;
    }

    let metadata = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return,
    };

    let file_size = metadata.len();

    let mut current_offset = {
        let off = offset.lock().await;
        *off
    };

    if file_size < current_offset {
        current_offset = 0;
    }

    if file_size <= current_offset {
        return;
    }

    if let Ok(mut file) = File::open(path).await {
        file.seek(std::io::SeekFrom::Start(current_offset))
            .await
            .ok();

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await.ok();

        let mut start = 0;

        for i in 0..buffer.len() {
            if buffer[i] == b'\n' {
                let line_bytes = &buffer[start..i];

                if let Ok(line) = String::from_utf8(line_bytes.to_vec()) {
                    let mut p = parser.lock().await;
                    p.process_line(&line, event_app);
                    app.emit("log-line", line).ok();
                }

                start = i + 1;
            }
        }

        current_offset += start as u64;

        let mut off = offset.lock().await;
        *off = current_offset;
    }
}
