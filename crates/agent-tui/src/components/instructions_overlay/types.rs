//! Data types for the instructions overlay — the `InstructionsTab` enum
//! representing which tab is active in the overlay.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InstructionsTab {
    System,
    User,
    Project,
    Effective,
}

impl InstructionsTab {
    pub(super) fn next(self) -> Self {
        match self {
            Self::System => Self::User,
            Self::User => Self::Project,
            Self::Project => Self::Effective,
            Self::Effective => Self::System,
        }
    }

    pub(super) fn previous(self) -> Self {
        match self {
            Self::System => Self::Effective,
            Self::User => Self::System,
            Self::Project => Self::User,
            Self::Effective => Self::Project,
        }
    }
}
