//! Data types for the MCP overlay — tab/mode/filter enums, entry structs,
//! and small helper functions used across the overlay submodules.

/// Active tab within the MCP overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum McpOverlayTab {
    Runtime,
    Settings,
    Installed,
    Catalog,
    Sources,
    Tools,
    Resources,
    Prompts,
}

impl McpOverlayTab {
    pub(super) fn next(self) -> Self {
        match self {
            Self::Runtime => Self::Settings,
            Self::Settings => Self::Installed,
            Self::Installed => Self::Catalog,
            Self::Catalog => Self::Sources,
            Self::Sources => Self::Tools,
            Self::Tools => Self::Resources,
            Self::Resources => Self::Prompts,
            Self::Prompts => Self::Runtime,
        }
    }

    pub(super) fn previous(self) -> Self {
        match self {
            Self::Runtime => Self::Prompts,
            Self::Settings => Self::Runtime,
            Self::Installed => Self::Settings,
            Self::Catalog => Self::Installed,
            Self::Sources => Self::Catalog,
            Self::Tools => Self::Sources,
            Self::Resources => Self::Tools,
            Self::Prompts => Self::Resources,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Runtime => "Runtime",
            Self::Settings => "Settings",
            Self::Installed => "Installed",
            Self::Catalog => "Catalog",
            Self::Sources => "Sources",
            Self::Tools => "Tools",
            Self::Resources => "Resources",
            Self::Prompts => "Prompts",
        }
    }
}

/// Per-server health snapshot used in the Runtime tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct McpHealthState {
    pub(super) healthy: bool,
    pub(super) tool_count: usize,
    pub(super) error: Option<String>,
}

/// Trust-level filter for catalog browsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CatalogTrustFilter {
    All,
    Community,
    Verified,
}

impl CatalogTrustFilter {
    pub(super) fn next(self) -> Self {
        match self {
            Self::All => Self::Community,
            Self::Community => Self::Verified,
            Self::Verified => Self::All,
        }
    }

    pub(super) fn min_rank(self) -> Option<u8> {
        match self {
            Self::All => None,
            Self::Community => Some(1),
            Self::Verified => Some(2),
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Community => "community+",
            Self::Verified => "verified",
        }
    }
}

/// Current interaction mode of the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum McpOverlayMode {
    List,
    ServerEditor,
    SourceEditor,
    CatalogFilter,
    CatalogInstallConfig,
}

/// Status of a catalog install operation (shown inline in the catalog list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CatalogInstallStatus {
    Installing,
    Installed { server_id: String, started: bool },
    AlreadyInstalled { server_id: String },
    RuntimeMissing { missing_runtimes: Vec<String> },
    MissingEnv { missing_env_keys: Vec<String> },
    Failed { message: String },
}

// ─── Helper functions ───────────────────────────────────────────────────────

/// Map a trust label to a numeric rank for filtering.
pub(super) fn trust_rank(value: &str) -> u8 {
    match value {
        "verified" => 2,
        "community" => 1,
        _ => 0,
    }
}

/// Composite key for resource preview cache lookups.
pub(super) fn resource_preview_key(server_id: &str, uri: &str) -> String {
    format!("{server_id}\n{uri}")
}

/// Composite key for catalog install status tracking.
pub(super) fn catalog_install_key(source: &str, catalog_id: &str) -> String {
    format!("{source}\n{catalog_id}")
}
