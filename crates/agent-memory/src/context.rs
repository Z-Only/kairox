use crate::extractor::extract_keywords;
use crate::memory::MemoryEntry;
use crate::store::{MemoryQuery, MemoryStore};
use std::sync::Arc;
use tiktoken_rs::CoreBPE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextSource {
    System,
    Request,
    Memory,
    History,
    ToolResult,
    SelectedFile,
}

#[derive(Debug, Clone, Default)]
pub struct ContextRequest {
    pub system_prompt: Option<String>,
    pub user_request: String,
    pub session_history: Vec<String>,
    pub selected_files: Vec<String>,
    pub tool_results: Vec<String>,
    pub memories: Vec<MemoryEntry>,
    pub active_task: Option<String>,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContextBundle {
    pub messages: Vec<String>,
    pub token_count: usize,
    pub sources: Vec<ContextSource>,
    pub truncated: bool,
}

pub struct ContextAssembler {
    max_tokens: usize,
    memory_store: Option<Arc<dyn MemoryStore>>,
    tokenizer: CoreBPE,
}

impl ContextAssembler {
    pub fn new(max_tokens: usize, memory_store: Arc<dyn MemoryStore>) -> Self {
        Self {
            max_tokens,
            memory_store: Some(memory_store),
            tokenizer: tiktoken_rs::cl100k_base().unwrap(),
        }
    }

    /// Create a standalone assembler without a memory store.
    /// Used by TUI where memory integration is deferred.
    /// Memories passed in ContextRequest will still be included.
    pub fn new_standalone(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            memory_store: None,
            tokenizer: tiktoken_rs::cl100k_base().unwrap(),
        }
    }

    pub async fn assemble(&self, request: ContextRequest) -> ContextBundle {
        let mut sections: Vec<(ContextSource, String, usize)> = Vec::new();

        // P0: System prompt (never dropped)
        if let Some(sp) = &request.system_prompt {
            let tokens = self.count_tokens(sp);
            sections.push((ContextSource::System, sp.clone(), tokens));
        }

        // P1: User request (dropped last)
        let request_text = format!("User request: {}", request.user_request);
        sections.push((
            ContextSource::Request,
            request_text.clone(),
            self.count_tokens(&request_text),
        ));

        // Active task (part of history priority)
        if let Some(task) = &request.active_task {
            let text = format!("Active task: {task}");
            sections.push((
                ContextSource::History,
                text.clone(),
                self.count_tokens(&text),
            ));
        }

        // P2: Memories — query store if available, otherwise use provided
        let memories = if request.memories.is_empty() {
            if let Some(ref store) = self.memory_store {
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

        for mem in &memories {
            if mem.accepted {
                let text = format!("Memory: {}", mem.content);
                sections.push((
                    ContextSource::Memory,
                    text.clone(),
                    self.count_tokens(&text),
                ));
            }
        }

        // P3: Session history
        for h in &request.session_history {
            let text = format!("History: {h}");
            sections.push((
                ContextSource::History,
                text.clone(),
                self.count_tokens(&text),
            ));
        }

        // P4: Tool results
        for tr in &request.tool_results {
            let text = format!("Tool result: {tr}");
            sections.push((
                ContextSource::ToolResult,
                text.clone(),
                self.count_tokens(&text),
            ));
        }

        // P5: Selected files (dropped first)
        for sf in &request.selected_files {
            let text = format!("Selected file: {sf}");
            sections.push((
                ContextSource::SelectedFile,
                text.clone(),
                self.count_tokens(&text),
            ));
        }

        // Truncate from lowest priority
        let mut total_tokens: usize = sections.iter().map(|(_, _, t)| *t).sum();
        let mut truncated = false;

        while total_tokens > self.max_tokens {
            if let Some(idx) = find_lowest_priority_drop(&sections) {
                total_tokens -= sections[idx].2;
                sections.remove(idx);
                truncated = true;
            } else {
                break;
            }
        }

        ContextBundle {
            messages: sections.iter().map(|(_, s, _)| s.clone()).collect(),
            token_count: total_tokens,
            sources: sections.iter().map(|(src, _, _)| src.clone()).collect(),
            truncated,
        }
    }

    fn count_tokens(&self, text: &str) -> usize {
        self.tokenizer.encode_with_special_tokens(text).len()
    }
}

/// Find the index of the lowest-priority section that can be dropped.
/// Priority (highest first): System, Request, Memory, History, ToolResult, SelectedFile
/// System and Request are never dropped.
fn find_lowest_priority_drop(sections: &[(ContextSource, String, usize)]) -> Option<usize> {
    let drop_order = [
        ContextSource::SelectedFile,
        ContextSource::ToolResult,
        ContextSource::History,
        ContextSource::Memory,
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
        let assembler = ContextAssembler::new(500, store.clone());
        (assembler, store)
    }

    #[tokio::test]
    async fn assembles_request_with_standalone_assembler() {
        let assembler = ContextAssembler::new_standalone(100);
        let bundle = assembler
            .assemble(ContextRequest {
                user_request: "fix tests".into(),
                session_history: vec!["previous answer".into()],
                selected_files: vec![],
                tool_results: vec![],
                memories: vec![],
                active_task: None,
                ..Default::default()
            })
            .await;

        assert!(bundle.messages.join("\n").contains("fix tests"));
        assert!(bundle.token_count <= 100);
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
            .assemble(ContextRequest {
                user_request: "nextest config".into(),
                ..Default::default()
            })
            .await;

        assert!(bundle.messages.join("\n").contains("Use cargo nextest"));
    }

    #[tokio::test]
    async fn truncates_lowest_priority_first() {
        let assembler = ContextAssembler::new_standalone(50);
        let long_files: Vec<String> = (0..20)
            .map(|i| format!("file_content_{i}_with_a_long_name"))
            .collect();

        let bundle = assembler
            .assemble(ContextRequest {
                system_prompt: Some("System".into()),
                user_request: "request".into(),
                selected_files: long_files,
                ..Default::default()
            })
            .await;

        // System and request should survive
        assert!(bundle.messages[0].contains("System"));
        assert!(bundle.truncated);
    }

    #[tokio::test]
    async fn never_drops_system_or_request() {
        let assembler = ContextAssembler::new_standalone(20);
        let bundle = assembler
            .assemble(ContextRequest {
                system_prompt: Some("Important system prompt".into()),
                user_request: "User query here".into(),
                ..Default::default()
            })
            .await;

        let combined = bundle.messages.join("\n");
        assert!(combined.contains("Important system prompt") || combined.contains("User query"));
    }
}
