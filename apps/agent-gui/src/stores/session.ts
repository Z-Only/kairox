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
  currentProfile: "fast",
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
    case "UserMessageAdded": {
      const typed = p as { type: "UserMessageAdded"; content: string };
      sessionState.projection.messages.push({
        role: "user",
        content: typed.content
      });
      sessionState.isStreaming = true;
      break;
    }
    case "ModelTokenDelta": {
      const typed = p as { type: "ModelTokenDelta"; delta: string };
      sessionState.projection.token_stream += typed.delta;
      break;
    }
    case "AssistantMessageCompleted": {
      const typed = p as { type: "AssistantMessageCompleted"; content: string };
      sessionState.projection.messages.push({
        role: "assistant",
        content: typed.content
      });
      sessionState.projection.token_stream = "";
      sessionState.isStreaming = false;
      break;
    }
    case "SessionCancelled":
      sessionState.projection.cancelled = true;
      sessionState.isStreaming = false;
      break;
    case "AgentTaskCreated": {
      const typed = p as { type: "AgentTaskCreated"; title: string };
      sessionState.projection.task_titles.push(typed.title);
      break;
    }
    case "AgentTaskStarted":
      // Task is now running — no projection change needed
      break;
    case "AgentTaskCompleted":
      // Task finished successfully — no projection change needed
      break;
    case "AgentTaskFailed": {
      const typed = p as { type: "AgentTaskFailed"; error: string };
      // Show the error as an assistant message and reset streaming state
      sessionState.projection.messages.push({
        role: "assistant",
        content: `[error] ${typed.error || "Unknown error"}`
      });
      sessionState.projection.token_stream = "";
      sessionState.isStreaming = false;
      break;
    }
    case "ContextAssembled":
    case "ModelRequestStarted":
    case "ModelToolCallRequested":
    case "ToolInvocationStarted":
    case "ToolInvocationCompleted":
    case "ToolInvocationFailed":
    case "PermissionRequested":
    case "PermissionGranted":
    case "PermissionDenied":
    case "FilePatchProposed":
    case "FilePatchApplied":
    case "MemoryProposed":
    case "MemoryAccepted":
    case "MemoryRejected":
    case "ReviewerFindingAdded":
      // Trace/permission/memory/patch/review events — handled by useTraceStore
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
