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

// ===== P3 hand-written mirrors for context-mgmt types =====
//
// These shapes mirror the Rust source-of-truth definitions 1:1 and are
// hand-maintained because the local `just gen-types` pipeline (which runs
// `cargo run --bin export-specta`) currently OOMs on this machine before
// emitting the generated bindings. Task 12 of the P3 plan
// (`docs/superpowers/plans/2026-05-08-context-p3-ui-context-meter.md`)
// runs the full type-gen + check-types loop in a clean environment and
// will replace this block with re-exports from "../generated/events".
// Until then this block is the canonical TS surface for these types and
// MUST be kept in lock-step with the Rust definitions listed below.
//
// Source-of-truth files (each must match field-for-field):
//   - crates/agent-core/src/context_types.rs   (ContextSource, ContextUsage)
//   - crates/agent-core/src/projection.rs      (ProjectedModelLimits, CompactionStatus)
//   - crates/agent-models/src/model_registry.rs (LimitSource, ModelLimits)
//   - crates/agent-core/src/events.rs          (CompactionReason)
//
// Serde conventions used below:
//   - `#[serde(rename_all = "snake_case")]` enums → snake_case string literals
//   - `#[serde(tag = "type")]` enums             → discriminated unions on `type`
//   - Rust tuples (e.g. `(ContextSource, u64)`)  → JSON arrays
//
// Verified against the Rust source on 2026-05-09 at HEAD 27c9e0a.

/** Mirrors `crates/agent-core/src/context_types.rs::ContextSource`. */
export type ContextSource =
  | "system"
  | "tool_definitions"
  | "request"
  | "memory"
  | "history"
  | "tool_result"
  | "selected_file"
  | "compaction_summary";

/** Mirrors `crates/agent-core/src/context_types.rs::ContextUsage`. */
export interface ContextUsage {
  total_tokens: number;
  budget_tokens: number;
  context_window: number;
  output_reservation: number;
  /** Rust `Vec<(ContextSource, u64)>` serialises as `[source, count][]`. */
  by_source: [ContextSource, number][];
  estimator: string;
  corrected_by_real_usage: boolean;
}

/** Mirrors `crates/agent-models/src/model_registry.rs::LimitSource`. */
export type LimitSource = "user_config" | "builtin_registry" | "runtime_probe" | "fallback";

/** Mirrors `crates/agent-models/src/model_registry.rs::ModelLimits`. */
export interface ModelLimits {
  context_window: number;
  output_limit: number;
  source: LimitSource;
}

/**
 * Mirrors `crates/agent-core/src/projection.rs::ProjectedModelLimits`.
 *
 * `source` is intentionally a plain `string` (not `LimitSource`) because the
 * Rust struct stores the discriminant as `String` to avoid the
 * `agent-core` ← `agent-models` dependency boundary. Values are still drawn
 * from the snake-case `LimitSource` set: "user_config" | "builtin_registry"
 * | "runtime_probe" | "fallback".
 */
export interface ProjectedModelLimits {
  context_window: number;
  output_limit: number;
  source: string;
}

/** Mirrors `crates/agent-core/src/projection.rs::CompactionStatus`. */
export type CompactionStatus =
  | { type: "Idle" }
  | { type: "Running" }
  | { type: "Failed"; error: string };

/**
 * Mirrors `crates/agent-core/src/events.rs::CompactionReason`.
 *
 * Only two variants exist in the Rust enum — do not invent additional ones.
 */
export type CompactionReason = { type: "UserRequested" } | { type: "Threshold"; ratio: number };

// ===== UI projection types (not generated from Rust) =====

/**
 * Extended role types for message attribution.
 * Phase 1 had only "user" | "assistant".
 * Phase 3 adds planner/worker/reviewer for multi-agent DAG execution.
 */
export type ProjectedRole = "user" | "assistant" | "planner" | "worker" | "reviewer" | "system";

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
  /** P3: last context usage snapshot (set by `ContextAssembled` events). */
  last_context_usage: ContextUsage | null;
  /** P3: resolved model limits for the current profile. */
  model_limits: ProjectedModelLimits | null;
  /** P3: compaction lifecycle status (Idle / Running / Failed). */
  compaction: CompactionStatus;
}

// ===== Command response types (from tauri-specta) =====
export type {
  WorkspaceInfoResponse,
  SessionInfoResponse,
  MemoryEntryResponse,
  ProfileInfo,
  ProfileWithLimits,
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
export function agentRoleToProjectedRole(role: AgentRole): "planner" | "worker" | "reviewer" {
  const map: Record<AgentRole, "planner" | "worker" | "reviewer"> = {
    Planner: "planner",
    Worker: "worker",
    Reviewer: "reviewer"
  };
  return map[role] || "worker";
}
