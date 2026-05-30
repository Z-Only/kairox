//! Build information embedded at compile time.
//!
//! Each binary crate (agent-tui, agent-gui-tauri) injects `KAIROX_VERSION`,
//! `KAIROX_GIT_HASH`, and `KAIROX_BUILD_TIME` via their `build.rs`.
//! This module reads them with `option_env!` fallbacks so that library-level
//! compilation (which doesn't run those build scripts) still compiles.

/// Build information embedded at compile time.
pub struct BuildInfo {
    pub version: &'static str,
    pub git_hash: &'static str,
    pub build_time: &'static str,
}

impl BuildInfo {
    /// Construct from compile-time env vars injected by the binary crate's `build.rs`.
    ///
    /// Falls back to `CARGO_PKG_VERSION` / `"dev"` / `"unknown"` when the
    /// `KAIROX_*` env vars are absent (e.g. during IDE analysis or when
    /// compiling agent-core as a library without its binary wrapper).
    pub fn from_env() -> Self {
        Self {
            version: option_env!("KAIROX_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
            git_hash: option_env!("KAIROX_GIT_HASH").unwrap_or("dev"),
            build_time: option_env!("KAIROX_BUILD_TIME").unwrap_or("unknown"),
        }
    }
}

impl std::fmt::Display for BuildInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({} {})",
            self.version, self.git_hash, self.build_time
        )
    }
}

#[cfg(test)]
#[path = "build_info_tests.rs"]
mod tests;
