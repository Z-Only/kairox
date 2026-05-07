import { describe, it, expect, beforeEach } from "vitest";
import { sessionState, applyEvent, setProjection, resetProjection, streamsByTask } from "./session";
import type { DomainEvent, AgentRole, EventPayload } from "../types";
import { agentState, clearAgents } from "./agents";

beforeEach(() => {
  sessionState.sessions = [];
  sessionState.currentSessionId = null;
  sessionState.isStreaming = false;
  sessionState.connected = false;
  resetProjection();
  clearAgents();
});

function makeEvent(payload: EventPayload, sourceAgentId = "agent_system"): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "wrk_1",
    session_id: "ses_1",
    timestamp: "2026-05-06T00:00:00Z",
    source_agent_id: sourceAgentId,
    privacy: "full_trace",
    event_type: payload.type,
    payload
  } as DomainEvent;
}

describe("applyEvent", () => {
  it("projects UserMessageAdded", () => {
    applyEvent(
      makeEvent({
        type: "UserMessageAdded",
        message_id: "m1",
        content: "hello"
      })
    );

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("user");
    expect(sessionState.projection.messages[0].content).toBe("hello");
    expect(sessionState.isStreaming).toBe(true);
  });

  it("accumulates ModelTokenDelta into token_stream", () => {
    applyEvent(makeEvent({ type: "ModelTokenDelta", delta: "hel" }));
    applyEvent(makeEvent({ type: "ModelTokenDelta", delta: "lo" }));

    expect(sessionState.projection.token_stream).toBe("hello");
  });

  it("finalizes on AssistantMessageCompleted", () => {
    applyEvent(
      makeEvent({
        type: "AssistantMessageCompleted",
        message_id: "m2",
        content: "hi there"
      })
    );

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("assistant");
    expect(sessionState.projection.messages[0].content).toBe("hi there");
    expect(sessionState.projection.token_stream).toBe("");
    expect(sessionState.isStreaming).toBe(false);
  });

  it("attributes AssistantMessageCompleted to agent when source_agent_id is known", () => {
    agentState.agents.set("agent_w1", {
      id: "agent_w1",
      role: "Worker" as AgentRole,
      taskId: "t1",
      status: "running",
      startedAt: Date.now(),
      completedAt: null
    });

    applyEvent(
      makeEvent(
        {
          type: "AssistantMessageCompleted",
          message_id: "m3",
          content: "worker response"
        },
        "agent_w1"
      )
    );

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("worker");
    expect(sessionState.projection.messages[0].sourceAgentId).toBe("agent_w1");
  });

  it("marks cancelled on SessionCancelled", () => {
    applyEvent(makeEvent({ type: "SessionCancelled", reason: "user stopped" }));

    expect(sessionState.projection.cancelled).toBe(true);
    expect(sessionState.isStreaming).toBe(false);
  });

  it("handles TaskDecomposed event", () => {
    applyEvent(
      makeEvent({
        type: "TaskDecomposed",
        parent_task_id: "parent",
        sub_task_ids: ["sub1", "sub2", "sub3"]
      })
    );

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("system");
    expect(sessionState.projection.messages[0].content).toContain("3 sub-tasks");
  });

  it("handles TaskBlocked event", () => {
    applyEvent(
      makeEvent({
        type: "TaskBlocked",
        task_id: "t1",
        blocking_task_id: "t0",
        reason: "dependency failed"
      })
    );

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("system");
    expect(sessionState.projection.messages[0].content).toContain("blocked");
  });

  it("handles TaskRetried event", () => {
    applyEvent(
      makeEvent({
        type: "TaskRetried",
        task_id: "t1",
        attempt: 2
      })
    );

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("system");
    expect(sessionState.projection.messages[0].content).toContain("attempt 2");
  });

  it("ignores AgentSpawned and AgentIdle events gracefully", () => {
    applyEvent(
      makeEvent({
        type: "AgentSpawned",
        agent_id: "a1",
        role: "Worker",
        task_id: "t1"
      })
    );
    applyEvent(makeEvent({ type: "AgentIdle", agent_id: "a1" }));

    // These should not create messages (handled by agents store)
    expect(sessionState.projection.messages).toHaveLength(0);
  });

  it("ignores unknown event types gracefully", () => {
    applyEvent(makeEvent({ type: "FutureEvent" }));

    expect(sessionState.projection.messages).toHaveLength(0);
  });
});

describe("setProjection", () => {
  it("replaces the current projection", () => {
    setProjection({
      messages: [
        { role: "user", content: "existing" },
        { role: "assistant", content: "reply" }
      ],
      task_titles: ["task 1"],
      token_stream: "",
      cancelled: false,
      task_graph: { tasks: [] }
    });

    expect(sessionState.projection.messages).toHaveLength(2);
    expect(sessionState.isStreaming).toBe(false);
  });
});

describe("resetProjection", () => {
  it("clears all projection state and agent state", () => {
    applyEvent(makeEvent({ type: "UserMessageAdded", message_id: "m1", content: "hi" }));

    resetProjection();

    expect(sessionState.projection.messages).toHaveLength(0);
    expect(sessionState.projection.token_stream).toBe("");
    expect(sessionState.projection.cancelled).toBe(false);
    expect(sessionState.isStreaming).toBe(false);
    expect(streamsByTask.size).toBe(0);
  });
});
