#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerAgent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerAgent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerAgent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerFinding {
    pub severity: String,
    pub message: String,
}

impl ReviewerAgent {
    pub fn review_diff(diff: &str) -> Vec<ReviewerFinding> {
        let mut findings = Vec::new();
        if diff.contains("rm -rf") {
            findings.push(ReviewerFinding {
                severity: "high".into(),
                message: "destructive shell command requires explicit approval".into(),
            });
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewer_flags_destructive_commands() {
        let findings = ReviewerAgent::review_diff("+ rm -rf target");
        assert_eq!(findings[0].severity, "high");
    }
}
