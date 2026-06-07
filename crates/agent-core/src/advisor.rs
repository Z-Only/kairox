//! Advisor (self-reflection) types for agent tool-call review.
//!
//! A secondary model can review the primary agent's planned tool calls
//! before execution, reducing costly mistakes during autonomous operation.

use serde::{Deserialize, Serialize};

/// How aggressively the advisor reviews tool calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AdvisorMode {
    /// No advisor review (default).
    #[default]
    Off,
    /// Review only high-risk tool calls (destructive shell commands,
    /// file writes outside workspace, etc.).
    Lightweight,
    /// Review every batch of tool calls before execution.
    Full,
}

impl std::fmt::Display for AdvisorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Lightweight => write!(f, "lightweight"),
            Self::Full => write!(f, "full"),
        }
    }
}

/// The advisor's verdict on a batch of tool calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AdvisorVerdict {
    /// Proceed with the tool calls as planned.
    Approve,
    /// Proceed but with noted concerns.
    ApproveWithWarnings,
    /// Block execution — the agent should reconsider.
    Reject,
}

impl std::fmt::Display for AdvisorVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Approve => write!(f, "approve"),
            Self::ApproveWithWarnings => write!(f, "approve_with_warnings"),
            Self::Reject => write!(f, "reject"),
        }
    }
}

/// A concern raised by the advisor about a specific tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AdvisorConcern {
    /// Which tool call this concern relates to (by tool name).
    pub tool_name: String,
    /// Severity: "high", "medium", "low".
    pub severity: String,
    /// Human-readable description of the concern.
    pub message: String,
}

/// Complete advisor review result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AdvisorReview {
    /// Overall verdict.
    pub verdict: AdvisorVerdict,
    /// Individual concerns about specific tool calls.
    pub concerns: Vec<AdvisorConcern>,
    /// Optional summary explanation from the advisor.
    pub summary: String,
    /// Which model profile was used for the review.
    pub advisor_profile: String,
}

#[cfg(test)]
#[path = "advisor_tests.rs"]
mod tests;
