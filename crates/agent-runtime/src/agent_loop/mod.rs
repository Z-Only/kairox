mod budget;
mod messages;
mod runner;
mod tool_loop;

pub(crate) use budget::{build_model_messages_within_budget, should_trigger_auto_compaction};
pub(crate) use messages::build_model_messages;
pub(crate) use runner::run_agent_loop;
pub(crate) use runner::AgentLoopDeps;
pub(crate) use runner::{latest_model_profile_for, latest_model_reasoning_effort_for};
pub(crate) use tool_loop::execute_tool_calls;

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
