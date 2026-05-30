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
#[path = "manifest_tests.rs"]
mod tests;
