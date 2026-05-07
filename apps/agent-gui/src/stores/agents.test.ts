import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useAgentsStore } from "@/stores/agents";
import type { DomainEvent } from "@/types";

beforeEach(() => {
  setActivePinia(createPinia());
});

function makeAgentSpawnedEvent(agentId: string, role: string, taskId: string): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "wrk_1",
    session_id: "ses_1",
    timestamp: "2026-05-06T00:00:00Z",
    source_agent_id: "agent_system",
    privacy: "full_trace",
    event_type: "AgentSpawned",
    payload: {
      type: "AgentSpawned",
      agent_id: agentId,
      role,
      task_id: taskId
    }
  } as DomainEvent;
}

function makeAgentIdleEvent(agentId: string): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "wrk_1",
    session_id: "ses_1",
    timestamp: "2026-05-06T00:00:01Z",
    source_agent_id: agentId,
    privacy: "full_trace",
    event_type: "AgentIdle",
    payload: { type: "AgentIdle", agent_id: agentId }
  } as DomainEvent;
}

describe("applyAgentEvent", () => {
  it("handles AgentSpawned event", () => {
    const store = useAgentsStore();
    const event = makeAgentSpawnedEvent("agent_1", "Worker", "task_1");
    store.applyAgentEvent(event.payload);

    const agent = store.agents.get("agent_1");
    expect(agent).toBeDefined();
    expect(agent!.id).toBe("agent_1");
    expect(agent!.role).toBe("Worker");
    expect(agent!.taskId).toBe("task_1");
    expect(agent!.status).toBe("running");
    expect(agent!.completedAt).toBeNull();
  });

  it("handles AgentIdle event", () => {
    const store = useAgentsStore();
    const spawned = makeAgentSpawnedEvent("agent_1", "Planner", "task_1");
    store.applyAgentEvent(spawned.payload);

    const idle = makeAgentIdleEvent("agent_1");
    store.applyAgentEvent(idle.payload);

    const agent = store.agents.get("agent_1");
    expect(agent!.status).toBe("idle");
    expect(agent!.completedAt).not.toBeNull();
  });

  it("marks agent as completed on AgentTaskCompleted", () => {
    const store = useAgentsStore();
    const spawned = makeAgentSpawnedEvent("agent_2", "Worker", "task_1");
    store.applyAgentEvent(spawned.payload);

    store.applyAgentEvent({
      type: "AgentTaskCompleted",
      task_id: "task_1"
    });

    const agent = store.agents.get("agent_2");
    expect(agent!.status).toBe("completed");
  });

  it("marks agent as failed on AgentTaskFailed", () => {
    const store = useAgentsStore();
    store.applyAgentEvent(makeAgentSpawnedEvent("agent_3", "Worker", "task_2").payload);

    store.applyAgentEvent({
      type: "AgentTaskFailed",
      task_id: "task_2",
      error: "Model error"
    });

    const agent = store.agents.get("agent_3");
    expect(agent!.status).toBe("failed");
  });

  it("resets agent to running on TaskRetried", () => {
    const store = useAgentsStore();
    store.applyAgentEvent(makeAgentSpawnedEvent("agent_4", "Worker", "task_3").payload);

    store.applyAgentEvent({
      type: "AgentTaskFailed",
      task_id: "task_3",
      error: "Oops"
    });
    expect(store.agents.get("agent_4")!.status).toBe("failed");

    store.applyAgentEvent({
      type: "TaskRetried",
      task_id: "task_3",
      attempt: 1
    });
    expect(store.agents.get("agent_4")!.status).toBe("running");
    expect(store.agents.get("agent_4")!.completedAt).toBeNull();
  });

  it("ignores unknown event types", () => {
    const store = useAgentsStore();
    store.applyAgentEvent({
      type: "UserMessageAdded",
      message_id: "m1",
      content: "hi"
    });
    expect(store.agents.size).toBe(0);
  });
});

describe("runningAgents computed", () => {
  it("returns only running agents", () => {
    const store = useAgentsStore();
    store.applyAgentEvent(makeAgentSpawnedEvent("a1", "Worker", "t1").payload);
    store.applyAgentEvent(makeAgentSpawnedEvent("a2", "Worker", "t2").payload);
    store.applyAgentEvent(makeAgentIdleEvent("a2").payload);

    expect(store.runningAgents).toHaveLength(1);
    expect(store.runningAgents[0].id).toBe("a1");
  });
});

describe("agentsByRole computed", () => {
  it("groups agents by role", () => {
    const store = useAgentsStore();
    store.applyAgentEvent(makeAgentSpawnedEvent("p1", "Planner", "t0").payload);
    store.applyAgentEvent(makeAgentSpawnedEvent("w1", "Worker", "t1").payload);
    store.applyAgentEvent(makeAgentSpawnedEvent("w2", "Worker", "t2").payload);

    const byRole = store.agentsByRole;
    expect(byRole.get("Planner")).toHaveLength(1);
    expect(byRole.get("Worker")).toHaveLength(2);
  });
});

describe("agentCountsByRole computed", () => {
  it("counts agents per role", () => {
    const store = useAgentsStore();
    store.applyAgentEvent(makeAgentSpawnedEvent("p1", "Planner", "t0").payload);
    store.applyAgentEvent(makeAgentSpawnedEvent("w1", "Worker", "t1").payload);
    store.applyAgentEvent(makeAgentSpawnedEvent("w2", "Worker", "t2").payload);
    store.applyAgentEvent(makeAgentSpawnedEvent("r1", "Reviewer", "t3").payload);

    const counts = store.agentCountsByRole;
    expect(counts.get("Planner")).toBe(1);
    expect(counts.get("Worker")).toBe(2);
    expect(counts.get("Reviewer")).toBe(1);
  });
});

describe("agentLabel", () => {
  it("returns P for single planner", () => {
    const store = useAgentsStore();
    store.applyAgentEvent(makeAgentSpawnedEvent("p1", "Planner", "t0").payload);
    expect(store.agentLabel("p1")).toBe("P");
  });

  it("returns W:1, W:2 for multiple workers", () => {
    const store = useAgentsStore();
    store.applyAgentEvent(makeAgentSpawnedEvent("w1", "Worker", "t1").payload);
    store.applyAgentEvent(makeAgentSpawnedEvent("w2", "Worker", "t2").payload);

    expect(store.agentLabel("w1")).toBe("W:1");
    expect(store.agentLabel("w2")).toBe("W:2");
  });

  it("returns ? for unknown agent", () => {
    const store = useAgentsStore();
    expect(store.agentLabel("unknown")).toBe("?");
  });
});

describe("clearAgents", () => {
  it("clears all agents", () => {
    const store = useAgentsStore();
    store.applyAgentEvent(makeAgentSpawnedEvent("a1", "Worker", "t1").payload);
    store.clearAgents();
    expect(store.agents.size).toBe(0);
  });
});
