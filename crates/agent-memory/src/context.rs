use crate::memory::MemoryEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextRequest {
    pub user_request: String,
    pub session_history: Vec<String>,
    pub selected_files: Vec<String>,
    pub tool_results: Vec<String>,
    pub memories: Vec<MemoryEntry>,
    pub active_task: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBundle {
    pub messages: Vec<String>,
    pub token_estimate: usize,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ContextAssembler {
    max_tokens: usize,
}

impl ContextAssembler {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }

    pub fn assemble(&self, request: ContextRequest) -> ContextBundle {
        let mut messages = Vec::new();
        messages.push(format!("User request: {}", request.user_request));
        if let Some(active_task) = request.active_task {
            messages.push(format!("Active task: {active_task}"));
        }
        messages.extend(
            request
                .session_history
                .into_iter()
                .map(|item| format!("History: {item}")),
        );
        messages.extend(
            request
                .selected_files
                .into_iter()
                .map(|item| format!("Selected file: {item}")),
        );
        messages.extend(
            request
                .tool_results
                .into_iter()
                .map(|item| format!("Tool result: {item}")),
        );
        messages.extend(
            request
                .memories
                .into_iter()
                .filter(|memory| memory.accepted)
                .map(|memory| format!("Memory: {}", memory.content)),
        );

        let mut token_estimate = estimate_tokens(&messages.join("\n"));
        while token_estimate > self.max_tokens && messages.len() > 1 {
            messages.remove(1);
            token_estimate = estimate_tokens(&messages.join("\n"));
        }

        ContextBundle {
            sources: messages
                .iter()
                .map(|message| message.split(':').next().unwrap_or("context").to_string())
                .collect(),
            messages,
            token_estimate,
        }
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryEntry, MemoryScope};

    #[test]
    fn assembles_request_history_and_workspace_memory_within_budget() {
        let assembler = ContextAssembler::new(100);
        let bundle = assembler.assemble(ContextRequest {
            user_request: "fix tests".into(),
            session_history: vec!["previous answer".into()],
            selected_files: vec!["Cargo.toml".into()],
            tool_results: vec!["cargo test failed".into()],
            memories: vec![MemoryEntry {
                id: "mem1".into(),
                scope: MemoryScope::Workspace,
                content: "Use cargo test --workspace".into(),
                accepted: true,
            }],
            active_task: Some("repair failing test".into()),
        });

        assert!(bundle.messages.join("\n").contains("fix tests"));
        assert!(bundle
            .messages
            .join("\n")
            .contains("Use cargo test --workspace"));
        assert!(bundle.token_estimate <= 100);
    }
}
