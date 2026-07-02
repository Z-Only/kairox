// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore` and `ref` explicitly.
import { defineStore } from "pinia";
import { ref } from "vue";
import type { DomainEvent } from "@/types";
import type { TraceEntryData } from "@/types/trace";

const TOOL_TITLE_MAX_CHARS = 120;
type ModelStreamStatusPayload = Extract<DomainEvent["payload"], { type: "ModelStreamStatus" }>;

function compactPreviewLine(value: string): string {
  return value.trim().split(/\r?\n/).find(Boolean)?.trim() ?? "";
}

function truncateTitle(value: string): string {
  if (value.length <= TOOL_TITLE_MAX_CHARS) return value;
  return `${value.slice(0, TOOL_TITLE_MAX_CHARS - 1)}…`;
}

function commandFromToolPreview(preview: string): string | null {
  const trimmed = preview.trim();
  const jsonArgs = trimmed.match(/^[\w.-]+\((.*)\)$/s)?.[1];
  if (!jsonArgs) return null;
  try {
    const parsed = JSON.parse(jsonArgs);
    if (parsed && typeof parsed === "object" && typeof parsed.command === "string") {
      const command = compactPreviewLine(parsed.command);
      return command ? truncateTitle(command) : null;
    }
  } catch {
    return null;
  }
  return null;
}

function titleFromToolPreview(toolId: string, inputPreview?: string): string {
  if (!inputPreview) return toolId;
  const summary =
    commandFromToolPreview(inputPreview) ?? truncateTitle(compactPreviewLine(inputPreview));
  return summary ? `${toolId}: ${summary}` : toolId;
}

function modelStreamStatusPreview(p: ModelStreamStatusPayload): string {
  const hasRetryCount = p.retry_attempt > 0 || p.max_retries > 0;
  const state = p.retrying
    ? hasRetryCount
      ? `retry ${p.retry_attempt}/${p.max_retries}`
      : "retrying"
    : hasRetryCount
      ? `failed retry ${p.retry_attempt}/${p.max_retries}`
      : "failed";
  const message = compactPreviewLine(p.message);
  return `${p.phase} ${state}${message ? `: ${message}` : ""}`;
}

export const useTraceStore = defineStore("trace", () => {
  const entries = ref<TraceEntryData[]>([]);
  const density = ref<"L1" | "L2" | "L3">("L2");

  /** Set of entry IDs currently in the trace, used for dedup. */
  const entryIds = new Set<string>();

  function updateEntry(id: string, updates: Partial<TraceEntryData>): boolean {
    const idx = entries.value.findIndex((e) => e.id === id);
    if (idx !== -1) {
      Object.assign(entries.value[idx], updates);
      return true;
    }
    return false;
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

  function toolEntryId(invocationId: string): string {
    // Tool and permission events share provider tool_call_id values; keep
    // the raw id available for resolve_permission-backed prompt entries.
    return `tool-${invocationId}`;
  }

  function nextCancellationEntryId(): string {
    let index = 1;
    while (entryIds.has(`session-cancelled-${index}`)) {
      index++;
    }
    return `session-cancelled-${index}`;
  }

  function latestModelEntry(status?: TraceEntryData["status"]): TraceEntryData | null {
    for (let index = entries.value.length - 1; index >= 0; index--) {
      const entry = entries.value[index];
      if (
        entry.kind === "tool" &&
        entry.toolId === "model" &&
        (status === undefined || entry.status === status)
      ) {
        return entry;
      }
    }
    return null;
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
        // ChatPanel renders user messages directly from the session
        // projection via `useChatStream`. The trace store used to push a
        // pseudo-tool entry here so the legacy trace view could double as
        // a chat log; that entry now shows up as a duplicate row in the
        // unified chat stream, so we drop it.
        break;
      }

      case "SessionCancelled": {
        pushEntry({
          id: nextCancellationEntryId(),
          kind: "cancellation",
          status: "completed",
          toolId: "cancellation",
          title: "Session cancelled",
          startedAt: Date.now(),
          expanded: false,
          reason: p.reason,
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

      case "ModelStreamStatus": {
        const preview = modelStreamStatusPreview(p);
        const status = p.retrying ? "running" : "failed";
        const now = Date.now();
        const modelEntry = latestModelEntry("running");
        if (modelEntry) {
          modelEntry.status = status;
          modelEntry.title = truncateTitle(`Model stream ${preview}`);
          modelEntry.outputPreview = preview;
          modelEntry.rawEvent = rawJson(event);
          if (status === "failed") {
            modelEntry.durationMs = now - modelEntry.startedAt;
          }
          break;
        }
        pushEntry({
          id: `model-${now}-${Math.random().toString(36).slice(2, 6)}`,
          kind: "tool",
          status,
          toolId: "model",
          title: truncateTitle(`Model stream ${preview}`),
          startedAt: now,
          expanded: false,
          outputPreview: preview,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "ModelTokenDelta": {
        break;
      }

      case "AssistantMessageCompleted": {
        // Close out the in-flight `ModelRequestStarted` entry, if any,
        // with the assistant content as an outputPreview. The previous
        // fallback that pushed a pseudo "assistant" tool entry when no
        // running model existed has been removed: assistant turns are
        // rendered directly in ChatPanel via `useChatStream`, and the
        // pseudo entry was duplicating those rows in the unified feed.
        const runningModel = latestModelEntry("running");
        if (runningModel) {
          runningModel.status = "completed";
          runningModel.durationMs = Date.now() - runningModel.startedAt;
          runningModel.outputPreview = p.content.slice(0, 200);
          runningModel.rawEvent = rawJson(event);
        }
        break;
      }

      case "ModelToolCallRequested": {
        pushEntry({
          id: toolEntryId(p.tool_call_id),
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
        const inputPreview = compactPreviewLine(p.input_preview ?? "");
        const title = titleFromToolPreview(p.tool_id, inputPreview || undefined);
        const updates: Partial<TraceEntryData> = {
          status: "running",
          toolId: p.tool_id,
          title,
          rawEvent: rawJson(event)
        };
        if (inputPreview) {
          updates.input = inputPreview;
        }
        if (updateEntry(toolEntryId(p.invocation_id), updates)) {
          break;
        }
        pushEntry({
          id: toolEntryId(p.invocation_id),
          kind: "tool",
          status: "running",
          toolId: p.tool_id,
          title,
          startedAt: Date.now(),
          input: inputPreview || undefined,
          expanded: false,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "ToolInvocationCompleted": {
        updateEntry(toolEntryId(p.invocation_id), {
          status: "completed",
          durationMs: p.duration_ms,
          outputPreview: p.output_preview,
          exitCode: p.exit_code,
          truncated: p.truncated,
          images: p.images && p.images.length > 0 ? p.images : undefined,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "ToolInvocationFailed": {
        updateEntry(toolEntryId(p.invocation_id), {
          status: "failed",
          rawEvent: rawJson(event)
        });
        break;
      }

      case "TrajectoryCompleted": {
        const failed = p.outcome !== "success";
        pushEntry({
          id: `trajectory-${p.trajectory_id}`,
          kind: "tool",
          status: failed ? "failed" : "completed",
          toolId: "trajectory",
          title: `Trajectory completed: ${p.outcome}`,
          startedAt: Date.now(),
          expanded: false,
          outputPreview: `${p.step_count} steps`,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "AgentTaskFailed": {
        updateEntry(p.task_id, {
          status: "failed",
          reason: p.error,
          outputPreview: p.error,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "PermissionRequested": {
        const inputPreview = compactPreviewLine(p.preview);
        updateEntry(toolEntryId(p.request_id), {
          input: inputPreview,
          title: titleFromToolPreview(p.tool_id, inputPreview),
          rawEvent: rawJson(event)
        });
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

      case "TaskConfirmationRequested": {
        pushEntry({
          id: p.request_id,
          kind: "task_confirmation",
          status: "pending",
          toolId: "task_confirmation.request",
          title: p.prompt,
          startedAt: Date.now(),
          expanded: true,
          options: p.options,
          allowMultiple: p.allow_multiple,
          allowCustom: p.allow_custom,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "TaskConfirmationResolved": {
        updateEntry(p.request_id, {
          status: "completed",
          selectedOptionIds: p.selected_option_ids,
          customResponse: p.custom_response,
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
          reason: p.reason,
          rawEvent: rawJson(event)
        });
        updateEntry(toolEntryId(p.request_id), {
          status: "failed",
          outputPreview: p.reason,
          reason: p.reason,
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
        if (entryIds.has(p.memory_id)) {
          updateEntry(p.memory_id, {
            status: "completed",
            rawEvent: rawJson(event)
          });
        } else {
          pushEntry({
            id: p.memory_id,
            kind: "memory",
            status: "completed",
            toolId: "memory.store",
            title: `Save ${p.scope} memory`,
            startedAt: Date.now(),
            expanded: false,
            scope: p.scope,
            content: p.content,
            rawEvent: rawJson(event)
          });
        }
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

      case "MonitorStarted": {
        pushEntry({
          id: p.monitor_id,
          kind: "monitor",
          status: "running",
          toolId: "monitor",
          title: p.description,
          startedAt: Date.now(),
          expanded: false,
          input: p.command,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "MonitorEvent": {
        updateEntry(p.monitor_id, {
          outputPreview: p.line
        });
        break;
      }

      case "MonitorStopped": {
        updateEntry(p.monitor_id, {
          status: "completed",
          reason: p.reason.type,
          rawEvent: rawJson(event)
        });
        break;
      }

      case "MonitorFailed": {
        updateEntry(p.monitor_id, {
          status: "failed",
          outputPreview: p.error,
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
