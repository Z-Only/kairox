use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyEffect {
    Read,
    Write { paths: Vec<PathBuf> },
    Shell { destructive: bool },
    Network { hosts: Vec<String> },
    Destructive,
    McpInvoke { server: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRisk {
    pub tool_id: String,
    pub effect: PolicyEffect,
}

impl PolicyRisk {
    pub fn read(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: PolicyEffect::Read,
        }
    }

    pub fn write(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: PolicyEffect::Write { paths: Vec::new() },
        }
    }

    pub fn write_paths(tool_id: impl Into<String>, paths: Vec<PathBuf>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: PolicyEffect::Write { paths },
        }
    }

    pub fn shell(tool_id: impl Into<String>, destructive: bool) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: PolicyEffect::Shell { destructive },
        }
    }

    pub fn destructive(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: PolicyEffect::Destructive,
        }
    }

    pub fn network(tool_id: impl Into<String>, hosts: Vec<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: PolicyEffect::Network { hosts },
        }
    }

    pub fn mcp(tool_id: impl Into<String>, server: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: PolicyEffect::McpInvoke {
                server: server.into(),
            },
        }
    }
}

#[cfg(test)]
#[path = "effect_tests.rs"]
mod tests;
