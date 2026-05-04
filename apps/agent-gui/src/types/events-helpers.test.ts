import { describe, it, expect } from "vitest";
import type { EventPayload } from "../generated/events";
import { matchPayload, matchPartialPayload } from "./events-helpers";

/**
 * Helper: produces a fallback handler that returns its variant name.
 * Used to fill all 25 keys for exhaustive `matchPayload` without excess verbosity.
 */
function fallback<R>(val: R) {
  return () => val;
}

describe("matchPayload", () => {
  it("routes variant correctly", () => {
    const payload: EventPayload = {
      type: "UserMessageAdded",
      message_id: "msg-1",
      content: "Hello world"
    };

    const result = matchPayload(payload, {
      UserMessageAdded: (p) => `msg:${p.message_id}:${p.content}`,
      WorkspaceOpened: fallback("no"),
      SessionInitialized: fallback("no"),
      AgentTaskCreated: fallback("no"),
      AgentTaskStarted: fallback("no"),
      ContextAssembled: fallback("no"),
      ModelRequestStarted: fallback("no"),
      ModelTokenDelta: fallback("no"),
      ModelToolCallRequested: fallback("no"),
      PermissionRequested: fallback("no"),
      PermissionGranted: fallback("no"),
      PermissionDenied: fallback("no"),
      ToolInvocationStarted: fallback("no"),
      ToolInvocationCompleted: fallback("no"),
      ToolInvocationFailed: fallback("no"),
      FilePatchProposed: fallback("no"),
      FilePatchApplied: fallback("no"),
      MemoryProposed: fallback("no"),
      MemoryAccepted: fallback("no"),
      MemoryRejected: fallback("no"),
      ReviewerFindingAdded: fallback("no"),
      AssistantMessageCompleted: fallback("no"),
      AgentTaskCompleted: fallback("no"),
      AgentTaskFailed: fallback("no"),
      SessionCancelled: fallback("no")
    });

    expect(result).toBe("msg:msg-1:Hello world");
  });

  it("type narrowing provides variant-specific fields", () => {
    const payload: EventPayload = {
      type: "ToolInvocationCompleted",
      invocation_id: "inv-42",
      tool_id: "shell",
      output_preview: "done",
      exit_code: 0,
      duration_ms: 1234,
      truncated: false
    };

    const result = matchPayload(payload, {
      ToolInvocationCompleted: (p) =>
        `${p.tool_id}:${p.exit_code}:${p.duration_ms}ms`,
      WorkspaceOpened: fallback("no"),
      SessionInitialized: fallback("no"),
      UserMessageAdded: fallback("no"),
      AgentTaskCreated: fallback("no"),
      AgentTaskStarted: fallback("no"),
      ContextAssembled: fallback("no"),
      ModelRequestStarted: fallback("no"),
      ModelTokenDelta: fallback("no"),
      ModelToolCallRequested: fallback("no"),
      PermissionRequested: fallback("no"),
      PermissionGranted: fallback("no"),
      PermissionDenied: fallback("no"),
      ToolInvocationStarted: fallback("no"),
      ToolInvocationFailed: fallback("no"),
      FilePatchProposed: fallback("no"),
      FilePatchApplied: fallback("no"),
      MemoryProposed: fallback("no"),
      MemoryAccepted: fallback("no"),
      MemoryRejected: fallback("no"),
      ReviewerFindingAdded: fallback("no"),
      AssistantMessageCompleted: fallback("no"),
      AgentTaskCompleted: fallback("no"),
      AgentTaskFailed: fallback("no"),
      SessionCancelled: fallback("no")
    });

    expect(result).toBe("shell:0:1234ms");
  });
});

describe("matchPartialPayload", () => {
  it("handles specified variant", () => {
    const payload: EventPayload = {
      type: "SessionCancelled",
      reason: "user abort"
    };

    const result = matchPartialPayload(payload, {
      SessionCancelled: (p) => p.reason
    });

    expect(result).toBe("user abort");
  });

  it("returns undefined for unhandled variant", () => {
    const payload: EventPayload = {
      type: "ModelTokenDelta",
      delta: "hello"
    };

    const result = matchPartialPayload(payload, {
      SessionCancelled: (p) => p.reason
    });

    expect(result).toBeUndefined();
  });
});
