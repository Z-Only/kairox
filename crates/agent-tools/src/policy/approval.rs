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
mod tests {
    use super::*;

    #[test]
    fn display_roundtrip() {
        for p in [
            ApprovalPolicy::Never,
            ApprovalPolicy::OnRequest,
            ApprovalPolicy::Always,
        ] {
            assert_eq!(p.to_string().parse::<ApprovalPolicy>().unwrap(), p);
        }
    }

    #[test]
    fn fromstr_aliases() {
        assert_eq!(
            "OnRequest".parse::<ApprovalPolicy>().unwrap(),
            ApprovalPolicy::OnRequest
        );
        assert_eq!(
            "on-request".parse::<ApprovalPolicy>().unwrap(),
            ApprovalPolicy::OnRequest
        );
    }

    #[test]
    fn fromstr_invalid() {
        assert!("bogus".parse::<ApprovalPolicy>().is_err());
    }

    #[test]
    fn serde_snake_case() {
        let s = serde_json::to_string(&ApprovalPolicy::OnRequest).unwrap();
        assert_eq!(s, "\"on_request\"");
        let back: ApprovalPolicy = serde_json::from_str(&s).unwrap();
        assert_eq!(back, ApprovalPolicy::OnRequest);
    }

    #[test]
    fn default_is_on_request() {
        assert_eq!(ApprovalPolicy::default(), ApprovalPolicy::OnRequest);
    }
}
