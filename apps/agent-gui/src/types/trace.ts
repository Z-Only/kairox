export type TraceEntryStatus = "running" | "completed" | "failed" | "pending";

export type TraceEntryKind =
  | "tool"
  | "permission"
  | "task_confirmation"
  | "memory"
  | "monitor"
  | "cancellation";

export interface ImageAttachment {
  media_type: string;
  data: string;
  label?: string | null;
}

export interface TaskConfirmationOption {
  id: string;
  label: string;
  description?: string | null;
}

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
  options?: TaskConfirmationOption[];
  allowMultiple?: boolean;
  allowCustom?: boolean;
  selectedOptionIds?: string[];
  customResponse?: string | null;
  /** Image attachments from tool output (e.g. screenshots). */
  images?: ImageAttachment[];
}
