import { reactive } from "vue";
import type { DomainEvent } from "../types";
import type { TraceEntryData } from "../types/trace";

export const traceState = reactive({
  entries: [] as TraceEntryData[],
  density: "L2" as "L1" | "L2" | "L3"
});

function updateEntry(id: string, updates: Partial<TraceEntryData>) {
  const idx = traceState.entries.findIndex((e) => e.id === id);
  if (idx !== -1) {
    Object.assign(traceState.entries[idx], updates);
  }
}

export function applyTraceEvent(event: DomainEvent) {
  const p = event.payload;
  switch (p.type) {
    case "ToolInvocationStarted":
      traceState.entries.push({
        id: p.invocation_id,
        kind: "tool",
        status: "running",
        toolId: p.tool_id,
        title: p.tool_id,
        startedAt: Date.now(),
        expanded: false
      });
      break;

    case "ToolInvocationCompleted":
      updateEntry(p.invocation_id, {
        status: "completed",
        durationMs: p.duration_ms,
        outputPreview: p.output_preview,
        exitCode: p.exit_code,
        truncated: p.truncated
      });
      break;

    case "ToolInvocationFailed":
      updateEntry(p.invocation_id, {
        status: "failed"
      });
      break;

    case "PermissionRequested":
      traceState.entries.push({
        id: p.request_id,
        kind: "permission",
        status: "pending",
        toolId: p.tool_id,
        title: p.preview || p.tool_id,
        startedAt: Date.now(),
        expanded: true
      });
      break;

    case "PermissionGranted":
      updateEntry(p.request_id, { status: "completed" });
      break;

    case "PermissionDenied":
      updateEntry(p.request_id, { status: "failed" });
      break;

    case "MemoryProposed":
      traceState.entries.push({
        id: p.memory_id,
        kind: "memory",
        status: "pending",
        toolId: "memory.store",
        title: `Save ${p.scope} memory`,
        startedAt: Date.now(),
        expanded: true,
        scope: p.scope,
        content: p.content
      });
      break;

    case "MemoryAccepted":
      updateEntry(p.memory_id, { status: "completed" });
      break;

    case "MemoryRejected":
      updateEntry(p.memory_id, { status: "failed", reason: p.reason });
      break;
  }
}

export function clearTrace() {
  traceState.entries = [];
}
