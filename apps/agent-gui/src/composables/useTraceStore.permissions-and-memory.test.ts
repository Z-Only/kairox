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
  // 11. Permission lifecycle
  // -----------------------------------------------------------------------
  describe("Permission lifecycle", () => {
    it("PermissionRequested creates a pending entry", () => {
      applyTraceEvent(
        makeEvent({
          type: "PermissionRequested",
          request_id: "perm-1",
          tool_id: "shell",
          preview: "rm -rf /tmp/test"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.id).toBe("perm-1");
      expect(entry.kind).toBe("permission");
      expect(entry.status).toBe("pending");
      expect(entry.toolId).toBe("shell");
      expect(entry.title).toBe("rm -rf /tmp/test");
      expect(entry.expanded).toBe(true);
    });

    it("PermissionRequested uses tool_id as title when preview is empty", () => {
      applyTraceEvent(
        makeEvent({
          type: "PermissionRequested",
          request_id: "perm-2",
          tool_id: "shell",
          preview: ""
        })
      );

      expect(traceState.entries[0].title).toBe("shell");
    });

    it("PermissionGranted updates to completed", () => {
      applyTraceEvent(
        makeEvent({
          type: "PermissionRequested",
          request_id: "perm-1",
          tool_id: "shell",
          preview: "ls"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "PermissionGranted",
          request_id: "perm-1"
        })
      );

      expect(traceState.entries[0].status).toBe("completed");
    });

    it("PermissionDenied updates to failed", () => {
      applyTraceEvent(
        makeEvent({
          type: "PermissionRequested",
          request_id: "perm-1",
          tool_id: "shell",
          preview: "rm -rf /"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "PermissionDenied",
          request_id: "perm-1",
          reason: "Dangerous command"
        })
      );

      expect(traceState.entries[0].status).toBe("failed");
    });
  });

  // -----------------------------------------------------------------------
  // 12. Memory lifecycle
  // -----------------------------------------------------------------------
  describe("Memory lifecycle", () => {
    it("MemoryProposed creates a pending entry with scope and content", () => {
      applyTraceEvent(
        makeEvent({
          type: "MemoryProposed",
          memory_id: "mem-1",
          scope: "Workspace",
          key: "project-style",
          content: "Use tabs for indentation"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.id).toBe("mem-1");
      expect(entry.kind).toBe("memory");
      expect(entry.status).toBe("pending");
      expect(entry.toolId).toBe("memory.store");
      expect(entry.title).toBe("Save Workspace memory");
      expect(entry.scope).toBe("Workspace");
      expect(entry.content).toBe("Use tabs for indentation");
      expect(entry.expanded).toBe(true);
    });

    it("MemoryAccepted updates to completed", () => {
      applyTraceEvent(
        makeEvent({
          type: "MemoryProposed",
          memory_id: "mem-1",
          scope: "User",
          key: null,
          content: "I prefer dark mode"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "MemoryAccepted",
          memory_id: "mem-1",
          scope: "User",
          key: null,
          content: "I prefer dark mode"
        })
      );

      expect(traceState.entries[0].status).toBe("completed");
    });

    it("MemoryRejected updates to failed with reason", () => {
      applyTraceEvent(
        makeEvent({
          type: "MemoryProposed",
          memory_id: "mem-1",
          scope: "Session",
          key: null,
          content: "temp note"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "MemoryRejected",
          memory_id: "mem-1",
          reason: "Not relevant"
        })
      );

      expect(traceState.entries[0].status).toBe("failed");
      expect(traceState.entries[0].reason).toBe("Not relevant");
    });
  });
});
