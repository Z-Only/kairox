use std::collections::HashMap;
use std::sync::Arc;

use agent_core::AgentRole;

use crate::agent_settings::effective_agent_by_name;
use crate::agents::planner::PlannerStrategy;
use crate::agents::reviewer::ReviewerStrategy;
use crate::agents::worker::WorkerStrategy;
use crate::agents::AgentStrategy;

const PLANNER_AGENT_NAME: &str = "default";
const WORKER_AGENT_NAME: &str = "worker";
const REVIEWER_AGENT_NAME: &str = "code-reviewer";

pub(crate) fn strategies_from_agent_settings(
    agent_views: &[agent_core::facade::AgentSettingsView],
) -> HashMap<AgentRole, Arc<dyn AgentStrategy>> {
    let mut strategies: HashMap<AgentRole, Arc<dyn AgentStrategy>> = HashMap::new();

    strategies.insert(
        AgentRole::Planner,
        match effective_agent_by_name(agent_views, PLANNER_AGENT_NAME) {
            Some(view) => Arc::new(PlannerStrategy::from_agent_view(view)),
            None => Arc::new(PlannerStrategy::new()),
        },
    );

    strategies.insert(
        AgentRole::Worker,
        match effective_agent_by_name(agent_views, WORKER_AGENT_NAME) {
            Some(view) => Arc::new(WorkerStrategy::from_agent_view(view)),
            None => Arc::new(WorkerStrategy::new()),
        },
    );

    strategies.insert(
        AgentRole::Reviewer,
        match effective_agent_by_name(agent_views, REVIEWER_AGENT_NAME) {
            Some(view) => Arc::new(ReviewerStrategy::from_agent_view(view)),
            None => Arc::new(ReviewerStrategy::new()),
        },
    );

    strategies
}
