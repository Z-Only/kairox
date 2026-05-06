//! Application facade — the primary integration point for Kairox.
//!
//! All UIs (TUI, GUI) interact with the runtime through the [`AppFacade`] trait.
//! This trait provides a stable, object-safe interface for workspace management,
//! session lifecycle, messaging, permissions, and event streaming.

use crate::{DomainEvent, SessionId, TaskId, WorkspaceId};
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Marketplace DTOs (mirror types)
// ---------------------------------------------------------------------------
//
// agent-core does not depend on agent-mcp (would create a dependency cycle).
// We define mirror DTOs here using only primitive/serde-friendly types and
// have `agent-runtime` translate between these and the canonical
// `agent_mcp::catalog` / `agent_mcp::installer` types.
//
// `install_spec_json`, `requirements_json`, and `default_env_json` carry
// their richer payloads as JSON-encoded strings so that callers (the GUI)
// receive enough information without having to re-derive cycle-creating
// types in `agent-core`.

/// A query against the catalog. All fields are optional; an empty query
/// returns every entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CatalogQuery {
    pub keyword: Option<String>,
    pub category: Option<String>,
    /// Minimum trust level (lower-case: "unverified" | "community" | "verified").
    pub trust_min: Option<String>,
    /// Filter by source id (e.g. "builtin").
    pub source: Option<String>,
    pub limit: Option<usize>,
}

/// A single MCP server entry returned by the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ServerEntry {
    pub id: String,
    pub source: String,
    pub display_name: String,
    pub summary: String,
    pub description: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    /// Lower-case trust level: "unverified" | "community" | "verified".
    pub trust: String,
    pub icon: Option<String>,
    /// JSON-encoded `agent_mcp::catalog::InstallSpec`.
    pub install_spec_json: String,
    /// JSON-encoded `Vec<agent_mcp::catalog::RuntimeRequirement>`.
    pub requirements_json: String,
    /// JSON-encoded `Vec<agent_mcp::catalog::EnvVarSpec>`.
    pub default_env_json: String,
}

/// A user-initiated install request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallRequest {
    pub catalog_id: String,
    pub source: String,
    pub server_id_override: Option<String>,
    pub env_overrides: BTreeMap<String, String>,
    pub trust_grant: bool,
    pub auto_start: bool,
}

/// Outcome of an install attempt. The `kind` field is a discriminator:
/// `"installed" | "runtime_missing" | "already_installed" | "invalid_env"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallOutcomeView {
    pub kind: String,
    pub server_id: Option<String>,
    pub started: Option<bool>,
    pub missing_runtimes: Vec<String>,
    pub missing_env_keys: Vec<String>,
}

/// An entry currently installed in the runtime (marketplace + hand-edited).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstalledEntry {
    pub server_id: String,
    pub catalog_id: Option<String>,
    pub source: Option<String>,
    pub display_name: String,
    pub installed_at: String,
    pub running: bool,
}

/// A configured remote catalog source (Phase 2). Mirror DTO of
/// `agent_mcp::RemoteSourceConfig` plus the implicit builtin source.
/// Lives in `agent-core` because the GUI needs to render it without
/// depending on `agent-mcp`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CatalogSourceView {
    pub id: String,
    pub display_name: String,
    /// Lower-case kind discriminator: "builtin" | "kairox_json" | "smithery".
    pub kind: String,
    /// Empty for the builtin source.
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: u32,
    /// Lower-case trust level cap: "unverified" | "community" | "verified".
    pub default_trust: String,
    pub enabled: bool,
    pub cache_ttl_seconds: Option<u64>,
    /// Last error observed when querying this source, if any.
    pub last_error: Option<String>,
}

/// Request body for `add_catalog_source`. Mirrors
/// `agent_mcp::RemoteSourceConfig` field-for-field; the runtime fills in
/// defaults for omitted optional fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AddCatalogSourceRequest {
    pub id: String,
    pub display_name: String,
    /// "kairox_json" | "smithery"
    pub kind: String,
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: Option<u32>,
    pub default_trust: Option<String>,
    pub enabled: Option<bool>,
    pub cache_ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Workspace metadata returned after opening a workspace.
pub struct WorkspaceInfo {
    pub workspace_id: WorkspaceId,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Request to start a new agent session within a workspace.
pub struct StartSessionRequest {
    pub workspace_id: WorkspaceId,
    pub model_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Request to send a user message to an active session.
pub struct SendMessageRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// User decision on a permission request (approve or deny).
pub struct PermissionDecision {
    pub request_id: String,
    pub approve: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A single trace entry wrapping a domain event, used for trace panel display.
pub struct TraceEntry {
    pub event: DomainEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Metadata for a session, used for listing and display.
pub struct SessionMeta {
    pub session_id: SessionId,
    pub workspace_id: WorkspaceId,
    pub title: String,
    pub model_profile: String,
    pub model_id: Option<String>,
    pub provider: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// A snapshot of a single task in the task graph.
pub struct TaskSnapshot {
    pub id: TaskId,
    pub title: String,
    pub role: crate::AgentRole,
    pub state: crate::TaskState,
    pub dependencies: Vec<TaskId>,
    pub error: Option<String>,
    pub retry_count: usize,
    pub max_retries: usize,
    pub assigned_agent_id: Option<String>,
    pub failure_reason: Option<crate::TaskFailureReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// A snapshot of the entire task graph for a session.
pub struct TaskGraphSnapshot {
    pub tasks: Vec<TaskSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// Status information about a running or completed agent.
pub struct AgentStatusInfo {
    pub agent_id: String,
    pub role: crate::AgentRole,
    pub task_id: Option<TaskId>,
    pub status: String,
}

#[async_trait]
/// The primary integration point for Kairox.
///
/// All user interfaces (TUI, GUI) interact with the runtime through this trait.
/// The canonical implementation is [`crate::LocalRuntime`](agent_runtime::LocalRuntime),
/// but any mock or test implementation can substitute.
///
/// # Object Safety
///
/// This trait is object-safe and can be used as `dyn AppFacade`.
pub trait AppFacade: Send + Sync {
    /// Open a workspace at the given filesystem path. Returns workspace metadata.
    async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo>;
    /// Start a new agent session within a workspace using the specified model profile.
    async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId>;
    /// Send a user message to an active session. The agent loop runs in the background.
    async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()>;
    /// Submit a user decision on a pending permission request.
    async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()>;
    /// Cancel a running session.
    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> crate::Result<()>;
    /// Get the projected (rolled-up) state of a session, including messages and task titles.
    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> crate::Result<crate::projection::SessionProjection>;
    /// Get the full trace of domain events for a session.
    async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>>;
    /// Subscribe to a real-time stream of domain events for a session.
    fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent>;
    /// Subscribe to a real-time stream of all domain events across all sessions.
    fn subscribe_all(&self) -> BoxStream<'static, DomainEvent>;
    /// List all workspaces known to the runtime.
    async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>>;
    /// List all sessions in a workspace, including soft-deleted ones.
    async fn list_sessions(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<SessionMeta>>;
    /// Rename a session.
    async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()>;
    /// Soft-delete a session (marks as deleted without removing data).
    async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()>;
    /// Clean up sessions that were soft-deleted longer than the specified duration ago.
    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> crate::Result<usize>;
    /// Get the current task graph snapshot for a session.
    async fn get_task_graph(&self, session_id: SessionId) -> crate::Result<TaskGraphSnapshot>;
    /// Retry a failed or blocked task, resetting it to pending and unblocking dependents.
    async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()>;
    /// Cancel a specific task in the session's task graph.
    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()>;
    /// Get the status of all agents associated with a session's task graph.
    async fn get_agent_status(&self, session_id: SessionId) -> crate::Result<Vec<AgentStatusInfo>>;

    // -----------------------------------------------------------------------
    // Marketplace catalog (Phase 1: built-in catalog only).
    // -----------------------------------------------------------------------

    /// List catalog entries, optionally filtered by `query`.
    async fn list_catalog(&self, query: CatalogQuery) -> crate::Result<Vec<ServerEntry>> {
        let _ = query;
        Ok(Vec::new())
    }

    /// Get a single catalog entry by id (and optional source filter).
    async fn get_catalog_entry(
        &self,
        id: String,
        source: Option<String>,
    ) -> crate::Result<Option<ServerEntry>> {
        let _ = (id, source);
        Ok(None)
    }

    /// Refresh catalog data from all (or one named) source.
    async fn refresh_catalog(&self, source: Option<String>) -> crate::Result<()> {
        let _ = source;
        Ok(())
    }

    /// Install a catalog entry, returning a structured outcome.
    async fn install_catalog_entry(
        &self,
        request: InstallRequest,
    ) -> crate::Result<InstallOutcomeView> {
        let _ = request;
        Ok(InstallOutcomeView {
            kind: "runtime_missing".into(),
            server_id: None,
            started: None,
            missing_runtimes: Vec::new(),
            missing_env_keys: Vec::new(),
        })
    }

    /// Uninstall a previously installed entry.
    async fn uninstall_catalog_entry(&self, server_id: String) -> crate::Result<()> {
        let _ = server_id;
        Ok(())
    }

    /// List entries currently installed (marketplace + hand-edited).
    async fn list_installed_entries(&self) -> crate::Result<Vec<InstalledEntry>> {
        Ok(Vec::new())
    }

    // -----------------------------------------------------------------------
    // Marketplace catalog sources (Phase 2: remote sources).
    // -----------------------------------------------------------------------

    /// List all configured catalog sources, including the implicit builtin
    /// source. Always includes `last_error` if the most recent fetch failed.
    async fn list_catalog_sources(&self) -> crate::Result<Vec<CatalogSourceView>> {
        Ok(vec![CatalogSourceView {
            id: "builtin".into(),
            display_name: "Built-in".into(),
            kind: "builtin".into(),
            url: String::new(),
            api_key_env: None,
            priority: 0,
            default_trust: "verified".into(),
            enabled: true,
            cache_ttl_seconds: None,
            last_error: None,
        }])
    }

    /// Add a new remote catalog source. Persists to the marketplace TOML
    /// and registers the provider with the runtime.
    async fn add_catalog_source(&self, request: AddCatalogSourceRequest) -> crate::Result<()> {
        let _ = request;
        Ok(())
    }

    /// Remove a remote catalog source by id. Removing the builtin source
    /// is a no-op (it cannot be removed).
    async fn remove_catalog_source(&self, id: String) -> crate::Result<()> {
        let _ = id;
        Ok(())
    }

    /// Enable or disable a catalog source without removing it.
    async fn set_catalog_source_enabled(&self, id: String, enabled: bool) -> crate::Result<()> {
        let _ = (id, enabled);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_facade_is_object_safe(_: &dyn AppFacade) {}

    struct NoopFacade;

    #[async_trait]
    impl AppFacade for NoopFacade {
        async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo> {
            Ok(WorkspaceInfo {
                workspace_id: WorkspaceId::new(),
                path,
            })
        }

        async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId> {
            let _ = request;
            Ok(SessionId::new())
        }

        async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()> {
            let _ = request;
            Ok(())
        }

        async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()> {
            let _ = decision;
            Ok(())
        }

        /// Cancel a running session.
        async fn cancel_session(
            &self,
            workspace_id: WorkspaceId,
            session_id: SessionId,
        ) -> crate::Result<()> {
            let _ = (workspace_id, session_id);
            Ok(())
        }

        /// Get the projected (rolled-up) state of a session, including messages and task titles.
        async fn get_session_projection(
            &self,
            session_id: SessionId,
        ) -> crate::Result<crate::projection::SessionProjection> {
            let _ = session_id;
            Ok(crate::projection::SessionProjection::default())
        }

        async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>> {
            let _ = session_id;
            Ok(Vec::new())
        }

        fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent> {
            let _ = session_id;
            Box::pin(futures::stream::empty())
        }

        fn subscribe_all(&self) -> BoxStream<'static, DomainEvent> {
            Box::pin(futures::stream::empty())
        }

        async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>> {
            Ok(Vec::new())
        }

        async fn list_sessions(
            &self,
            workspace_id: &WorkspaceId,
        ) -> crate::Result<Vec<SessionMeta>> {
            let _ = workspace_id;
            Ok(Vec::new())
        }

        async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()> {
            let _ = (session_id, title);
            Ok(())
        }

        async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
            let _ = session_id;
            Ok(())
        }

        async fn cleanup_expired_sessions(
            &self,
            older_than: std::time::Duration,
        ) -> crate::Result<usize> {
            let _ = older_than;
            Ok(0)
        }

        async fn get_task_graph(&self, session_id: SessionId) -> crate::Result<TaskGraphSnapshot> {
            let _ = session_id;
            Ok(TaskGraphSnapshot::default())
        }

        async fn retry_task(
            &self,
            workspace_id: WorkspaceId,
            session_id: SessionId,
            task_id: TaskId,
        ) -> crate::Result<()> {
            let _ = (workspace_id, session_id, task_id);
            Ok(())
        }

        async fn cancel_task(
            &self,
            workspace_id: WorkspaceId,
            session_id: SessionId,
            task_id: TaskId,
        ) -> crate::Result<()> {
            let _ = (workspace_id, session_id, task_id);
            Ok(())
        }

        async fn get_agent_status(
            &self,
            session_id: SessionId,
        ) -> crate::Result<Vec<AgentStatusInfo>> {
            let _ = session_id;
            Ok(Vec::new())
        }
    }

    #[test]
    fn facade_is_object_safe() {
        let facade = NoopFacade;
        assert_facade_is_object_safe(&facade);
    }
}
