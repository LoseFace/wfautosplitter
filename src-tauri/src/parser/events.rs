use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SplitInfo {
    pub id: String,
    pub name: String,
    pub order: u32,
    pub step_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupInfo {
    pub group_id: String,
    pub first_split_name: String,
}

#[derive(Debug, Clone)]
pub enum LogEvent {
    RunStarted {
        template_id: String,
        template_name: String,
        sequential_mode: bool,
        exclude_time_between_groups: bool,
        group_id: String,
        group_splits: Vec<SplitInfo>,
        run_start_time: Option<f64>,
        all_groups: Vec<GroupInfo>,
    },
    SplitCompleted {
        group_id: String,
        split_name: String,
        split_time: Option<f64>,
        is_end_mission: bool,
    },
    GroupCompleted {
        group_id: String,
    },
    GroupAdded {
        group_id: String,
        group_splits: Vec<SplitInfo>,
    },
    RunReset,
    RunFinished {
        total_time: f64,
    },
    TimerPause {
        frozen_time: f64,
    },
    TimerResume,
    DisruptionPrediction {
        predicted_time: Option<f64>,
        completed_rounds: u32,
    },
}