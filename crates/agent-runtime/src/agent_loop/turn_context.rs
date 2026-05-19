use crate::agent_loop::AgentLoopDeps;
use crate::context_budget;
use crate::event_emitter::append_and_broadcast;
use agent_core::{AgentId, DomainEvent, EventPayload, PrivacyClassification};
use agent_models::{ModelLimits, ToolDefinition};
use agent_store::EventStore;

/// All the context prepared for a single model turn.
pub(crate) struct TurnContext {
    pub(crate) model_profile_alias: String,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) budget: agent_memory::ContextBudget,
    pub(crate) system_prompt: String,
    pub(crate) tool_definitions: Vec<ToolDefinition>,
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

    let reasoning_effort = deps
        .config
        .profiles
        .iter()
        .find(|(alias, def)| alias == &model_profile_alias && def.enabled)
        .and_then(|(_, def)| {
            def.supports_reasoning.unwrap_or(false).then(|| {
                super::latest_model_reasoning_effort_for(session_events)
                    .unwrap_or_else(|| "low".into())
            })
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
    let mut system_prompt = super::SYSTEM_PROMPT.to_string();
    if let Some(ref instructions) = deps.config.instructions {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(instructions);
    }
    if let Some(section) =
        crate::memory_handler::retrieve_memory_section(deps.memory_store, &request.content).await
    {
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

    let active_skill_blocks = super::runner::load_active_skill_blocks(
        deps.skill_registry,
        deps.active_skills,
        &request.session_id,
    )
    .await?;

    let project_instructions = if let Some(ref root_path) = deps.root_path {
        let summary = crate::project::read_project_instruction_summary(root_path).await;
        summary.contents
    } else {
        None
    };

    let assembler = agent_memory::ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            agent_memory::ContextRequest {
                system_prompt: Some(system_prompt.clone()),
                project_instructions,
                active_skills: active_skill_blocks.clone(),
                user_request: request.content.clone(),
                session_history,
                tool_definitions: tool_definitions.clone(),
                ..Default::default()
            },
            budget.clone(),
        )
        .await;

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

    // Auto-compaction trigger (fire-and-forget).
    {
        let already_compacting = {
            let states = deps.session_states.lock().await;
            states
                .get(&request.session_id.to_string())
                .map(|s| s.compacting)
                .unwrap_or(false)
        };
        let threshold = deps.config.context.auto_compact_threshold;
        if super::should_trigger_auto_compaction(&usage, threshold, already_compacting) {
            let store_clone = deps.store.clone();
            let model_clone = deps.model.clone();
            let tx_clone = deps.event_tx.clone();
            let states_clone = deps.session_states.clone();
            let workspace_id = request.workspace_id.clone();
            let session_id = request.session_id.clone();
            let ratio = usage.ratio();
            let profile_alias = deps
                .config
                .context
                .compactor_profile
                .clone()
                .unwrap_or_else(|| model_profile_alias.clone());
            tokio::spawn(async move {
                let _ = crate::compaction::compact_session(
                    &*store_clone,
                    &tx_clone,
                    &*model_clone,
                    &profile_alias,
                    &states_clone,
                    workspace_id,
                    session_id,
                    agent_core::CompactionReason::Threshold { ratio },
                )
                .await;
            });
        }
    }

    Ok(TurnContext {
        model_profile_alias,
        reasoning_effort,
        budget,
        system_prompt,
        tool_definitions,
    })
}
