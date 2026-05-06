import { onMounted, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { DomainEvent, TaskState } from "@/types";
import { useSessionStore } from "@/stores/session";
import { applyTraceEvent } from "./useTraceStore";
import { useTaskGraphStore } from "@/stores/taskGraph";
import { useUiStore } from "@/stores/ui";
import { useMcpStore } from "@/stores/mcp";
import { useAgentsStore } from "@/stores/agents";
import { useCatalogStore } from "@/stores/catalog";

export function useTauriEvents() {
  const session = useSessionStore();
  const taskGraph = useTaskGraphStore();
  const ui = useUiStore();
  const mcp = useMcpStore();
  const agents = useAgentsStore();
  const catalog = useCatalogStore();

  let unlisten: (() => void) | null = null;

  onMounted(async () => {
    unlisten = await listen<DomainEvent>("session-event", (tauriEvent) => {
      const domainEvent = tauriEvent.payload;
      const sessionId: string | undefined = domainEvent.session_id;
      if (
        sessionId &&
        session.currentSessionId &&
        sessionId === session.currentSessionId
      ) {
        session.applyEvent(domainEvent);
        applyTraceEvent(domainEvent);

        const p = domainEvent.payload;
        switch (p.type) {
          case "AgentTaskCreated": {
            if (!taskGraph.tasks.some((t) => t.id === p.task_id)) {
              taskGraph.tasks.push({
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
              if (taskGraph.currentSessionId === sessionId) {
                taskGraph.tasks = [...taskGraph.tasks];
              }
            }
            break;
          }
          case "AgentTaskStarted": {
            const task = taskGraph.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Running" as TaskState;
              taskGraph.tasks = [...taskGraph.tasks];
            }
            break;
          }
          case "AgentTaskCompleted": {
            const task = taskGraph.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Completed" as TaskState;
              taskGraph.tasks = [...taskGraph.tasks];
            }
            break;
          }
          case "AgentTaskFailed": {
            const task = taskGraph.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Failed" as TaskState;
              task.error = p.error;
              taskGraph.tasks = [...taskGraph.tasks];
            }
            if (p.error) {
              ui.pushNotification("error", p.error);
            }
            break;
          }
          case "TaskBlocked": {
            const task = taskGraph.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Blocked" as TaskState;
              task.error = p.reason || "Dependency failed";
              taskGraph.tasks = [...taskGraph.tasks];
            }
            break;
          }
          case "TaskDecomposed":
            break;
          case "TaskRetried": {
            const task = taskGraph.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Running" as TaskState;
              task.retry_count = p.attempt;
              task.error = null;
              taskGraph.tasks = [...taskGraph.tasks];
            }
            break;
          }
        }

        agents.applyAgentEvent(domainEvent.payload);
      }

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
          mcp.handleMcpEvent(payload);
          break;
        case "CatalogSourceAdded":
          void catalog.fetchSources();
          break;
        case "CatalogSourceFailed":
          catalog.handleSourceFailed(payload.source, payload.error);
          break;
      }
    });
    session.connected = true;
  });

  onUnmounted(() => {
    unlisten?.();
    session.connected = false;
  });
}
