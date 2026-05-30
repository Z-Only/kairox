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
    Plugin,
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
#[path = "types_tests.rs"]
mod tests;
