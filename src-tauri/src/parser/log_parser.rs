use super::loader::load_templates;
use super::models::StepType;
use super::runtime::RuntimeTemplate;
use tauri::AppHandle;
use super::events::{GroupInfo, LogEvent, SplitInfo};
use crossbeam_channel::Sender;
use uuid::Uuid;
use std::collections::HashSet;

const DISRUPTION_MISSION_START: &str = "OnStateStarted";
const DISRUPTION_ROUND_START:   &str = "SentientArtifactMission.lua: Disruption: Intro door was unlocked";
const DISRUPTION_ROUND_DONE:    &str = "Disruption: State change: ARTIFACT_ROUND_DONE";
const DISRUPTION_TOTAL_ROUNDS:  u32  = 45;

struct DisruptionTracker {
    mission_start_log_time: Option<f64>,
    first_round_log_time: Option<f64>,
    pre_round_offset: Option<f64>,
    current_round_start_log_time: Option<f64>,
    round_durations_sum: f64,
    completed_rounds: u32,
    active: bool,
}

impl DisruptionTracker {
    fn new() -> Self {
        Self {
            mission_start_log_time: None,
            first_round_log_time: None,
            pre_round_offset: None,
            current_round_start_log_time: None,
            round_durations_sum: 0.0,
            completed_rounds: 0,
            active: false,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn predicted_time(&self) -> Option<f64> {
        if self.completed_rounds == 0 {
            return None;
        }
        let offset = self.pre_round_offset?;
        let avg_total = self.round_durations_sum * DISRUPTION_TOTAL_ROUNDS as f64
            / self.completed_rounds as f64;
        Some(offset + avg_total)
    }
}

pub struct LogParser {
    templates: Vec<RuntimeTemplate>,
    active_run: Option<usize>,
    cancel_keyword: String,
    exit_keyword: String,
    failed_keyword: String,
    templates_modified: Option<std::time::SystemTime>,
    event_sender: Option<Sender<LogEvent>>,
    mission_aborts: i64,
    disruption: DisruptionTracker,
}

impl LogParser {
    pub fn with_event_sender(mut self, sender: Sender<LogEvent>) -> Self {
        self.event_sender = Some(sender);
        self
    }

    fn send_event(&self, event: LogEvent) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event);
        }
    }

    fn matches_keyword(line: &str, keyword: &str) -> bool {
        if keyword.contains("||") {
            keyword.split("||").any(|kw| line.contains(kw.trim()))
        } else {
            line.contains(keyword)
        }
    }

    fn matches_trigger(line: &str, keyword: &str) -> bool {
        Self::matches_keyword(line, keyword)
    }

    fn matches_mission_code(line: &str, code: &str) -> bool {
        if code.contains("||") {
            code.split("||").any(|kw| Self::mission_code_word_match(line, kw.trim()))
        } else {
            Self::mission_code_word_match(line, code)
        }
    }

    fn mission_code_word_match(line: &str, code: &str) -> bool {
        let Some(pos) = line.find(code) else { return false };
        let after = &line[pos + code.len()..];
        !after.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
    }

    fn matches_trigger_seq(
        line: &str,
        keyword: &str,
        group_idx: usize,
        step_idx: usize,
        pending: &mut HashSet<(usize, usize)>,
    ) -> bool {
        let key = (group_idx, step_idx);
        if pending.contains(&key) {
            let second = keyword.splitn(2, "=>").nth(1).unwrap_or("").trim();
            if Self::matches_keyword(line, second) {
                pending.remove(&key);
                return true;
            }
        } else {
            let first = keyword.splitn(2, "=>").next().unwrap_or("").trim();
            if Self::matches_keyword(line, first) {
                pending.insert(key);
            }
        }
        false
    }

    fn check_trigger(
        line: &str,
        keyword: &str,
        group_idx: usize,
        step_idx: usize,
        pending: &mut HashSet<(usize, usize)>,
    ) -> bool {
        if keyword.contains("=>") {
            Self::matches_trigger_seq(line, keyword, group_idx, step_idx, pending)
        } else {
            Self::matches_trigger(line, keyword)
        }
    }

    fn check_mission_code(
        line: &str,
        code: &str,
        group_idx: usize,
        step_idx: usize,
        pending: &mut HashSet<(usize, usize)>,
    ) -> bool {
        if code.contains("=>") {
            Self::matches_trigger_seq(line, code, group_idx, step_idx, pending)
        } else {
            Self::matches_mission_code(line, code)
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
            event_sender: None,
            mission_aborts: 0,
            disruption: DisruptionTracker::new(),
        };

        parser.reload_templates();
        parser
    }

    fn process_disruption_line(&mut self, line: &str) -> Option<LogEvent> {
        let time = Self::extract_time(line);

        if line.contains(DISRUPTION_MISSION_START) {
            if let Some(t) = time {
                self.disruption.reset();
                self.disruption.mission_start_log_time = Some(t);
                self.disruption.active = true;
            }
            return None;
        }

        if !self.disruption.active {
            return None;
        }

        if line.contains(DISRUPTION_ROUND_START) && self.disruption.first_round_log_time.is_none() {
            if let (Some(t), Some(mission_t)) = (time, self.disruption.mission_start_log_time) {
                self.disruption.first_round_log_time = Some(t);
                self.disruption.current_round_start_log_time = Some(t);
                let offset = (t - mission_t).max(0.0);
                self.disruption.pre_round_offset = Some(offset);
            }
            return None;
        }

        if line.contains(DISRUPTION_ROUND_DONE) {
            if let (Some(t), Some(round_start)) = (time, self.disruption.current_round_start_log_time) {
                let duration = (t - round_start).max(0.0);
                self.disruption.round_durations_sum += duration;
                self.disruption.completed_rounds += 1;
                self.disruption.current_round_start_log_time = Some(t);

                if self.disruption.completed_rounds >= DISRUPTION_TOTAL_ROUNDS {
                    self.disruption.active = false;
                    return Some(LogEvent::DisruptionPrediction {
                        predicted_time: None,
                        completed_rounds: self.disruption.completed_rounds,
                    });
                }

                if let Some(predicted) = self.disruption.predicted_time() {
                    return Some(LogEvent::DisruptionPrediction {
                        predicted_time: Some((predicted * 1000.0).round() / 1000.0),
                        completed_rounds: self.disruption.completed_rounds,
                    });
                }
            }
        }

        None
    }


    pub fn process_line(&mut self, line: &str, _app: &AppHandle) {
        if let Some(event) = self.process_disruption_line(line) {
            self.send_event(event);
        }

        if line.contains(&self.cancel_keyword)
            || line.contains(&self.exit_keyword)
            || line.contains(&self.failed_keyword)
        {
            self.disruption.reset();
            self.send_event(LogEvent::DisruptionPrediction {
                predicted_time: None,
                completed_rounds: 0,
            });
            self.reset_active_run("RUN RESET");
            return;
        }

        if let Some(index) = self.active_run {
            if self.handle_mission_mismatch(index, line) {
                self.try_start_new_run(line);
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
                            group_index: new_group_idx as u32,
                        })
                        .collect();

                    let _ = sender.send(LogEvent::GroupAdded {
                        group_id: group.id.clone(),
                        group_splits,
                    });
                }
            }

            let runtime = &mut self.templates[index];
            let event_sender = self.event_sender.as_ref();

            if Self::process_template(event_sender, runtime, line) {
                self.active_run = None;
            }

            return;
        }

        self.try_start_new_run(line);
    }

    fn try_start_new_run(&mut self, line: &str) {
        for i in 0..self.templates.len() {
            if let Some(group_index) = Self::try_start_group(&mut self.templates[i], line) {
                let runtime = &mut self.templates[i];
                self.active_run = Some(i);
                runtime.state = super::runtime::RunState::Running;
                runtime.active_group = Some(group_index);
                self.mission_aborts = 0;

                let trigger_only = Self::is_trigger_only_template(runtime);
                let name = runtime.template.name.clone();

                if let Some(sender) = &self.event_sender {
                    let group = &runtime.template.groups[group_index];

                    let group_splits = if runtime.template.sequential_mode {
                        runtime.template.groups.iter()
                            .enumerate()
                            .flat_map(|(g_idx, g)| g.steps.iter().map(move |s| (g_idx, s)))
                            .map(|(g_idx, s)| SplitInfo {
                                id: Uuid::new_v4().to_string(),
                                name: s.split_name.clone(),
                                order: s.order as u32,
                                step_type: format!("{:?}", s.step_type),
                                group_index: g_idx as u32,
                            })
                            .collect()
                    } else {
                        group.steps.iter()
                            .map(|s| SplitInfo {
                                id: Uuid::new_v4().to_string(),
                                name: s.split_name.clone(),
                                order: s.order as u32,
                                step_type: format!("{:?}", s.step_type),
                                group_index: 0,
                            })
                            .collect()
                    };

                    let run_start_time = if trigger_only {
                        Self::extract_time(line)
                    } else {
                        None
                    };

                    let all_groups: Vec<GroupInfo> = runtime.template.groups.iter()
                        .map(|g| GroupInfo {
                            group_id: g.id.clone(),
                            first_split_name: g.steps.first()
                                .map(|s| s.split_name.clone())
                                .unwrap_or_default(),
                        })
                        .collect();

                    let _ = sender.send(LogEvent::RunStarted {
                        template_id: runtime.template.id.clone(),
                        template_name: name.clone(),
                        sequential_mode: runtime.template.sequential_mode,
                        exclude_time_between_groups: runtime.template.exclude_time_between_groups,
                        group_id: group.id.clone(),
                        group_splits,
                        run_start_time,
                        all_groups,
                    });
                }
                return;
            }
        }
    }

    fn check_for_new_group(&mut self, index: usize, line: &str) -> Option<usize> {
        let runtime = &mut self.templates[index];

        if Self::is_trigger_only_template(runtime) {
            return None;
        }

        for (g_idx, g) in runtime.template.groups.iter().enumerate() {
            if runtime.finished_groups[g_idx] || Some(g_idx) == runtime.active_group {
                continue;
            }

            let start = &g.steps[0];
            if let Some(code) = &start.mission_code {
                if !code.is_empty() && Self::check_mission_code(line, code, g_idx, 0, &mut runtime.pending_mission) {
                    return Some(g_idx);
                }
            }
        }

        None
    }

    pub fn reset_active_run(&mut self, _reason: &str) {
        if let Some(index) = self.active_run {
            let runtime = &mut self.templates[index];
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
                if !code.is_empty() && Self::check_mission_code(line, code, g_idx, 0, &mut runtime.pending_mission) {
                    runtime.reset();
                    self.active_run = None;
                    self.send_event(LogEvent::RunReset);
                    return true;
                }
            }
        }

        false
    }

    fn try_start_group(runtime: &mut RuntimeTemplate, line: &str) -> Option<usize> {
        let trigger_only = Self::is_trigger_only_template(runtime);

        for group_index in 0..runtime.template.groups.len() {
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

            let step = &runtime.template.groups[group_index].steps[0];

            if trigger_only {
                if Self::check_trigger(
                    line,
                    &step.trigger_keyword.clone(),
                    group_index,
                    0,
                    &mut runtime.pending_sequence,
                ) {
                    return Some(group_index);
                }
                continue;
            }

            if let Some(code) = &step.mission_code.clone() {
                if !code.is_empty() && Self::check_mission_code(line, code, group_index, 0, &mut runtime.pending_mission) {
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
    ) -> bool {
        let time = Self::extract_time(line);

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
                let trigger_only = Self::is_trigger_only_template(runtime);

                for group_index in 0..runtime.template.groups.len() {
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

                    if !trigger_only {
                        let mission_code = runtime.template.groups[group_index].steps[0]
                            .mission_code
                            .clone();
                        match mission_code {
                            Some(ref code) if !code.is_empty() => {
                                if !Self::check_mission_code(
                                    line,
                                    code,
                                    group_index,
                                    0,
                                    &mut runtime.pending_mission,
                                ) {
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }

                    let keyword = runtime.template.groups[group_index].steps[0].trigger_keyword.clone();
                    if !Self::check_trigger(line, &keyword, group_index, 0, &mut runtime.pending_sequence) {
                        continue;
                    }

                    runtime.active_group = Some(group_index);
                    runtime.step_index[group_index] = 0;

                    if runtime.run_start_time.is_none() {
                        if let Some(t) = time {
                            runtime.run_start_time = Some(t);
                        }
                    }

                    if let Some(sender) = event_sender {
                        let group = &runtime.template.groups[group_index];
                        let step = &group.steps[0];
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
        let current_step = runtime.step_index[group_index];

        if current_step >= runtime.template.groups[group_index].steps.len() {
            return false;
        }

        let keyword = runtime.template.groups[group_index].steps[current_step].trigger_keyword.clone();

        if !Self::check_trigger(line, &keyword, group_index, current_step, &mut runtime.pending_sequence) {
            return false;
        }

        let step_type = runtime.template.groups[group_index].steps[current_step].step_type.clone();
        let split_name = runtime.template.groups[group_index].steps[current_step].split_name.clone();
        let group_id = runtime.template.groups[group_index].id.clone();

        if step_type == StepType::StartMission {
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
                group_id: group_id.clone(),
                split_name: split_name.clone(),
                split_time: time,
                is_end_mission: step_type == StepType::EndMission,
            });
        }

        runtime.step_index[group_index] += 1;

        if let Some(t) = time {
            runtime.last_split_time = Some(t);
        }

        if step_type == StepType::EndMission {
            runtime.finished_groups[group_index] = true;
            runtime.active_group = None;

            if let Some(sender) = event_sender {
                let _ = sender.send(LogEvent::GroupCompleted {
                    group_id: group_id.clone(),
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

                    if let Some(sender) = event_sender {
                        let _ = sender.send(LogEvent::RunFinished {
                            total_time: total,
                        });
                    }
                }

                runtime.reset();
                return true;
            }
        }

        false
    }
}