//! Data types for the plugin overlay — tab and mode enums
//! used across the overlay submodules.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PluginTab {
    Installed,
    Catalog,
    Sources,
}

impl PluginTab {
    pub(super) fn next(self) -> Self {
        match self {
            Self::Installed => Self::Catalog,
            Self::Catalog => Self::Sources,
            Self::Sources => Self::Installed,
        }
    }

    pub(super) fn previous(self) -> Self {
        match self {
            Self::Installed => Self::Sources,
            Self::Catalog => Self::Installed,
            Self::Sources => Self::Catalog,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Installed => "Installed",
            Self::Catalog => "Catalog",
            Self::Sources => "Sources",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PluginOverlayMode {
    List,
    CatalogSearch,
}
