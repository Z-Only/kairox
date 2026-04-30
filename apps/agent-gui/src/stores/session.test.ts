import { describe, it, expect, beforeEach } from "vitest";
import {
  sessionState,
  applyEvent,
  setProjection,
  resetProjection
} from "./session";
import type { DomainEvent } from "../types";

beforeEach(() => {
  sessionState.sessions = [];
  sessionState.currentSessionId = null;
  sessionState.isStreaming = false;
  sessionState.connected = false;
  resetProjection();
});

describe("applyEvent", () => {
  it("projects UserMessageAdded", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "UserMessageAdded",
      payload: { type: "UserMessageAdded", message_id: "m1", content: "hello" }
    } as DomainEvent);

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("user");
    expect(sessionState.projection.messages[0].content).toBe("hello");
    expect(sessionState.isStreaming).toBe(true);
  });

  it("accumulates ModelTokenDelta into token_stream", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "ModelTokenDelta",
      payload: { type: "ModelTokenDelta", delta: "hel" }
    } as DomainEvent);
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:01Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "ModelTokenDelta",
      payload: { type: "ModelTokenDelta", delta: "lo" }
    } as DomainEvent);

    expect(sessionState.projection.token_stream).toBe("hello");
  });

  it("finalizes on AssistantMessageCompleted", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "AssistantMessageCompleted",
      payload: {
        type: "AssistantMessageCompleted",
        message_id: "m2",
        content: "hi there"
      }
    } as DomainEvent);

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("assistant");
    expect(sessionState.projection.messages[0].content).toBe("hi there");
    expect(sessionState.projection.token_stream).toBe("");
    expect(sessionState.isStreaming).toBe(false);
  });

  it("marks cancelled on SessionCancelled", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "SessionCancelled",
      payload: { type: "SessionCancelled", reason: "user stopped" }
    } as DomainEvent);

    expect(sessionState.projection.cancelled).toBe(true);
    expect(sessionState.isStreaming).toBe(false);
  });

  it("ignores unknown event types gracefully", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "FutureEvent",
      payload: { type: "FutureEvent" }
    } as DomainEvent);

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
      cancelled: false
    });

    expect(sessionState.projection.messages).toHaveLength(2);
    expect(sessionState.isStreaming).toBe(false);
  });
});

describe("resetProjection", () => {
  it("clears all projection state", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "UserMessageAdded",
      payload: { type: "UserMessageAdded", message_id: "m1", content: "hi" }
    } as DomainEvent);

    resetProjection();

    expect(sessionState.projection.messages).toHaveLength(0);
    expect(sessionState.projection.token_stream).toBe("");
    expect(sessionState.projection.cancelled).toBe(false);
    expect(sessionState.isStreaming).toBe(false);
  });
});
