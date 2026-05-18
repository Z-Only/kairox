//! Agent strategy trait and built-in strategy implementations.
//!
//! The `AgentStrategy` trait defines how different agent roles (Planner, Worker,
//! Reviewer) interact with the DAG executor. Each strategy determines how to
//! build context, make decisions, and process tool results.

use agent_core::{AgentId, AgentRole, DomainEvent, TaskId};
use agent_models::{ModelMessage, ToolCall};

use async_trait::async_trait;

use crate::task_graph::{AgentTask, TaskGraph};

pub mod planner;
pub mod reviewer;
pub mod worker;

// Re-export the simple agents for backward compatibility.
pub use planner::PlannerAgent;
pub use reviewer::{ReviewerAgent, ReviewerFinding};
pub use worker::WorkerAgent;

/// Context provided to a strategy for each step of execution.
#[derive(Debug, Clone)]
pub struct StepContext {
    pub session_id: agent_core::SessionId,
    pub workspace_id: agent_core::WorkspaceId,
    pub user_message: String,
    pub source_agent_id: AgentId,
}

/// Outcome of a single agent step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// The agent loop should continue (tool calls need processing).
    Continue,
    /// The agent has completed its task successfully.
    Completed,
    /// The agent was cancelled.
    Cancelled,
    /// A permission request is pending (interactive mode).
    PermissionRequired,
    /// Maximum iterations reached.
    MaxIterations,
}

/// Decision returned by an agent after processing a model response.
#[derive(Debug, Clone)]
pub enum AgentDecision {
    /// Request a model call with the given tool definitions.
    RequestModel {
        tools: Vec<agent_models::ToolDefinition>,
    },
    /// The agent has a final text response (no tool calls needed).
    Respond(String),
    /// The planner has decomposed the goal into sub-tasks.
    Decompose { sub_tasks: Vec<SubTaskDef> },
    /// The reviewer has completed its review.
    ReviewComplete {
        approved: bool,
        findings: Vec<ReviewerFinding>,
    },
}

/// Action to take after receiving a tool result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResultAction {
    /// Continue the agent loop (feed the result to the model).
    Continue,
    /// Retry the tool call up to `max_retries` times.
    Retry { max_retries: usize },
    /// Abort the task with an error message.
    Abort(String),
}

/// Definition of a sub-task produced by the PlannerAgent during decomposition.
#[derive(Debug, Clone)]
pub struct SubTaskDef {
    pub title: String,
    pub role: AgentRole,
    pub dependencies: Vec<TaskId>,
    pub description: String,
}

/// The `AgentStrategy` trait defines how an agent role interacts with the DAG executor.
///
/// Each strategy is a stateless object that provides:
/// - Which role it serves (Planner, Worker, Reviewer)
/// - How to build the model context for a given task
/// - How to decide what to do with a model response
/// - How to process tool results
#[async_trait]
pub trait AgentStrategy: Send + Sync {
    /// The agent role this strategy implements.
    fn role(&self) -> AgentRole;

    /// Build the model messages for a given task, incorporating context from
    /// the task graph and session history.
    async fn build_context(
        &self,
        task: &AgentTask,
        graph: &TaskGraph,
        session_events: &[DomainEvent],
    ) -> Vec<ModelMessage>;

    /// Decide what to do given the current context and model messages.
    async fn decide(&self, ctx: &StepContext, messages: Vec<ModelMessage>) -> AgentDecision;

    /// Process the result of a tool call and determine the next action.
    async fn process_tool_result(
        &self,
        tool_call: &ToolCall,
        result: &str,
        iteration: usize,
    ) -> ToolResultAction;

    /// Optional model profile override from agent settings.
    fn model_profile_override(&self) -> Option<&str> {
        None
    }

    /// Optional permission mode override from agent settings.
    fn permission_mode_override(&self) -> Option<&str> {
        None
    }

    /// Skills configured for this agent.
    fn skills(&self) -> &[String] {
        &[]
    }

    /// Tool allowlist configured for this agent (empty = all allowed).
    fn tools_allowlist(&self) -> &[String] {
        &[]
    }
}
