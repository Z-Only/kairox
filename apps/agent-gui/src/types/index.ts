// ===== Auto-generated types (from specta) =====
export type {
  EventPayload,
  DomainEvent,
  AgentRole,
  TaskState,
  TaskSnapshot,
  TaskGraphSnapshot,
  PrivacyClassification,
  MemoryScope
} from "../generated/events";

// ===== UI projection types (not generated from Rust) =====
export type ProjectedRole = "user" | "assistant";

export interface ProjectedMessage {
  role: ProjectedRole;
  content: string;
}

export interface SessionProjection {
  messages: ProjectedMessage[];
  task_titles: string[];
  task_graph: TaskGraphSnapshot;
  token_stream: string;
  cancelled: boolean;
}

// ===== Command response types (from tauri-specta) =====
export type {
  WorkspaceInfoResponse,
  SessionInfoResponse,
  MemoryEntryResponse,
  ProfileInfo,
  ProfileDetailResponse,
  TaskSnapshotResponse
} from "../generated/commands";

// ===== Session metadata (matches Rust SessionMeta but used independently) =====
export interface SessionMeta {
  session_id: string;
  workspace_id: string;
  title: string;
  model_profile: string;
  model_id: string | null;
  provider: string | null;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
}
