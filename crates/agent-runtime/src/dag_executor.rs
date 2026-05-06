//! DAG-driven task executor — Phase 2 implementation.
//!
//! This module will implement the DAG executor that:
//! 1. Uses PlannerAgent to decompose user goals into sub-task DAGs
//! 2. Schedules ready tasks via `tokio::JoinSet` with a concurrency semaphore
//! 3. Assigns AgentStrategy to each task based on its role
//! 4. Handles failure cascade (BlockDependents) and retry/skip recovery
//!
//! Currently a stub — to be implemented in Phase 2.

use agent_models::ModelClient;
use agent_store::EventStore;

/// Configuration for the DAG executor.
#[derive(Debug, Clone)]
pub struct DagConfig {
    /// Maximum number of tasks that can execute concurrently.
    pub max_concurrency: usize,
}

impl Default for DagConfig {
    fn default() -> Self {
        Self { max_concurrency: 3 }
    }
}

/// Placeholder for the DAG executor.
/// Will be implemented in Phase 2.
pub struct DagExecutor<S, M>
where
    S: EventStore,
    M: ModelClient,
{
    _store: std::marker::PhantomData<S>,
    _model: std::marker::PhantomData<M>,
    config: DagConfig,
}

impl<S, M> DagExecutor<S, M>
where
    S: EventStore,
    M: ModelClient,
{
    pub fn new(config: DagConfig) -> Self {
        Self {
            _store: std::marker::PhantomData,
            _model: std::marker::PhantomData,
            config,
        }
    }

    pub fn config(&self) -> &DagConfig {
        &self.config
    }
}
