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
  | ChatPermissionGroupStreamItem
  | ChatCompactionStreamItem
  | ChatMonitorStreamItem
  | ChatCancellationStreamItem;

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
  /** Epoch milliseconds when the source trace entry started. */
  startedAt?: number;
  input?: string;
  outputPreview?: string;
  scope?: string;
  /** Structured image attachments from tool output (e.g. screenshots). */
  images?: Array<{ media_type: string; data: string; label?: string | null }>;
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

/**
 * Cluster of ≥2 consecutive pending permission prompts emitted by the
 * chat-stream fold. The builder replaces a run of consecutive pending
 * {@link ChatPermissionStreamItem} entries with one of these so the
 * chat panel can render a single "N pending permissions" badge instead
 * of N stacked prompts.
 *
 * Runs are broken by any non-permission item, any resolved permission
 * (accepted / denied), or a single lone pending permission (which stays
 * as the original {@link ChatPermissionStreamItem} variant).
 */
export interface ChatPermissionGroupStreamItem {
  kind: "permission_group";
  /** Synthetic id derived from the first underlying permission id. */
  id: string;
  /** `startedAt` of the FIRST pending permission in the run. */
  startedAt: number;
  /** Distinct tool ids present in the cluster, in first-seen order. */
  toolIds: string[];
  /** Underlying permission request ids in cluster order. */
  permissionIds: string[];
  /** Number of permission prompts in the cluster (equals `permissionIds.length`). */
  count: number;
}

export interface ChatCompactionStreamItem {
  kind: "compaction";
  /** Stable per compaction transition — derived from the status discriminator. */
  id: string;
  status: CompactionStatus;
}

export interface ChatMonitorStreamItem {
  kind: "monitor";
  id: string;
  description: string;
  status: "running" | "completed" | "failed";
  /** Most recent stdout line from the monitor process. */
  lastLine?: string;
  /** The shell command being monitored. */
  command?: string;
  /** Stop reason label when status is "completed". */
  stopReason?: string;
}

export interface ChatCancellationStreamItem {
  kind: "cancellation";
  id: string;
  reason?: string;
}
