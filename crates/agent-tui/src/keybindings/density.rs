/// Controls how much detail the trace panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TraceDensity {
    /// One-line summary per tool call.
    #[default]
    Summary,
    /// Expand tool call arguments and results.
    Expanded,
    /// Show every event in the stream (including heartbeats).
    FullEventStream,
    TaskGraph,
}

impl TraceDensity {
    /// Cycle to the next density level.
    pub fn next(self) -> Self {
        match self {
            Self::Summary => Self::Expanded,
            Self::Expanded => Self::FullEventStream,
            Self::FullEventStream => Self::TaskGraph,
            Self::TaskGraph => Self::Summary,
        }
    }
}
