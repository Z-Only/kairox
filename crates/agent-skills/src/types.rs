use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct SkillId(String);

impl SkillId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SkillId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSourceKind {
    Builtin,
    User,
    Workspace,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillActivationMode {
    #[default]
    Manual,
    Suggest,
    Auto,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct SkillActivation {
    #[serde(default)]
    pub mode: SkillActivationMode,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct SkillPermissionDeclaration {
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub can_request_tools: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SkillSource {
    pub kind: SkillSourceKind,
    pub root: PathBuf,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SkillMetadata {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub source: SkillSource,
    pub activation: SkillActivation,
    pub permissions: SkillPermissionDeclaration,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SkillDocument {
    pub metadata: SkillMetadata,
    pub body_markdown: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_id_creation_and_display() {
        let id = SkillId::new("code-review");
        assert_eq!(id.as_str(), "code-review");
        assert_eq!(id.to_string(), "code-review");
    }

    #[test]
    fn skill_id_ordering() {
        let mut ids = [
            SkillId::new("zeta"),
            SkillId::new("alpha"),
            SkillId::new("beta"),
        ];
        ids.sort();
        assert_eq!(ids[0].as_str(), "alpha");
        assert_eq!(ids[1].as_str(), "beta");
        assert_eq!(ids[2].as_str(), "zeta");
    }

    #[test]
    fn skill_id_serde_roundtrip() {
        let id = SkillId::new("code-review");
        let json = serde_json::to_string(&id).unwrap();
        let back: SkillId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn skill_activation_defaults_to_manual() {
        let activation = SkillActivation::default();
        assert_eq!(activation.mode, SkillActivationMode::Manual);
        assert!(activation.keywords.is_empty());
    }

    #[test]
    fn skill_source_kind_serde_is_snake_case() {
        assert_eq!(
            serde_json::to_value(SkillSourceKind::Builtin).unwrap(),
            serde_json::json!("builtin")
        );
        assert_eq!(
            serde_json::to_value(SkillSourceKind::User).unwrap(),
            serde_json::json!("user")
        );
        assert_eq!(
            serde_json::to_value(SkillSourceKind::Workspace).unwrap(),
            serde_json::json!("workspace")
        );
    }

    #[test]
    fn skill_metadata_serde_roundtrip() {
        let metadata = SkillMetadata {
            id: SkillId::new("test"),
            name: "Test Skill".into(),
            description: "A test".into(),
            version: Some("1.0.0".into()),
            source: SkillSource {
                kind: SkillSourceKind::User,
                root: "/tmp/skills".into(),
                path: "/tmp/skills/test/SKILL.md".into(),
            },
            activation: SkillActivation {
                mode: SkillActivationMode::Suggest,
                keywords: vec!["audit".into()],
            },
            permissions: SkillPermissionDeclaration {
                tools: vec!["shell".into()],
                can_request_tools: vec!["search".into()],
            },
        };
        let json = serde_json::to_string_pretty(&metadata).unwrap();
        let back: SkillMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back, metadata);
    }
}
