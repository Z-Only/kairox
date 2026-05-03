import { onMounted, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { DomainEvent } from "../types";
import { sessionState, applyEvent } from "../stores/session";
import { applyTraceEvent } from "./useTraceStore";
import { refreshTaskGraph, taskGraphState } from "../stores/taskGraph";

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

        // Refresh task graph on AgentTask* events
        switch (domainEvent.payload.type) {
          case "AgentTaskCreated":
          case "AgentTaskStarted":
          case "AgentTaskCompleted":
          case "AgentTaskFailed":
            if (taskGraphState.currentSessionId === sessionId) {
              refreshTaskGraph(sessionId);
            }
            break;
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
