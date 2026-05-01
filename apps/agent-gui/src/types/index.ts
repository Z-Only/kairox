export type ProjectedRole = "user" | "assistant";

export interface ProjectedMessage {
  role: ProjectedRole;
  content: string;
}

export interface SessionProjection {
  messages: ProjectedMessage[];
  task_titles: string[];
  token_stream: string;
  cancelled: boolean;
}

export type EventPayload =
  | { type: "WorkspaceOpened"; path: string }
  | { type: "UserMessageAdded"; message_id: string; content: string }
  | { type: "AgentTaskCreated"; task_id: string; title: string }
  | { type: "ModelTokenDelta"; delta: string }
  | { type: "AssistantMessageCompleted"; message_id: string; content: string }
  | { type: "ModelToolCallRequested"; tool_call_id: string; tool_id: string }
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
