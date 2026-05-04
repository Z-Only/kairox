import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
  SessionProjection,
  SessionInfoResponse,
  DomainEvent
} from "../types";
import { clearTrace, applyTraceEvent } from "../composables/useTraceStore";
import { taskGraphState, clearTaskGraph } from "./taskGraph";

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
    task_graph: { tasks: [] },
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
      sessionState.projection.messages.push({
        role: "user",
        content: p.content
      });
      sessionState.isStreaming = true;
      break;
    }
    case "ModelTokenDelta": {
      sessionState.projection.token_stream += p.delta;
      break;
    }
    case "AssistantMessageCompleted": {
      sessionState.projection.messages.push({
        role: "assistant",
        content: p.content
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
      sessionState.projection.task_titles.push(p.title);
      break;
    }
    case "AgentTaskStarted":
      // Task is now running — no projection change needed
      break;
    case "AgentTaskCompleted":
      // Safety net: when a task completes, ensure streaming is reset.
      // This catches the edge case where the agent loop ends with a
      // tool-only response (empty AssistantMessageCompleted) and the
      // root task completes, but isStreaming wasn't properly cleared.
      sessionState.isStreaming = false;
      break;
    case "AgentTaskFailed": {
      // Show the error as an assistant message and reset streaming state
      sessionState.projection.messages.push({
        role: "assistant",
        content: `[error] ${p.error || "Unknown error"}`
      });
      sessionState.projection.token_stream = "";
      sessionState.isStreaming = false;
      break;
    }
    case "SessionInitialized":
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
    case "WorkspaceOpened":
      // Trace/permission/memory/patch/review events — handled by useTraceStore
      break;
  }
}

/**
 * Replace the current projection entirely (used after session switch).
 * Also populates the task graph from the projection's embedded task_graph.
 */
export function setProjection(projection: SessionProjection) {
  sessionState.projection = projection;
  sessionState.isStreaming = false;
  // Populate task graph from the projection data (rebuilt from persistent events)
  if (projection.task_graph?.tasks) {
    taskGraphState.tasks = projection.task_graph.tasks;
    taskGraphState.currentSessionId = sessionState.currentSessionId;
  }
}

/**
 * Reset projection to empty state (used before switching sessions).
 */
export function resetProjection() {
  sessionState.projection = {
    messages: [],
    task_titles: [],
    task_graph: { tasks: [] },
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
        clearTaskGraph();
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
        clearTaskGraph();
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
