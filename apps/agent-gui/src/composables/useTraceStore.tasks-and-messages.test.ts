import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { traceState, applyTraceEvent, clearTrace } from "./useTraceStore";
import { makeEvent } from "./useTraceStore.test-utils";

describe("useTraceStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    clearTrace();
  });

  // -----------------------------------------------------------------------
  // 1. AgentTaskCreated
  // -----------------------------------------------------------------------
  describe("AgentTaskCreated", () => {
    it("creates a completed entry with task details", () => {
      applyTraceEvent(
        makeEvent({
          type: "AgentTaskCreated",
          task_id: "t-1",
          title: "Build feature",
          role: "Worker",
          dependencies: []
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.id).toBe("t-1");
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("completed");
      expect(entry.toolId).toBe("task");
      expect(entry.title).toBe("Build feature");
    });

    it("deduplicates by task_id", () => {
      const event = makeEvent({
        type: "AgentTaskCreated",
        task_id: "t-1",
        title: "Build feature",
        role: "Worker",
        dependencies: []
      });

      applyTraceEvent(event);
      applyTraceEvent(event);

      expect(traceState.entries).toHaveLength(1);
    });
  });

  // -----------------------------------------------------------------------
  // 2. UserMessageAdded
  // -----------------------------------------------------------------------
  describe("UserMessageAdded", () => {
    it("creates a completed entry with user prefix", () => {
      applyTraceEvent(
        makeEvent({
          type: "UserMessageAdded",
          message_id: "msg-1",
          content: "Hello world"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.id).toBe("msg-1");
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("completed");
      expect(entry.toolId).toBe("user");
      expect(entry.title).toBe("User: Hello world");
      expect(entry.input).toBe("Hello world");
    });

    it("truncates content at 80 chars with …", () => {
      const longContent = "a".repeat(81);
      applyTraceEvent(
        makeEvent({
          type: "UserMessageAdded",
          message_id: "msg-2",
          content: longContent
        })
      );

      const entry = traceState.entries[0];
      expect(entry.title).toBe(`User: ${"a".repeat(80)}…`);
      // input still has full content
      expect(entry.input).toBe(longContent);
    });

    it("does not add … for content at exactly 80 chars", () => {
      const exact = "b".repeat(80);
      applyTraceEvent(
        makeEvent({
          type: "UserMessageAdded",
          message_id: "msg-3",
          content: exact
        })
      );

      const entry = traceState.entries[0];
      expect(entry.title).not.toContain("…");
    });

    it("deduplicates by message_id", () => {
      const event = makeEvent({
        type: "UserMessageAdded",
        message_id: "msg-dup",
        content: "dup"
      });

      applyTraceEvent(event);
      applyTraceEvent(event);

      expect(traceState.entries).toHaveLength(1);
    });
  });
});
