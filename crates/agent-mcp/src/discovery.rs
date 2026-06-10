//! MCP server discovery and capability caching.
//!
//! Provides [`DiscoveryCache`] which wraps an [`McpClient`] and lazily fetches
//! tools, resources, and prompts from an MCP server, caching the results with
//! support for cache invalidation.

use crate::types::*;
use crate::{McpClient, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Cache of discovered tools, resources, and prompts from an MCP server.
///
/// Wraps an [`McpClient`] and lazily fetches discovery data on first access.
/// Supports invalidation of individual caches or all caches at once, allowing
/// callers to force a re-fetch on next access.
pub struct DiscoveryCache {
    client: Arc<McpClient>,
    tools: Mutex<Option<Vec<McpToolDef>>>,
    resources: Mutex<Option<Vec<McpResourceDef>>>,
    prompts: Mutex<Option<Vec<McpPromptDef>>>,
}

impl DiscoveryCache {
    /// Create a new `DiscoveryCache` wrapping the given client.
    pub fn new(client: Arc<McpClient>) -> Self {
        Self {
            client,
            tools: Mutex::new(None),
            resources: Mutex::new(None),
            prompts: Mutex::new(None),
        }
    }

    /// Get tools, fetching from the server if not cached.
    pub async fn tools(&self) -> Result<Vec<McpToolDef>> {
        let mut cache = self.tools.lock().await;
        match cache.as_ref() {
            Some(cached) => Ok(cached.clone()),
            None => {
                let fetched = self.client.discover_tools().await?;
                let result = fetched.clone();
                *cache = Some(fetched);
                Ok(result)
            }
        }
    }

    /// Get resources, fetching from the server if not cached.
    pub async fn resources(&self) -> Result<Vec<McpResourceDef>> {
        let mut cache = self.resources.lock().await;
        match cache.as_ref() {
            Some(cached) => Ok(cached.clone()),
            None => {
                let fetched = self.client.discover_resources().await?;
                let result = fetched.clone();
                *cache = Some(fetched);
                Ok(result)
            }
        }
    }

    /// Get prompts, fetching from the server if not cached.
    pub async fn prompts(&self) -> Result<Vec<McpPromptDef>> {
        let mut cache = self.prompts.lock().await;
        match cache.as_ref() {
            Some(cached) => Ok(cached.clone()),
            None => {
                let fetched = self.client.discover_prompts().await?;
                let result = fetched.clone();
                *cache = Some(fetched);
                Ok(result)
            }
        }
    }

    /// Invalidate the tools cache (force re-fetch on next access).
    pub async fn invalidate_tools(&self) {
        let mut cache = self.tools.lock().await;
        *cache = None;
    }

    /// Invalidate the resources cache (force re-fetch on next access).
    pub async fn invalidate_resources(&self) {
        let mut cache = self.resources.lock().await;
        *cache = None;
    }

    /// Invalidate the prompts cache (force re-fetch on next access).
    pub async fn invalidate_prompts(&self) {
        let mut cache = self.prompts.lock().await;
        *cache = None;
    }

    /// Invalidate all caches (force re-fetch on next access).
    pub async fn invalidate_all(&self) {
        self.invalidate_tools().await;
        self.invalidate_resources().await;
        self.invalidate_prompts().await;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
