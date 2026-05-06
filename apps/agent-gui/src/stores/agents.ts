import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { EventPayload, AgentRole } from "@/types";

export type AgentStatus = "idle" | "running" | "completed" | "failed";

export interface AgentInfo {
  /** Unique agent identifier (from Rust AgentId). */
  id: string;
  /** The agent's role in the DAG execution. */
  role: AgentRole;
  /** The task this agent is assigned to, or null if idle. */
  taskId: string | null;
  /** Current execution status. */
  status: AgentStatus;
  /** Unix timestamp (ms) when the agent was spawned. */
  startedAt: number;
  /** Unix timestamp (ms) when the agent went idle/finished, or null. */
  completedAt: number | null;
}

export const useAgentsStore = defineStore("agents", () => {
  const agents = ref(new Map<string, AgentInfo>());

  /** Agents currently in the "running" state. */
  const runningAgents = computed(() =>
    [...agents.value.values()].filter((a) => a.status === "running")
  );

  /** Group agents by their role. */
  const agentsByRole = computed(() => {
    const map = new Map<AgentRole, AgentInfo[]>();
    for (const agent of agents.value.values()) {
      const list = map.get(agent.role) || [];
      list.push(agent);
      map.set(agent.role, list);
    }
    return map;
  });

  /** Count of agents per role. */
  const agentCountsByRole = computed(() => {
    const map = new Map<AgentRole, number>();
    for (const agent of agents.value.values()) {
      map.set(agent.role, (map.get(agent.role) || 0) + 1);
    }
    return map;
  });

  /**
   * Apply an event payload to the agent store.
   * Handles AgentSpawned, AgentIdle, TaskRetried, and task state changes
   * that affect agent attribution.
   */
  function applyAgentEvent(payload: EventPayload) {
    switch (payload.type) {
      case "AgentSpawned": {
        const role = payload.role as AgentRole;
        agents.value.set(payload.agent_id, {
          id: payload.agent_id,
          role,
          taskId: payload.task_id || null,
          status: "running",
          startedAt: Date.now(),
          completedAt: null
        });
        break;
      }
      case "AgentIdle": {
        const agent = agents.value.get(payload.agent_id);
        if (agent) {
          agent.status = "idle";
          agent.completedAt = Date.now();
        }
        break;
      }
      case "AgentTaskFailed": {
        for (const agent of agents.value.values()) {
          if (agent.taskId === payload.task_id && agent.status === "running") {
            agent.status = "failed";
            agent.completedAt = Date.now();
            break;
          }
        }
        break;
      }
      case "AgentTaskCompleted": {
        for (const agent of agents.value.values()) {
          if (agent.taskId === payload.task_id && agent.status === "running") {
            agent.status = "completed";
            agent.completedAt = Date.now();
            break;
          }
        }
        break;
      }
      case "TaskRetried": {
        for (const agent of agents.value.values()) {
          if (agent.taskId === payload.task_id && agent.status === "failed") {
            agent.status = "running";
            agent.completedAt = null;
            break;
          }
        }
        break;
      }
    }
  }

  /** Clear all agent state (used on session switch). */
  function clearAgents() {
    agents.value.clear();
  }

  /**
   * Generate a human-readable label for an agent.
   * Includes the role abbreviation and a sequential number per role.
   * E.g., "P" for primary planner, "W:1" for first worker, "W:2" for second, "R" for reviewer.
   */
  function agentLabel(agentId: string): string {
    const agent = agents.value.get(agentId);
    if (!agent) return "?";

    const roleAbbr: Record<string, string> = {
      Planner: "P",
      Worker: "W",
      Reviewer: "R"
    };

    const abbr = roleAbbr[agent.role] || agent.role.charAt(0);

    const sameRoleAgents = [...agents.value.values()]
      .filter((a) => a.role === agent.role)
      .sort((a, b) => a.startedAt - b.startedAt);

    if (sameRoleAgents.length <= 1) {
      return abbr;
    }

    const index = sameRoleAgents.findIndex((a) => a.id === agentId) + 1;
    return `${abbr}:${index}`;
  }

  return {
    agents,
    runningAgents,
    agentsByRole,
    agentCountsByRole,
    applyAgentEvent,
    clearAgents,
    agentLabel
  };
});
