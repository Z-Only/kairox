use agent_core::facade::AgentSettingsInput;
use agent_core::{CoreError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParsedAgentMarkdown {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub model_profile: Option<String>,
    pub reasoning_effort: Option<String>,
    pub skills: Vec<String>,
    pub nickname_candidates: Vec<String>,
    pub enabled: bool,
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawAgentFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    model_profile: Option<String>,
    #[serde(default)]
    reasoning_effort: Option<String>,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    nickname_candidates: Vec<String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

pub fn parse_agent_markdown(raw: &str) -> Result<ParsedAgentMarkdown> {
    let frontmatter_block = raw
        .strip_prefix("---\n")
        .ok_or_else(|| CoreError::InvalidState("missing agent frontmatter".into()))?;
    let (frontmatter_yaml, instructions) = frontmatter_block
        .split_once("\n---\n")
        .ok_or_else(|| CoreError::InvalidState("missing agent frontmatter".into()))?;

    let frontmatter: RawAgentFrontmatter = serde_yaml::from_str(frontmatter_yaml)
        .map_err(|error| CoreError::InvalidState(format!("invalid agent frontmatter: {error}")))?;
    let name = frontmatter
        .name
        .ok_or_else(|| CoreError::InvalidState("missing required agent field: name".into()))?;
    validate_agent_name(&name)?;
    let description = frontmatter.description.ok_or_else(|| {
        CoreError::InvalidState("missing required agent field: description".into())
    })?;

    Ok(ParsedAgentMarkdown {
        name,
        description,
        tools: frontmatter.tools,
        model_profile: frontmatter.model_profile,
        reasoning_effort: normalize_optional_string(frontmatter.reasoning_effort),
        skills: frontmatter.skills,
        nickname_candidates: frontmatter.nickname_candidates,
        enabled: frontmatter.enabled,
        instructions: instructions.to_string(),
    })
}

pub(super) fn render_agent_markdown(input: &AgentSettingsInput) -> Result<String> {
    let frontmatter = RawAgentFrontmatter {
        name: Some(input.name.clone()),
        description: Some(input.description.clone()),
        tools: input.tools.clone(),
        model_profile: input.model_profile.clone(),
        reasoning_effort: input.reasoning_effort.clone(),
        skills: input.skills.clone(),
        nickname_candidates: input.nickname_candidates.clone(),
        enabled: input.enabled,
    };
    let mut yaml = serde_yaml::to_string(&frontmatter)
        .map_err(|error| CoreError::InvalidState(format!("failed to render agent: {error}")))?;
    if let Some(stripped) = yaml.strip_prefix("---\n") {
        yaml = stripped.to_string();
    }
    Ok(format!(
        "---\n{}---\n{}\n",
        yaml,
        input.instructions.trim_end()
    ))
}

pub(super) fn validate_agent_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(CoreError::InvalidState("invalid agent name: empty".into()));
    };
    if !first.is_ascii_lowercase() {
        return Err(CoreError::InvalidState(format!(
            "invalid agent name: {name}"
        )));
    }
    if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_') {
        return Err(CoreError::InvalidState(format!(
            "invalid agent name: {name}"
        )));
    }
    Ok(())
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod parser_tests {
    use super::*;
    use agent_core::facade::AgentSettingsScope;

    fn agent_input(name: &str) -> AgentSettingsInput {
        AgentSettingsInput {
            scope: AgentSettingsScope::User,
            name: name.to_string(),
            description: "Test agent".to_string(),
            tools: vec![],
            model_profile: None,
            reasoning_effort: None,
            skills: vec![],
            nickname_candidates: vec![],
            enabled: true,
            instructions: "Body".to_string(),
        }
    }

    #[test]
    fn parse_agent_markdown_returns_full_view_for_well_formed_input() {
        let raw = "---\n\
            name: planner\n\
            description: Plans the work\n\
            tools:\n  - shell\n  - search\n\
            model_profile: fast\n\
            reasoning_effort: high\n\
            skills:\n  - skill-a\n\
            nickname_candidates:\n  - planr\n\
            enabled: true\n\
            ---\nDetailed instructions.\n";

        let parsed = parse_agent_markdown(raw).expect("well-formed markdown should parse");

        assert_eq!(parsed.name, "planner");
        assert_eq!(parsed.description, "Plans the work");
        assert_eq!(parsed.tools, vec!["shell", "search"]);
        assert_eq!(parsed.model_profile.as_deref(), Some("fast"));
        assert_eq!(parsed.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(parsed.skills, vec!["skill-a"]);
        assert_eq!(parsed.nickname_candidates, vec!["planr"]);
        assert!(parsed.enabled);
        assert_eq!(parsed.instructions, "Detailed instructions.\n");
    }

    #[test]
    fn parse_agent_markdown_defaults_enabled_to_true_when_omitted() {
        let raw = "---\nname: helper\ndescription: A helper\n---\nbody";
        let parsed = parse_agent_markdown(raw).expect("should parse");
        assert!(parsed.enabled, "enabled should default to true");
        assert!(parsed.tools.is_empty());
        assert!(parsed.model_profile.is_none());
    }

    #[test]
    fn parse_agent_markdown_rejects_input_without_opening_frontmatter() {
        let raw = "no frontmatter here\nbody only\n";
        let error = parse_agent_markdown(raw).expect_err("should reject");
        let CoreError::InvalidState(msg) = error else {
            panic!("expected InvalidState error");
        };
        assert!(msg.contains("missing agent frontmatter"), "got: {msg}");
    }

    #[test]
    fn parse_agent_markdown_rejects_input_without_closing_frontmatter() {
        let raw = "---\nname: planner\ndescription: text\nbody without closing fence";
        let error = parse_agent_markdown(raw).expect_err("should reject");
        let CoreError::InvalidState(msg) = error else {
            panic!("expected InvalidState error");
        };
        assert!(msg.contains("missing agent frontmatter"), "got: {msg}");
    }

    #[test]
    fn parse_agent_markdown_surfaces_yaml_errors_with_diagnostic_text() {
        let raw = "---\nname: planner\ndescription: : :\n---\nbody";
        let error = parse_agent_markdown(raw).expect_err("should reject");
        let CoreError::InvalidState(msg) = error else {
            panic!("expected InvalidState error");
        };
        assert!(msg.contains("invalid agent frontmatter"), "got: {msg}");
    }

    #[test]
    fn parse_agent_markdown_rejects_missing_required_name_field() {
        let raw = "---\ndescription: A helper\n---\nbody";
        let error = parse_agent_markdown(raw).expect_err("should reject");
        let CoreError::InvalidState(msg) = error else {
            panic!("expected InvalidState error");
        };
        assert!(
            msg.contains("missing required agent field: name"),
            "got: {msg}"
        );
    }

    #[test]
    fn parse_agent_markdown_rejects_missing_required_description_field() {
        let raw = "---\nname: planner\n---\nbody";
        let error = parse_agent_markdown(raw).expect_err("should reject");
        let CoreError::InvalidState(msg) = error else {
            panic!("expected InvalidState error");
        };
        assert!(
            msg.contains("missing required agent field: description"),
            "got: {msg}"
        );
    }

    #[test]
    fn parse_agent_markdown_rejects_invalid_agent_name_character() {
        let raw = "---\nname: Planner\ndescription: oops\n---\nbody";
        let error = parse_agent_markdown(raw).expect_err("should reject");
        let CoreError::InvalidState(msg) = error else {
            panic!("expected InvalidState error");
        };
        assert!(msg.contains("invalid agent name"), "got: {msg}");
    }

    #[test]
    fn validate_agent_name_accepts_lowercase_with_digits_dashes_and_underscores() {
        validate_agent_name("planner").unwrap();
        validate_agent_name("plan-2").unwrap();
        validate_agent_name("plan_v2").unwrap();
        validate_agent_name("a").unwrap();
    }

    #[test]
    fn validate_agent_name_rejects_empty_name() {
        let error = validate_agent_name("").expect_err("empty name rejects");
        let CoreError::InvalidState(msg) = error else {
            panic!("expected InvalidState error");
        };
        assert!(msg.contains("empty"), "got: {msg}");
    }

    #[test]
    fn validate_agent_name_rejects_uppercase_first_character() {
        let error = validate_agent_name("Planner").expect_err("uppercase-first rejects");
        let CoreError::InvalidState(msg) = error else {
            panic!("expected InvalidState error");
        };
        assert!(msg.contains("Planner"), "got: {msg}");
    }

    #[test]
    fn validate_agent_name_rejects_disallowed_inner_characters() {
        let error = validate_agent_name("plan.ner").expect_err("dot rejects");
        let CoreError::InvalidState(msg) = error else {
            panic!("expected InvalidState error");
        };
        assert!(msg.contains("plan.ner"), "got: {msg}");
        validate_agent_name("plan ner").expect_err("space rejects");
        validate_agent_name("plan!").expect_err("punct rejects");
    }

    #[test]
    fn render_agent_markdown_round_trips_through_parse() {
        let input = agent_input("planner");
        let rendered = render_agent_markdown(&input).expect("render should succeed");

        assert!(rendered.starts_with("---\n"), "rendered: {rendered}");
        let parsed = parse_agent_markdown(&rendered).expect("rendered output should parse back");
        assert_eq!(parsed.name, input.name);
        assert_eq!(parsed.description, input.description);
        assert_eq!(parsed.instructions, "Body\n");
        assert!(parsed.enabled);
    }

    #[test]
    fn render_agent_markdown_carries_all_optional_fields_and_lists() {
        let mut input = agent_input("multi");
        input.tools = vec!["shell".into(), "fs.read".into()];
        input.model_profile = Some("fast".into());
        input.reasoning_effort = Some("medium".into());
        input.skills = vec!["audit".into()];
        input.nickname_candidates = vec!["m".into(), "mu".into()];
        input.enabled = false;

        let rendered = render_agent_markdown(&input).expect("render");
        let parsed = parse_agent_markdown(&rendered).expect("re-parse");

        assert_eq!(parsed.tools, vec!["shell", "fs.read"]);
        assert_eq!(parsed.model_profile.as_deref(), Some("fast"));
        assert_eq!(parsed.reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(parsed.skills, vec!["audit"]);
        assert_eq!(parsed.nickname_candidates, vec!["m", "mu"]);
        assert!(!parsed.enabled);
    }
}
