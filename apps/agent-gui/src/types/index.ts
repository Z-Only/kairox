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

export type AgentRole = "Planner" | "Worker" | "Reviewer";
export type TaskState =
  | "Pending"
  | "Running"
  | "Blocked"
  | "Completed"
  | "Failed"
  | "Cancelled";

export interface TaskSnapshot {
  id: string;
  title: string;
  role: AgentRole;
  state: TaskState;
  dependencies: string[];
  error: string | null;
}

export interface TaskGraphSnapshot {
  tasks: TaskSnapshot[];
}

export type EventPayload =
  | { type: "WorkspaceOpened"; path: string }
  | { type: "SessionInitialized"; model_profile: string }
  | { type: "UserMessageAdded"; message_id: string; content: string }
  | {
      type: "AgentTaskCreated";
      task_id: string;
      title: string;
      role: AgentRole;
      dependencies: string[];
    }
  | { type: "AgentTaskStarted"; task_id: string }
  | { type: "AgentTaskCompleted"; task_id: string }
  | { type: "AgentTaskFailed"; task_id: string; error: string }
  | { type: "ContextAssembled"; token_estimate: number; sources: string[] }
  | { type: "ModelRequestStarted"; model_profile: string; model_id: string }
  | { type: "ModelTokenDelta"; delta: string }
  | { type: "ModelToolCallRequested"; tool_call_id: string; tool_id: string }
  | { type: "AssistantMessageCompleted"; message_id: string; content: string }
  | {
      type: "PermissionRequested";
      request_id: string;
      tool_id: string;
      preview: string;
    }
  | { type: "PermissionGranted"; request_id: string }
  | { type: "PermissionDenied"; request_id: string; reason: string }
  | { type: "ToolInvocationStarted"; invocation_id: string; tool_id: string }
  | {
      type: "ToolInvocationCompleted";
      invocation_id: string;
      tool_id: string;
      output_preview: string;
      exit_code: number | null;
      duration_ms: number;
      truncated: boolean;
    }
  | {
      type: "ToolInvocationFailed";
      invocation_id: string;
      tool_id: string;
      error: string;
    }
  | { type: "FilePatchProposed"; patch_id: string; diff: string }
  | { type: "FilePatchApplied"; patch_id: string }
  | {
      type: "MemoryProposed";
      memory_id: string;
      scope: string;
      key: string | null;
      content: string;
    }
  | {
      type: "MemoryAccepted";
      memory_id: string;
      scope: string;
      key: string | null;
      content: string;
    }
  | { type: "MemoryRejected"; memory_id: string; reason: string }
  | {
      type: "ReviewerFindingAdded";
      finding_id: string;
      severity: string;
      message: string;
    }
  | { type: "SessionCancelled"; reason: string }
  | { type: string };

export interface DomainEvent {
  schema_version: number;
  workspace_id: string;
  session_id: string;
  timestamp: string;
  source_agent_id: string;
  privacy: string;
  event_type: string;
  payload: EventPayload;
}

export interface SessionInfoResponse {
  id: string;
  title: string;
  profile: string;
}

export interface WorkspaceInfoResponse {
  workspace_id: string;
  path: string;
}

export interface ProfileInfo {
  alias: string;
  provider: string;
  model_id: string;
  local: boolean;
  has_api_key: boolean;
}

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

export interface ProfileDetail {
  alias: string;
  provider: string;
  model_id: string;
  local: boolean;
  has_api_key: boolean;
}
