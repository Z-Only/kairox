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
  // 13. clearTrace
  // -----------------------------------------------------------------------
  describe("clearTrace", () => {
    it("clears all entries and allows re-adding the same ID", () => {
      applyTraceEvent(
        makeEvent({
          type: "AgentTaskCreated",
          task_id: "t-1",
          title: "First",
          role: "Worker",
          dependencies: []
        })
      );

      expect(traceState.entries).toHaveLength(1);

      clearTrace();

      expect(traceState.entries).toHaveLength(0);

      // Re-adding the same ID should now succeed
      applyTraceEvent(
        makeEvent({
          type: "AgentTaskCreated",
          task_id: "t-1",
          title: "First again",
          role: "Worker",
          dependencies: []
        })
      );

      expect(traceState.entries).toHaveLength(1);
      expect(traceState.entries[0].title).toBe("First again");
    });

    it("clears when there are no entries (no-op)", () => {
      expect(() => clearTrace()).not.toThrow();
      expect(traceState.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------------------
  // 14. density state
  // -----------------------------------------------------------------------
  describe("density state", () => {
    it('defaults to "L2"', () => {
      clearTrace();
      expect(traceState.density).toBe("L2");
    });

    it("can be mutated to L3", () => {
      traceState.density = "L3";
      expect(traceState.density).toBe("L3");
    });

    it("can be mutated to L1", () => {
      traceState.density = "L1";
      expect(traceState.density).toBe("L1");
    });
  });

  // -----------------------------------------------------------------------
  // Edge cases
  // -----------------------------------------------------------------------
  describe("edge cases", () => {
    it("unhandled event types do not create entries", () => {
      applyTraceEvent(
        makeEvent({
          type: "WorkspaceOpened",
          path: "/home/user/project"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "SessionInitialized",
          model_profile: "test"
        })
      );

      expect(traceState.entries).toHaveLength(0);
    });

    it("rawEvent is stored as valid JSON", () => {
      const event = makeEvent({
        type: "AgentTaskCreated",
        task_id: "t-raw",
        title: "Raw test",
        role: "Planner",
        dependencies: []
      });

      applyTraceEvent(event);

      const raw = traceState.entries[0].rawEvent;
      expect(raw).toBeDefined();
      expect(() => JSON.parse(raw!)).not.toThrow();
      const parsed = JSON.parse(raw!);
      expect(parsed.payload.type).toBe("AgentTaskCreated");
    });

    it("multiple events in sequence maintain correct order", () => {
      applyTraceEvent(
        makeEvent({
          type: "AgentTaskCreated",
          task_id: "t-1",
          title: "First task",
          role: "Worker",
          dependencies: []
        })
      );

      // UserMessageAdded is intentionally a no-op for the trace store
      // — ChatPanel renders user turns directly. The next visible entry
      // should come from ContextAssembled.
      applyTraceEvent(
        makeEvent({
          type: "UserMessageAdded",
          message_id: "msg-1",
          content: "Hello"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "ContextAssembled",
          usage: {
            total_tokens: 500,
            budget_tokens: 100_000,
            context_window: 128_000,
            output_reservation: 28_000,
            by_source: [["selected_file", 500]],
            estimator: "cl100k_base",
            corrected_by_real_usage: false
          }
        })
      );

      expect(traceState.entries).toHaveLength(2);
      expect(traceState.entries[0].id).toBe("t-1");
      // ContextAssembled has generated ID, just check it exists
      expect(traceState.entries[1].toolId).toBe("context");
    });
  });
});
