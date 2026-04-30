#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    ReadOnly,
    Suggest,
    Agent,
    Autonomous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionOutcome {
    Allowed,
    RequiresApproval,
    Denied(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolEffect {
    Read,
    Write,
    Shell { destructive: bool },
    Network,
    Destructive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRisk {
    pub tool_id: String,
    pub effect: ToolEffect,
}

impl ToolRisk {
    pub fn read(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: ToolEffect::Read,
        }
    }

    pub fn write(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: ToolEffect::Write,
        }
    }

    pub fn shell(tool_id: impl Into<String>, destructive: bool) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: ToolEffect::Shell { destructive },
        }
    }

    pub fn destructive(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: ToolEffect::Destructive,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PermissionEngine {
    mode: PermissionMode,
}

impl PermissionEngine {
    pub fn new(mode: PermissionMode) -> Self {
        Self { mode }
    }

    pub fn decide(&self, risk: &ToolRisk) -> PermissionOutcome {
        match (self.mode, &risk.effect) {
            (PermissionMode::ReadOnly, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::ReadOnly, ToolEffect::Write) => {
                PermissionOutcome::Denied("read-only mode blocks writes".into())
            }
            (PermissionMode::ReadOnly, ToolEffect::Shell { .. }) => {
                PermissionOutcome::Denied("read-only mode blocks shell execution".into())
            }
            (PermissionMode::ReadOnly, ToolEffect::Network) => {
                PermissionOutcome::Denied("read-only mode blocks network access".into())
            }
            (PermissionMode::ReadOnly, ToolEffect::Destructive) => {
                PermissionOutcome::Denied("read-only mode blocks destructive operations".into())
            }
            (PermissionMode::Suggest, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::Suggest, _) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Agent, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::Agent, ToolEffect::Write) => PermissionOutcome::Allowed,
            (PermissionMode::Agent, ToolEffect::Shell { destructive: false }) => {
                PermissionOutcome::Allowed
            }
            (PermissionMode::Agent, ToolEffect::Destructive) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Agent, _) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Autonomous, ToolEffect::Shell { destructive: true }) => {
                PermissionOutcome::RequiresApproval
            }
            (PermissionMode::Autonomous, ToolEffect::Network) => {
                PermissionOutcome::RequiresApproval
            }
            (PermissionMode::Autonomous, ToolEffect::Destructive) => {
                PermissionOutcome::RequiresApproval
            }
            (PermissionMode::Autonomous, _) => PermissionOutcome::Allowed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_allows_reads_and_blocks_shell_writes() {
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);

        assert_eq!(
            engine.decide(&ToolRisk::read("fs.read")),
            PermissionOutcome::Allowed
        );
        assert_eq!(
            engine.decide(&ToolRisk::write("fs.write")),
            PermissionOutcome::Denied("read-only mode blocks writes".into())
        );
        assert_eq!(
            engine.decide(&ToolRisk::shell("shell.exec", false)),
            PermissionOutcome::Denied("read-only mode blocks shell execution".into())
        );
    }

    #[test]
    fn suggest_requires_approval_for_effectful_tools() {
        let engine = PermissionEngine::new(PermissionMode::Suggest);

        assert_eq!(
            engine.decide(&ToolRisk::write("patch.apply")),
            PermissionOutcome::RequiresApproval
        );
    }

    #[test]
    fn autonomous_still_requires_approval_for_destructive_shell() {
        let engine = PermissionEngine::new(PermissionMode::Autonomous);

        assert_eq!(
            engine.decide(&ToolRisk::shell("shell.exec", true)),
            PermissionOutcome::RequiresApproval
        );
    }

    #[test]
    fn destructive_risk_requires_approval_even_in_autonomous_mode() {
        let engine = PermissionEngine::new(PermissionMode::Autonomous);
        let risk = ToolRisk::destructive("rm.rf");
        assert_eq!(engine.decide(&risk), PermissionOutcome::RequiresApproval);
    }

    #[test]
    fn destructive_risk_denied_in_readonly_mode() {
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);
        let risk = ToolRisk::destructive("rm.rf");
        assert_eq!(
            engine.decide(&risk),
            PermissionOutcome::Denied("read-only mode blocks destructive operations".into())
        );
    }

    #[test]
    fn destructive_risk_requires_approval_in_suggest_mode() {
        let engine = PermissionEngine::new(PermissionMode::Suggest);
        let risk = ToolRisk::destructive("rm.rf");
        assert_eq!(engine.decide(&risk), PermissionOutcome::RequiresApproval);
    }

    #[test]
    fn destructive_risk_requires_approval_in_agent_mode() {
        let engine = PermissionEngine::new(PermissionMode::Agent);
        let risk = ToolRisk::destructive("rm.rf");
        assert_eq!(engine.decide(&risk), PermissionOutcome::RequiresApproval);
    }
}
