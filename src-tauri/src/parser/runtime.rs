use super::models::Template;

pub enum RunState {
    Idle,
    Running,
}

pub struct RuntimeTemplate {
    pub template: Template,
    pub state: RunState,
    pub finished_groups: Vec<bool>,
    pub active_group: Option<usize>,
    pub step_index: Vec<usize>,

    pub run_start_time: Option<f64>,
    pub last_split_time: Option<f64>,

    pub group_end_time: Vec<Option<f64>>,
    pub excluded_time: f64,
    pub first_group_shown: bool
}

impl RuntimeTemplate {
    pub fn new(template: Template) -> Self {
        let groups = template.groups.len();
        Self {
            template,
            state: RunState::Idle,
            finished_groups: vec![false; groups],
            active_group: None,
            step_index: vec![0; groups],

            run_start_time: None,
            last_split_time: None,

            group_end_time: vec![None; groups],
            excluded_time: 0.0,
            first_group_shown: false
        }
    }

    pub fn reset(&mut self) {
        self.state = RunState::Idle;
        self.run_start_time = None;
        self.active_group = None;
        self.last_split_time = None;
        self.excluded_time = 0.0;
        self.first_group_shown = false;

        for v in &mut self.group_end_time {
            *v = None;
        }
        for v in &mut self.finished_groups {
            *v = false;
        }
        for v in &mut self.step_index {
            *v = 0;
        }
    }

    pub fn all_groups_finished(&self) -> bool {
        self.finished_groups.iter().all(|x| *x)
    }
}

