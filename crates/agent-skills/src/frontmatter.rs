use serde::Deserialize;

use crate::types::{SkillActivation, SkillPermissionDeclaration};
use crate::{Result, SkillError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedSkillMarkdown {
    pub frontmatter: SkillFrontmatter,
    pub activation: SkillActivation,
    pub permissions: SkillPermissionDeclaration,
    pub body_markdown: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    #[serde(default)]
    kairox: RawKairoxFrontmatter,
}

#[derive(Debug, Default, Deserialize)]
struct RawKairoxFrontmatter {
    #[serde(default)]
    activation: SkillActivation,
    #[serde(default)]
    permissions: SkillPermissionDeclaration,
}

pub fn parse_skill_markdown(raw: &str) -> Result<ParsedSkillMarkdown> {
    let frontmatter_block = raw
        .strip_prefix("---\n")
        .ok_or(SkillError::MissingFrontmatter)?;
    let (frontmatter_yaml, body_markdown) = frontmatter_block
        .split_once("\n---\n")
        .ok_or(SkillError::MissingFrontmatter)?;

    let raw_frontmatter: RawSkillFrontmatter = serde_yaml::from_str(frontmatter_yaml)
        .map_err(|error| SkillError::InvalidFrontmatter(error.to_string()))?;
    let name = raw_frontmatter
        .name
        .ok_or(SkillError::MissingRequiredField { field: "name" })?;
    let description = raw_frontmatter
        .description
        .ok_or(SkillError::MissingRequiredField {
            field: "description",
        })?;

    Ok(ParsedSkillMarkdown {
        frontmatter: SkillFrontmatter {
            name,
            description,
            version: raw_frontmatter.version,
        },
        activation: raw_frontmatter.kairox.activation,
        permissions: raw_frontmatter.kairox.permissions,
        body_markdown: body_markdown.to_owned(),
    })
}

#[cfg(test)]
#[path = "frontmatter_tests.rs"]
mod tests;
