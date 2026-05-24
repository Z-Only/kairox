/**
 * Discriminated union for the unified chat stream.
 *
 * The chat-stream spine folds three previously-separate UI feeds —
 * the projection's chronological messages, the trace store's
 * tool / permission / memory entries, and the projection's
 * context-compaction status — into a single chronologically-ordered
 * list that ChatPanel will render in a later PR.
 *
 * This file defines the value-shape only; the fold itself lives in
 * `apps/agent-gui/src/composables/useChatStream.ts`.
 */
import type { ProjectedRole, CompactionStatus } from "@/types";

export type ChatStreamItem =
  | ChatMessageStreamItem
  | ChatToolCallStreamItem
  | ChatPermissionStreamItem
  | ChatCompactionStreamItem;

export interface ChatMessageStreamItem {
  kind: "message";
  /** Stable per message — derived from the message's index in `projection.messages`. */
  id: string;
  role: ProjectedRole;
  content: string;
  sourceAgentId?: string;
}

export interface ChatToolCallStreamItem {
  kind: "tool_call";
  /** Trace entry id — stable across the entry's lifecycle. */
  id: string;
  toolId: string;
  title?: string;
  status: "running" | "completed" | "failed" | "pending";
  durationMs?: number;
  input?: string;
  outputPreview?: string;
  scope?: string;
}

export interface ChatPermissionStreamItem {
  kind: "permission";
  /**
   * Trace entry id — for tool permissions this equals the request id
   * accepted by the `resolve_permission` IPC command.
   */
  id: string;
  toolId?: string;
  title?: string;
  input?: string;
  reason?: string;
  scope?: string;
  content?: string;
  rawEvent?: string;
  /**
   * Source of the permission entry:
   *   - `"tool"`   → derived from a trace entry with `kind === "permission"`
   *   - `"memory"` → derived from a trace entry with `kind === "memory"`
   */
  variant: "tool" | "memory";
}

export interface ChatCompactionStreamItem {
  kind: "compaction";
  /** Stable per compaction transition — derived from the status discriminator. */
  id: string;
  status: CompactionStatus;
}
