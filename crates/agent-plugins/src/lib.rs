pub mod manifest;
pub mod marketplace;
pub mod settings;

pub use manifest::{
    read_plugin_manifest, PluginCompatibilityHints, PluginComponentInventory, PluginInterface,
    PluginManifestKind, PluginManifestView, PluginPermissionHints,
};
pub use marketplace::{parse_marketplace, MarketplaceFile, MarketplacePluginEntry};
pub use settings::{
    discover_plugin_settings, write_plugin_state, PluginRoot, PluginScope,
    PluginSettingsProjection, PluginSettingsView,
};

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("plugin manifest not found")]
    ManifestNotFound,
    #[error("invalid plugin manifest: {0}")]
    InvalidManifest(String),
    #[error("invalid plugin state file: {0}")]
    InvalidStateFile(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PluginError>;
