import { describe, it, expect, beforeEach } from "vitest";
import { traceState, applyTraceEvent, clearTrace } from "./useTraceStore";
import type { DomainEvent } from "../types";

// ---------------------------------------------------------------------------
// Helper: build a DomainEvent with sensible defaults
// ---------------------------------------------------------------------------

function makeEvent(payload: DomainEvent["payload"]): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "ws-1",
    session_id: "sess-1",
    timestamp: new Date().toISOString(),
    source_agent_id: "agent-1",
    privacy: "full_trace",
    event_type: payload.type,
    payload
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("useTraceStore", () => {
  beforeEach(() => {
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

  // -----------------------------------------------------------------------
  // 3. ContextAssembled
  // -----------------------------------------------------------------------
  describe("ContextAssembled", () => {
    it("creates a completed entry with token estimate and sources as outputPreview", () => {
      applyTraceEvent(
        makeEvent({
          type: "ContextAssembled",
          token_estimate: 1500,
          sources: ["file-a.rs", "file-b.ts"]
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("completed");
      expect(entry.toolId).toBe("context");
      expect(entry.title).toBe("Context assembled (1500 tokens)");
      expect(entry.outputPreview).toBe("file-a.rs, file-b.ts");
    });

    it("generates unique IDs for multiple ContextAssembled events", async () => {
      applyTraceEvent(
        makeEvent({
          type: "ContextAssembled",
          token_estimate: 100,
          sources: []
        })
      );

      // Small delay to ensure different Date.now()
      await new Promise((r) => setTimeout(r, 2));

      applyTraceEvent(
        makeEvent({
          type: "ContextAssembled",
          token_estimate: 200,
          sources: []
        })
      );

      expect(traceState.entries).toHaveLength(2);
      expect(traceState.entries[0].id).not.toBe(traceState.entries[1].id);
    });
  });

  // -----------------------------------------------------------------------
  // 4. ModelRequestStarted
  // -----------------------------------------------------------------------
  describe("ModelRequestStarted", () => {
    it("creates a running entry with model info", () => {
      applyTraceEvent(
        makeEvent({
          type: "ModelRequestStarted",
          model_profile: "gpt-4",
          model_id: "gpt-4-0613"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("running");
      expect(entry.toolId).toBe("model");
      expect(entry.title).toBe("Model: gpt-4 / gpt-4-0613");
    });
  });

  // -----------------------------------------------------------------------
  // 5. ModelTokenDelta
  // -----------------------------------------------------------------------
  describe("ModelTokenDelta", () => {
    it("is skipped — no entry created", () => {
      applyTraceEvent(
        makeEvent({
          type: "ModelTokenDelta",
          delta: "Hello"
        })
      );

      expect(traceState.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------------------
  // 6. AssistantMessageCompleted
  // -----------------------------------------------------------------------
  describe("AssistantMessageCompleted", () => {
    it("updates running model entry to completed", () => {
      applyTraceEvent(
        makeEvent({
          type: "ModelRequestStarted",
          model_profile: "gpt-4",
          model_id: "gpt-4-0613"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "AssistantMessageCompleted",
          message_id: "asst-1",
          content: "Here is the answer"
        })
      );

      // Model entry should be updated, no new assistant entry created
      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.status).toBe("completed");
      expect(entry.outputPreview).toBe("Here is the answer");
      expect(entry.durationMs).toBeGreaterThanOrEqual(0);
    });

    it("creates assistant entry when no running model exists", () => {
      applyTraceEvent(
        makeEvent({
          type: "AssistantMessageCompleted",
          message_id: "asst-2",
          content: "Standalone response"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.id).toBe("asst-2");
      expect(entry.toolId).toBe("assistant");
      expect(entry.status).toBe("completed");
      expect(entry.title).toBe("Assistant response");
      expect(entry.outputPreview).toBe("Standalone response");
    });

    it("deduplicates assistant entry by message_id", () => {
      // No running model, so assistant entry is created
      const event = makeEvent({
        type: "AssistantMessageCompleted",
        message_id: "asst-dup",
        content: "Response"
      });

      applyTraceEvent(event);
      applyTraceEvent(event);

      expect(traceState.entries).toHaveLength(1);
    });

    it("truncates outputPreview at 200 chars", () => {
      const longContent = "x".repeat(201);
      applyTraceEvent(
        makeEvent({
          type: "AssistantMessageCompleted",
          message_id: "asst-long",
          content: longContent
        })
      );

      const entry = traceState.entries[0];
      expect(entry.outputPreview).toBe("x".repeat(200));
    });
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
      expect(entry.id).toBe("tc-1");
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
      expect(entry.id).toBe("inv-1");
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
          token_estimate: 500,
          sources: ["src/main.rs"]
        })
      );

      expect(traceState.entries).toHaveLength(3);
      expect(traceState.entries[0].id).toBe("t-1");
      expect(traceState.entries[1].id).toBe("msg-1");
      // ContextAssembled has generated ID, just check it exists
      expect(traceState.entries[2].toolId).toBe("context");
    });
  });
});
