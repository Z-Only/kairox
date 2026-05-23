//! MCP Registry API DTOs and mapping to internal [`crate::catalog::ServerEntry`] schema.
//!
//! Pure mapping layer — no IO, no caching, no network. Separated from
//! [`super::mcp_registry::McpRegistryProvider`] so the mapping logic can
//! be tested independently of the provider's fetch/cache/lock machinery.
//!
//! - [`types`]: API response DTOs (`McpListResponse`, `McpServerWrapper`, ...).
//! - [`parser`]: pure helpers and the [`map_mcp_to_entry`] mapping function.

mod parser;
mod types;

#[cfg(test)]
mod tests;

pub(super) use parser::{is_latest, map_mcp_to_entry};
pub(super) use types::McpListResponse;
