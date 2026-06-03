/**
 * Chat-stream fold for the v0.30.0 unified ChatPanel feed.
 *
 * The pure {@link buildChatStream} function folds session messages, trace
 * permission / tool / memory entries, and context-compaction status into a
 * single ordered list of {@link ChatStreamItem} values. The composable
 * {@link useChatStream} wraps the same fold in a Vue `computed` over the
 * Pinia session + trace stores.
 *
 * Ordering for this lane is deliberately deterministic — messages stay in
 * projection order, while trace-derived items are sorted by
 * `TraceEntryData.startedAt` ascending (insertion order if timestamps are
 * equal) and grouped into user turns. A trace group is inserted directly
 * after its user prompt and before the assistant output for that turn.
 *
 * Wiring into ChatPanel happens in a follow-up PR — this composable is
 * pure read-only and does not mutate either store.
 */
import { computed, type ComputedRef } from "vue";
import type { CompactionStatus, ProjectedRole } from "@/types";
import type { TraceEntryData } from "@/types/trace";
import type {
  ChatCompactionStreamItem,
  ChatMessageStreamItem,
  ChatMonitorStreamItem,
  ChatPermissionGroupStreamItem,
  ChatPermissionStreamItem,
  ChatStreamItem,
  ChatToolCallStreamItem
} from "@/types/chatStream";
import { useSessionStore } from "@/stores/session";
import { useTraceStore } from "@/stores/trace";

/** Minimal shape the builder needs from a projected message. */
export interface ChatStreamMessageInput {
  role: ProjectedRole;
  content: string;
  sourceAgentId?: string;
}

/**
 * Pure builder that folds messages, trace entries, and compaction status
 * into the chat-stream feed.
 *
 * Inputs are treated as immutable; the builder never mutates either array.
 */
export function buildChatStream(
  messages: ReadonlyArray<ChatStreamMessageInput>,
  traceEntries: ReadonlyArray<TraceEntryData>,
  compaction: CompactionStatus
): ChatStreamItem[] {
  const items: ChatStreamItem[] = [];
  const traceItems = buildTraceStreamItems(traceEntries);
  const traceGroups = groupTraceItemsByTurn(traceItems);
  let traceGroupIndex = 0;

  // 1. Messages stay in projection order, with stable index-based ids. Each
  //    user message owns the next trace group, so context and tool cards
  //    appear before the assistant answer for that same turn.
  for (let index = 0; index < messages.length; index++) {
    const message = messages[index];
    const messageItem: ChatMessageStreamItem = {
      kind: "message",
      id: `msg-${index}`,
      role: message.role,
      content: message.content
    };
    if (message.sourceAgentId !== undefined) {
      messageItem.sourceAgentId = message.sourceAgentId;
    }
    items.push(messageItem);
    if (message.role === "user" && traceGroupIndex < traceGroups.length) {
      items.push(...traceGroups[traceGroupIndex]);
      traceGroupIndex++;
    }
  }

  // 2. Sessions can have trace entries without a projected user message
  //    (restored legacy state, background events, or partial turns). Keep
  //    those visible after all projected messages.
  for (; traceGroupIndex < traceGroups.length; traceGroupIndex++) {
    items.push(...traceGroups[traceGroupIndex]);
  }

  // 3. Compaction item appended last when the status is anything other
  //    than Idle.
  if (compaction.type !== "Idle") {
    const compactionItem: ChatCompactionStreamItem = {
      kind: "compaction",
      id: `compaction-${compaction.type}`,
      status: compaction
    };
    items.push(compactionItem);
  }

  return items;
}

function buildTraceStreamItems(traceEntries: ReadonlyArray<TraceEntryData>): ChatStreamItem[] {
  const traceItems: ChatStreamItem[] = [];

  // Trace-derived items, sorted by `startedAt` ascending (stable — equal
  // timestamps preserve insertion order).
  const traceEntriesSorted = [...traceEntries]
    .map((entry, insertionIndex) => ({ entry, insertionIndex }))
    .sort((a, b) => {
      const delta = a.entry.startedAt - b.entry.startedAt;
      if (delta !== 0) return delta;
      return a.insertionIndex - b.insertionIndex;
    });

  // Walk the sorted trace entries, collapsing runs of ≥2 consecutive pending
  // permission items into a single `permission_group`. Tool calls, resolved
  // permissions (which return `null` from `traceEntryToStreamItem`), or any
  // other non-permission item all break the run.
  //
  // We carry the source `TraceEntryData` alongside each pending permission so
  // the group builder can read `startedAt` from the source without smuggling it
  // through the stream-item layer.
  type PendingRunEntry = { item: ChatPermissionStreamItem; source: TraceEntryData };
  let pendingRun: PendingRunEntry[] = [];
  const flushPendingRun = () => {
    if (pendingRun.length === 0) return;
    if (pendingRun.length === 1) {
      // Lone pending permission stays as the original `Permission` variant.
      traceItems.push(pendingRun[0].item);
    } else {
      traceItems.push(buildPermissionGroup(pendingRun));
    }
    pendingRun = [];
  };

  for (const { entry } of traceEntriesSorted) {
    const traceItem = traceEntryToStreamItem(entry);
    if (!traceItem) {
      // Resolved permissions and unknown kinds — break the run but do not
      // append anything (they were filtered out by `traceEntryToStreamItem`).
      flushPendingRun();
      continue;
    }
    if (traceItem.kind === "permission") {
      pendingRun.push({ item: traceItem, source: entry });
    } else {
      flushPendingRun();
      traceItems.push(traceItem);
    }
  }
  flushPendingRun();

  return traceItems;
}

function groupTraceItemsByTurn(traceItems: ReadonlyArray<ChatStreamItem>): ChatStreamItem[][] {
  const groups: ChatStreamItem[][] = [];
  let currentGroup: ChatStreamItem[] = [];

  for (const item of traceItems) {
    if (isTraceTurnStart(item) && currentGroup.length > 0) {
      groups.push(currentGroup);
      currentGroup = [];
    }
    currentGroup.push(item);
  }

  if (currentGroup.length > 0) groups.push(currentGroup);
  return groups;
}

function isTraceTurnStart(item: ChatStreamItem): boolean {
  return item.kind === "tool_call" && item.toolId === "context";
}

function traceEntryToStreamItem(entry: TraceEntryData): ChatStreamItem | null {
  switch (entry.kind) {
    case "tool": {
      const item: ChatToolCallStreamItem = {
        kind: "tool_call",
        id: entry.id,
        toolId: entry.toolId ?? "",
        status: entry.status
      };
      if (entry.title !== undefined) item.title = entry.title;
      if (entry.durationMs !== undefined) item.durationMs = entry.durationMs;
      item.startedAt = entry.startedAt;
      if (entry.input !== undefined) item.input = entry.input;
      if (entry.outputPreview !== undefined) item.outputPreview = entry.outputPreview;
      if (entry.scope !== undefined) item.scope = entry.scope;
      return item;
    }
    case "permission":
      // Resolved permissions stay visible in TraceTimeline but disappear
      // from the inline chat stream — accept/deny is a one-shot action.
      if (entry.status !== "pending") return null;
      return buildPermissionItem(entry, "tool");
    case "memory":
      if (entry.status !== "pending") return null;
      return buildPermissionItem(entry, "memory");
    case "monitor": {
      const monitorItem: ChatMonitorStreamItem = {
        kind: "monitor",
        id: entry.id,
        description: entry.title,
        status:
          entry.status === "running"
            ? "running"
            : entry.status === "failed"
              ? "failed"
              : "completed"
      };
      if (entry.outputPreview !== undefined) monitorItem.lastLine = entry.outputPreview;
      if (entry.input !== undefined) monitorItem.command = entry.input;
      if (entry.reason !== undefined) monitorItem.stopReason = entry.reason;
      return monitorItem;
    }
    default:
      // Defensive: ignore any future / unknown trace kinds rather than
      // surfacing them as half-typed items.
      return null;
  }
}

/**
 * Build a `permission_group` item from a run of ≥2 consecutive pending
 * permission stream items. The group's `startedAt` matches the FIRST
 * permission's source `startedAt`; `toolIds` is the first-seen-order
 * de-duplicated list of tool ids present in the cluster (entries without
 * a `toolId`, e.g. memory permissions, contribute no entry);
 * `permissionIds` preserves cluster order one-to-one with the underlying
 * requests.
 */
function buildPermissionGroup(
  run: ReadonlyArray<{ item: ChatPermissionStreamItem; source: TraceEntryData }>
): ChatPermissionGroupStreamItem {
  const permissionIds = run.map(({ item }) => item.id);
  const toolIds: string[] = [];
  const seenToolIds = new Set<string>();
  for (const { item } of run) {
    if (item.toolId !== undefined && !seenToolIds.has(item.toolId)) {
      seenToolIds.add(item.toolId);
      toolIds.push(item.toolId);
    }
  }
  return {
    kind: "permission_group",
    id: `permission-group-${run[0].item.id}`,
    startedAt: run[0].source.startedAt,
    toolIds,
    permissionIds,
    count: run.length
  };
}

function buildPermissionItem(
  entry: TraceEntryData,
  variant: "tool" | "memory"
): ChatPermissionStreamItem {
  const item: ChatPermissionStreamItem = {
    kind: "permission",
    id: entry.id,
    variant
  };
  if (entry.toolId !== undefined) item.toolId = entry.toolId;
  if (entry.title !== undefined) item.title = entry.title;
  if (entry.input !== undefined) item.input = entry.input;
  if (entry.reason !== undefined) item.reason = entry.reason;
  if (entry.scope !== undefined) item.scope = entry.scope;
  if (entry.content !== undefined) item.content = entry.content;
  if (entry.rawEvent !== undefined) item.rawEvent = entry.rawEvent;
  return item;
}

/**
 * Vue composable that exposes the folded chat stream as a reactive
 * `ComputedRef`. Reads `projection.messages` and `projection.compaction`
 * from the session store and `entries` from the trace store; never
 * mutates either.
 */
export function useChatStream(): ComputedRef<ChatStreamItem[]> {
  const session = useSessionStore();
  const trace = useTraceStore();
  return computed(() =>
    buildChatStream(session.projection.messages, trace.entries, session.projection.compaction)
  );
}
