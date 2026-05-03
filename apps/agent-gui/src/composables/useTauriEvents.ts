import { onMounted, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { DomainEvent } from "../types";
import { sessionState, applyEvent } from "../stores/session";
import { applyTraceEvent } from "./useTraceStore";
import { taskGraphState } from "../stores/taskGraph";

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
            const typed = p as {
              type: "AgentTaskCreated";
              task_id: string;
              title: string;
              role: string;
              dependencies: string[];
            };
            // Only add if not already present (dedup against projection load)
            if (!taskGraphState.tasks.some((t) => t.id === typed.task_id)) {
              taskGraphState.tasks.push({
                id: typed.task_id,
                title: typed.title,
                role: typed.role as "Planner" | "Worker" | "Reviewer",
                state: "Pending",
                dependencies: typed.dependencies,
                error: null
              });
              if (taskGraphState.currentSessionId === sessionId) {
                // Trigger reactivity
                taskGraphState.tasks = [...taskGraphState.tasks];
              }
            }
            break;
          }
          case "AgentTaskStarted": {
            const typed = p as { type: "AgentTaskStarted"; task_id: string };
            const task = taskGraphState.tasks.find(
              (t) => t.id === typed.task_id
            );
            if (task) {
              task.state = "Running";
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
          case "AgentTaskCompleted": {
            const typed = p as {
              type: "AgentTaskCompleted";
              task_id: string;
            };
            const task = taskGraphState.tasks.find(
              (t) => t.id === typed.task_id
            );
            if (task) {
              task.state = "Completed";
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
          case "AgentTaskFailed": {
            const typed = p as {
              type: "AgentTaskFailed";
              task_id: string;
              error: string;
            };
            const task = taskGraphState.tasks.find(
              (t) => t.id === typed.task_id
            );
            if (task) {
              task.state = "Failed";
              task.error = typed.error;
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
        }
      }
    });
    sessionState.connected = true;
  });

  onUnmounted(() => {
    unlisten?.();
    sessionState.connected = false;
  });
}
