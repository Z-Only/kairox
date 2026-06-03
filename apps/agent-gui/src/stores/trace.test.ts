import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useTraceStore } from "@/stores/trace";
import type { DomainEvent } from "@/types";

/** Build a minimal DomainEvent with only the payload — other envelope
 *  fields are filled with fixed stubs so tests stay focused. */
function mkEvent(payload: DomainEvent["payload"]): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "ws-1",
    session_id: "sess-1",
    timestamp: new Date().toISOString(),
    source_agent_id: "agent-1",
    privacy: "Internal",
    event_type: payload.type,
    payload
  };
}

describe("trace store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  // -----------------------------------------------------------
  // Initial state
  // -----------------------------------------------------------
  it("starts with empty entries and L2 density", () => {
    const trace = useTraceStore();
    expect(trace.entries).toEqual([]);
    expect(trace.density).toBe("L2");
  });

  // -----------------------------------------------------------
  // applyTraceEvent — AgentTaskCreated
  // -----------------------------------------------------------
  describe("AgentTaskCreated", () => {
    it("adds a completed tool entry", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "AgentTaskCreated",
          task_id: "t-1",
          title: "Plan subtasks",
          role: "Planner",
          dependencies: []
        })
      );
      expect(trace.entries).toHaveLength(1);
      const entry = trace.entries[0];
      expect(entry.id).toBe("t-1");
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("completed");
      expect(entry.toolId).toBe("task");
      expect(entry.title).toBe("Plan subtasks");
      expect(entry.rawEvent).toContain("AgentTaskCreated");
    });

    it("deduplicates entries with the same task_id", () => {
      const trace = useTraceStore();
      const event = mkEvent({
        type: "AgentTaskCreated",
        task_id: "t-dup",
        title: "Same task",
        role: "Worker",
        dependencies: []
      });
      trace.applyTraceEvent(event);
      trace.applyTraceEvent(event);
      expect(trace.entries).toHaveLength(1);
    });

    it("marks an existing task entry as failed on AgentTaskFailed", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "AgentTaskCreated",
          task_id: "t-cancelled",
          title: "Long model turn",
          role: "Planner",
          dependencies: []
        })
      );

      trace.applyTraceEvent(
        mkEvent({
          type: "AgentTaskFailed",
          task_id: "t-cancelled",
          agent_id: "agent-1",
          error: "cancelled by user"
        })
      );

      expect(trace.entries).toHaveLength(1);
      expect(trace.entries[0].status).toBe("failed");
      expect(trace.entries[0].reason).toBe("cancelled by user");
      expect(trace.entries[0].rawEvent).toContain("AgentTaskFailed");
    });
  });

  // -----------------------------------------------------------
  // applyTraceEvent — UserMessageAdded (no-op)
  // -----------------------------------------------------------
  describe("UserMessageAdded", () => {
    it("does not add an entry", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({ type: "UserMessageAdded", message_id: "msg-1", content: "hi" })
      );
      expect(trace.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------
  // applyTraceEvent — SessionCancelled
  // -----------------------------------------------------------
  describe("SessionCancelled", () => {
    it("adds a durable cancellation entry for the chat stream", () => {
      const trace = useTraceStore();

      trace.applyTraceEvent(
        mkEvent({ type: "SessionCancelled", reason: "user requested cancellation" })
      );

      expect(trace.entries).toHaveLength(1);
      expect(trace.entries[0]).toMatchObject({
        id: "session-cancelled-1",
        kind: "cancellation",
        status: "completed",
        toolId: "cancellation",
        title: "Session cancelled",
        reason: "user requested cancellation"
      });
    });
  });

  // -----------------------------------------------------------
  // applyTraceEvent — ContextAssembled
  // -----------------------------------------------------------
  describe("ContextAssembled", () => {
    it("adds a completed context entry with source labels", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "ContextAssembled",
          usage: {
            total_tokens: 1000,
            budget_tokens: 4000,
            context_window: 8000,
            output_reservation: 2000,
            by_source: [
              ["system", 400],
              ["history", 600]
            ],
            estimator: "tiktoken",
            corrected_by_real_usage: false
          }
        })
      );
      expect(trace.entries).toHaveLength(1);
      const entry = trace.entries[0];
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("completed");
      expect(entry.toolId).toBe("context");
      expect(entry.title).toContain("1000");
      expect(entry.title).toContain("4000");
      expect(entry.outputPreview).toContain("system:400");
      expect(entry.outputPreview).toContain("history:600");
    });
  });

  // -----------------------------------------------------------
  // applyTraceEvent — ModelRequestStarted / AssistantMessageCompleted
  // -----------------------------------------------------------
  describe("ModelRequestStarted → AssistantMessageCompleted", () => {
    it("adds a running model entry, then completes it", () => {
      const trace = useTraceStore();

      trace.applyTraceEvent(
        mkEvent({
          type: "ModelRequestStarted",
          model_profile: "sonnet",
          model_id: "claude-sonnet-4-20250514"
        })
      );
      expect(trace.entries).toHaveLength(1);
      expect(trace.entries[0].status).toBe("running");
      expect(trace.entries[0].title).toContain("sonnet");

      trace.applyTraceEvent(
        mkEvent({
          type: "AssistantMessageCompleted",
          message_id: "m-1",
          content: "Hello, world!"
        })
      );
      expect(trace.entries[0].status).toBe("completed");
      expect(trace.entries[0].durationMs).toBeDefined();
      expect(trace.entries[0].outputPreview).toBe("Hello, world!");
    });

    it("does nothing when no running model entry exists", () => {
      const trace = useTraceStore();
      // AssistantMessageCompleted without a preceding ModelRequestStarted
      trace.applyTraceEvent(
        mkEvent({
          type: "AssistantMessageCompleted",
          message_id: "m-orphan",
          content: "orphan"
        })
      );
      expect(trace.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------
  // applyTraceEvent — ModelTokenDelta (no-op)
  // -----------------------------------------------------------
  describe("ModelTokenDelta", () => {
    it("does not add an entry", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(mkEvent({ type: "ModelTokenDelta", delta: "tok" }));
      expect(trace.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------
  // applyTraceEvent — ModelToolCallRequested
  // -----------------------------------------------------------
  describe("ModelToolCallRequested", () => {
    it("adds a running tool-call entry", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "ModelToolCallRequested",
          tool_call_id: "tc-1",
          tool_id: "shell"
        })
      );
      expect(trace.entries).toHaveLength(1);
      expect(trace.entries[0].id).toBe("tool-tc-1");
      expect(trace.entries[0].status).toBe("running");
      expect(trace.entries[0].toolId).toBe("shell");
      expect(trace.entries[0].title).toContain("shell");
    });
  });

  // -----------------------------------------------------------
  // applyTraceEvent — ToolInvocationStarted / Completed / Failed
  // -----------------------------------------------------------
  describe("ToolInvocation lifecycle", () => {
    it("ToolInvocationStarted adds a running entry", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "ToolInvocationStarted",
          invocation_id: "inv-1",
          tool_id: "fs.read"
        })
      );
      expect(trace.entries).toHaveLength(1);
      expect(trace.entries[0].id).toBe("tool-inv-1");
      expect(trace.entries[0].status).toBe("running");
    });

    it("ToolInvocationCompleted updates existing entry", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "ToolInvocationStarted",
          invocation_id: "inv-2",
          tool_id: "shell"
        })
      );
      trace.applyTraceEvent(
        mkEvent({
          type: "ToolInvocationCompleted",
          invocation_id: "inv-2",
          tool_id: "shell",
          output_preview: "ok",
          exit_code: 0,
          duration_ms: 42,
          truncated: false
        })
      );
      expect(trace.entries).toHaveLength(1);
      expect(trace.entries[0].status).toBe("completed");
      expect(trace.entries[0].durationMs).toBe(42);
      expect(trace.entries[0].outputPreview).toBe("ok");
      expect(trace.entries[0].exitCode).toBe(0);
      expect(trace.entries[0].truncated).toBe(false);
    });

    it("ToolInvocationFailed marks entry as failed", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "ToolInvocationStarted",
          invocation_id: "inv-3",
          tool_id: "patch"
        })
      );
      trace.applyTraceEvent(
        mkEvent({
          type: "ToolInvocationFailed",
          invocation_id: "inv-3",
          tool_id: "patch",
          error: "conflict"
        })
      );
      expect(trace.entries[0].status).toBe("failed");
    });

    it("ToolInvocationCompleted is a no-op for unknown invocation_id", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "ToolInvocationCompleted",
          invocation_id: "ghost",
          tool_id: "shell",
          output_preview: "",
          exit_code: 0,
          duration_ms: 1,
          truncated: false
        })
      );
      expect(trace.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------
  // applyTraceEvent — Permission lifecycle
  // -----------------------------------------------------------
  describe("Permission lifecycle", () => {
    it("PermissionRequested adds a pending permission entry", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "PermissionRequested",
          request_id: "perm-1",
          tool_id: "shell",
          preview: "rm -rf /"
        })
      );
      expect(trace.entries).toHaveLength(1);
      const entry = trace.entries[0];
      expect(entry.id).toBe("perm-1");
      expect(entry.kind).toBe("permission");
      expect(entry.status).toBe("pending");
      expect(entry.expanded).toBe(true);
      expect(entry.title).toBe("rm -rf /");
    });

    it("keeps permission prompts separate from same-id tool calls", () => {
      const trace = useTraceStore();

      trace.applyTraceEvent(
        mkEvent({
          type: "ModelToolCallRequested",
          tool_call_id: "toolu-1",
          tool_id: "shell.exec"
        })
      );
      trace.applyTraceEvent(
        mkEvent({
          type: "PermissionRequested",
          request_id: "toolu-1",
          tool_id: "shell.exec",
          preview: 'shell.exec({"command":"printf ok"})'
        })
      );

      expect(trace.entries).toHaveLength(2);
      const permission = trace.entries.find((entry) => entry.kind === "permission");
      const tool = trace.entries.find((entry) => entry.kind === "tool");
      expect(permission?.id).toBe("toolu-1");
      expect(permission?.status).toBe("pending");
      expect(permission?.title).toContain("printf ok");
      expect(tool?.id).not.toBe(permission?.id);
      expect(tool?.toolId).toBe("shell.exec");

      trace.applyTraceEvent(mkEvent({ type: "PermissionGranted", request_id: "toolu-1" }));
      trace.applyTraceEvent(
        mkEvent({
          type: "ToolInvocationStarted",
          invocation_id: "toolu-1",
          tool_id: "shell.exec"
        })
      );
      trace.applyTraceEvent(
        mkEvent({
          type: "ToolInvocationCompleted",
          invocation_id: "toolu-1",
          tool_id: "shell.exec",
          output_preview: "APPROVAL-GRANT-6F2D",
          exit_code: 0,
          duration_ms: 12,
          truncated: false
        })
      );

      expect(permission?.status).toBe("completed");
      expect(tool?.status).toBe("completed");
      expect(tool?.outputPreview).toBe("APPROVAL-GRANT-6F2D");
      expect(tool?.exitCode).toBe(0);
    });

    it("PermissionGranted marks entry as completed", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "PermissionRequested",
          request_id: "perm-2",
          tool_id: "shell",
          preview: "ls"
        })
      );
      trace.applyTraceEvent(mkEvent({ type: "PermissionGranted", request_id: "perm-2" }));
      expect(trace.entries[0].status).toBe("completed");
    });

    it("PermissionDenied marks entry as failed", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "PermissionRequested",
          request_id: "perm-3",
          tool_id: "shell",
          preview: "danger"
        })
      );
      trace.applyTraceEvent(
        mkEvent({ type: "PermissionDenied", request_id: "perm-3", reason: "nope" })
      );
      expect(trace.entries[0].status).toBe("failed");
    });

    it("PermissionDenied fails the same-id model tool-call entry", () => {
      const trace = useTraceStore();

      trace.applyTraceEvent(
        mkEvent({
          type: "ModelToolCallRequested",
          tool_call_id: "toolu-deny-1",
          tool_id: "shell.exec"
        })
      );
      trace.applyTraceEvent(
        mkEvent({
          type: "PermissionRequested",
          request_id: "toolu-deny-1",
          tool_id: "shell.exec",
          preview: 'shell.exec({"command":"touch denied.txt"})'
        })
      );
      trace.applyTraceEvent(
        mkEvent({
          type: "PermissionDenied",
          request_id: "toolu-deny-1",
          reason: "denied by user"
        })
      );

      const tool = trace.entries.find((entry) => entry.kind === "tool");
      const permission = trace.entries.find((entry) => entry.kind === "permission");
      expect(tool?.id).toBe("tool-toolu-deny-1");
      expect(tool?.status).toBe("failed");
      expect(tool?.outputPreview).toBe("denied by user");
      expect(permission?.status).toBe("failed");
    });
  });

  // -----------------------------------------------------------
  // applyTraceEvent — Memory lifecycle
  // -----------------------------------------------------------
  describe("Memory lifecycle", () => {
    it("MemoryProposed adds a pending memory entry with scope and content", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "MemoryProposed",
          memory_id: "mem-1",
          scope: "user",
          key: "lang",
          content: "Prefers Rust"
        })
      );
      expect(trace.entries).toHaveLength(1);
      const entry = trace.entries[0];
      expect(entry.id).toBe("mem-1");
      expect(entry.kind).toBe("memory");
      expect(entry.status).toBe("pending");
      expect(entry.toolId).toBe("memory.store");
      expect(entry.scope).toBe("user");
      expect(entry.content).toBe("Prefers Rust");
      expect(entry.expanded).toBe(true);
    });

    it("MemoryAccepted marks entry as completed", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "MemoryProposed",
          memory_id: "mem-2",
          scope: "session",
          key: null,
          content: "temp"
        })
      );
      trace.applyTraceEvent(
        mkEvent({
          type: "MemoryAccepted",
          memory_id: "mem-2",
          scope: "session",
          key: null,
          content: "temp"
        })
      );
      expect(trace.entries[0].status).toBe("completed");
    });

    it("MemoryAccepted without a proposal adds a completed memory entry", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "MemoryAccepted",
          memory_id: "mem-session-auto",
          scope: "session",
          key: "turn-note",
          content: "auto accepted session note"
        })
      );

      expect(trace.entries).toHaveLength(1);
      expect(trace.entries[0]).toMatchObject({
        id: "mem-session-auto",
        kind: "memory",
        status: "completed",
        toolId: "memory.store",
        title: "Save session memory",
        scope: "session",
        content: "auto accepted session note",
        expanded: false
      });
    });

    it("MemoryRejected marks entry as failed with reason", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "MemoryProposed",
          memory_id: "mem-3",
          scope: "user",
          key: null,
          content: "junk"
        })
      );
      trace.applyTraceEvent(
        mkEvent({ type: "MemoryRejected", memory_id: "mem-3", reason: "duplicate" })
      );
      expect(trace.entries[0].status).toBe("failed");
      expect(trace.entries[0].reason).toBe("duplicate");
    });
  });

  // -----------------------------------------------------------
  // clearTrace
  // -----------------------------------------------------------
  describe("clearTrace", () => {
    it("removes all entries and resets dedup set", () => {
      const trace = useTraceStore();
      trace.applyTraceEvent(
        mkEvent({
          type: "AgentTaskCreated",
          task_id: "t-clear",
          title: "task",
          role: "Worker",
          dependencies: []
        })
      );
      expect(trace.entries).toHaveLength(1);

      trace.clearTrace();
      expect(trace.entries).toHaveLength(0);

      // Re-adding the same ID should succeed after clear (dedup set was reset)
      trace.applyTraceEvent(
        mkEvent({
          type: "AgentTaskCreated",
          task_id: "t-clear",
          title: "task again",
          role: "Worker",
          dependencies: []
        })
      );
      expect(trace.entries).toHaveLength(1);
      expect(trace.entries[0].title).toBe("task again");
    });
  });

  // -----------------------------------------------------------
  // density ref
  // -----------------------------------------------------------
  describe("density", () => {
    it("can be set to L1 / L2 / L3", () => {
      const trace = useTraceStore();
      expect(trace.density).toBe("L2");

      trace.density = "L1";
      expect(trace.density).toBe("L1");

      trace.density = "L3";
      expect(trace.density).toBe("L3");
    });
  });

  // -----------------------------------------------------------
  // rawJson helper (exercised through rawEvent on entries)
  // -----------------------------------------------------------
  describe("rawEvent / rawJson", () => {
    it("stores a JSON serialization of the event on each entry", () => {
      const trace = useTraceStore();
      const event = mkEvent({
        type: "ToolInvocationStarted",
        invocation_id: "inv-raw",
        tool_id: "search"
      });
      trace.applyTraceEvent(event);
      const raw = trace.entries[0].rawEvent;
      expect(raw).toBeDefined();
      const parsed = JSON.parse(raw!);
      expect(parsed.payload.type).toBe("ToolInvocationStarted");
    });
  });
});
