use crate::types::{ActivityFunction, OrchestratorFunction};
use std::collections::HashMap;

#[derive(Default)]
pub struct Registry {
    orchestrators: HashMap<String, OrchestratorFunction>,
    activities: HashMap<String, ActivityFunction>,
}

impl Registry {
    pub fn add_orchestrator(&mut self, name: String, func: OrchestratorFunction) {
        self.orchestrators.insert(name, func);
    }
    pub fn add_activity(&mut self, name: String, func: ActivityFunction) {
        self.activities.insert(name, func);
    }
    pub fn get_orchestrator(&self, name: &str) -> Option<&OrchestratorFunction> {
        self.orchestrators.get(name)
    }
    pub fn get_activity(&self, name: &str) -> Option<&ActivityFunction> {
        self.activities.get(name)
    }
}
