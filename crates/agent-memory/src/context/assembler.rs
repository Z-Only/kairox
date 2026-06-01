use std::sync::Arc;

use agent_core::{ContextSource, ContextUsage};
use tiktoken_rs::CoreBPE;

use crate::extractor::extract_keywords;
use crate::memory::MemoryEntry;
use crate::store::{MemoryQuery, MemoryStore};

use super::budget::ContextBudget;
use super::image_pruning::{prune_images, ImageEntry, ImagePruningStrategy};
use super::window::find_lowest_priority_drop;

#[derive(Debug, Clone, Default)]
pub struct ContextRequest {
    pub system_prompt: Option<String>,
    pub project_instructions: Option<String>,
    pub user_request: String,
    pub session_history: Vec<String>,
    pub selected_files: Vec<String>,
    pub tool_results: Vec<String>,
    pub memories: Vec<MemoryEntry>,
    pub active_skills: Vec<String>,
    pub active_task: Option<String>,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
    /// MCP + built-in tool schemas to be injected into the model request.
    /// They're serialised once and counted as a single ToolDefinitions section.
    pub tool_definitions: Vec<agent_models::ToolDefinition>,
    /// Images present in the conversation, ordered by position.
    /// Each entry carries a position index and estimated token cost.
    pub images: Vec<ImageEntry>,
    /// Strategy for pruning images when context is tight.
    pub image_pruning: ImagePruningStrategy,
}

#[derive(Debug, Clone)]
pub struct ContextBundle {
    pub messages: Vec<String>,
    pub sources: Vec<ContextSource>,
    pub truncated: bool,
    pub usage: ContextUsage,
}

pub struct ContextAssembler {
    memory_store: Option<Arc<dyn MemoryStore>>,
    tokenizer: CoreBPE,
}

impl ContextAssembler {
    pub fn new(memory_store: Arc<dyn MemoryStore>) -> Self {
        Self {
            memory_store: Some(memory_store),
            tokenizer: tiktoken_rs::cl100k_base().expect("cl100k_base bundled with tiktoken-rs"),
        }
    }

    /// Create a standalone assembler without a memory store.
    /// Used by TUI where memory integration is deferred.
    /// Memories passed in ContextRequest will still be included.
    pub fn new_standalone() -> Self {
        Self {
            memory_store: None,
            tokenizer: tiktoken_rs::cl100k_base().expect("cl100k_base bundled with tiktoken-rs"),
        }
    }

    pub async fn assemble(&self, request: ContextRequest, budget: ContextBudget) -> ContextBundle {
        let mut sections: Vec<(ContextSource, String, u64)> = Vec::new();

        // P0: System prompt (never dropped)
        if let Some(sp) = &request.system_prompt {
            let n = self.count_tokens(sp);
            sections.push((ContextSource::System, sp.clone(), n));
        }

        // P0.25: Project instructions — high-priority guidance from project files,
        // placed after System prompt, before active skills.
        if let Some(pi) = &request.project_instructions {
            let block = format!("<project-instructions>\n{pi}\n</project-instructions>");
            let n = self.count_tokens(&block);
            sections.push((ContextSource::ProjectInstruction, block, n));
        }

        // P0.5: Active skills — high-priority session guidance, below System
        // but above tool definitions.
        if !request.active_skills.is_empty() {
            let block = format!(
                "<active_skills>\n{}\n</active_skills>",
                request.active_skills.join("\n")
            );
            let tokens = self.count_tokens(&block);
            sections.push((ContextSource::Skill, block, tokens));
        }

        // P0.75: Tool definitions — bundle as one JSON block (so the model adapter
        // can recover the structured array). Counted once.
        if !request.tool_definitions.is_empty() {
            let payload = serde_json::to_string(&request.tool_definitions)
                .unwrap_or_else(|_| String::from("[]"));
            let n = self.count_tokens(&payload);
            sections.push((ContextSource::ToolDefinitions, payload, n));
        }

        // P1: User request (dropped second-to-last)
        let request_text = format!("User request: {}", request.user_request);
        let n = self.count_tokens(&request_text);
        sections.push((ContextSource::Request, request_text, n));

        // Active task (part of history priority)
        if let Some(task) = &request.active_task {
            let text = format!("Active task: {task}");
            let n = self.count_tokens(&text);
            sections.push((ContextSource::History, text, n));
        }

        // P2: Memories — query store if available, otherwise use provided
        let memories = if request.memories.is_empty() {
            if let Some(store) = &self.memory_store {
                let keywords = extract_keywords(&request.user_request);
                store
                    .query(MemoryQuery {
                        scope: None,
                        keywords,
                        limit: 20,
                        session_id: request.session_id.clone(),
                        workspace_id: request.workspace_id.clone(),
                    })
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            request.memories.clone()
        };
        for mem in memories.iter().filter(|m| m.accepted) {
            let text = format!("Memory: {}", mem.content);
            let n = self.count_tokens(&text);
            sections.push((ContextSource::Memory, text, n));
        }

        // P3: Session history
        for h in &request.session_history {
            let text = format!("History: {h}");
            let n = self.count_tokens(&text);
            sections.push((ContextSource::History, text, n));
        }

        // P4: Tool results
        for tr in &request.tool_results {
            let text = format!("Tool result: {tr}");
            let n = self.count_tokens(&text);
            sections.push((ContextSource::ToolResult, text, n));
        }

        // P4.5: Images — prune first, then add survivors.
        // Images are the lowest priority and are dropped before SelectedFile.
        {
            let mut images = request.images.clone();
            prune_images(&mut images, &request.image_pruning);
            for img in &images {
                let text = format!("Image: {}", img.content);
                let n = img.estimated_tokens.max(self.count_tokens(&text));
                sections.push((ContextSource::Image, text, n));
            }
        }

        // P5: Selected files (dropped second after images)
        for sf in &request.selected_files {
            let text = format!("Selected file: {sf}");
            let n = self.count_tokens(&text);
            sections.push((ContextSource::SelectedFile, text, n));
        }

        // Pass 1: per-source caps (drop LIFO inside the capped category).
        let mut truncated = false;
        for (capped_src, cap) in &budget.source_caps {
            loop {
                let total: u64 = sections
                    .iter()
                    .filter(|(s, _, _)| s == capped_src)
                    .map(|(_, _, n)| *n)
                    .sum();
                if total <= *cap {
                    break;
                }
                // Drop the LAST occurrence of this source (LIFO).
                if let Some(idx) = sections.iter().rposition(|(s, _, _)| s == capped_src) {
                    sections.remove(idx);
                    truncated = true;
                } else {
                    break;
                }
            }
        }

        // Pass 2: global budget — drop lowest-priority section repeatedly.
        let input_budget = budget.input_budget();
        let mut total: u64 = sections.iter().map(|(_, _, n)| *n).sum();
        while total > input_budget {
            let Some(idx) = find_lowest_priority_drop(&sections) else {
                break;
            };
            total -= sections[idx].2;
            sections.remove(idx);
            truncated = true;
        }

        // Build per-source breakdown for ContextUsage.
        let mut by_source: Vec<(ContextSource, u64)> = Vec::new();
        for (src, _, n) in &sections {
            if let Some(entry) = by_source.iter_mut().find(|(s, _)| s == src) {
                entry.1 += n;
            } else {
                by_source.push((*src, *n));
            }
        }

        let usage = ContextUsage {
            total_tokens: total,
            budget_tokens: input_budget,
            context_window: budget.context_window,
            output_reservation: budget.output_reservation,
            by_source,
            estimator: "cl100k_base".to_string(),
            corrected_by_real_usage: false,
        };

        ContextBundle {
            messages: sections.iter().map(|(_, s, _)| s.clone()).collect(),
            sources: sections.iter().map(|(src, _, _)| *src).collect(),
            truncated,
            usage,
        }
    }

    fn count_tokens(&self, text: &str) -> u64 {
        self.tokenizer.encode_with_special_tokens(text).len() as u64
    }
}
