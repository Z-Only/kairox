import { reactive } from "vue";
import type { DomainEvent } from "../types";
import type { TraceEntryData } from "../types/trace";

export const traceState = reactive({
  entries: [] as TraceEntryData[],
  density: "L2" as "L1" | "L2" | "L3"
});

/** Set of entry IDs currently in the trace, used for dedup. */
const entryIds = new Set<string>();

function updateEntry(id: string, updates: Partial<TraceEntryData>) {
  const idx = traceState.entries.findIndex((e) => e.id === id);
  if (idx !== -1) {
    Object.assign(traceState.entries[idx], updates);
  }
}

/** Add an entry only if its ID is not already present. Returns true if added. */
function pushEntry(entry: TraceEntryData): boolean {
  if (entryIds.has(entry.id)) {
    return false;
  }
  entryIds.add(entry.id);
  traceState.entries.push(entry);
  return true;
}

/** Store the raw JSON of the event for L3 display. */
function rawJson(event: DomainEvent): string {
  try {
    return JSON.stringify(event, null, 2);
  } catch {
    return "";
  }
}

export function applyTraceEvent(event: DomainEvent) {
  const p = event.payload;
  switch (p.type) {
    case "AgentTaskCreated": {
      const typed = p as {
        type: "AgentTaskCreated";
        task_id: string;
        title: string;
        role: string;
        dependencies: string[];
      };
      pushEntry({
        id: typed.task_id,
        kind: "tool",
        status: "completed",
        toolId: "task",
        title: typed.title,
        startedAt: Date.now(),
        expanded: false,
        rawEvent: rawJson(event)
      });
      break;
    }

    case "UserMessageAdded": {
      const typed = p as {
        type: "UserMessageAdded";
        message_id: string;
        content: string;
      };
      pushEntry({
        id: typed.message_id,
        kind: "tool",
        status: "completed",
        toolId: "user",
        title: `User: ${typed.content.slice(0, 80)}${typed.content.length > 80 ? "…" : ""}`,
        startedAt: Date.now(),
        expanded: false,
        input: typed.content,
        rawEvent: rawJson(event)
      });
      break;
    }

    case "ContextAssembled": {
      const typed = p as {
        type: "ContextAssembled";
        token_estimate: number;
        sources: string[];
      };
      // ContextAssembled events have no unique ID; use a generated one
      // that cannot conflict with real-time events (different format).
      pushEntry({
        id: `ctx-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        kind: "tool",
        status: "completed",
        toolId: "context",
        title: `Context assembled (${typed.token_estimate} tokens)`,
        startedAt: Date.now(),
        expanded: false,
        outputPreview: typed.sources.join(", "),
        rawEvent: rawJson(event)
      });
      break;
    }

    case "ModelRequestStarted": {
      const typed = p as {
        type: "ModelRequestStarted";
        model_profile: string;
        model_id: string;
      };
      // ModelRequestStarted events have no durable ID; use a generated one.
      pushEntry({
        id: `model-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        kind: "tool",
        status: "running",
        toolId: "model",
        title: `Model: ${typed.model_profile} / ${typed.model_id}`,
        startedAt: Date.now(),
        expanded: false,
        rawEvent: rawJson(event)
      });
      break;
    }

    case "ModelTokenDelta": {
      // Skip — too many per request; not useful as trace entries
      break;
    }

    case "AssistantMessageCompleted": {
      const typed = p as {
        type: "AssistantMessageCompleted";
        message_id: string;
        content: string;
      };
      const runningModel = traceState.entries.find(
        (e) =>
          e.kind === "tool" && e.toolId === "model" && e.status === "running"
      );
      if (runningModel) {
        runningModel.status = "completed";
        runningModel.durationMs = Date.now() - runningModel.startedAt;
        runningModel.outputPreview = typed.content.slice(0, 200);
        runningModel.rawEvent = rawJson(event);
      } else if (!entryIds.has(typed.message_id)) {
        pushEntry({
          id: typed.message_id,
          kind: "tool",
          status: "completed",
          toolId: "assistant",
          title: "Assistant response",
          startedAt: Date.now(),
          expanded: false,
          outputPreview: typed.content.slice(0, 200),
          rawEvent: rawJson(event)
        });
      }
      break;
    }

    case "ModelToolCallRequested": {
      const typed = p as {
        type: "ModelToolCallRequested";
        tool_call_id: string;
        tool_id: string;
      };
      pushEntry({
        id: typed.tool_call_id,
        kind: "tool",
        status: "running",
        toolId: typed.tool_id,
        title: `Tool call: ${typed.tool_id}`,
        startedAt: Date.now(),
        expanded: false,
        rawEvent: rawJson(event)
      });
      break;
    }

    case "ToolInvocationStarted": {
      const typed = p as {
        type: "ToolInvocationStarted";
        invocation_id: string;
        tool_id: string;
      };
      pushEntry({
        id: typed.invocation_id,
        kind: "tool",
        status: "running",
        toolId: typed.tool_id,
        title: typed.tool_id,
        startedAt: Date.now(),
        expanded: false,
        rawEvent: rawJson(event)
      });
      break;
    }

    case "ToolInvocationCompleted": {
      const typed = p as {
        type: "ToolInvocationCompleted";
        invocation_id: string;
        tool_id: string;
        output_preview: string;
        exit_code: number | null;
        duration_ms: number;
        truncated: boolean;
      };
      updateEntry(typed.invocation_id, {
        status: "completed",
        durationMs: typed.duration_ms,
        outputPreview: typed.output_preview,
        exitCode: typed.exit_code,
        truncated: typed.truncated,
        rawEvent: rawJson(event)
      });
      break;
    }

    case "ToolInvocationFailed": {
      const typed = p as {
        type: "ToolInvocationFailed";
        invocation_id: string;
        tool_id: string;
        error: string;
      };
      updateEntry(typed.invocation_id, {
        status: "failed",
        rawEvent: rawJson(event)
      });
      break;
    }

    case "PermissionRequested": {
      const typed = p as {
        type: "PermissionRequested";
        request_id: string;
        tool_id: string;
        preview: string;
      };
      pushEntry({
        id: typed.request_id,
        kind: "permission",
        status: "pending",
        toolId: typed.tool_id,
        title: typed.preview || typed.tool_id,
        startedAt: Date.now(),
        expanded: true,
        rawEvent: rawJson(event)
      });
      break;
    }

    case "PermissionGranted": {
      const typed = p as { type: "PermissionGranted"; request_id: string };
      updateEntry(typed.request_id, {
        status: "completed",
        rawEvent: rawJson(event)
      });
      break;
    }

    case "PermissionDenied": {
      const typed = p as { type: "PermissionDenied"; request_id: string };
      updateEntry(typed.request_id, {
        status: "failed",
        rawEvent: rawJson(event)
      });
      break;
    }

    case "MemoryProposed": {
      const typed = p as {
        type: "MemoryProposed";
        memory_id: string;
        scope: string;
        key: string | null;
        content: string;
      };
      pushEntry({
        id: typed.memory_id,
        kind: "memory",
        status: "pending",
        toolId: "memory.store",
        title: `Save ${typed.scope} memory`,
        startedAt: Date.now(),
        expanded: true,
        scope: typed.scope,
        content: typed.content,
        rawEvent: rawJson(event)
      });
      break;
    }

    case "MemoryAccepted": {
      const typed = p as {
        type: "MemoryAccepted";
        memory_id: string;
      };
      updateEntry(typed.memory_id, {
        status: "completed",
        rawEvent: rawJson(event)
      });
      break;
    }

    case "MemoryRejected": {
      const typed = p as {
        type: "MemoryRejected";
        memory_id: string;
        reason: string;
      };
      updateEntry(typed.memory_id, {
        status: "failed",
        reason: typed.reason,
        rawEvent: rawJson(event)
      });
      break;
    }
  }
}

export function clearTrace() {
  traceState.entries = [];
  entryIds.clear();
}
