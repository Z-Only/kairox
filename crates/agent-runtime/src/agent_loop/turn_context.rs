use crate::agent_loop::AgentLoopDeps;
use crate::context_budget;
use crate::event_emitter::append_and_broadcast;
use agent_core::{AgentId, DomainEvent, EventPayload, PrivacyClassification};
use agent_memory::{WorkspaceDocument, WorkspaceIndexOptions};
use agent_models::types::ServerTool;
use agent_models::{ModelLimits, ToolDefinition};
use agent_store::EventStore;
use std::path::Path;
use std::sync::Arc;

/// All the context prepared for a single model turn.
pub(crate) struct TurnContext {
    pub(crate) model_profile_alias: String,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) budget: agent_memory::ContextBudget,
    pub(crate) system_prompt: String,
    pub(crate) tool_definitions: Vec<ToolDefinition>,
    pub(crate) server_tools: Vec<ServerTool>,
}

pub(crate) fn server_tools_for_profile(
    config: &agent_config::Config,
    model_profile_alias: &str,
) -> Vec<ServerTool> {
    config
        .profiles
        .iter()
        .find(|(alias, def)| alias == model_profile_alias && def.enabled)
        .map(|(_, def)| {
            agent_models::types::server_tools_from_profile(
                def.server_tool_code_execution.unwrap_or(false),
                def.server_tool_web_search.unwrap_or(false),
            )
        })
        .unwrap_or_default()
}

/// Prepare everything the model request needs for this turn:
/// profile, limits, budget, system prompt, tool definitions, session
/// history, active skill blocks, project instructions, context assembly,
/// usage correction, ContextAssembled event, and auto-compaction.
pub(crate) async fn prepare_turn_context<S, M>(
    deps: &AgentLoopDeps<'_, S, M>,
    request: &agent_core::SendMessageRequest,
    session_events: &[DomainEvent],
) -> agent_core::Result<TurnContext>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    // Resolve model profile alias from session events.
    let model_profile_alias: String = super::latest_model_profile_for(session_events);
    let server_tools = server_tools_for_profile(deps.config, &model_profile_alias);

    let reasoning_effort = deps
        .config
        .profiles
        .iter()
        .find(|(alias, def)| alias == &model_profile_alias && def.enabled)
        .and_then(|(_, def)| {
            agent_config::profile_supports_reasoning(def)
                .then(|| super::latest_model_reasoning_effort_for(session_events))
                .flatten()
        });

    // Resolve ModelLimits.
    let limits = {
        let states = deps.session_states.lock().await;
        states
            .get(&request.session_id.to_string())
            .and_then(|s| s.model_limits.clone())
    }
    .unwrap_or_else(|| {
        let profile_def = deps
            .config
            .profiles
            .iter()
            .find(|(alias, _)| alias == &model_profile_alias)
            .map(|(_, def)| def);
        match profile_def {
            Some(def) => agent_config::resolve_limits(def),
            None => {
                let from_event = session_events.iter().rev().find_map(|e| {
                    if let agent_core::EventPayload::ModelProfileSwitched {
                        context_window,
                        output_limit,
                        limit_source,
                        ..
                    } = &e.payload
                    {
                        Some(ModelLimits {
                            context_window: *context_window,
                            output_limit: *output_limit,
                            source: match limit_source.as_str() {
                                "user_config" => agent_models::LimitSource::UserConfig,
                                "builtin_registry" => agent_models::LimitSource::BuiltinRegistry,
                                "runtime_probe" => agent_models::LimitSource::RuntimeProbe,
                                _ => agent_models::LimitSource::Fallback,
                            },
                        })
                    } else {
                        None
                    }
                });
                from_event.unwrap_or_else(|| agent_models::lookup_limits("fake", "fake"))
            }
        }
    });

    let budget = context_budget::build_budget(&limits);

    // Tool definitions.
    let tool_definitions: Vec<ToolDefinition> = {
        let registry = deps.tool_registry.lock().await;
        registry
            .list_all()
            .await
            .into_iter()
            .map(|td| ToolDefinition {
                name: td.tool_id,
                description: td.description,
                parameters: td.parameters,
            })
            .collect()
    };

    // System prompt with instructions + memory.
    let mut base_system_prompt = super::SYSTEM_PROMPT.to_string();
    if let Some(ref instructions) = deps.config.instructions {
        base_system_prompt.push_str("\n\n");
        base_system_prompt.push_str(instructions);
    }
    let git_branch = deps
        .root_path
        .as_deref()
        .and_then(crate::project::current_git_branch);
    let relevant_memories = crate::memory_handler::retrieve_relevant_memories_for_context(
        deps.memory_store,
        &request.content,
        Some(request.session_id.as_str().to_string()),
        Some(request.workspace_id.as_str().to_string()),
        git_branch.clone(),
    )
    .await;
    let mut system_prompt = base_system_prompt.clone();
    if let Some(section) = crate::memory_handler::render_memory_section(&relevant_memories) {
        system_prompt.push_str(&section);
    }

    // Session history strings for the assembler.
    let session_history: Vec<String> = session_events
        .iter()
        .filter_map(|e| match &e.payload {
            EventPayload::UserMessageAdded { content, .. } => Some(format!("user: {content}")),
            EventPayload::AssistantMessageCompleted { content, .. } => {
                Some(format!("assistant: {content}"))
            }
            EventPayload::ToolInvocationCompleted {
                tool_id,
                output_preview,
                ..
            } => Some(format!("tool[{tool_id}]: {output_preview}")),
            _ => None,
        })
        .collect();
    let git_context = if let Some(root_path) = deps.root_path.as_deref() {
        let mut conversation_context = session_history.clone();
        conversation_context.push(format!("user: {}", request.content));
        crate::project::build_git_context(root_path, &conversation_context)
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };

    let active_skill_blocks = super::runner::load_active_skill_blocks(
        deps.skill_registry,
        deps.active_skills,
        &request.session_id,
        session_events,
    )
    .await?;

    let project_instructions = if let Some(ref root_path) = deps.root_path {
        let summary = crate::project::read_project_instruction_summary(root_path).await;
        summary.contents
    } else {
        None
    };
    let project_instruction_block = project_instructions.as_ref().map(|instructions| {
        format!("<project-instructions>\n{instructions}\n</project-instructions>")
    });

    if let Some(index) = deps.workspace_rag_index.as_ref() {
        hydrate_workspace_rag_index(index, request, session_events, deps.root_path.as_deref())
            .await;
    }

    let mut assembler = agent_memory::ContextAssembler::new_standalone();
    if let Some(index) = deps.workspace_rag_index.as_ref() {
        assembler = assembler.with_workspace_retriever(index.clone());
    }
    let bundle = assembler
        .assemble(
            agent_memory::ContextRequest {
                system_prompt: Some(base_system_prompt.clone()),
                project_instructions: project_instructions.clone(),
                memories: relevant_memories.clone(),
                active_skills: active_skill_blocks.clone(),
                user_request: request.content.clone(),
                session_history,
                session_id: Some(request.session_id.as_str().to_string()),
                workspace_id: Some(request.workspace_id.as_str().to_string()),
                branch: git_branch.clone(),
                git_context,
                tool_definitions: tool_definitions.clone(),
                // Keep the 5 most recent images and drop older ones so that
                // multi-turn screenshot conversations (computer.use, browser)
                // don't accumulate unbounded image tokens.
                image_pruning: agent_memory::ImagePruningStrategy::StripOldestImages { keep: 5 },
                ..Default::default()
            },
            budget.clone(),
        )
        .await;

    if let Some(block) = project_instruction_block {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&block);
    }

    if !active_skill_blocks.is_empty() {
        system_prompt.push_str("\n\n<active_skills>\n");
        system_prompt.push_str(&active_skill_blocks.join("\n"));
        system_prompt.push_str("\n</active_skills>");
    }

    // Apply per-session UsageCorrector.
    let mut usage = bundle.usage.clone();
    {
        let mut states = deps.session_states.lock().await;
        let entry = states
            .entry(request.session_id.to_string())
            .or_insert_with(crate::session::SessionState::default);
        if entry.usage_corrector.samples > 0 {
            usage.total_tokens = entry.usage_corrector.apply(usage.total_tokens);
            for (_, n) in &mut usage.by_source {
                *n = entry.usage_corrector.apply(*n);
            }
            usage.corrected_by_real_usage = true;
        }
        entry.last_estimated_tokens = usage.total_tokens;
    }

    // Emit ContextAssembled event.
    let assembled_event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextAssembled {
            usage: usage.clone(),
        },
    );
    append_and_broadcast(&**deps.store, deps.event_tx, &assembled_event).await?;

    // Auto-compaction is now scheduled at TURN END by
    // `LocalRuntimeTurnExecutor::maybe_schedule_auto_compaction`, routed
    // through `SessionExecutionRuntime::run_operation` so the actor
    // serializes it behind any in-flight turn. Triggering at turn-start
    // raced with the turn's own event writes.

    Ok(TurnContext {
        model_profile_alias,
        reasoning_effort,
        budget,
        system_prompt,
        tool_definitions,
        server_tools,
    })
}

async fn hydrate_workspace_rag_index(
    index: &Arc<agent_memory::WorkspaceRagIndex>,
    request: &agent_core::SendMessageRequest,
    session_events: &[DomainEvent],
    root_path: Option<&Path>,
) {
    if let Some(root_path) = root_path {
        if let Err(error) = index
            .index_workspace_files(
                request.workspace_id.as_str(),
                root_path,
                WorkspaceIndexOptions::default(),
            )
            .await
        {
            tracing::warn!(
                error = %error,
                workspace_id = %request.workspace_id.as_str(),
                root_path = %root_path.display(),
                "workspace RAG file indexing failed"
            );
        }
    }

    if let Some(transcript) = render_past_conversation_document(session_events, &request.content) {
        if let Err(error) = index
            .index_document(WorkspaceDocument::past_conversation(
                request.workspace_id.as_str(),
                request.session_id.as_str(),
                transcript,
            ))
            .await
        {
            tracing::warn!(
                error = %error,
                workspace_id = %request.workspace_id.as_str(),
                session_id = %request.session_id.as_str(),
                "workspace RAG conversation indexing failed"
            );
        }
    }
}

fn render_past_conversation_document(
    session_events: &[DomainEvent],
    current_user_content: &str,
) -> Option<String> {
    let mut lines: Vec<String> = session_events
        .iter()
        .filter_map(|event| match &event.payload {
            EventPayload::UserMessageAdded { content, .. } => Some(format!("user: {content}")),
            EventPayload::AssistantMessageCompleted { content, .. } => {
                Some(format!("assistant: {content}"))
            }
            EventPayload::ToolInvocationCompleted {
                tool_id,
                output_preview,
                ..
            } => Some(format!("tool[{tool_id}]: {output_preview}")),
            _ => None,
        })
        .collect();

    let current_user_line = format!("user: {current_user_content}");
    if lines.last() == Some(&current_user_line) {
        lines.pop();
    }

    (lines.len() >= 2).then(|| lines.join("\n"))
}

#[cfg(test)]
#[path = "turn_context_tests.rs"]
mod tests;
