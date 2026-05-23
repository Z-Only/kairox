import type { DomainEvent } from "../types";

// ---------------------------------------------------------------------------
// Helper: build a DomainEvent with sensible defaults
// ---------------------------------------------------------------------------

export function makeEvent(payload: DomainEvent["payload"]): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "ws-1",
    session_id: "sess-1",
    timestamp: new Date().toISOString(),
    source_agent_id: "agent-1",
    privacy: "full_trace",
    event_type: payload.type,
    payload
  };
}
