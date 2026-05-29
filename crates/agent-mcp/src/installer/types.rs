//! Data types for the marketplace installer.
//!
//! Holds the pure data structures used by [`crate::installer::Installer`]: the
//! install outcome surfaced to the GUI, the persisted server record, the host
//! runtime probe abstraction, and the installer error type.

use crate::catalog::{RuntimeKind, RuntimeRequirement};
use async_trait::async_trait;

/// Outcome of an [`crate::installer::Installer::install`] call. Surfaced to the GUI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallOutcomeView {
    Installed { server_id: String, started: bool },
    RuntimeMissing { missing: Vec<RuntimeRequirement> },
    AlreadyInstalled { server_id: String },
    InvalidEnv { missing_keys: Vec<String> },
}

/// Metadata for a server persisted in the marketplace-managed MCP config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledServerRecord {
    pub server_id: String,
    pub catalog_id: Option<String>,
    pub source: Option<String>,
}

/// Detects whether a host runtime is available.
#[async_trait]
pub trait RuntimeProbe: Send + Sync {
    async fn is_available(&self, kind: RuntimeKind) -> bool;
}

/// Default probe using the `which` crate to look up binaries on PATH.
pub struct OsRuntimeProbe;

#[async_trait]
impl RuntimeProbe for OsRuntimeProbe {
    async fn is_available(&self, kind: RuntimeKind) -> bool {
        let bin = match kind {
            RuntimeKind::Node => "node",
            RuntimeKind::Python => "python3",
            RuntimeKind::Uvx => "uvx",
            RuntimeKind::Docker => "docker",
            RuntimeKind::Bun => "bun",
            RuntimeKind::Deno => "deno",
            RuntimeKind::Other => return true,
        };
        which::which(bin).is_ok()
    }
}

/// Errors emitted by the installer when filesystem or TOML operations fail.
#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error("io: {0}")]
    Io(String),
    #[error("toml: {0}")]
    Toml(String),
    #[error("invalid: {0}")]
    Invalid(String),
}
