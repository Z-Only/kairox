import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
  SessionProjection,
  SessionInfoResponse,
  DomainEvent
} from "../types";
import { clearTrace, applyTraceEvent } from "../composables/useTraceStore";

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
  workspaceId: null as string | null,
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

export async function deleteSession(sessionId: string) {
  try {
    await invoke("delete_session", { sessionId });
    sessionState.sessions = sessionState.sessions.filter(
      (s) => s.id !== sessionId
    );
    // If we deleted the current session, switch to the first remaining one
    if (sessionState.currentSessionId === sessionId) {
      if (sessionState.sessions.length > 0) {
        const firstSession = sessionState.sessions[0];
        sessionState.currentSessionId = firstSession.id;
        sessionState.currentProfile = firstSession.profile;
        resetProjection();
        clearTrace();
        try {
          const projection: SessionProjection = await invoke("switch_session", {
            sessionId: firstSession.id
          });
          setProjection(projection);
          const traceStrings: string[] = await invoke("get_trace", {
            sessionId: firstSession.id
          });
          for (const jsonStr of traceStrings) {
            try {
              applyTraceEvent(JSON.parse(jsonStr));
            } catch {
              // Skip malformed trace entries
            }
          }
        } catch (e) {
          console.error("Failed to switch after delete:", e);
        }
      } else {
        sessionState.currentSessionId = null;
        resetProjection();
        clearTrace();
      }
    }
  } catch (e) {
    console.error("Failed to delete session:", e);
  }
}

export async function renameSession(sessionId: string, title: string) {
  try {
    await invoke("rename_session", { sessionId, title });
    const session = sessionState.sessions.find((s) => s.id === sessionId);
    if (session) {
      session.title = title;
    }
  } catch (e) {
    console.error("Failed to rename session:", e);
  }
}

export async function recoverSessions(): Promise<boolean> {
  try {
    const workspaces: { workspace_id: string; path: string }[] =
      await invoke("list_workspaces");
    if (workspaces.length === 0) {
      return false;
    }

    const ws = workspaces[0];
    sessionState.workspaceId = ws.workspace_id;

    // Tell the Rust backend which workspace to use so that
    // list_sessions and other commands work correctly.
    await invoke("restore_workspace", { workspaceId: ws.workspace_id });

    sessionState.sessions = await invoke("list_sessions");

    if (sessionState.sessions.length > 0) {
      const firstSession = sessionState.sessions[0];
      sessionState.currentSessionId = firstSession.id;
      sessionState.currentProfile = firstSession.profile;

      try {
        const projection: SessionProjection = await invoke("switch_session", {
          sessionId: firstSession.id
        });
        setProjection(projection);
        const traceStrings: string[] = await invoke("get_trace", {
          sessionId: firstSession.id
        });
        for (const jsonStr of traceStrings) {
          try {
            applyTraceEvent(JSON.parse(jsonStr));
          } catch {
            // Skip malformed trace entries
          }
        }
      } catch (e) {
        console.error("Failed to load session history:", e);
      }
    }

    sessionState.initialized = true;
    return true;
  } catch (e) {
    console.error("Failed to recover sessions:", e);
    return false;
  }
}
