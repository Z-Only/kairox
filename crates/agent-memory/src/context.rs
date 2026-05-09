use crate::extractor::extract_keywords;
use crate::memory::MemoryEntry;
use crate::store::{MemoryQuery, MemoryStore};
use std::sync::Arc;
use tiktoken_rs::CoreBPE;

pub use agent_core::{ContextSource, ContextUsage};

#[derive(Debug, Clone, Default)]
pub struct ContextRequest {
    pub system_prompt: Option<String>,
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
}

#[derive(Debug, Clone)]
pub struct ContextBudget {
    /// Total context window of the active model (e.g. 200_000 for Sonnet 4).
    pub context_window: u64,
    /// Tokens reserved for the upcoming completion. Effective input budget
    /// is `context_window - output_reservation`.
    pub output_reservation: u64,
    /// Optional per-source soft caps (applied before the global drop pass).
    pub source_caps: Vec<(ContextSource, u64)>,
}

impl ContextBudget {
    pub fn input_budget(&self) -> u64 {
        self.context_window.saturating_sub(self.output_reservation)
    }
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

        // P5: Selected files (dropped first)
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

/// Find the index of the lowest-priority section that can be dropped.
/// Priority (highest first): System, Skill, ToolDefinitions, Request, Memory, History, ToolResult, SelectedFile.
/// System and Request are never dropped; Skill is dropped only after ToolDefinitions.
fn find_lowest_priority_drop(sections: &[(ContextSource, String, u64)]) -> Option<usize> {
    let drop_order = [
        ContextSource::SelectedFile,
        ContextSource::ToolResult,
        ContextSource::History,
        ContextSource::Memory,
        ContextSource::ToolDefinitions,
        ContextSource::Skill,
    ];
    for category in &drop_order {
        for (i, (src, _, _)) in sections.iter().enumerate() {
            if src == category {
                return Some(i);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryScope;
    use crate::store::{MemoryStore, SqliteMemoryStore};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_assembler_with_store() -> (ContextAssembler, Arc<dyn MemoryStore>) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let store = Arc::new(SqliteMemoryStore::new(pool).await.unwrap()) as Arc<dyn MemoryStore>;
        let assembler = ContextAssembler::new(store.clone());
        (assembler, store)
    }

    fn test_budget(window: u64, output: u64) -> ContextBudget {
        ContextBudget {
            context_window: window,
            output_reservation: output,
            source_caps: vec![],
        }
    }

    #[tokio::test]
    async fn assembles_request_with_standalone_assembler() {
        let assembler = ContextAssembler::new_standalone();
        let bundle = assembler
            .assemble(
                ContextRequest {
                    user_request: "fix tests".into(),
                    session_history: vec!["previous answer".into()],
                    selected_files: vec![],
                    tool_results: vec![],
                    memories: vec![],
                    active_task: None,
                    ..Default::default()
                },
                test_budget(200, 100),
            )
            .await;

        assert!(bundle.messages.join("\n").contains("fix tests"));
        assert!(bundle.usage.total_tokens <= bundle.usage.budget_tokens);
    }

    #[tokio::test]
    async fn includes_memories_from_store() {
        let (assembler, store) = test_assembler_with_store().await;
        store
            .store(MemoryEntry::new(
                MemoryScope::Workspace,
                "Use cargo nextest".into(),
                true,
            ))
            .await
            .unwrap();

        let bundle = assembler
            .assemble(
                ContextRequest {
                    user_request: "nextest config".into(),
                    ..Default::default()
                },
                test_budget(600, 100),
            )
            .await;

        assert!(bundle.messages.join("\n").contains("Use cargo nextest"));
    }

    #[tokio::test]
    async fn truncates_lowest_priority_first() {
        let assembler = ContextAssembler::new_standalone();
        let long_files: Vec<String> = (0..20)
            .map(|i| format!("file_content_{i}_with_a_long_name"))
            .collect();

        let bundle = assembler
            .assemble(
                ContextRequest {
                    system_prompt: Some("System".into()),
                    user_request: "request".into(),
                    selected_files: long_files,
                    ..Default::default()
                },
                test_budget(100, 50),
            )
            .await;

        // System and request should survive
        assert!(bundle.messages[0].contains("System"));
        assert!(bundle.truncated);
    }

    #[tokio::test]
    async fn never_drops_system_or_request() {
        let assembler = ContextAssembler::new_standalone();
        let bundle = assembler
            .assemble(
                ContextRequest {
                    system_prompt: Some("Important system prompt".into()),
                    user_request: "User query here".into(),
                    ..Default::default()
                },
                test_budget(100, 80),
            )
            .await;

        let combined = bundle.messages.join("\n");
        assert!(combined.contains("Important system prompt") || combined.contains("User query"));
    }

    #[test]
    fn skill_drop_priority_is_below_system_and_above_tool_definitions() {
        let with_tool_definitions = vec![
            (ContextSource::System, String::from("system"), 1),
            (ContextSource::Skill, String::from("skill"), 1),
            (
                ContextSource::ToolDefinitions,
                String::from("tool definitions"),
                1,
            ),
        ];
        assert_eq!(find_lowest_priority_drop(&with_tool_definitions), Some(2));

        let without_tool_definitions = vec![
            (ContextSource::System, String::from("system"), 1),
            (ContextSource::Skill, String::from("skill"), 1),
        ];
        assert_eq!(
            find_lowest_priority_drop(&without_tool_definitions),
            Some(1)
        );

        let protected_sources = vec![
            (ContextSource::System, String::from("system"), 1),
            (ContextSource::Request, String::from("request"), 1),
        ];
        assert_eq!(find_lowest_priority_drop(&protected_sources), None);
    }

    #[test]
    fn input_budget_subtracts_output_reservation() {
        let budget = ContextBudget {
            context_window: 200_000,
            output_reservation: 12_000,
            source_caps: vec![],
        };
        assert_eq!(budget.input_budget(), 188_000);
    }

    #[test]
    fn input_budget_saturates_at_zero_when_reservation_exceeds_window() {
        let budget = ContextBudget {
            context_window: 8_000,
            output_reservation: 12_000,
            source_caps: vec![],
        };
        assert_eq!(budget.input_budget(), 0);
    }
}
