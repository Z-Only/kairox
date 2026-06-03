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
  // 7. ModelToolCallRequested
  // -----------------------------------------------------------------------
  describe("ModelToolCallRequested", () => {
    it("creates a running tool call entry", () => {
      applyTraceEvent(
        makeEvent({
          type: "ModelToolCallRequested",
          tool_call_id: "tc-1",
          tool_id: "shell"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.id).toBe("tool-tc-1");
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("running");
      expect(entry.toolId).toBe("shell");
      expect(entry.title).toBe("Tool call: shell");
    });
  });

  // -----------------------------------------------------------------------
  // 8. ToolInvocationStarted
  // -----------------------------------------------------------------------
  describe("ToolInvocationStarted", () => {
    it("creates a running invocation entry", () => {
      applyTraceEvent(
        makeEvent({
          type: "ToolInvocationStarted",
          invocation_id: "inv-1",
          tool_id: "fs_read"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.id).toBe("tool-inv-1");
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("running");
      expect(entry.toolId).toBe("fs_read");
      expect(entry.title).toBe("fs_read");
    });
  });

  // -----------------------------------------------------------------------
  // 9. ToolInvocationCompleted
  // -----------------------------------------------------------------------
  describe("ToolInvocationCompleted", () => {
    it("updates a running invocation to completed", () => {
      applyTraceEvent(
        makeEvent({
          type: "ToolInvocationStarted",
          invocation_id: "inv-1",
          tool_id: "shell"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "ToolInvocationCompleted",
          invocation_id: "inv-1",
          tool_id: "shell",
          output_preview: "done",
          exit_code: 0,
          duration_ms: 120,
          truncated: false
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.status).toBe("completed");
      expect(entry.durationMs).toBe(120);
      expect(entry.outputPreview).toBe("done");
      expect(entry.exitCode).toBe(0);
      expect(entry.truncated).toBe(false);
    });

    it("does not crash when updating a missing entry", () => {
      expect(() => {
        applyTraceEvent(
          makeEvent({
            type: "ToolInvocationCompleted",
            invocation_id: "nonexistent",
            tool_id: "shell",
            output_preview: "",
            exit_code: null,
            duration_ms: 0,
            truncated: false
          })
        );
      }).not.toThrow();

      expect(traceState.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------------------
  // 10. ToolInvocationFailed
  // -----------------------------------------------------------------------
  describe("ToolInvocationFailed", () => {
    it("updates a running invocation to failed", () => {
      applyTraceEvent(
        makeEvent({
          type: "ToolInvocationStarted",
          invocation_id: "inv-fail",
          tool_id: "shell"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "ToolInvocationFailed",
          invocation_id: "inv-fail",
          tool_id: "shell",
          error: "Permission denied"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      expect(traceState.entries[0].status).toBe("failed");
    });

    it("does not crash when updating a missing entry", () => {
      expect(() => {
        applyTraceEvent(
          makeEvent({
            type: "ToolInvocationFailed",
            invocation_id: "nonexistent",
            tool_id: "shell",
            error: "boom"
          })
        );
      }).not.toThrow();

      expect(traceState.entries).toHaveLength(0);
    });
  });
});
