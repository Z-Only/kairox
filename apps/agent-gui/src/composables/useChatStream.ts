/**
 * Chat-stream fold for the v0.30.0 unified ChatPanel feed.
 *
 * The pure {@link buildChatStream} function folds session messages, trace
 * permission / tool / memory entries, and context-compaction status into a
 * single ordered list of {@link ChatStreamItem} values. The composable
 * {@link useChatStream} wraps the same fold in a Vue `computed` over the
 * Pinia session + trace stores.
 *
 * Ordering for this lane is deliberately deterministic — all message
 * items in their original projection order, then all trace-derived items
 * sorted by `TraceEntryData.startedAt` ascending (insertion order if the
 * timestamps are equal), then the compaction item if present. True
 * chronological interleaving across message / trace timestamps ships in a
 * later PR; this lane only commits to an order that tests can pin.
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

  // 1. Messages first, in projection order, with stable index-based ids.
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
  }

  // 2. Trace-derived items, sorted by `startedAt` ascending (stable —
  //    equal timestamps preserve insertion order).
  const traceEntriesSorted = [...traceEntries]
    .map((entry, insertionIndex) => ({ entry, insertionIndex }))
    .sort((a, b) => {
      const delta = a.entry.startedAt - b.entry.startedAt;
      if (delta !== 0) return delta;
      return a.insertionIndex - b.insertionIndex;
    });

  for (const { entry } of traceEntriesSorted) {
    const traceItem = traceEntryToStreamItem(entry);
    if (traceItem) items.push(traceItem);
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
      if (entry.input !== undefined) item.input = entry.input;
      if (entry.outputPreview !== undefined) item.outputPreview = entry.outputPreview;
      if (entry.scope !== undefined) item.scope = entry.scope;
      return item;
    }
    case "permission":
      return buildPermissionItem(entry, "tool");
    case "memory":
      return buildPermissionItem(entry, "memory");
    default:
      // Defensive: ignore any future / unknown trace kinds rather than
      // surfacing them as half-typed items.
      return null;
  }
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
