use super::*;
use agent_core::FailurePolicy;

#[test]
fn dag_config_default_values() {
    let config = DagConfig::default();
    assert_eq!(config.max_concurrency, 3);
    assert_eq!(config.failure_policy, FailurePolicy::BlockDependents);
}
