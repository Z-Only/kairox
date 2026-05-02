export type TraceEntryStatus = "running" | "completed" | "failed" | "pending";

export type TraceEntryKind = "tool" | "permission" | "memory";

export interface TraceEntryData {
  id: string;
  kind: TraceEntryKind;
  status: TraceEntryStatus;
  toolId?: string;
  title: string;
  startedAt: number;
  durationMs?: number;
  input?: string;
  outputPreview?: string;
  outputFull?: string;
  rawEvent?: string;
  exitCode?: number | null;
  truncated?: boolean;
  expanded: boolean;
  scope?: string;
  content?: string;
  reason?: string;
}
