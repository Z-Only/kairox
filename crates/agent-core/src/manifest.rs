use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionType {
    Skill,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub extension_type: ExtensionType,
    pub triggers: Vec<String>,
    pub prompt_templates: Vec<String>,
    pub required_tools: Vec<String>,
    pub required_permissions: Vec<String>,
    pub core_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skill_manifest() {
        let manifest: ExtensionManifest = toml::from_str(
            r#"
id = "skill.code-review"
name = "Code Review"
version = "0.1.0"
description = "Review code changes"
extension_type = "skill"
triggers = ["review"]
prompt_templates = ["Check correctness and tests"]
required_tools = ["git.diff"]
required_permissions = ["filesystem.read"]
core_version = ">=0.1.0"
"#,
        )
        .unwrap();

        assert_eq!(manifest.id, "skill.code-review");
        assert_eq!(manifest.extension_type, ExtensionType::Skill);
        assert_eq!(manifest.required_tools, vec!["git.diff"]);
    }
}
