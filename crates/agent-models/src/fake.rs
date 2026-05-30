use crate::{ModelClient, ModelEvent, ModelRequest};
use async_trait::async_trait;
use futures::{stream, stream::BoxStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Optional tool-call shape emitted by the fake client after its token stream.
/// Used by the eval harness and other tests that need deterministic tool-call
/// lifecycle events from the runtime.
#[derive(Debug)]
struct FakeToolCall {
    tool_id: String,
    arguments: serde_json::Value,
    /// One-shot guard: the tool call is emitted on the first `stream` call
    /// only. Subsequent calls (driven by the agent loop after the tool
    /// result is appended) skip the tool emission so the loop can terminate.
    emitted: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct FakeModelClient {
    tokens: Vec<String>,
    tool_call: Option<Arc<FakeToolCall>>,
}

impl FakeModelClient {
    pub fn new(tokens: Vec<String>) -> Self {
        Self {
            tokens,
            tool_call: None,
        }
    }

    /// Emit a single `fs.read` tool call (`{"path":"README.md"}`) after the
    /// configured tokens. Kept for backwards compatibility with existing
    /// tests that assume this fixed shape; new callers should prefer
    /// [`FakeModelClient::with_tool_call_for`] to avoid relying on a
    /// `README.md` being present in the current workspace.
    pub fn with_tool_call(self) -> Self {
        self.with_tool_call_for("fs.read", serde_json::json!({"path": "README.md"}))
    }

    /// Emit a single tool call with the supplied `tool_id` and JSON
    /// `arguments` after the configured tokens. Choose a `tool_id` whose
    /// arguments are valid for the workspace the runtime drives (for
    /// example, `fs.list` with `{"path":"."}` works in any temp dir).
    ///
    /// The tool call is emitted only on the first `stream` invocation; on
    /// subsequent invocations (driven by the agent loop after the tool
    /// result is appended) the client emits just its token stream so the
    /// loop can terminate cleanly with a single `ToolInvocationCompleted`.
    pub fn with_tool_call_for(
        mut self,
        tool_id: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        self.tool_call = Some(Arc::new(FakeToolCall {
            tool_id: tool_id.into(),
            arguments,
            emitted: AtomicBool::new(false),
        }));
        self
    }
}

#[async_trait]
impl ModelClient for FakeModelClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> crate::Result<BoxStream<'static, crate::Result<ModelEvent>>> {
        let _ = request;
        let mut events: Vec<crate::Result<ModelEvent>> = self
            .tokens
            .iter()
            .cloned()
            .map(ModelEvent::TokenDelta)
            .map(Ok)
            .collect();

        if let Some(tool_call) = &self.tool_call {
            // One-shot: emit the tool call only on the first stream call.
            if tool_call
                .emitted
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                events.push(Ok(ModelEvent::ToolCallRequested {
                    tool_call_id: "call_fake_1".into(),
                    tool_id: tool_call.tool_id.clone(),
                    arguments: tool_call.arguments.clone(),
                }));
            }
        }

        events.push(Ok(ModelEvent::Completed { usage: None }));
        Ok(Box::pin(stream::iter(events)))
    }
}

#[cfg(test)]
#[path = "fake_tests.rs"]
mod tests;
