//! Application facade — the primary integration point for Kairox.
//!
//! All UIs (TUI, GUI) interact with the runtime through the [`AppFacade`] trait.
//! This trait provides a stable, object-safe interface for workspace management,
//! session lifecycle, messaging, permissions, and event streaming.

mod mcp;
mod project;
mod session;
mod skills;

pub use mcp::McpFacade;
pub use project::ProjectFacade;
pub use session::SessionFacade;
pub use skills::SkillsFacade;

use crate::{DomainEvent, ProjectId, SessionId, TaskId, WorkspaceId};
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub tool_count: Option<usize>,
    pub last_error: Option<String>,
    pub writable: bool,
    pub config_path: Option<String>,
    pub description: Option<String>,
    pub source: String,
}

/// Concrete effective-view wrapper for MCP server settings.
/// Combines [`EffectiveItem`] metadata with a [`McpServerSettingsView`].
/// This is a non-generic type so it can safely derive both serde and specta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EffectiveMcpServerView {
    pub value: McpServerSettingsView,
    pub source: crate::config_scope::ConfigScope,
    pub overrides: Option<crate::config_scope::ConfigScope>,
    pub enabled: bool,
    #[serde(rename = "disabledBy")]
    pub disabled_by: Option<crate::config_scope::ConfigScope>,
    pub writable: bool,
    pub deletable: bool,
}

impl EffectiveMcpServerView {
    pub fn from_effective(item: crate::EffectiveItem<McpServerSettingsView>) -> Self {
        Self {
            value: item.value,
            source: item.source,
            overrides: item.overrides,
            enabled: item.enabled,
            disabled_by: item.disabled_by,
            writable: item.writable,
            deletable: item.deletable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProfileSettingsInput {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    #[serde(default = "crate::facade::default_true")]
    pub enabled: bool,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub context_window: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub output_limit: Option<u64>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub context_window: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub output_limit: Option<u64>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
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
    pub package_url: Option<String>,
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub install_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub github_stars: Option<u64>,
    pub security_score: Option<u32>,
    pub rating: Option<f64>,
    pub package: String,
    pub package_url: Option<String>,
}

/// Query against the skills catalog.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillCatalogQuery {
    pub keyword: Option<String>,
    pub sources: Option<Vec<String>>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
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
    pub contents: Option<String>,
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub retry_count: usize,
    #[cfg_attr(feature = "specta", specta(type = u32))]
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

/// AppFacade is the complete application facade, combining all sub-traits.
///
/// All UIs (TUI, GUI) interact with the runtime through this trait.
/// The canonical implementation is [`agent_runtime::LocalRuntime`],
/// but any mock or test implementation can substitute.
///
/// # Object Safety
///
/// This trait is object-safe and can be used as `dyn AppFacade`.
/// Every method has a default implementation that delegates to the
/// corresponding sub-trait, so implementors only need to implement
/// the sub-traits and write `impl AppFacade for T {}`.
#[async_trait::async_trait]
pub trait AppFacade: SessionFacade + SkillsFacade + McpFacade + ProjectFacade {
    // ── Session ─────────────────────────────────────────────────────────

    async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo> {
        SessionFacade::open_workspace(self, path).await
    }
    async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId> {
        SessionFacade::start_session(self, request).await
    }
    async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()> {
        SessionFacade::send_message(self, request).await
    }
    async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()> {
        SessionFacade::decide_permission(self, decision).await
    }
    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> crate::Result<()> {
        SessionFacade::cancel_session(self, workspace_id, session_id).await
    }
    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> crate::Result<crate::projection::SessionProjection> {
        SessionFacade::get_session_projection(self, session_id).await
    }
    async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>> {
        SessionFacade::get_trace(self, session_id).await
    }
    fn subscribe_session(
        &self,
        session_id: SessionId,
    ) -> futures::stream::BoxStream<'static, crate::DomainEvent> {
        SessionFacade::subscribe_session(self, session_id)
    }
    fn subscribe_all(&self) -> futures::stream::BoxStream<'static, crate::DomainEvent> {
        SessionFacade::subscribe_all(self)
    }
    async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>> {
        SessionFacade::list_workspaces(self).await
    }
    async fn list_sessions(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<SessionMeta>> {
        SessionFacade::list_sessions(self, workspace_id).await
    }
    async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()> {
        SessionFacade::rename_session(self, session_id, title).await
    }
    async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
        SessionFacade::soft_delete_session(self, session_id).await
    }
    async fn permanently_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
        SessionFacade::permanently_delete_session(self, session_id).await
    }
    async fn restore_archived_session(&self, session_id: &SessionId) -> crate::Result<()> {
        SessionFacade::restore_archived_session(self, session_id).await
    }
    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> crate::Result<usize> {
        SessionFacade::cleanup_expired_sessions(self, older_than).await
    }
    async fn get_task_graph(&self, session_id: SessionId) -> crate::Result<TaskGraphSnapshot> {
        SessionFacade::get_task_graph(self, session_id).await
    }
    async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()> {
        SessionFacade::retry_task(self, workspace_id, session_id, task_id).await
    }
    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()> {
        SessionFacade::cancel_task(self, workspace_id, session_id, task_id).await
    }
    async fn get_agent_status(&self, session_id: SessionId) -> crate::Result<Vec<AgentStatusInfo>> {
        SessionFacade::get_agent_status(self, session_id).await
    }

    // ── Skills ──────────────────────────────────────────────────────────

    async fn list_skills(&self) -> crate::Result<Vec<SkillView>> {
        SkillsFacade::list_skills(self).await
    }
    async fn get_skill(&self, skill_id: String) -> crate::Result<Option<SkillDetail>> {
        SkillsFacade::get_skill(self, skill_id).await
    }
    async fn activate_skill(
        &self,
        request: ActivateSkillRequest,
    ) -> crate::Result<ActiveSkillView> {
        SkillsFacade::activate_skill(self, request).await
    }
    async fn deactivate_skill(&self, request: DeactivateSkillRequest) -> crate::Result<()> {
        SkillsFacade::deactivate_skill(self, request).await
    }
    async fn list_active_skills(
        &self,
        session_id: SessionId,
    ) -> crate::Result<Vec<ActiveSkillView>> {
        SkillsFacade::list_active_skills(self, session_id).await
    }
    /// List all configured model profiles for settings UI.
    async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> crate::Result<Vec<ProfileSettingsView>> {
        McpFacade::list_profile_settings(self, source_filter).await
    }

    /// Move a profile up or down in display order.
    async fn move_profile_in_order(&self, alias: String, direction: i32) -> crate::Result<()> {
        McpFacade::move_profile_in_order(self, alias, direction).await
    }

    /// Open the config directory in the system file manager.
    async fn open_config_dir(&self) -> crate::Result<Option<String>> {
        McpFacade::open_config_dir(self).await
    }

    /// Open the profiles.toml config file with the system default text editor.
    async fn open_profiles_config_file(&self) -> crate::Result<Option<String>> {
        McpFacade::open_profiles_config_file(self).await
    }

    /// List skills for settings UI.
    async fn list_skill_settings(&self) -> crate::Result<Vec<SkillSettingsView>> {
        SkillsFacade::list_skill_settings(self).await
    }
    async fn get_skill_settings_detail(
        &self,
        skill_id: String,
    ) -> crate::Result<Option<SkillSettingsDetail>> {
        SkillsFacade::get_skill_settings_detail(self, skill_id).await
    }
    async fn set_skill_enabled(&self, skill_id: String, enabled: bool) -> crate::Result<()> {
        SkillsFacade::set_skill_enabled(self, skill_id, enabled).await
    }
    async fn delete_skill_settings(&self, skill_id: String) -> crate::Result<()> {
        SkillsFacade::delete_skill_settings(self, skill_id).await
    }
    async fn search_remote_skills(
        &self,
        query: String,
    ) -> crate::Result<Vec<RemoteSkillSearchResult>> {
        SkillsFacade::search_remote_skills(self, query).await
    }
    async fn install_remote_skill(
        &self,
        request: InstallRemoteSkillRequest,
    ) -> crate::Result<SkillSettingsView> {
        SkillsFacade::install_remote_skill(self, request).await
    }
    async fn install_github_skill(
        &self,
        request: InstallGithubSkillRequest,
    ) -> crate::Result<SkillSettingsView> {
        SkillsFacade::install_github_skill(self, request).await
    }
    async fn update_skill(&self, skill_id: String) -> crate::Result<SkillSettingsView> {
        SkillsFacade::update_skill(self, skill_id).await
    }
    async fn list_skill_catalog(
        &self,
        query: SkillCatalogQuery,
    ) -> crate::Result<Vec<SkillCatalogEntry>> {
        SkillsFacade::list_skill_catalog(self, query).await
    }
    async fn list_skill_sources(&self) -> crate::Result<Vec<SkillSourceView>> {
        SkillsFacade::list_skill_sources(self).await
    }
    async fn add_skill_source(&self, config: SkillSourceView) -> crate::Result<()> {
        SkillsFacade::add_skill_source(self, config).await
    }
    async fn remove_skill_source(&self, id: String) -> crate::Result<()> {
        SkillsFacade::remove_skill_source(self, id).await
    }
    async fn set_skill_source_enabled(&self, id: String, enabled: bool) -> crate::Result<()> {
        SkillsFacade::set_skill_source_enabled(self, id, enabled).await
    }
    async fn refresh_skill_catalog(&self) -> crate::Result<()> {
        SkillsFacade::refresh_skill_catalog(self).await
    }
    async fn open_skills_dir(&self) -> crate::Result<Option<String>> {
        SkillsFacade::open_skills_dir(self).await
    }

    // ── MCP / Marketplace / Profile ─────────────────────────────────────

    async fn list_mcp_server_settings(
        &self,
        source_filter: Option<String>,
    ) -> crate::Result<Vec<McpServerSettingsView>> {
        McpFacade::list_mcp_server_settings(self, source_filter).await
    }
    async fn upsert_mcp_server_settings(
        &self,
        input: McpServerSettingsInput,
    ) -> crate::Result<McpServerSettingsView> {
        McpFacade::upsert_mcp_server_settings(self, input).await
    }
    async fn delete_mcp_server_settings(&self, server_id: String) -> crate::Result<()> {
        McpFacade::delete_mcp_server_settings(self, server_id).await
    }
    async fn set_mcp_server_enabled(&self, server_id: String, enabled: bool) -> crate::Result<()> {
        McpFacade::set_mcp_server_enabled(self, server_id, enabled).await
    }
    async fn open_mcp_config_file(&self) -> crate::Result<Option<String>> {
        McpFacade::open_mcp_config_file(self).await
    }
    async fn upsert_profile_settings(
        &self,
        input: ProfileSettingsInput,
    ) -> crate::Result<ProfileSettingsView> {
        McpFacade::upsert_profile_settings(self, input).await
    }
    async fn set_profile_enabled(&self, alias: String, enabled: bool) -> crate::Result<()> {
        McpFacade::set_profile_enabled(self, alias, enabled).await
    }
    async fn delete_profile_settings(&self, alias: String) -> crate::Result<()> {
        McpFacade::delete_profile_settings(self, alias).await
    }
    async fn list_catalog(&self, query: CatalogQuery) -> crate::Result<Vec<ServerEntry>> {
        McpFacade::list_catalog(self, query).await
    }
    async fn get_catalog_entry(
        &self,
        id: String,
        source: Option<String>,
    ) -> crate::Result<Option<ServerEntry>> {
        McpFacade::get_catalog_entry(self, id, source).await
    }
    async fn refresh_catalog(&self, source: Option<String>) -> crate::Result<()> {
        McpFacade::refresh_catalog(self, source).await
    }
    async fn install_catalog_entry(
        &self,
        request: InstallRequest,
    ) -> crate::Result<InstallOutcomeView> {
        McpFacade::install_catalog_entry(self, request).await
    }
    async fn uninstall_catalog_entry(&self, server_id: String) -> crate::Result<()> {
        McpFacade::uninstall_catalog_entry(self, server_id).await
    }
    async fn list_installed_entries(&self) -> crate::Result<Vec<InstalledEntry>> {
        McpFacade::list_installed_entries(self).await
    }
    async fn list_catalog_sources(&self) -> crate::Result<Vec<CatalogSourceView>> {
        McpFacade::list_catalog_sources(self).await
    }
    async fn add_catalog_source(&self, request: AddCatalogSourceRequest) -> crate::Result<()> {
        McpFacade::add_catalog_source(self, request).await
    }
    async fn remove_catalog_source(&self, id: String) -> crate::Result<()> {
        McpFacade::remove_catalog_source(self, id).await
    }
    async fn set_catalog_source_enabled(&self, id: String, enabled: bool) -> crate::Result<()> {
        McpFacade::set_catalog_source_enabled(self, id, enabled).await
    }

    // ── Projects ────────────────────────────────────────────────────────

    async fn list_projects(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<ProjectMeta>> {
        ProjectFacade::list_projects(self, workspace_id).await
    }
    async fn create_blank_project(
        &self,
        workspace_id: WorkspaceId,
        display_name: Option<String>,
    ) -> crate::Result<ProjectMeta> {
        ProjectFacade::create_blank_project(self, workspace_id, display_name).await
    }
    async fn add_existing_project(
        &self,
        workspace_id: WorkspaceId,
        path: String,
    ) -> crate::Result<ProjectMeta> {
        ProjectFacade::add_existing_project(self, workspace_id, path).await
    }
    async fn rename_project(
        &self,
        project_id: ProjectId,
        display_name: String,
    ) -> crate::Result<()> {
        ProjectFacade::rename_project(self, project_id, display_name).await
    }
    async fn remove_project(&self, project_id: ProjectId) -> crate::Result<()> {
        ProjectFacade::remove_project(self, project_id).await
    }
    async fn restore_project_session(&self, session_id: SessionId) -> crate::Result<ProjectMeta> {
        ProjectFacade::restore_project_session(self, session_id).await
    }
    async fn update_project_order(&self, project_ids: Vec<ProjectId>) -> crate::Result<()> {
        ProjectFacade::update_project_order(self, project_ids).await
    }
    async fn update_project_expanded(
        &self,
        project_id: ProjectId,
        expanded: bool,
    ) -> crate::Result<()> {
        ProjectFacade::update_project_expanded(self, project_id, expanded).await
    }
    async fn create_project_draft_session(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<SessionId> {
        ProjectFacade::create_project_draft_session(self, project_id).await
    }
    async fn create_project_worktree_session(
        &self,
        project_id: ProjectId,
        branch_name: String,
    ) -> crate::Result<SessionId> {
        ProjectFacade::create_project_worktree_session(self, project_id, branch_name).await
    }
    async fn list_project_sessions(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<Vec<SessionMeta>> {
        ProjectFacade::list_project_sessions(self, project_id).await
    }
    async fn list_archived_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> crate::Result<Vec<SessionMeta>> {
        ProjectFacade::list_archived_sessions(self, workspace_id).await
    }
    async fn get_project_git_status(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<ProjectGitStatus> {
        ProjectFacade::get_project_git_status(self, project_id).await
    }
    async fn get_session_git_status(
        &self,
        session_id: SessionId,
    ) -> crate::Result<ProjectGitStatus> {
        ProjectFacade::get_session_git_status(self, session_id).await
    }
    async fn init_project_git(&self, project_id: ProjectId) -> crate::Result<ProjectGitStatus> {
        ProjectFacade::init_project_git(self, project_id).await
    }
    async fn get_project_instruction_summary(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<ProjectInstructionSummary> {
        ProjectFacade::get_project_instruction_summary(self, project_id).await
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
    use async_trait::async_trait;
    use futures::stream::BoxStream;

    fn assert_facade_is_object_safe(_: &dyn AppFacade) {}

    struct NoopFacade;

    #[async_trait]
    impl SessionFacade for NoopFacade {
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

        async fn cancel_session(
            &self,
            workspace_id: WorkspaceId,
            session_id: SessionId,
        ) -> crate::Result<()> {
            let _ = (workspace_id, session_id);
            Ok(())
        }

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

    #[async_trait]
    impl SkillsFacade for NoopFacade {}

    #[async_trait]
    impl McpFacade for NoopFacade {}

    #[async_trait]
    impl ProjectFacade for NoopFacade {}

    impl AppFacade for NoopFacade {}

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

#[cfg(test)]
mod task_snapshot_tests {
    use super::*;
    use crate::{AgentRole, TaskFailureReason, TaskId, TaskState};

    #[test]
    fn task_snapshot_field_access() {
        let snapshot = TaskSnapshot {
            id: TaskId::new(),
            title: "review PR #42".into(),
            role: AgentRole::Reviewer,
            state: TaskState::Pending,
            dependencies: vec![],
            error: None,
            retry_count: 0,
            max_retries: 3,
            assigned_agent_id: None,
            failure_reason: None,
        };

        // Verify fields hold the values we set.
        assert_eq!(snapshot.title, "review PR #42");
        assert_eq!(snapshot.role, AgentRole::Reviewer);
        assert_eq!(snapshot.state, TaskState::Pending);
        assert!(snapshot.dependencies.is_empty());
        assert!(snapshot.error.is_none());
        assert_eq!(snapshot.retry_count, 0);
        assert_eq!(snapshot.max_retries, 3);
        assert!(snapshot.assigned_agent_id.is_none());
        assert!(snapshot.failure_reason.is_none());
    }

    #[test]
    fn task_snapshot_with_error_and_failure_reason() {
        let failure = TaskFailureReason::ToolExhausted {
            tool_id: "shell.exec".into(),
            attempts: 3,
            last_error: "command not found".into(),
        };
        let snapshot = TaskSnapshot {
            id: TaskId::new(),
            title: "run tests".into(),
            role: AgentRole::Worker,
            state: TaskState::Failed,
            dependencies: vec![],
            error: Some("max retries exceeded".into()),
            retry_count: 3,
            max_retries: 3,
            assigned_agent_id: Some("agent_worker_test".into()),
            failure_reason: Some(failure.clone()),
        };

        assert_eq!(snapshot.state, TaskState::Failed);
        assert_eq!(snapshot.error.as_deref(), Some("max retries exceeded"));
        assert_eq!(snapshot.retry_count, 3);
        assert_eq!(
            snapshot.assigned_agent_id.as_deref(),
            Some("agent_worker_test")
        );
        assert_eq!(snapshot.failure_reason, Some(failure));
    }

    #[test]
    fn task_graph_snapshot_contains_tasks() {
        let task1 = TaskSnapshot {
            id: TaskId::new(),
            title: "plan architecture".into(),
            role: AgentRole::Planner,
            state: TaskState::Completed,
            dependencies: vec![],
            error: None,
            retry_count: 0,
            max_retries: 3,
            assigned_agent_id: Some("agent_planner".into()),
            failure_reason: None,
        };
        let task2 = TaskSnapshot {
            id: TaskId::new(),
            title: "implement feature".into(),
            role: AgentRole::Worker,
            state: TaskState::Running,
            dependencies: vec![task1.id.clone()],
            error: None,
            retry_count: 0,
            max_retries: 3,
            assigned_agent_id: Some("agent_worker_impl".into()),
            failure_reason: None,
        };

        let graph = TaskGraphSnapshot {
            tasks: vec![task1.clone(), task2.clone()],
        };

        assert_eq!(graph.tasks.len(), 2);
        assert!(graph.tasks.contains(&task1));
        assert!(graph.tasks.contains(&task2));

        // task2 depends on task1.
        assert_eq!(graph.tasks[1].dependencies, vec![task1.id.clone()]);
    }

    #[test]
    fn task_graph_snapshot_serializes_roundtrip() {
        let task = TaskSnapshot {
            id: TaskId::new(),
            title: "verify".into(),
            role: AgentRole::Reviewer,
            state: TaskState::Completed,
            dependencies: vec![],
            error: None,
            retry_count: 0,
            max_retries: 3,
            assigned_agent_id: None,
            failure_reason: None,
        };
        let graph = TaskGraphSnapshot { tasks: vec![task] };

        let json = serde_json::to_string(&graph).unwrap();
        let back: TaskGraphSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(graph, back);
    }
}
