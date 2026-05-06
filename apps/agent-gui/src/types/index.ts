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

/**
 * Extended role types for message attribution.
 * Phase 1 had only "user" | "assistant".
 * Phase 3 adds planner/worker/reviewer for multi-agent DAG execution.
 */
export type ProjectedRole =
  | "user"
  | "assistant"
  | "planner"
  | "worker"
  | "reviewer"
  | "system";

export interface ProjectedMessage {
  role: ProjectedRole;
  content: string;
  /** The agent that produced this message, when applicable. */
  sourceAgentId?: string;
  /** The task this message is associated with, when applicable. */
  taskId?: string;
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
  TaskSnapshotResponse,
  McpServerStatusResponse,
  McpToolDefResponse,
  McpResourceDefResponse,
  McpPromptDefResponse,
  McpContentBlockResponse
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

/**
 * Map an AgentRole from the generated types to a ProjectedRole.
 * Used when attributing messages to specific agents.
 */
export function agentRoleToProjectedRole(
  role: AgentRole
): "planner" | "worker" | "reviewer" {
  const map: Record<AgentRole, "planner" | "worker" | "reviewer"> = {
    Planner: "planner",
    Worker: "worker",
    Reviewer: "reviewer"
  };
  return map[role] || "worker";
}
