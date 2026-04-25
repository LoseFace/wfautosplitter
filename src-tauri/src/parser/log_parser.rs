use super::loader::load_templates;
use super::models::StepType;
use super::runtime::RuntimeTemplate;
use tauri::{AppHandle, Emitter};
use super::events::{LogEvent, SplitInfo};
use crossbeam_channel::Sender;
use uuid::Uuid;

pub struct LogParser {
    templates: Vec<RuntimeTemplate>,
    active_run: Option<usize>,

    cancel_keyword: String,
    exit_keyword: String,
    failed_keyword: String,

    templates_modified: Option<std::time::SystemTime>,

    player_nickname: Option<String>,

    event_sender: Option<Sender<LogEvent>>,

    mission_aborts: i64,

    netracell_icons: Vec<String>,
    netracell_icon_count: usize,
}

impl LogParser {
    pub fn with_event_sender(mut self, sender: Sender<LogEvent>) -> Self {
        self.event_sender = Some(sender);
        self
    }

    pub fn with_nickname(mut self, nickname: Option<String>) -> Self {
        self.player_nickname = nickname;
        self
    }

    fn send_event(&self, event: LogEvent) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event);
        }
    }

    fn matches_trigger(line: &str, trigger_keyword: &str) -> bool {
        if trigger_keyword.contains("&&") {
            trigger_keyword
                .split("&&")
                .any(|kw| line.contains(kw.trim()))
        } else {
            line.contains(trigger_keyword)
        }
    }

    fn matches_mission_code(line: &str, code: &str) -> bool {
        if code.contains("&&") {
            code.split("&&")
                .any(|kw| line.contains(kw.trim()))
        } else {
            line.contains(code)
        }
    }

    fn is_trigger_only_template(runtime: &RuntimeTemplate) -> bool {
        runtime.template.groups.iter().all(|g| {
            g.steps.iter().all(|s| {
                s.mission_code
                    .as_ref()
                    .map(|c| c.is_empty())
                    .unwrap_or(true)
            })
        })
    }

    pub fn reload_templates(&mut self) {
        if let Some(index) = self.active_run {
            let runtime = &mut self.templates[index];
            let name = runtime.template.name.clone();
            println!("[{}] RUN RESET (templates updated)", name);
            runtime.reset();
            self.send_event(LogEvent::RunReset);
            self.active_run = None;
        }

        let mut path = dirs::config_dir().unwrap();
        path.push("WFAutoSplitter/templates.json");

        if let Ok(meta) = std::fs::metadata(&path) {
            let modified = meta.modified().ok();
            if self.templates_modified == modified {
                return;
            }
            self.templates_modified = modified;
        }
        let templates = load_templates()
            .into_iter()
            .filter(|t| t.is_active)
            .map(RuntimeTemplate::new)
            .collect();
        self.templates = templates;
        println!("Template has been updated");
    }

    fn extract_netracell_icon(line: &str) -> Option<String> {
        const VALID: &[&str] = &["01","02","03","04","05","06","07","09"];
        let keyword = "BurdenHudIcon";
        if let Some(pos) = line.find(keyword) {
            let after = &line[pos + keyword.len()..];
            if after.len() >= 2 {
                let num = &after[..2];
                if VALID.contains(&num) {
                    return Some(num.to_string());
                }
            }
        }
        None
    }

    fn extract_time(line: &str) -> Option<f64> {
        let mut end = 0;
        for c in line.chars() {
            if c.is_ascii_digit() || c == '.' {
                end += 1;
            } else {
                break;
            }
        }
        if end == 0 {
            return None;
        }
        line[..end].parse::<f64>().ok()
    }

    // fn format_time(sec: f64) -> String {
    //     let total_ms = (sec * 1000.0) as u64;
    //     let ms = total_ms % 1000;
    //     let total_s = total_ms / 1000;
    //     let s = total_s % 60;
    //     let total_m = total_s / 60;
    //     let m = total_m % 60;
    //     let h = total_m / 60;

    //     let mut out = String::new();
    //     if h > 0 { out.push_str(&format!("{}h ", h)); }
    //     if m > 0 { out.push_str(&format!("{}m ", m)); }
    //     out.push_str(&format!("{}s ", s));
    //     out.push_str(&format!("{}ms", ms));
    //     out
    // }

    fn detect_player_login(&mut self, line: &str, app: &AppHandle) {
        let keyword = "Logged in ";
        if let Some(pos) = line.find(keyword) {
            let part = &line[pos + keyword.len()..];
            if let Some(end) = part.find('(') {
                let nickname = part[..end].trim().to_string();
                if self.player_nickname.as_ref() != Some(&nickname) {
                    self.player_nickname = Some(nickname.clone());
                    let _ = app.emit("player-nickname", &nickname);
                    // println!("Player nickname updated: {}", nickname);
                }
            }
        }
    }

    pub fn new() -> Self {
        let templates = load_templates()
            .into_iter()
            .filter(|t| t.is_active)
            .map(RuntimeTemplate::new)
            .collect();

        let mut parser = Self {
            templates,
            active_run: None,
            cancel_keyword: "TopMenu.lua: Abort".to_string(),
            exit_keyword: "Exiting main loop".to_string(),
            failed_keyword: "EndOfMatch.lua: Mission Failed".to_string(),
            templates_modified: None,
            player_nickname: None,
            event_sender: None,
            mission_aborts: 0,
            netracell_icons: Vec::new(),
            netracell_icon_count: 0,
        };

        parser.reload_templates();
        parser
    }

    pub fn process_line(&mut self, line: &str, app: &AppHandle) {
        self.detect_player_login(line, app);

        if line.contains(&self.cancel_keyword)
            || line.contains(&self.exit_keyword)
            || line.contains(&self.failed_keyword)
        {
            self.reset_active_run("RUN RESET");
            return;
        }

        if self.active_run.is_some() && self.netracell_icon_count < 4 {
            if let Some(num) = Self::extract_netracell_icon(line) {
                if !self.netracell_icons.contains(&num) {
                    self.netracell_icons.push(num);
                    self.netracell_icon_count += 1;
                    if self.netracell_icon_count == 4 {
                        self.send_event(LogEvent::NetracellIcons {
                            icons: self.netracell_icons.clone(),
                        });
                    }
                }
            }
        }
        if let Some(index) = self.active_run {
            if self.handle_mission_mismatch(index, line) {
                self.try_start_new_run(line);
                self.netracell_icon_count = 0;
                return;
            }

            if let Some(new_group_idx) = self.check_for_new_group(index, line) {
                let runtime = &mut self.templates[index];
                let group = &runtime.template.groups[new_group_idx];

                runtime.active_group = Some(new_group_idx);
                runtime.step_index[new_group_idx] = 0;

                if let Some(sender) = &self.event_sender {
                    let group_splits: Vec<SplitInfo> = group.steps.iter()
                        .map(|s| SplitInfo {
                            id: Uuid::new_v4().to_string(),
                            name: s.split_name.clone(),
                            order: s.order as u32,
                            step_type: format!("{:?}", s.step_type),
                        })
                        .collect();

                    let _ = sender.send(LogEvent::GroupAdded {
                        group_id: group.id.clone(),
                        group_splits,
                    });
                }

                // println!("[{}] Added new group: {}",
                //     self.templates[index].template.name,
                //     self.templates[index].template.groups[new_group_idx].steps[0].split_name);
            }

            let runtime = &mut self.templates[index];
            let event_sender = self.event_sender.as_ref();

            if Self::process_template(
                event_sender,
                runtime,
                line,
                self.player_nickname.as_deref().unwrap_or(""),
            ) {
                self.active_run = None;
            }

            return;
        }

        self.try_start_new_run(line);
    }

    fn try_start_new_run(&mut self, line: &str) {
        self.netracell_icons.clear();
        self.netracell_icon_count = 0;
        
        for i in 0..self.templates.len() {
            if let Some(group_index) = Self::try_start_group(&self.templates[i], line) {
                let runtime = &mut self.templates[i];
                self.active_run = Some(i);
                runtime.state = super::runtime::RunState::Running;
                runtime.active_group = Some(group_index);
                self.mission_aborts = 0;

                let trigger_only = Self::is_trigger_only_template(runtime);
                let name = runtime.template.name.clone();
                // println!("[{}] START RUN", name);

                if let Some(sender) = &self.event_sender {
                    let group = &runtime.template.groups[group_index];

                    let group_splits = if runtime.template.sequential_mode {
                        runtime.template.groups.iter()
                            .flat_map(|g| g.steps.iter())
                            .map(|s| SplitInfo {
                                id: Uuid::new_v4().to_string(),
                                name: s.split_name.clone(),
                                order: s.order as u32,
                                step_type: format!("{:?}", s.step_type),
                            })
                            .collect()
                    } else {
                        group.steps.iter()
                            .map(|s| SplitInfo {
                                id: Uuid::new_v4().to_string(),
                                name: s.split_name.clone(),
                                order: s.order as u32,
                                step_type: format!("{:?}", s.step_type),
                            })
                            .collect()
                    };

                    let _ = sender.send(LogEvent::RunStarted {
                        template_id: runtime.template.id.clone(),
                        template_name: name.clone(),
                        player_nickname: self.player_nickname.clone().unwrap_or_default(),
                        sequential_mode: runtime.template.sequential_mode,
                        exclude_time_between_groups: runtime.template.exclude_time_between_groups,
                        group_id: group.id.clone(),
                        group_splits,
                        run_start_time: if trigger_only { Self::extract_time(line) } else { None },
                    });
                }

                let runtime = &mut self.templates[i];
                if trigger_only {
                    runtime.step_index[group_index] = 1;
                    if let Some(t) = Self::extract_time(line) {
                        runtime.run_start_time = Some(t);
                        runtime.last_split_time = Some(t);
                    //     println!("[{}] {} - {:.3}",
                    //         runtime.template.name,
                    //         runtime.template.groups[group_index].steps[0].split_name,
                    //         t);
                    // } else {
                    //     println!("[{}] {}",
                    //         runtime.template.name,
                    //         runtime.template.groups[group_index].steps[0].split_name);
                    }
                } else {
                    runtime.step_index[group_index] = 0;
                }

                return;
            }
        }
    }

    fn check_for_new_group(&self, index: usize, line: &str) -> Option<usize> {
        let runtime = &self.templates[index];

        if runtime.template.sequential_mode {
            return None;
        }

        for (g_idx, group) in runtime.template.groups.iter().enumerate() {
            if runtime.finished_groups[g_idx] {
                continue;
            }
            if runtime.active_group == Some(g_idx) {
                continue;
            }

            let start_step = &group.steps[0];
            if let Some(code) = &start_step.mission_code {
                if !code.is_empty() && Self::matches_mission_code(line, code) {
                    return Some(g_idx);
                }
            }
        }

        None
    }

    pub fn reset_active_run(&mut self, reason: &str) {
        if let Some(index) = self.active_run {
            let runtime = &mut self.templates[index];
            // let name = runtime.template.name.clone();
            // println!("[{}] {}", name, reason);
            runtime.reset();
            self.active_run = None;
            self.mission_aborts += 1;
            self.send_event(LogEvent::RunReset);
        }
    }

    fn handle_mission_mismatch(&mut self, index: usize, line: &str) -> bool {
        if Self::is_trigger_only_template(&self.templates[index]) {
            return false;
        }

        let runtime = &mut self.templates[index];

        for (g_idx, g) in runtime.template.groups.iter().enumerate() {
            if !runtime.finished_groups[g_idx] {
                continue;
            }
            let start = &g.steps[0];
            if let Some(code) = &start.mission_code {
                if !code.is_empty() && Self::matches_mission_code(line, code) {
                    // let name = runtime.template.name.clone();
                    // println!("[{}] RUN RESET (restarted finished mission)", name);
                    runtime.reset();
                    self.active_run = None;
                    self.send_event(LogEvent::RunReset);
                    return true;
                }
            }
        }

        false
    }

    fn try_start_group(runtime: &RuntimeTemplate, line: &str) -> Option<usize> {
        let trigger_only = Self::is_trigger_only_template(runtime);

        for (group_index, group) in runtime.template.groups.iter().enumerate() {
            if runtime.finished_groups[group_index] {
                continue;
            }

            if !trigger_only && runtime.template.sequential_mode {
                let first_unfinished = runtime
                    .finished_groups
                    .iter()
                    .position(|x| !*x)
                    .unwrap_or(0);

                if group_index != first_unfinished {
                    continue;
                }
            }

            let step = &group.steps[0];

            if trigger_only {
                if Self::matches_trigger(line, &step.trigger_keyword) {
                    return Some(group_index);
                }
                continue;
            }

            if let Some(code) = &step.mission_code {
                if !code.is_empty() && Self::matches_mission_code(line, code) {
                    return Some(group_index);
                }
            }
        }

        None
    }

    fn process_template(
        event_sender: Option<&Sender<LogEvent>>,
        runtime: &mut RuntimeTemplate,
        line: &str,
        player_nickname: &str,
    ) -> bool {
        let time = Self::extract_time(line);
        // let template_name = runtime.template.name.clone();

        if runtime.active_group.is_none() {
            if runtime.template.sequential_mode {
                let first_unfinished = runtime.finished_groups.iter()
                    .position(|&finished| !finished);
                if let Some(idx) = first_unfinished {
                    runtime.active_group = Some(idx);
                    runtime.step_index[idx] = 0;
                }
            }

            if runtime.template.sequential_mode && runtime.active_group.is_some() {
            } else {
                for (group_index, group) in runtime.template.groups.iter().enumerate() {
                    if runtime.finished_groups[group_index] {
                        continue;
                    }

                    if runtime.template.sequential_mode {
                        let first_unfinished = runtime
                            .finished_groups
                            .iter()
                            .position(|x| !*x)
                            .unwrap_or(0);

                        if group_index != first_unfinished {
                            continue;
                        }
                    }

                    let step = &group.steps[0];

                    if !Self::matches_trigger(line, &step.trigger_keyword) {
                        continue;
                    }

                    runtime.active_group = Some(group_index);
                    runtime.step_index[group_index] = 0;

                    if runtime.run_start_time.is_none() {
                        if let Some(t) = time {
                            runtime.run_start_time = Some(t);
                        }
                    }

                    // if let Some(t) = time {
                    //     println!("[{}] {} - {:.3}", template_name, step.split_name, t);
                    // } else {
                    //     println!("[{}] {}", template_name, step.split_name);
                    // }

                    if let Some(sender) = event_sender {
                        let _ = sender.send(LogEvent::SplitCompleted {
                            group_id: group.id.clone(),
                            split_name: step.split_name.clone(),
                            split_time: time,
                            is_end_mission: false,
                        });
                    }

                    runtime.step_index[group_index] += 1;
                    return false;
                }

                return false;
            }
        }

        let group_index = runtime.active_group.unwrap();
        let group = &runtime.template.groups[group_index];
        let current_step = runtime.step_index[group_index];

        if current_step >= group.steps.len() {
            return false;
        }

        let step = &group.steps[current_step];

        if !Self::matches_trigger(line, &step.trigger_keyword) {
            return false;
        }

        if step.step_type == StepType::StartMission {
            if runtime.run_start_time.is_none() {
                if let Some(t) = time {
                    runtime.run_start_time = Some(t);
                }
            }

            if runtime.template.exclude_time_between_groups {
                let last_end = runtime.group_end_time.iter()
                    .filter_map(|&t| t)
                    .last();
                if let (Some(prev_end), Some(start)) = (last_end, time) {
                    runtime.excluded_time += start - prev_end;
                }
            }
        }

        if let Some(sender) = event_sender {
            let _ = sender.send(LogEvent::SplitCompleted {
                group_id: group.id.clone(),
                split_name: step.split_name.clone(),
                split_time: time,
                is_end_mission: step.step_type == StepType::EndMission,
            });
        }

        runtime.step_index[group_index] += 1;

        if let Some(t) = time {
            runtime.last_split_time = Some(t);
        //     println!("[{}] {} - {:.3}", template_name, step.split_name, t);
        // } else {
        //     println!("[{}] {}", template_name, step.split_name);
        }

        if step.step_type == StepType::EndMission {
            runtime.finished_groups[group_index] = true;
            runtime.active_group = None;

            if let Some(sender) = event_sender {
                let _ = sender.send(LogEvent::GroupCompleted {
                    group_id: group.id.clone(),
                });
            }

            if let Some(t) = time {
                runtime.group_end_time[group_index] = Some(t);
            }

            if runtime.all_groups_finished() {
                if let (Some(start), Some(end)) = (runtime.run_start_time, runtime.last_split_time) {
                    let mut total = end - start;

                    if runtime.excluded_time > 0.0 {
                        total -= runtime.excluded_time;
                    }
                    if total < 0.0 {
                        total = 0.0;
                    }
                    let total = (total * 1000.0).round() / 1000.0;
                    // let formatted = Self::format_time(total);

                    if let Some(sender) = event_sender {
                        let _ = sender.send(LogEvent::RunFinished {
                            total_time: total,
                            player_nickname: player_nickname.to_string(),
                        });
                    }

                //     println!("[{}] END RUN {:.3} ({})", template_name, total, formatted);
                // } else {
                //     println!("[{}] END RUN", template_name);
                }

                runtime.reset();
                return true;
            }
        }

        false
    }
}