//! Application facade — the primary integration point for Kairox.
//!
//! All UIs (TUI, GUI) interact with the runtime through the [`AppFacade`] trait.
//! This trait provides a stable, object-safe interface for workspace management,
//! session lifecycle, messaging, permissions, and event streaming.

use crate::{DomainEvent, ProjectId, SessionId, TaskId, WorkspaceId};
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
    /// Lower-case kind discriminator: "builtin" | "mcp_registry".
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
    /// "mcp_registry"
    pub kind: String,
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: Option<u32>,
    pub default_trust: Option<String>,
    pub enabled: Option<bool>,
    pub cache_ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub source: String,
    pub activation_mode: String,
    pub keywords: Vec<String>,
    pub tools: Vec<String>,
    pub can_request_tools: Vec<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillDetail {
    pub view: SkillView,
    pub body_markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ActivateSkillRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub skill_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DeactivateSkillRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub skill_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ActiveSkillView {
    pub skill_id: String,
    pub name: String,
    pub source: String,
    pub activation_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpServerSettingsTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
    },
    Sse {
        url: String,
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpServerSettingsInput {
    pub name: String,
    pub transport: McpServerSettingsTransport,
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpServerSettingsView {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub runtime_status: String,
    pub trusted: bool,
    pub tool_count: Option<usize>,
    pub last_error: Option<String>,
    pub writable: bool,
    pub config_path: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProfileSettingsInput {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    #[serde(default = "crate::facade::default_true")]
    pub enabled: bool,
    pub context_window: Option<u64>,
    pub output_limit: Option<u64>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub max_tokens: Option<u64>,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProfileSettingsView {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub enabled: bool,
    pub context_window: Option<u64>,
    pub output_limit: Option<u64>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub max_tokens: Option<u64>,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub has_api_key: bool,
    pub writable: bool,
    pub config_path: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillSettingsScope {
    Project,
    User,
    Builtin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillInstallSource {
    Local,
    Registry,
    Github,
    Builtin,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillUpdateState {
    Unknown,
    UpToDate,
    UpdateAvailable,
    CheckFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillSettingsView {
    pub settings_id: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub scope: SkillSettingsScope,
    pub path: String,
    pub enabled: bool,
    pub activation_mode: String,
    pub install_source: SkillInstallSource,
    pub update_state: SkillUpdateState,
    pub effective: bool,
    pub shadowed_by: Option<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
    pub editable: bool,
    pub deletable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillSettingsDetail {
    pub view: SkillSettingsView,
    pub content: String,
    pub source_chain: Vec<SkillSettingsView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RemoteSkillSearchResult {
    pub name: String,
    pub description: String,
    pub repository: Option<String>,
    pub install_count: Option<u64>,
    pub source_url: String,
    pub package: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillInstallTarget {
    Project,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallRemoteSkillRequest {
    pub package: String,
    pub source: String,
    pub target: SkillInstallTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallGithubSkillRequest {
    pub source: String,
    pub target: SkillInstallTarget,
}

// ── Skills catalog / marketplace ───────────────────────────────────────

/// A single skill entry returned by the skills catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillCatalogEntry {
    pub catalog_id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub source_url: String,
    pub install_count: Option<u64>,
    pub github_stars: Option<u64>,
    pub security_score: Option<u32>,
    pub rating: Option<f64>,
    pub package: String,
}

/// Query against the skills catalog.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillCatalogQuery {
    pub keyword: Option<String>,
    pub sources: Option<Vec<String>>,
    pub limit: Option<usize>,
}

/// JSON field mapping for a skill source API response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillFieldMappingView {
    pub name_path: String,
    pub description_path: String,
    pub install_count_path: Option<String>,
    pub github_stars_path: Option<String>,
    pub package_path: String,
    pub source_url_path: Option<String>,
}

impl Default for SkillFieldMappingView {
    fn default() -> Self {
        Self {
            name_path: "name".into(),
            description_path: "description".into(),
            install_count_path: Some("installs".into()),
            github_stars_path: None,
            package_path: "id".into(),
            source_url_path: None,
        }
    }
}

/// A configured skill catalog source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillSourceView {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub url: String,
    pub search_template: String,
    pub list_template: Option<String>,
    pub field_mapping: SkillFieldMappingView,
    pub enabled: bool,
    pub priority: u32,
    pub cache_ttl_seconds: u64,
    pub last_error: Option<String>,
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
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// Metadata for a file attached to a user message.
pub struct AttachmentInfo {
    /// Absolute filesystem path.
    pub path: String,
    /// Display filename.
    pub name: String,
    /// MIME type (e.g. "image/png", "application/pdf").
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Request to send a user message to an active session.
pub struct SendMessageRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub content: String,
    pub attachments: Vec<AttachmentInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// User decision on a permission request (approve or deny).
pub struct PermissionDecision {
    pub request_id: String,
    pub approve: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// A single trace entry wrapping a domain event, used for trace panel display.
///
/// Note: only `PartialEq` (not `Eq`) because the wrapped `DomainEvent::payload`
/// contains `f32` fields (`ContextUsage`, `CompactionReason::Threshold { ratio }`).
pub struct TraceEntry {
    pub event: DomainEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ProjectSessionVisibility {
    DraftHidden,
    Visible,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ProjectGitStatusKind {
    NotInitialized,
    Clean,
    Dirty,
    Detached,
    MissingPath,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectMeta {
    pub project_id: ProjectId,
    pub display_name: String,
    pub root_path: String,
    pub created_at: String,
    pub updated_at: String,
    pub removed_at: Option<String>,
    pub sort_order: i64,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectSessionBinding {
    pub session_id: SessionId,
    pub project_id: ProjectId,
    pub worktree_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectGitStatus {
    pub kind: ProjectGitStatusKind,
    pub branch: Option<String>,
    pub worktree_path: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectInstructionSummary {
    pub source_paths: Vec<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Metadata for a session, used for listing and display.
pub struct SessionMeta {
    pub project_id: Option<ProjectId>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub visibility: Option<ProjectSessionVisibility>,
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
    // Skills (native skill registry and session activation).
    // -----------------------------------------------------------------------

    /// List all discovered skills.
    async fn list_skills(&self) -> crate::Result<Vec<SkillView>> {
        Ok(Vec::new())
    }

    /// Get a single skill by id.
    async fn get_skill(&self, skill_id: String) -> crate::Result<Option<SkillDetail>> {
        let _ = skill_id;
        Ok(None)
    }

    /// Activate a skill for a session.
    async fn activate_skill(
        &self,
        request: ActivateSkillRequest,
    ) -> crate::Result<ActiveSkillView> {
        let _ = request;
        Err(crate::CoreError::InvalidState(
            "skill activation not supported".into(),
        ))
    }

    /// Deactivate a skill for a session.
    async fn deactivate_skill(&self, request: DeactivateSkillRequest) -> crate::Result<()> {
        let _ = request;
        Ok(())
    }

    /// List active skills for a session.
    async fn list_active_skills(
        &self,
        session_id: SessionId,
    ) -> crate::Result<Vec<ActiveSkillView>> {
        let _ = session_id;
        Ok(Vec::new())
    }

    /// List configured MCP servers for settings UI.
    async fn list_mcp_server_settings(&self) -> crate::Result<Vec<McpServerSettingsView>> {
        Ok(Vec::new())
    }

    /// Create or update an MCP server from settings UI.
    async fn upsert_mcp_server_settings(
        &self,
        input: McpServerSettingsInput,
    ) -> crate::Result<McpServerSettingsView> {
        let _ = input;
        Err(crate::CoreError::InvalidState(
            "MCP settings mutation not supported".into(),
        ))
    }

    /// Delete an MCP server from settings UI.
    async fn delete_mcp_server_settings(&self, server_id: String) -> crate::Result<()> {
        let _ = server_id;
        Err(crate::CoreError::InvalidState(
            "MCP settings deletion not supported".into(),
        ))
    }

    /// Enable or disable an MCP server from settings UI.
    async fn set_mcp_server_enabled(&self, server_id: String, enabled: bool) -> crate::Result<()> {
        let _ = (server_id, enabled);
        Err(crate::CoreError::InvalidState(
            "MCP settings enablement not supported".into(),
        ))
    }

    /// Open the MCP configuration file and return the path if available.
    async fn open_mcp_config_file(&self) -> crate::Result<Option<String>> {
        Ok(None)
    }

    /// List all configured model profiles for settings UI.
    async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> crate::Result<Vec<ProfileSettingsView>> {
        let _ = source_filter;
        Ok(Vec::new())
    }

    /// Create or update a model profile from settings UI.
    async fn upsert_profile_settings(
        &self,
        input: ProfileSettingsInput,
    ) -> crate::Result<ProfileSettingsView> {
        let _ = input;
        Err(crate::CoreError::InvalidState(
            "profile settings mutation not supported".into(),
        ))
    }

    /// Enable or disable a model profile from settings UI.
    async fn set_profile_enabled(&self, alias: String, enabled: bool) -> crate::Result<()> {
        let _ = (alias, enabled);
        Err(crate::CoreError::InvalidState(
            "profile settings enablement not supported".into(),
        ))
    }

    /// Delete a model profile from settings UI.
    async fn delete_profile_settings(&self, alias: String) -> crate::Result<()> {
        let _ = alias;
        Err(crate::CoreError::InvalidState(
            "profile settings deletion not supported".into(),
        ))
    }

    /// Move a profile up or down in display order.
    async fn move_profile_in_order(&self, alias: String, direction: i32) -> crate::Result<()> {
        let _ = (alias, direction);
        Err(crate::CoreError::InvalidState(
            "profile ordering not supported".into(),
        ))
    }

    /// Open the config directory in the system file manager.
    async fn open_config_dir(&self) -> crate::Result<Option<String>> {
        Ok(None)
    }

    /// List skills for settings UI.
    async fn list_skill_settings(&self) -> crate::Result<Vec<SkillSettingsView>> {
        Ok(Vec::new())
    }

    /// Get settings details for a single skill.
    async fn get_skill_settings_detail(
        &self,
        skill_id: String,
    ) -> crate::Result<Option<SkillSettingsDetail>> {
        let _ = skill_id;
        Ok(None)
    }

    /// Enable or disable a skill from settings UI.
    async fn set_skill_enabled(&self, skill_id: String, enabled: bool) -> crate::Result<()> {
        let _ = (skill_id, enabled);
        Err(crate::CoreError::InvalidState(
            "Skill settings enablement not supported".into(),
        ))
    }

    /// Delete a skill from settings UI.
    async fn delete_skill_settings(&self, skill_id: String) -> crate::Result<()> {
        let _ = skill_id;
        Err(crate::CoreError::InvalidState(
            "Skill deletion not supported".into(),
        ))
    }

    /// Search remote skills using the configured package source.
    async fn search_remote_skills(
        &self,
        query: String,
    ) -> crate::Result<Vec<RemoteSkillSearchResult>> {
        let _ = query;
        Ok(Vec::new())
    }

    /// Install a skill from a remote package source.
    async fn install_remote_skill(
        &self,
        request: InstallRemoteSkillRequest,
    ) -> crate::Result<SkillSettingsView> {
        let _ = request;
        Err(crate::CoreError::InvalidState(
            "Skill install not supported".into(),
        ))
    }

    /// Install a skill from a GitHub source.
    async fn install_github_skill(
        &self,
        request: InstallGithubSkillRequest,
    ) -> crate::Result<SkillSettingsView> {
        let _ = request;
        Err(crate::CoreError::InvalidState(
            "GitHub Skill install not supported".into(),
        ))
    }

    /// Update an installed skill.
    async fn update_skill(&self, skill_id: String) -> crate::Result<SkillSettingsView> {
        let _ = skill_id;
        Err(crate::CoreError::InvalidState(
            "Skill update not supported".into(),
        ))
    }

    // ── Skills catalog / marketplace ─────────────────────────────────

    /// List skill catalog entries, optionally filtered by query.
    async fn list_skill_catalog(
        &self,
        _query: SkillCatalogQuery,
    ) -> crate::Result<Vec<SkillCatalogEntry>> {
        Ok(Vec::new())
    }

    /// List configured skill catalog sources (includes builtins).
    async fn list_skill_sources(&self) -> crate::Result<Vec<SkillSourceView>> {
        Ok(Vec::new())
    }

    /// Add a new skill catalog source.
    async fn add_skill_source(&self, _config: SkillSourceView) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "skill sources not configured".into(),
        ))
    }

    /// Remove a skill catalog source.
    async fn remove_skill_source(&self, _id: String) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "skill sources not configured".into(),
        ))
    }

    /// Enable or disable a skill catalog source.
    async fn set_skill_source_enabled(&self, _id: String, _enabled: bool) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "skill sources not configured".into(),
        ))
    }

    /// Refresh skill catalog data from all sources.
    async fn refresh_skill_catalog(&self) -> crate::Result<()> {
        Ok(())
    }

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

    async fn list_projects(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<ProjectMeta>> {
        let _ = workspace_id;
        Ok(Vec::new())
    }

    async fn create_blank_project(
        &self,
        workspace_id: WorkspaceId,
        display_name: Option<String>,
    ) -> crate::Result<ProjectMeta> {
        let _ = workspace_id;
        let _ = display_name;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn add_existing_project(
        &self,
        workspace_id: WorkspaceId,
        path: String,
    ) -> crate::Result<ProjectMeta> {
        let _ = workspace_id;
        let _ = path;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn rename_project(
        &self,
        project_id: ProjectId,
        display_name: String,
    ) -> crate::Result<()> {
        let _ = project_id;
        let _ = display_name;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn remove_project(&self, project_id: ProjectId) -> crate::Result<()> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn restore_project_session(&self, session_id: SessionId) -> crate::Result<ProjectMeta> {
        let _ = session_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn update_project_order(&self, project_ids: Vec<ProjectId>) -> crate::Result<()> {
        let _ = project_ids;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn update_project_expanded(
        &self,
        project_id: ProjectId,
        expanded: bool,
    ) -> crate::Result<()> {
        let _ = project_id;
        let _ = expanded;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn create_project_draft_session(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<SessionId> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn create_project_worktree_session(
        &self,
        project_id: ProjectId,
        branch_name: String,
    ) -> crate::Result<SessionId> {
        let _ = project_id;
        let _ = branch_name;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn list_project_sessions(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<Vec<SessionMeta>> {
        let _ = project_id;
        Ok(Vec::new())
    }

    async fn list_archived_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> crate::Result<Vec<SessionMeta>> {
        let _ = workspace_id;
        Ok(Vec::new())
    }

    async fn get_project_git_status(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<ProjectGitStatus> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn get_session_git_status(
        &self,
        session_id: SessionId,
    ) -> crate::Result<ProjectGitStatus> {
        let _ = session_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn init_project_git(&self, project_id: ProjectId) -> crate::Result<ProjectGitStatus> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn get_project_instruction_summary(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<ProjectInstructionSummary> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }
}

#[cfg(test)]
mod facade_settings_dtos {
    use super::*;

    #[test]
    fn mcp_settings_input_serializes_stdio_transport() {
        let input = McpServerSettingsInput {
            name: "filesystem".to_string(),
            transport: McpServerSettingsTransport::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-filesystem".to_string(),
                ],
                env: BTreeMap::from([("ROOT".to_string(), "/tmp".to_string())]),
            },
            enabled: true,
            description: Some("Local files".to_string()),
        };

        let encoded = serde_json::to_string(&input).expect("input should serialize");
        assert!(encoded.contains("filesystem"));
        assert!(encoded.contains("stdio"));
    }

    #[test]
    fn skill_settings_view_distinguishes_scope_and_update_state() {
        let view = SkillSettingsView {
            settings_id: "project:review".to_string(),
            id: "review".to_string(),
            name: "review".to_string(),
            description: "Review code".to_string(),
            version: Some("1.2.3".to_string()),
            scope: SkillSettingsScope::Project,
            path: "/workspace/.kairox/skills/review/SKILL.md".to_string(),
            enabled: true,
            activation_mode: "suggest".to_string(),
            install_source: SkillInstallSource::Registry,
            update_state: SkillUpdateState::UpdateAvailable,
            effective: true,
            shadowed_by: None,
            valid: true,
            validation_error: None,
            editable: true,
            deletable: true,
        };

        assert_eq!(view.scope, SkillSettingsScope::Project);
        assert_eq!(view.update_state, SkillUpdateState::UpdateAvailable);
        assert!(view.editable);
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

#[cfg(test)]
mod project_tests {
    use super::*;

    #[test]
    fn project_id_round_trips_from_string() {
        let project_id = ProjectId::new();
        let encoded = project_id.to_string();

        let decoded = ProjectId::from_string(encoded.clone());

        assert_eq!(decoded.to_string(), encoded);
    }

    #[test]
    fn project_visibility_serializes_as_snake_case() {
        let value = serde_json::to_value(ProjectSessionVisibility::DraftHidden).unwrap();

        assert_eq!(value, serde_json::json!("draft_hidden"));
    }
}
