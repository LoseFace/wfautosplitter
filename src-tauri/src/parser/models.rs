use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub sequential_mode: bool,
    pub exclude_time_between_groups: bool,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    #[serde(rename = "type")]
    pub step_type: StepType,

    pub order: usize,
    pub trigger_keyword: String,
    pub split_name: String,

    #[serde(default)]
    pub mission_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepType {
    StartMission,
    MissionSplit,
    EndMission,
}
