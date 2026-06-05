//! Marketplace catalog DTOs.
//!
//! These mirror `agent-mcp` catalog and installer types without depending on
//! `agent-mcp`, which would create a dependency cycle.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub verified: bool,
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

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
