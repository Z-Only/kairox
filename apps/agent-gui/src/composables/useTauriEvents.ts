import { onMounted, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { DomainEvent, TaskState } from "../types";
import { sessionState, applyEvent } from "../stores/session";
import { applyTraceEvent } from "./useTraceStore";
import { taskGraphState } from "../stores/taskGraph";
import { addNotification } from "./useNotifications";
import { handleMcpEvent } from "../stores/mcp";
import { applyAgentEvent } from "../stores/agents";
import { fetchSources, handleSourceFailed } from "../stores/catalog";

export function useTauriEvents() {
  let unlisten: (() => void) | null = null;

  onMounted(async () => {
    unlisten = await listen<DomainEvent>("session-event", (tauriEvent) => {
      // Only process events for the current session.
      // DomainEvent has session_id at the top level.
      const domainEvent = tauriEvent.payload;
      const sessionId: string | undefined = domainEvent.session_id;
      if (
        sessionId &&
        sessionState.currentSessionId &&
        sessionId === sessionState.currentSessionId
      ) {
        applyEvent(domainEvent);
        applyTraceEvent(domainEvent);

        // Update task graph state from real-time events.
        // This mirrors the Rust SessionProjection::apply() logic
        // so the Tasks panel updates immediately without an async invoke.
        const p = domainEvent.payload;
        switch (p.type) {
          case "AgentTaskCreated": {
            // Only add if not already present (dedup against projection load)
            if (!taskGraphState.tasks.some((t) => t.id === p.task_id)) {
              taskGraphState.tasks.push({
                id: p.task_id,
                title: p.title,
                role: p.role,
                state: "Pending" as TaskState,
                dependencies: p.dependencies,
                error: null,
                retry_count: 0,
                max_retries: 3,
                assigned_agent_id: null,
                failure_reason: null
              });
              if (taskGraphState.currentSessionId === sessionId) {
                // Trigger reactivity
                taskGraphState.tasks = [...taskGraphState.tasks];
              }
            }
            break;
          }
          case "AgentTaskStarted": {
            const task = taskGraphState.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Running" as TaskState;
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
          case "AgentTaskCompleted": {
            const task = taskGraphState.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Completed" as TaskState;
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
          case "AgentTaskFailed": {
            const task = taskGraphState.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Failed" as TaskState;
              task.error = p.error;
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            if (p.error) {
              addNotification("error", p.error);
            }
            break;
          }
          case "TaskBlocked": {
            const task = taskGraphState.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Blocked" as TaskState;
              task.error = p.reason || "Dependency failed";
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
          case "TaskDecomposed": {
            // Task decomposition is informational — the sub-tasks
            // are created via separate AgentTaskCreated events
            break;
          }
          case "TaskRetried": {
            const task = taskGraphState.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Running" as TaskState;
              task.retry_count = p.attempt;
              task.error = null;
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
        }

        // Route agent lifecycle events to the agents store
        applyAgentEvent(domainEvent.payload);
      }

      // MCP events are not session-scoped — handle them regardless of session.
      const payload = domainEvent.payload;
      switch (payload.type) {
        case "McpServerStarting":
        case "McpServerReady":
        case "McpServerStopped":
        case "McpServerFailed":
        case "McpToolCallStarted":
        case "McpToolCallCompleted":
        case "McpTrustGranted":
        case "McpTrustRevoked":
          handleMcpEvent(payload);
          break;
        // Phase 2: catalog source lifecycle events are global, not session-scoped.
        case "CatalogSourceAdded":
          void fetchSources();
          break;
        case "CatalogSourceFailed":
          handleSourceFailed(payload.source, payload.error);
          break;
      }
    });
    sessionState.connected = true;
  });

  onUnmounted(() => {
    unlisten?.();
    sessionState.connected = false;
  });
}
