use agent_core::facade::AgentSettingsInput;
use agent_core::{CoreError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParsedAgentMarkdown {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub model_profile: Option<String>,
    pub permission_mode: Option<String>,
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
    permission_mode: Option<String>,
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
        permission_mode: frontmatter.permission_mode,
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
        permission_mode: input.permission_mode.clone(),
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
