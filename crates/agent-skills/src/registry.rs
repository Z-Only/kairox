use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use crate::types::{SkillDocument, SkillId, SkillMetadata, SkillSource, SkillSourceKind};
use crate::{parse_skill_markdown, Result};

#[async_trait::async_trait]
pub trait SkillRegistry: Send + Sync {
    fn list(&self) -> Vec<SkillMetadata>;

    fn get(&self, id: &SkillId) -> Option<SkillMetadata>;

    async fn load_document(&self, id: &SkillId) -> Result<SkillDocument>;
}

#[derive(Debug, Clone)]
pub struct SkillRoot {
    pub kind: SkillSourceKind,
    pub path: PathBuf,
    pub namespace: Option<String>,
}

impl SkillRoot {
    pub fn new(kind: SkillSourceKind, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            path: path.into(),
            namespace: None,
        }
    }

    pub fn with_namespace(
        kind: SkillSourceKind,
        path: impl Into<PathBuf>,
        namespace: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            path: path.into(),
            namespace: Some(namespace.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileSkillRegistry {
    skills: BTreeMap<SkillId, SkillMetadata>,
}

impl FileSkillRegistry {
    pub async fn discover(roots: Vec<SkillRoot>) -> Result<Self> {
        let mut skills = BTreeMap::new();

        for root in roots {
            if !tokio::fs::try_exists(&root.path).await? {
                continue;
            }

            let mut child_entries = tokio::fs::read_dir(&root.path).await?;
            while let Some(child_entry) = child_entries.next_entry().await? {
                if !child_entry.file_type().await?.is_dir() {
                    continue;
                }

                let skill_path = child_entry.path().join("SKILL.md");
                if !tokio::fs::try_exists(&skill_path).await? {
                    continue;
                }

                let raw_skill_markdown = match tokio::fs::read_to_string(&skill_path).await {
                    Ok(raw_skill_markdown) => raw_skill_markdown,
                    Err(error) => {
                        tracing::warn!(
                            skill_path = %skill_path.display(),
                            error = %error,
                            "skipping skill because its SKILL.md could not be read"
                        );
                        continue;
                    }
                };
                let parsed_skill = match parse_skill_markdown(&raw_skill_markdown) {
                    Ok(parsed_skill) => parsed_skill,
                    Err(error) => {
                        tracing::warn!(
                            skill_path = %skill_path.display(),
                            error = %error,
                            "skipping skill because its SKILL.md is invalid"
                        );
                        continue;
                    }
                };
                let raw_name = &parsed_skill.frontmatter.name;
                let skill_id = if let Some(ref ns) = root.namespace {
                    SkillId::new(format!("{ns}:{raw_name}"))
                } else {
                    SkillId::new(raw_name.clone())
                };

                skills.insert(
                    skill_id.clone(),
                    SkillMetadata {
                        id: skill_id,
                        name: parsed_skill.frontmatter.name,
                        description: parsed_skill.frontmatter.description,
                        version: parsed_skill.frontmatter.version,
                        source: SkillSource {
                            kind: root.kind,
                            root: root.path.clone(),
                            path: skill_path,
                        },
                        activation: parsed_skill.activation,
                        permissions: parsed_skill.permissions,
                    },
                );
            }
        }

        Ok(Self { skills })
    }
}

#[async_trait::async_trait]
impl SkillRegistry for FileSkillRegistry {
    fn list(&self) -> Vec<SkillMetadata> {
        self.skills.values().cloned().collect()
    }

    fn get(&self, id: &SkillId) -> Option<SkillMetadata> {
        self.skills.get(id).cloned()
    }

    async fn load_document(&self, id: &SkillId) -> Result<SkillDocument> {
        let metadata = self.skills.get(id).cloned().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("skill not found: {id}"))
        })?;
        let raw_skill_markdown = tokio::fs::read_to_string(&metadata.source.path).await?;
        let parsed_skill = parse_skill_markdown(&raw_skill_markdown)?;

        Ok(SkillDocument {
            metadata,
            body_markdown: parsed_skill.body_markdown,
        })
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
