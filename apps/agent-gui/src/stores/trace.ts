// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore` and `ref` explicitly.
import { defineStore } from "pinia";
import { ref } from "vue";
import type { DomainEvent } from "@/types";
import type { TraceEntryData } from "@/types/trace";

export const useTraceStore = defineStore("trace", () => {
  const entries = ref<TraceEntryData[]>([]);
  const density = ref<"L1" | "L2" | "L3">("L2");

  /** Set of entry IDs currently in the trace, used for dedup. */
  const entryIds = new Set<string>();

  function updateEntry(id: string, updates: Partial<TraceEntryData>) {
    const idx = entries.value.findIndex((e) => e.id === id);
    if (idx !== -1) {
      Object.assign(entries.value[idx], updates);
    }
  }

  /** Add an entry only if its ID is not already present. Returns true if added. */
  function pushEntry(entry: TraceEntryData): boolean {
    if (entryIds.has(entry.id)) {
      return false;
    }
    entryIds.add(entry.id);
    entries.value.push(entry);
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

  function applyTraceEvent(event: DomainEvent) {
    const p = event.payload;
    switch (p.type) {
      case "AgentTaskCreated": {
        pushEntry({
          id: p.task_id,
          kind: "tool",
          status: "completed",
          toolId: "task",
          title: p.title,
          startedAt: Date.now(),
          expanded: false,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "UserMessageAdded": {
        pushEntry({
          id: p.message_id,
          kind: "tool",
          status: "completed",
          toolId: "user",
          title: `User: ${p.content.slice(0, 80)}${p.content.length > 80 ? "…" : ""}`,
          startedAt: Date.now(),
          expanded: false,
          input: p.content,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "ContextAssembled": {
        const sourceLabels = p.usage.by_source.map(([src, n]) => `${src}:${n}`).join(", ");
        pushEntry({
          id: `ctx-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
          kind: "tool",
          status: "completed",
          toolId: "context",
          title: `Context assembled (${p.usage.total_tokens} / ${p.usage.budget_tokens} tokens)`,
          startedAt: Date.now(),
          expanded: false,
          outputPreview: sourceLabels,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "ModelRequestStarted": {
        pushEntry({
          id: `model-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
          kind: "tool",
          status: "running",
          toolId: "model",
          title: `Model: ${p.model_profile} / ${p.model_id}`,
          startedAt: Date.now(),
          expanded: false,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "ModelTokenDelta": {
        break;
      }

      case "AssistantMessageCompleted": {
        const runningModel = entries.value.find(
          (e) => e.kind === "tool" && e.toolId === "model" && e.status === "running"
        );
        if (runningModel) {
          runningModel.status = "completed";
          runningModel.durationMs = Date.now() - runningModel.startedAt;
          runningModel.outputPreview = p.content.slice(0, 200);
          runningModel.rawEvent = rawJson(event);
        } else if (!entryIds.has(p.message_id)) {
          pushEntry({
            id: p.message_id,
            kind: "tool",
            status: "completed",
            toolId: "assistant",
            title: "Assistant response",
            startedAt: Date.now(),
            expanded: false,
            outputPreview: p.content.slice(0, 200),
            rawEvent: rawJson(event)
          });
        }
        break;
      }

      case "ModelToolCallRequested": {
        pushEntry({
          id: p.tool_call_id,
          kind: "tool",
          status: "running",
          toolId: p.tool_id,
          title: `Tool call: ${p.tool_id}`,
          startedAt: Date.now(),
          expanded: false,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "ToolInvocationStarted": {
        pushEntry({
          id: p.invocation_id,
          kind: "tool",
          status: "running",
          toolId: p.tool_id,
          title: p.tool_id,
          startedAt: Date.now(),
          expanded: false,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "ToolInvocationCompleted": {
        updateEntry(p.invocation_id, {
          status: "completed",
          durationMs: p.duration_ms,
          outputPreview: p.output_preview,
          exitCode: p.exit_code,
          truncated: p.truncated,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "ToolInvocationFailed": {
        updateEntry(p.invocation_id, {
          status: "failed",
          rawEvent: rawJson(event)
        });
        break;
      }

      case "PermissionRequested": {
        pushEntry({
          id: p.request_id,
          kind: "permission",
          status: "pending",
          toolId: p.tool_id,
          title: p.preview || p.tool_id,
          startedAt: Date.now(),
          expanded: true,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "PermissionGranted": {
        updateEntry(p.request_id, {
          status: "completed",
          rawEvent: rawJson(event)
        });
        break;
      }

      case "PermissionDenied": {
        updateEntry(p.request_id, {
          status: "failed",
          rawEvent: rawJson(event)
        });
        break;
      }

      case "MemoryProposed": {
        pushEntry({
          id: p.memory_id,
          kind: "memory",
          status: "pending",
          toolId: "memory.store",
          title: `Save ${p.scope} memory`,
          startedAt: Date.now(),
          expanded: true,
          scope: p.scope,
          content: p.content,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "MemoryAccepted": {
        updateEntry(p.memory_id, {
          status: "completed",
          rawEvent: rawJson(event)
        });
        break;
      }

      case "MemoryRejected": {
        updateEntry(p.memory_id, {
          status: "failed",
          reason: p.reason,
          rawEvent: rawJson(event)
        });
        break;
      }
    }
  }

  function clearTrace() {
    entries.value = [];
    entryIds.clear();
  }

  return {
    entries,
    density,
    applyTraceEvent,
    clearTrace
  };
});
