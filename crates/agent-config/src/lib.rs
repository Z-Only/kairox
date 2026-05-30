pub mod builder;
pub mod discovery;
pub mod effective;
pub mod limits;
pub mod loader;
mod types;

pub use builder::{build_ollama_clients, build_router};
pub use discovery::{find_config, find_config_upward, find_local_config};
pub use effective::{
    build_effective_mcp_server_settings_views, build_effective_mcp_servers,
    build_effective_profile_settings_views, build_effective_profiles,
};
pub use limits::resolve_limits;
pub use loader::{
    default_catalog_sources, load_from_str, load_with_marketplace_loaded,
    load_with_marketplace_overlay, merge_with_defaults, parse_catalog_sources, resolve_api_keys,
    resolve_mcp_env, validate, CatalogSourceConfig, CatalogSourceKind, LoadedConfig,
};

// Re-export all public types from the types module.
pub use types::*;
// Re-export crate-internal types so sibling modules can use them.
pub(crate) use types::{default_true, HookConfigToml};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
