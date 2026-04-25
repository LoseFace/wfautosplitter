use super::events::{LogEvent, SplitInfo};
use crossbeam_channel::Receiver;
use std::collections::HashMap;
use tauri::Emitter;
use serde::Serialize;
use uuid::Uuid;

use crate::db::database::init_db;
use crate::db::runs::{insert_run, increment_aborts, Run, Split};

#[derive(Serialize, Clone)]
pub struct OverlaySplit {
    pub id: String,
    pub name: String,
    pub group_id: String,
    pub order: u32,
    pub is_completed: bool,
    pub split_time: Option<f64>,
}

#[derive(Serialize, Clone)]
pub struct OverlayState {
    pub template_name: String,
    pub template_id: String,
    pub sequential_mode: bool,
    pub splits: Vec<OverlaySplit>,
    pub current_timer: Option<f64>,
    pub is_running: bool,
    pub is_trigger_only: bool,
}

pub struct OverlayBridge {
    receiver: Receiver<LogEvent>,
    app_handle: tauri::AppHandle,
}

struct OverlayBridgeState {
    template_name: String,
    sequential_mode: bool,
    splits: Vec<OverlaySplit>,
    added_groups: HashMap<String, bool>,
    run_start_time: Option<f64>,
    current_timer: Option<f64>,
    excluded_time: f64,
    last_group_end_time: Option<f64>,
    exclude_time_between_groups: bool,
    is_running: bool,
    is_trigger_only: bool,
    netracell_icons: Vec<String>,
    template_id: String,
    player_nickname: String,
}

impl OverlayBridge {
    pub fn new(receiver: Receiver<LogEvent>, app_handle: tauri::AppHandle) -> Self {
        Self {
            receiver,
            app_handle,
        }
    }

    pub fn start(self) {
        std::thread::spawn(move || {
            let mut state = OverlayBridgeState {
                template_name: String::new(),
                sequential_mode: false,
                splits: Vec::new(),
                added_groups: HashMap::new(),
                run_start_time: None,
                current_timer: None,
                excluded_time: 0.0,
                last_group_end_time: None,
                exclude_time_between_groups: false,
                is_running: false,
                is_trigger_only: false,
                netracell_icons: Vec::new(),
                template_id: String::new(),
                player_nickname: String::new(),
            };

            while let Ok(event) = self.receiver.recv() {
                match event {
                    LogEvent::RunStarted {
                        template_id,
                        template_name,
                        player_nickname,
                        sequential_mode,
                        exclude_time_between_groups,
                        group_id,
                        group_splits,
                        run_start_time,
                    } => {
                        state.template_id = template_id;

                        state.template_name = template_name;
                        state.sequential_mode = sequential_mode;
                        state.exclude_time_between_groups = exclude_time_between_groups;
                        state.player_nickname = player_nickname;
                        state.splits.clear();
                        state.added_groups.clear();
                        state.run_start_time = run_start_time;
                        state.current_timer = Some(0.0);
                        state.excluded_time = 0.0;
                        state.last_group_end_time = None;
                        state.is_trigger_only = run_start_time.is_some();
                        state.is_running = true;

                        if run_start_time.is_some() {
                            let _ = self.app_handle.emit("first-split-received", ());
                        }

                        if sequential_mode {
                            let new_splits: Vec<OverlaySplit> = group_splits
                                .into_iter()
                                .enumerate()
                                .map(|(idx, s)| OverlaySplit {
                                    id: Uuid::new_v4().to_string(),
                                    name: s.name,
                                    group_id: "".to_string(),
                                    order: idx as u32,
                                    is_completed: false,
                                    split_time: None,
                                })
                                .collect();
                            state.splits = new_splits;
                        } else {
                            Self::add_group_splits(&mut state, group_id, group_splits);
                        }

                        Self::emit_state(&self.app_handle, &state);
                    }

                    LogEvent::SplitCompleted {
                        group_id,
                        split_name,
                        split_time,
                        is_end_mission,
                    } => {
                        let is_first_split_of_run = state.run_start_time.is_none();

                        if is_first_split_of_run {
                            state.run_start_time = split_time;
                            state.current_timer = Some(0.0);
                            let _ = self.app_handle.emit("first-split-received", ());
                        }

                        let is_first_split_of_new_group = !is_first_split_of_run
                            && state.exclude_time_between_groups
                            && state.last_group_end_time.is_some();

                        if is_first_split_of_new_group {
                            if let (Some(prev_end), Some(cur_start)) =
                                (state.last_group_end_time, split_time)
                            {
                                let gap = cur_start - prev_end;
                                if gap > 0.0 {
                                    state.excluded_time += gap;
                                }
                            }
                            state.last_group_end_time = None;
                            let _ = self.app_handle.emit("timer-resume", ());
                        }

                        let relative_time = match (state.run_start_time, split_time) {
                            (Some(start), Some(t)) => {
                                let mut rel = t - start - state.excluded_time;
                                if rel < 0.0 {
                                    rel = 0.0;
                                }
                                Some((rel * 1000.0).round() / 1000.0)
                            }
                            _ => None,
                        };

                        if state.sequential_mode {
                            if let Some(split) = state
                                .splits
                                .iter_mut()
                                .find(|s| s.name == split_name && !s.is_completed)
                            {
                                split.is_completed = true;
                                split.split_time = relative_time;
                                state.current_timer = relative_time;
                            }
                        } else if let Some(split) = state.splits.iter_mut().find(|s| {
                            s.group_id == group_id && s.name == split_name && !s.is_completed
                        }) {
                            split.is_completed = true;
                            split.split_time = relative_time;
                            state.current_timer = relative_time;
                        }

                        if is_end_mission {
                            state.added_groups.insert(group_id.clone(), true);
                            state.last_group_end_time = split_time;

                            if state.exclude_time_between_groups {
                                if let Some(frozen) = relative_time {
                                    let _ = self.app_handle.emit("timer-pause", frozen);
                                }
                            }
                        }

                        Self::emit_state(&self.app_handle, &state);
                    }

                    LogEvent::GroupCompleted { group_id: _ } => {
                        Self::emit_state(&self.app_handle, &state);
                    }

                    LogEvent::GroupAdded { group_id, group_splits } => {
                        if !state.added_groups.contains_key(&group_id) {
                            Self::add_group_splits(&mut state, group_id, group_splits);
                            Self::emit_state(&self.app_handle, &state);
                        }
                    }

                    LogEvent::RunReset => {
                        if state.is_running
                            && !state.template_id.is_empty()
                            && !state.player_nickname.is_empty()
                        {
                            let mut conn = init_db();
                            if let Err(e) = increment_aborts(
                                &mut conn,
                                &state.player_nickname,
                                &state.template_id,
                            ) {
                                eprintln!("[DB] Failed to increment aborts: {}", e);
                            }
                        }

                        state.is_running = false;
                        Self::emit_state(&self.app_handle, &state);
                    }

                    LogEvent::RunFinished { total_time, player_nickname } => {
                        state.player_nickname = player_nickname.clone();
                        state.is_running = false;
                        state.current_timer = Some(total_time);
                        Self::emit_state(&self.app_handle, &state);

                        let splits: Vec<Split> = state
                            .splits
                            .iter()
                            .filter(|s| s.is_completed)
                            .map(|s| Split {
                                split_index: s.order as i64,
                                split_name: s.name.clone(),
                                split_time: s.split_time.unwrap_or(0.0),
                            })
                            .collect();

                        let created_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;

                        let run = Run {
                            id: None,
                            nickname: player_nickname,
                            template_id: state.template_id.clone(),
                            template_name: state.template_name.clone(),
                            created_at,
                            total_time,
                            splits,
                        };

                        let mut conn = init_db();
                        match insert_run(&mut conn, run) {
                            Ok(id) => {
                                // println!("[DB] Run saved, id={}", id);
                                let _ = self.app_handle.emit("run-saved", id);
                            }
                            Err(e) => {
                                eprintln!("[DB] Failed to save run: {}", e);
                                let _ = self.app_handle.emit("run-save-error", e.to_string());
                            }
                        }
                    }

                    LogEvent::TimerPause { frozen_time } => {
                        let _ = self.app_handle.emit("timer-pause", frozen_time);
                    }

                    LogEvent::TimerResume => {
                        let _ = self.app_handle.emit("timer-resume", ());
                    }

                    LogEvent::NetracellIcons { icons } => {
                        state.netracell_icons = icons.clone();
                        let _ = self.app_handle.emit("netracell-icons", icons);
                    }
                }
            }
        });
    }

    fn add_group_splits(
        state: &mut OverlayBridgeState,
        group_id: String,
        splits: Vec<SplitInfo>,
    ) {
        let mut new_splits: Vec<OverlaySplit> = splits
            .into_iter()
            .map(|s| OverlaySplit {
                id: Uuid::new_v4().to_string(),
                name: s.name,
                group_id: group_id.clone(),
                order: s.order,
                is_completed: false,
                split_time: None,
            })
            .collect();

        if state.sequential_mode {
            let start_order = state.splits.len() as u32;
            for split in &mut new_splits {
                split.order = start_order + split.order;
            }
            state.splits.extend(new_splits);
        } else {
            let start_order = state.splits.len() as u32;
            for split in &mut new_splits {
                split.order = start_order + split.order;
            }
            state.splits.append(&mut new_splits);
        }

        state.added_groups.insert(group_id, false);
    }

    fn emit_state(app_handle: &tauri::AppHandle, state: &OverlayBridgeState) {
        let overlay_state = OverlayState {
            template_name: state.template_name.clone(),
            template_id: state.template_id.clone(),
            sequential_mode: state.sequential_mode,
            splits: state.splits.clone(),
            current_timer: state.current_timer,
            is_running: state.is_running,
            is_trigger_only: state.is_trigger_only,
        };

        let _ = app_handle.emit("overlay-update", overlay_state);
    }
}