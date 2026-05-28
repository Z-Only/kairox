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
mod tests {
    use super::*;
    use crate::{ModelEvent, ModelRequest};
    use futures::StreamExt;

    #[tokio::test]
    async fn streams_configured_tokens_then_completion() {
        let client = FakeModelClient::new(vec!["hello".into(), " ".into(), "world".into()]);
        let mut stream = client
            .stream(ModelRequest::user_text("test", "hi"))
            .await
            .unwrap();

        let mut seen = Vec::new();
        while let Some(event) = stream.next().await {
            seen.push(event.unwrap());
        }

        assert_eq!(
            seen,
            vec![
                ModelEvent::TokenDelta("hello".into()),
                ModelEvent::TokenDelta(" ".into()),
                ModelEvent::TokenDelta("world".into()),
                ModelEvent::Completed { usage: None },
            ]
        );
    }

    #[tokio::test]
    async fn optionally_includes_tool_call_event() {
        let client = FakeModelClient::new(vec!["reading".into()]).with_tool_call();
        let mut stream = client
            .stream(ModelRequest::user_text("test", "read"))
            .await
            .unwrap();

        let mut seen = Vec::new();
        while let Some(event) = stream.next().await {
            seen.push(event.unwrap());
        }

        assert!(matches!(&seen[1], ModelEvent::ToolCallRequested { .. }));
    }

    #[tokio::test]
    async fn with_tool_call_for_overrides_tool_id_and_arguments() {
        let client = FakeModelClient::new(vec!["listing".into()])
            .with_tool_call_for("fs.list", serde_json::json!({"path": "."}));
        let mut stream = client
            .stream(ModelRequest::user_text("test", "ls"))
            .await
            .unwrap();

        let mut seen = Vec::new();
        while let Some(event) = stream.next().await {
            seen.push(event.unwrap());
        }

        match &seen[1] {
            ModelEvent::ToolCallRequested {
                tool_id, arguments, ..
            } => {
                assert_eq!(tool_id, "fs.list");
                assert_eq!(arguments, &serde_json::json!({"path": "."}));
            }
            other => panic!("expected ToolCallRequested, got {other:?}"),
        }
    }

    /// Regression: the tool call must emit only on the first stream call,
    /// so the agent loop terminates after the runtime appends the tool
    /// result and re-invokes the model.
    #[tokio::test]
    async fn tool_call_emits_only_on_first_stream_call() {
        let client = FakeModelClient::new(vec!["listing".into()])
            .with_tool_call_for("fs.list", serde_json::json!({"path": "."}));

        let mut first_stream = client
            .stream(ModelRequest::user_text("test", "ls"))
            .await
            .unwrap();
        let mut first = Vec::new();
        while let Some(event) = first_stream.next().await {
            first.push(event.unwrap());
        }
        let first_tool_calls = first
            .iter()
            .filter(|e| matches!(e, ModelEvent::ToolCallRequested { .. }))
            .count();
        assert_eq!(
            first_tool_calls, 1,
            "first stream should emit one tool call"
        );

        let mut second_stream = client
            .stream(ModelRequest::user_text("test", "ls"))
            .await
            .unwrap();
        let mut second = Vec::new();
        while let Some(event) = second_stream.next().await {
            second.push(event.unwrap());
        }
        let second_tool_calls = second
            .iter()
            .filter(|e| matches!(e, ModelEvent::ToolCallRequested { .. }))
            .count();
        assert_eq!(
            second_tool_calls, 0,
            "subsequent stream calls should not re-emit the tool call",
        );
    }
}
