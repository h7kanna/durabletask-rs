use crate::types::{ActivityFunction, OrchestratorFunction};
use dashmap::mapref::one::Ref;
use dashmap::DashMap;

#[derive(Default)]
pub struct Registry {
    orchestrators: DashMap<String, OrchestratorFunction>,
    activities: DashMap<String, ActivityFunction>,
}

// TODO: Remove Dashmap dependency?
impl Registry {
    pub fn add_orchestrator(&mut self, name: String, func: OrchestratorFunction) {
        self.orchestrators.insert(name, func);
    }
    pub fn add_activity(&mut self, name: String, func: ActivityFunction) {
        self.activities.insert(name, func);
    }
    pub fn get_orchestrator(&self, name: &str) -> Option<Ref<String, OrchestratorFunction>> {
        self.orchestrators.get(name)
    }
    pub fn get_activity(&self, name: &str) -> Option<Ref<String, ActivityFunction>> {
        self.activities.get(name)
    }
}
