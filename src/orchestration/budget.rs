use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunBudget {
    pub max_steps: u32,
    pub max_inference_calls: u32,
    pub max_actions: u32,
    pub max_wall_time_secs: u64,
}

impl Default for RunBudget {
    fn default() -> Self {
        Self {
            max_steps: 25,
            max_inference_calls: 30,
            max_actions: 50,
            max_wall_time_secs: 300,
        }
    }
}
