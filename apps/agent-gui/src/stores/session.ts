import { reactive } from "vue";
import type {
  SessionProjection,
  SessionInfoResponse,
  DomainEvent
} from "../types";

/** Report a send error to the UI when the background task fails. */
export function reportSendError(message: string) {
  sessionState.projection.messages.push({
    role: "assistant",
    content: `[error] ${message}`
  });
  sessionState.projection.token_stream = "";
  sessionState.isStreaming = false;
}

export const sessionState = reactive({
  sessions: [] as SessionInfoResponse[],
  currentSessionId: null as string | null,
  projection: {
    messages: [],
    task_titles: [],
    token_stream: "",
    cancelled: false
  } as SessionProjection,
  currentProfile: "fake",
  isStreaming: false,
  connected: false,
  initialized: false
});

/**
 * Apply a DomainEvent to the local session projection.
 * Mirrors the Rust SessionProjection::apply() method.
 */
export function applyEvent(event: DomainEvent) {
  const p = event.payload;
  switch (p.type) {
    case "UserMessageAdded":
      sessionState.projection.messages.push({
        role: "user",
        content: p.content
      });
      sessionState.isStreaming = true;
      break;
    case "ModelTokenDelta":
      sessionState.projection.token_stream += p.delta;
      break;
    case "AssistantMessageCompleted":
      sessionState.projection.messages.push({
        role: "assistant",
        content: p.content
      });
      sessionState.projection.token_stream = "";
      sessionState.isStreaming = false;
      break;
    case "SessionCancelled":
      sessionState.projection.cancelled = true;
      sessionState.isStreaming = false;
      break;
    case "AgentTaskCreated":
      sessionState.projection.task_titles.push(p.title);
      break;
    case "ToolInvocationStarted":
    case "ToolInvocationCompleted":
    case "ToolInvocationFailed":
    case "PermissionRequested":
    case "PermissionGranted":
    case "PermissionDenied":
      // Trace/permission events — stored but not rendered in MVP
      break;
  }
}

/**
 * Replace the current projection entirely (used after session switch).
 */
export function setProjection(projection: SessionProjection) {
  sessionState.projection = projection;
  sessionState.isStreaming = false;
}

/**
 * Reset projection to empty state (used before switching sessions).
 */
export function resetProjection() {
  sessionState.projection = {
    messages: [],
    task_titles: [],
    token_stream: "",
    cancelled: false
  };
  sessionState.isStreaming = false;
}
