//! SDK session — programmatic agent interaction.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use agent_core::facade::{SendMessageRequest, StartSessionRequest};
use agent_core::AppFacade;
use agent_core::{DomainEvent, EventPayload, SessionId, WorkspaceId};
use agent_runtime::ui_bootstrap::UiRuntime;
use futures::stream::{BoxStream, Stream, StreamExt};

use crate::error::{SdkError, SdkResult};
use crate::hooks::SdkHook;

/// A live agent session bound to a workspace.
///
/// Use [`send_message`](SdkSession::send_message) to inject prompts and
/// receive a [`MessageStream`] of events.
pub struct SdkSession {
    runtime: Arc<UiRuntime>,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    hooks: Vec<Arc<dyn SdkHook>>,
}

impl SdkSession {
    /// Create a new session in the given workspace.
    pub(crate) async fn create(
        runtime: Arc<UiRuntime>,
        workspace_path: &std::path::Path,
        hooks: &[Arc<dyn SdkHook>],
    ) -> SdkResult<Self> {
        Self::create_with_profile(runtime, workspace_path, hooks, "default").await
    }

    /// Create a new session with a specific model profile.
    pub(crate) async fn create_with_profile(
        runtime: Arc<UiRuntime>,
        workspace_path: &std::path::Path,
        hooks: &[Arc<dyn SdkHook>],
        model_profile: &str,
    ) -> SdkResult<Self> {
        let workspace = runtime
            .open_workspace(workspace_path.display().to_string())
            .await?;
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: model_profile.to_string(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await?;
        Ok(Self {
            runtime,
            workspace_id: workspace.workspace_id,
            session_id,
            hooks: hooks.to_vec(),
        })
    }

    /// The unique session identifier.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// The workspace identifier.
    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }

    /// Send a text message to the agent and stream back events.
    pub async fn send_message(&self, content: &str) -> SdkResult<MessageStream> {
        let event_stream = self.runtime.subscribe_session(self.session_id.clone());

        self.runtime
            .send_message(SendMessageRequest {
                workspace_id: self.workspace_id.clone(),
                session_id: self.session_id.clone(),
                content: content.to_string(),
                display_content: None,
                attachments: vec![],
            })
            .await?;

        Ok(MessageStream::new(event_stream, self.hooks.clone()))
    }

    /// Send a message with file attachments.
    pub async fn send_message_with_attachments(
        &self,
        content: &str,
        attachments: Vec<agent_core::facade::AttachmentInfo>,
    ) -> SdkResult<MessageStream> {
        let event_stream = self.runtime.subscribe_session(self.session_id.clone());

        self.runtime
            .send_message(SendMessageRequest {
                workspace_id: self.workspace_id.clone(),
                session_id: self.session_id.clone(),
                content: content.to_string(),
                display_content: None,
                attachments,
            })
            .await?;

        Ok(MessageStream::new(event_stream, self.hooks.clone()))
    }

    /// Cancel the current turn in this session.
    pub async fn cancel(&self) -> SdkResult<()> {
        self.runtime
            .cancel_session(self.workspace_id.clone(), self.session_id.clone())
            .await?;
        Ok(())
    }

    /// Get the current trace (event history) for this session.
    pub async fn get_trace(&self) -> SdkResult<Vec<agent_core::facade::TraceEntry>> {
        let trace = self.runtime.get_trace(self.session_id.clone()).await?;
        Ok(trace)
    }

    /// Export the trace as a structured export.
    pub async fn export_trace(&self) -> SdkResult<agent_core::facade::TraceExport> {
        let export = self.runtime.export_trace(self.session_id.clone()).await?;
        Ok(export)
    }
}

/// A simplified event emitted from the message stream.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// The agent produced text content.
    Text(String),
    /// The agent is calling a tool.
    ToolCall {
        tool_name: String,
        tool_input: serde_json::Value,
    },
    /// A tool call completed with a result.
    ToolResult { tool_name: String, output: String },
    /// The agent's turn completed.
    TurnCompleted,
    /// An error occurred during the turn.
    Error(String),
    /// A raw domain event that doesn't map to the above categories.
    Other(Box<DomainEvent>),
}

impl StreamEvent {
    pub(crate) fn from_domain_event(event: DomainEvent) -> Self {
        match &event.payload {
            EventPayload::ModelTokenDelta { delta } => Self::Text(delta.clone()),
            EventPayload::ToolInvocationStarted { tool_id, .. } => Self::ToolCall {
                tool_name: tool_id.clone(),
                tool_input: serde_json::Value::Null,
            },
            EventPayload::ToolInvocationCompleted {
                tool_id,
                output_preview,
                ..
            } => Self::ToolResult {
                tool_name: tool_id.clone(),
                output: output_preview.clone(),
            },
            EventPayload::AssistantMessageCompleted { .. } => Self::TurnCompleted,
            EventPayload::AgentTaskFailed { error, .. } => Self::Error(error.clone()),
            EventPayload::ToolInvocationFailed { error, .. } => Self::Error(error.clone()),
            _ => Self::Other(Box::new(event)),
        }
    }
}

/// An async stream of [`StreamEvent`]s from a running agent turn.
///
/// Implements [`futures::Stream`] so it works with `while let Some(event) =
/// stream.next().await`.
pub struct MessageStream {
    inner: BoxStream<'static, DomainEvent>,
    /// Reserved for future hook integration (pre/post tool interception).
    _hooks: Vec<Arc<dyn SdkHook>>,
    completed: bool,
}

impl MessageStream {
    pub(crate) fn new(
        inner: BoxStream<'static, DomainEvent>,
        hooks: Vec<Arc<dyn SdkHook>>,
    ) -> Self {
        Self {
            inner,
            _hooks: hooks,
            completed: false,
        }
    }

    /// Collect all events until the turn completes, returning the final
    /// assistant text and the full event log.
    pub async fn collect_all(mut self) -> SdkResult<CollectedResponse> {
        let mut text_parts = Vec::new();
        let mut events = Vec::new();

        while let Some(event) = self.next().await {
            match &event {
                StreamEvent::Text(text) => text_parts.push(text.clone()),
                StreamEvent::TurnCompleted => {
                    events.push(event);
                    break;
                }
                StreamEvent::Error(err) => {
                    return Err(SdkError::Core(agent_core::CoreError::InvalidState(
                        err.clone(),
                    )));
                }
                _ => {}
            }
            events.push(event);
        }

        Ok(CollectedResponse {
            text: text_parts.join(""),
            events,
        })
    }
}

impl Stream for MessageStream {
    type Item = StreamEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.completed {
            return Poll::Ready(None);
        }

        match self.inner.poll_next_unpin(cx) {
            Poll::Ready(Some(event)) => {
                let stream_event = StreamEvent::from_domain_event(event);
                if matches!(
                    stream_event,
                    StreamEvent::TurnCompleted | StreamEvent::Error(_)
                ) {
                    self.completed = true;
                }
                Poll::Ready(Some(stream_event))
            }
            Poll::Ready(None) => {
                self.completed = true;
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// The complete response from a turn, collected after streaming finishes.
#[derive(Debug, Clone)]
pub struct CollectedResponse {
    /// The full assistant text, concatenated from all stream chunks.
    pub text: String,
    /// All events emitted during the turn.
    pub events: Vec<StreamEvent>,
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
