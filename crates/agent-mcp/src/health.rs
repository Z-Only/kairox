//! Health and connectivity checks for MCP servers.
//!
//! This module provides functions to verify that an MCP server is reachable
//! and responsive, separate from the core lifecycle (start/stop/restart) logic.

use crate::lifecycle::ServerLifecycle;
use crate::types::ConnectivityResult;
use std::time::Duration;

/// Test connectivity to an MCP server.
///
/// Ensures the server is running, then calls `tools/list` (or equivalent
/// discovery) to verify the server responds. Returns a
/// [`ConnectivityResult`] indicating success with tool count, or failure
/// with a reason.
///
/// The `timeout` parameter controls how long to wait for the server to
/// start and respond. If `None`, defaults to 15 seconds.
pub async fn check_connectivity(
    lifecycle: &mut ServerLifecycle,
    timeout: Option<Duration>,
) -> ConnectivityResult {
    let timeout = timeout.unwrap_or(Duration::from_secs(15));

    // Ensure the server is running, with a timeout.
    let client = match tokio::time::timeout(timeout, lifecycle.ensure_running()).await {
        Ok(Ok(client)) => client,
        Ok(Err(e)) => {
            return ConnectivityResult::Failed {
                reason: format!("failed to start server: {e}"),
            };
        }
        Err(_elapsed) => {
            return ConnectivityResult::Failed {
                reason: format!(
                    "timed out after {}s waiting for server to start",
                    timeout.as_secs()
                ),
            };
        }
    };

    // Discover tools to verify the server is responsive.
    match tokio::time::timeout(timeout, client.discover_tools()).await {
        Ok(Ok(tools)) => {
            lifecycle.mark_active();
            ConnectivityResult::Connected {
                tool_count: tools.len() as u32,
            }
        }
        Ok(Err(e)) => ConnectivityResult::Failed {
            reason: format!("tool discovery failed: {e}"),
        },
        Err(_elapsed) => ConnectivityResult::Failed {
            reason: format!(
                "timed out after {}s waiting for tool discovery",
                timeout.as_secs()
            ),
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "health_tests.rs"]
mod tests;
