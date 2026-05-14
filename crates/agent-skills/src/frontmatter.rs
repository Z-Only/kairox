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
mod tests {
    use super::parse_skill_markdown;
    use crate::types::SkillActivationMode;
    use crate::SkillError;

    #[test]
    fn parses_required_frontmatter_and_body() {
        let raw = r#"---
name: Code Review
description: Reviews code for correctness and maintainability
version: 1.2.3
kairox:
  activation:
    mode: suggest
    keywords:
      - review
      - audit
  permissions:
    tools:
      - shell
      - read_file
    can_request_tools:
      - web_search
---
# Code Review

Review the current change.
"#;

        let parsed = parse_skill_markdown(raw).expect("frontmatter should parse");

        assert_eq!(parsed.frontmatter.name, "Code Review");
        assert_eq!(
            parsed.frontmatter.description,
            "Reviews code for correctness and maintainability"
        );
        assert_eq!(parsed.frontmatter.version.as_deref(), Some("1.2.3"));
        assert_eq!(parsed.activation.mode, SkillActivationMode::Suggest);
        assert_eq!(parsed.activation.keywords, vec!["review", "audit"]);
        assert_eq!(parsed.permissions.tools, vec!["shell", "read_file"]);
        assert_eq!(parsed.permissions.can_request_tools, vec!["web_search"]);
        assert_eq!(
            parsed.body_markdown,
            "# Code Review\n\nReview the current change.\n"
        );
    }

    #[test]
    fn rejects_missing_required_name() {
        let raw = r#"---
description: Reviews code for correctness and maintainability
---
# Code Review
"#;

        let error = parse_skill_markdown(raw).expect_err("missing name should be rejected");

        assert!(matches!(
            error,
            SkillError::MissingRequiredField { field: "name" }
        ));
    }

    #[test]
    fn rejects_missing_required_description() {
        let raw = r#"---
name: Code Review
---
# Code Review
"#;

        let error = parse_skill_markdown(raw).expect_err("missing description should be rejected");

        assert!(matches!(
            error,
            SkillError::MissingRequiredField {
                field: "description"
            }
        ));
    }

    #[test]
    fn rejects_missing_frontmatter_delimiters() {
        let raw = "# Just markdown\n\nNo frontmatter here.\n";
        let error = parse_skill_markdown(raw).expect_err("no delimiters should be rejected");
        assert!(matches!(error, SkillError::MissingFrontmatter));
    }

    #[test]
    fn rejects_missing_closing_frontmatter() {
        let raw = "---\nname: Test\ndescription: Desc\n# No closing delimiter\n";
        let error =
            parse_skill_markdown(raw).expect_err("missing closing delimiter should be rejected");
        assert!(matches!(error, SkillError::MissingFrontmatter));
    }

    #[test]
    fn rejects_invalid_yaml_in_frontmatter() {
        let raw = "---\n{{invalid_yaml\n---\nBody\n";
        let error = parse_skill_markdown(raw).expect_err("invalid YAML should be rejected");
        assert!(matches!(error, SkillError::InvalidFrontmatter(_)));
    }

    #[test]
    fn parses_minimal_frontmatter_with_defaults() {
        let raw = "---\nname: minimal\ndescription: Minimal skill\n---\nBody\n";
        let parsed = parse_skill_markdown(raw).expect("minimal frontmatter should parse");
        assert_eq!(parsed.frontmatter.name, "minimal");
        assert_eq!(parsed.activation.mode, SkillActivationMode::Manual);
        assert!(parsed.activation.keywords.is_empty());
        assert!(parsed.permissions.tools.is_empty());
        assert_eq!(parsed.body_markdown, "Body\n");
    }

    #[test]
    fn parses_frontmatter_with_optional_version_only() {
        let raw = "---\nname: versioned\ndescription: Has version\nversion: 2.0.0\n---\nBody\n";
        let parsed = parse_skill_markdown(raw).expect("versioned frontmatter should parse");
        assert_eq!(parsed.frontmatter.version.as_deref(), Some("2.0.0"));
    }
}
