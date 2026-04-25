use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub sequential_mode: bool,
    pub exclude_time_between_groups: bool,
    pub groups: Vec<Group>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Group {
    pub id: String,
    pub steps: Vec<Step>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum Step {
    StartMission {
        order: u32,
        trigger_keyword: String,
        split_name: String,
        mission_code: Option<String>,
    },
    MissionSplit {
        order: u32,
        trigger_keyword: String,
        split_name: String,
    },
    EndMission {
        order: u32,
        trigger_keyword: String,
        split_name: String,
    },
}
