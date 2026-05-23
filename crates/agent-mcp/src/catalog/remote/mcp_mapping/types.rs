//! API response DTOs for the MCP Registry.
//!
//! Pure data — no IO, no caching, no network. Used by [`super::parser`] to
//! produce internal [`crate::catalog::ServerEntry`] values and by
//! [`super::super::mcp_registry::McpRegistryProvider`] for paginated fetches.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct McpListResponse {
    #[serde(default)]
    pub servers: Vec<McpServerWrapper>,
    #[serde(default)]
    pub metadata: Option<McpMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct McpMetadata {
    #[serde(default, rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

/// Each item in `servers` wraps a `server` object and `_meta`.
#[derive(Debug, Deserialize)]
pub struct McpServerWrapper {
    pub server: McpServer,
    #[serde(default, rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct McpServer {
    /// Scoped name like `com.example/my-server`.
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default, rename = "websiteUrl")]
    pub website_url: Option<String>,
    #[serde(default)]
    pub remotes: Vec<McpRemote>,
    #[serde(default)]
    pub packages: Vec<McpPackage>,
    #[serde(default)]
    pub repository: Option<McpRepository>,
}

#[derive(Debug, Deserialize)]
pub struct McpRemote {
    #[serde(rename = "type")]
    pub transport_type: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<McpRemoteHeader>,
}

#[derive(Debug, Deserialize)]
pub struct McpRemoteHeader {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "isRequired")]
    pub is_required: Option<bool>,
    #[serde(default, rename = "isSecret")]
    pub is_secret: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct McpPackage {
    #[serde(rename = "registryType")]
    pub registry_type: String,
    pub identifier: String,
    #[serde(default, rename = "version")]
    pub _version: Option<String>,
    #[serde(default)]
    pub transport: Option<McpPackageTransport>,
    #[serde(default, rename = "environmentVariables")]
    pub environment_variables: Vec<McpEnvVar>,
}

#[derive(Debug, Deserialize)]
pub struct McpPackageTransport {
    #[serde(rename = "type")]
    pub transport_type: String,
}

#[derive(Debug, Deserialize)]
pub struct McpEnvVar {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "isRequired")]
    pub is_required: Option<bool>,
    #[serde(default, rename = "isSecret")]
    pub is_secret: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct McpRepository {
    #[serde(default)]
    pub url: Option<String>,
}
