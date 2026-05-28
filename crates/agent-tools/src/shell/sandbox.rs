//! Execution sandbox helpers: env allow-list, default budgets, and output
//! truncation.
//!
//! The shell tool clears the inherited environment and only forwards the
//! variables in [`ALLOWED_ENV_VARS`] to keep the child process predictable.
//! Captured stdout/stderr is bounded by [`truncate_bytes`] so a runaway
//! command cannot blow the in-memory output buffer.

use std::time::Duration;

pub(super) const DEFAULT_TIMEOUT_SECS: u64 = 30;
pub(super) const DEFAULT_MAX_OUTPUT_BYTES: usize = 102_400; // 100 KB
pub(crate) const ALLOWED_ENV_VARS: &[&str] =
    &["PATH", "HOME", "LANG", "TERM", "USER", "TMPDIR", "SHELL"];

/// Default per-invocation timeout.
pub(super) fn default_timeout() -> Duration {
    Duration::from_secs(DEFAULT_TIMEOUT_SECS)
}

/// Default cap on captured stdio bytes.
pub(super) fn default_max_output_bytes() -> usize {
    DEFAULT_MAX_OUTPUT_BYTES
}

/// Apply the sandboxed environment to a tokio child command: clear the
/// inherited env, then forward only the entries from [`ALLOWED_ENV_VARS`]
/// that are present in the parent process.
pub(super) fn apply_sandbox_env(cmd: &mut tokio::process::Command) {
    cmd.env_clear();
    for var in ALLOWED_ENV_VARS {
        if let Ok(val) = std::env::var(var) {
            cmd.env(var, val);
        }
    }
}

/// Truncate `data` to at most `limit` bytes. Returns the trimmed buffer
/// and a flag indicating whether truncation occurred.
pub(super) fn truncate_bytes(data: &[u8], limit: usize) -> (Vec<u8>, bool) {
    if data.len() <= limit {
        (data.to_vec(), false)
    } else {
        (data[..limit].to_vec(), true)
    }
}
