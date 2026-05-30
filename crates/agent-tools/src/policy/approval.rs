use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    Never,
    #[default]
    OnRequest,
    Always,
}

impl ApprovalPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::OnRequest => "on_request",
            Self::Always => "always",
        }
    }
}

impl fmt::Display for ApprovalPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ApprovalPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "never" => Ok(Self::Never),
            "on_request" | "onrequest" | "on-request" => Ok(Self::OnRequest),
            "always" => Ok(Self::Always),
            other => Err(format!(
                "unknown approval policy `{other}`; expected never|on_request|always"
            )),
        }
    }
}

#[cfg(test)]
#[path = "approval_tests.rs"]
mod tests;
