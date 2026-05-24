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
  //
  // UserMessageAdded events used to push a pseudo-tool entry with
  // `toolId: "user"` so the trace timeline could double as a chat log.
  // Now that ChatPanel renders messages directly from the session
  // projection via `useChatStream`, those pseudo entries are pure noise
  // in the unified feed and have been removed from the trace store.
  // -----------------------------------------------------------------------
  describe("UserMessageAdded", () => {
    it("does not push an entry into the trace store", () => {
      applyTraceEvent(
        makeEvent({
          type: "UserMessageAdded",
          message_id: "msg-1",
          content: "Hello world"
        })
      );

      expect(traceState.entries).toHaveLength(0);
    });

    it("is a no-op even for long content", () => {
      applyTraceEvent(
        makeEvent({
          type: "UserMessageAdded",
          message_id: "msg-2",
          content: "a".repeat(500)
        })
      );

      expect(traceState.entries).toHaveLength(0);
    });

    it("does not leave behind a duplicate-blocking ID", () => {
      // Apply UserMessageAdded twice with the same message_id, then
      // try to apply a real trace-producing event using the same id.
      // Because UserMessageAdded never registers an ID, AgentTaskCreated
      // re-using that id should still succeed.
      const sharedId = "msg-shared";
      applyTraceEvent(
        makeEvent({
          type: "UserMessageAdded",
          message_id: sharedId,
          content: "first"
        })
      );
      applyTraceEvent(
        makeEvent({
          type: "UserMessageAdded",
          message_id: sharedId,
          content: "second"
        })
      );
      applyTraceEvent(
        makeEvent({
          type: "AgentTaskCreated",
          task_id: sharedId,
          title: "Reusing id",
          role: "Worker",
          dependencies: []
        })
      );

      expect(traceState.entries).toHaveLength(1);
      expect(traceState.entries[0].toolId).toBe("task");
    });
  });
});
