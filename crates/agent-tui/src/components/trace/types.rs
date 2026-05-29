#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelTab {
    Trace,
    Tasks,
    Memory,
}

impl RightPanelTab {
    pub(super) fn next(self) -> Self {
        match self {
            Self::Trace => Self::Tasks,
            Self::Tasks => Self::Memory,
            Self::Memory => Self::Trace,
        }
    }

    pub(super) fn previous(self) -> Self {
        match self {
            Self::Trace => Self::Memory,
            Self::Tasks => Self::Trace,
            Self::Memory => Self::Tasks,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Trace => "Trace",
            Self::Tasks => "Tasks",
            Self::Memory => "Memory",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceStatus {
    Running,
    Success,
    Failed,
    Pending,
}

impl std::fmt::Display for TraceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "⏳"),
            Self::Success => write!(f, "✓"),
            Self::Failed => write!(f, "✕"),
            Self::Pending => write!(f, "?"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceKind {
    Tool,
    Memory,
}
