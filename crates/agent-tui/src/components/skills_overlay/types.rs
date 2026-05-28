//! Data types for the skills overlay — tab/mode enums and body-view struct
//! used across the overlay submodules.

/// Inline detail view shown when the user presses Enter on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyView {
    pub skill_id: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SkillTab {
    Discovered,
    Installed,
    Catalog,
    Sources,
}

impl SkillTab {
    pub(super) fn next(self) -> Self {
        match self {
            Self::Discovered => Self::Installed,
            Self::Installed => Self::Catalog,
            Self::Catalog => Self::Sources,
            Self::Sources => Self::Discovered,
        }
    }

    pub(super) fn previous(self) -> Self {
        match self {
            Self::Discovered => Self::Sources,
            Self::Installed => Self::Discovered,
            Self::Catalog => Self::Installed,
            Self::Sources => Self::Catalog,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Discovered => "Discovered",
            Self::Installed => "Installed",
            Self::Catalog => "Catalog",
            Self::Sources => "Sources",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SkillOverlayMode {
    List,
    CatalogDetail,
    CatalogFilter,
    SourceEditor,
}
