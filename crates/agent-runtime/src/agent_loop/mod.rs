mod budget;
mod messages;
mod runner;
mod stream_handler;
mod tool_loop;
mod turn_context;

pub(crate) use budget::{build_model_messages_within_budget, should_trigger_auto_compaction};
pub(crate) use messages::build_model_messages;
pub(crate) use runner::run_agent_loop;
pub(crate) use runner::{latest_model_profile_for, latest_model_reasoning_effort_for};
pub(crate) use stream_handler::process_model_stream;
pub(crate) use stream_handler::StreamOutput;
pub(crate) use tool_loop::execute_tool_calls;
pub(crate) use turn_context::prepare_turn_context;
pub(crate) use turn_context::server_tools_for_profile;
pub(crate) use turn_context::TurnContext;

use crate::task_graph::TaskGraph;
use agent_memory::MemoryStore;
use agent_models::ModelClient;
use agent_store::EventStore;
use agent_tools::{PermissionEngine, ToolRegistry};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Bundles every dependency `run_agent_loop` needs. Introduced to avoid a
/// 12-argument signature once `config` and `session_states` were added in
/// Task 8.
pub(crate) struct AgentLoopDeps<'a, S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    pub(crate) store: &'a Arc<S>,
    pub(crate) model: &'a Arc<M>,
    pub(crate) event_tx: &'a tokio::sync::broadcast::Sender<agent_core::DomainEvent>,
    pub(crate) tool_registry: &'a Arc<Mutex<ToolRegistry>>,
    pub(crate) permission_engine: &'a Arc<Mutex<PermissionEngine>>,
    pub(crate) pending_permissions: &'a crate::permission::PendingPermissionsMap,
    pub(crate) memory_store: &'a Option<Arc<dyn MemoryStore>>,
    pub(crate) task_graphs: &'a Arc<Mutex<HashMap<String, TaskGraph>>>,
    pub(crate) config: &'a Arc<agent_config::Config>,
    pub(crate) session_states: &'a Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
    pub(crate) skill_registry: &'a Option<Arc<dyn agent_skills::SkillRegistry>>,
    pub(crate) active_skills: &'a Arc<Mutex<HashMap<String, Vec<String>>>>,
    pub(crate) turn_cancellation: CancellationToken,
    pub(crate) root_path: Option<std::path::PathBuf>,
}

pub const SYSTEM_PROMPT: &str = "\
You are Kairox, a helpful AI assistant with memory capabilities.\n\n\
## Memory Protocol\n\
When you learn something worth remembering about the user or workspace, \
use <memory> tags to save it. Examples:\n\
- <memory scope=\"session\">Temporary note for this session</memory>\n\
- <memory scope=\"user\" key=\"preferred-language\">User prefers Rust</memory>\n\
- <memory scope=\"workspace\" key=\"build-cmd\">Use cargo nextest</memory>\n\n\
Guidelines:\n\
- Use scope=\"session\" for temporary notes (auto-accepted)\n\
- Use scope=\"user\" for user preferences (requires approval)\n\
- Use scope=\"workspace\" for project settings (requires approval)\n\
- Always include a key when using user or workspace scope\n\
- You may include multiple <memory> tags in one response\n\
- The <memory> tags will be stripped from displayed output, so also state \
the information naturally in your response.\n\
";

pub const MAX_AGENT_LOOP_ITERATIONS: usize = 20;
