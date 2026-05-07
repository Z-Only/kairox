import { describe, it, expect, beforeEach } from "vitest";
import {
  agentState,
  applyAgentEvent,
  clearAgents,
  agentLabel,
  runningAgents,
  agentsByRole,
  agentCountsByRole
} from "./agents";
import type { DomainEvent } from "../types";

beforeEach(() => {
  clearAgents();
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
    const event = makeAgentSpawnedEvent("agent_1", "Worker", "task_1");
    applyAgentEvent(event.payload);

    const agent = agentState.agents.get("agent_1");
    expect(agent).toBeDefined();
    expect(agent!.id).toBe("agent_1");
    expect(agent!.role).toBe("Worker");
    expect(agent!.taskId).toBe("task_1");
    expect(agent!.status).toBe("running");
    expect(agent!.completedAt).toBeNull();
  });

  it("handles AgentIdle event", () => {
    const spawned = makeAgentSpawnedEvent("agent_1", "Planner", "task_1");
    applyAgentEvent(spawned.payload);

    const idle = makeAgentIdleEvent("agent_1");
    applyAgentEvent(idle.payload);

    const agent = agentState.agents.get("agent_1");
    expect(agent!.status).toBe("idle");
    expect(agent!.completedAt).not.toBeNull();
  });

  it("marks agent as completed on AgentTaskCompleted", () => {
    const spawned = makeAgentSpawnedEvent("agent_2", "Worker", "task_1");
    applyAgentEvent(spawned.payload);

    applyAgentEvent({
      type: "AgentTaskCompleted",
      task_id: "task_1"
    });

    const agent = agentState.agents.get("agent_2");
    expect(agent!.status).toBe("completed");
  });

  it("marks agent as failed on AgentTaskFailed", () => {
    applyAgentEvent(makeAgentSpawnedEvent("agent_3", "Worker", "task_2").payload);

    applyAgentEvent({
      type: "AgentTaskFailed",
      task_id: "task_2",
      error: "Model error"
    });

    const agent = agentState.agents.get("agent_3");
    expect(agent!.status).toBe("failed");
  });

  it("resets agent to running on TaskRetried", () => {
    applyAgentEvent(makeAgentSpawnedEvent("agent_4", "Worker", "task_3").payload);

    // Fail it first
    applyAgentEvent({
      type: "AgentTaskFailed",
      task_id: "task_3",
      error: "Oops"
    });
    expect(agentState.agents.get("agent_4")!.status).toBe("failed");

    // Retry
    applyAgentEvent({ type: "TaskRetried", task_id: "task_3", attempt: 1 });
    expect(agentState.agents.get("agent_4")!.status).toBe("running");
    expect(agentState.agents.get("agent_4")!.completedAt).toBeNull();
  });

  it("ignores unknown event types", () => {
    applyAgentEvent({
      type: "UserMessageAdded",
      message_id: "m1",
      content: "hi"
    });
    expect(agentState.agents.size).toBe(0);
  });
});

describe("runningAgents computed", () => {
  it("returns only running agents", () => {
    applyAgentEvent(makeAgentSpawnedEvent("a1", "Worker", "t1").payload);
    applyAgentEvent(makeAgentSpawnedEvent("a2", "Worker", "t2").payload);
    applyAgentEvent(makeAgentIdleEvent("a2").payload);

    expect(runningAgents.value).toHaveLength(1);
    expect(runningAgents.value[0].id).toBe("a1");
  });
});

describe("agentsByRole computed", () => {
  it("groups agents by role", () => {
    applyAgentEvent(makeAgentSpawnedEvent("p1", "Planner", "t0").payload);
    applyAgentEvent(makeAgentSpawnedEvent("w1", "Worker", "t1").payload);
    applyAgentEvent(makeAgentSpawnedEvent("w2", "Worker", "t2").payload);

    const byRole = agentsByRole.value;
    expect(byRole.get("Planner")).toHaveLength(1);
    expect(byRole.get("Worker")).toHaveLength(2);
  });
});

describe("agentCountsByRole computed", () => {
  it("counts agents per role", () => {
    applyAgentEvent(makeAgentSpawnedEvent("p1", "Planner", "t0").payload);
    applyAgentEvent(makeAgentSpawnedEvent("w1", "Worker", "t1").payload);
    applyAgentEvent(makeAgentSpawnedEvent("w2", "Worker", "t2").payload);
    applyAgentEvent(makeAgentSpawnedEvent("r1", "Reviewer", "t3").payload);

    const counts = agentCountsByRole.value;
    expect(counts.get("Planner")).toBe(1);
    expect(counts.get("Worker")).toBe(2);
    expect(counts.get("Reviewer")).toBe(1);
  });
});

describe("agentLabel", () => {
  it("returns P for single planner", () => {
    applyAgentEvent(makeAgentSpawnedEvent("p1", "Planner", "t0").payload);
    expect(agentLabel("p1")).toBe("P");
  });

  it("returns W:1, W:2 for multiple workers", () => {
    applyAgentEvent(makeAgentSpawnedEvent("w1", "Worker", "t1").payload);
    applyAgentEvent(makeAgentSpawnedEvent("w2", "Worker", "t2").payload);

    expect(agentLabel("w1")).toBe("W:1");
    expect(agentLabel("w2")).toBe("W:2");
  });

  it("returns ? for unknown agent", () => {
    expect(agentLabel("unknown")).toBe("?");
  });
});

describe("clearAgents", () => {
  it("clears all agents", () => {
    applyAgentEvent(makeAgentSpawnedEvent("a1", "Worker", "t1").payload);
    clearAgents();
    expect(agentState.agents.size).toBe(0);
  });
});
