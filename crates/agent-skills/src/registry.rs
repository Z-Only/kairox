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
}

impl SkillRoot {
    pub fn new(kind: SkillSourceKind, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            path: path.into(),
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
                let skill_id = SkillId::new(parsed_skill.frontmatter.name.clone());

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
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::types::{SkillId, SkillSourceKind};

    use super::{FileSkillRegistry, SkillRegistry, SkillRoot};

    fn write_skill(root: &Path, directory_name: &str, name: &str, description: &str, body: &str) {
        let skill_directory = root.join(directory_name);
        fs::create_dir_all(&skill_directory).expect("skill directory should be created");

        let skill_markdown = format!("---\nname: {name}\ndescription: {description}\n---\n{body}");
        fs::write(skill_directory.join("SKILL.md"), skill_markdown)
            .expect("skill markdown should be written");
    }

    #[tokio::test]
    async fn workspace_skill_overrides_user_and_builtin_with_same_name() {
        let builtin_root = tempfile::tempdir().expect("built-in root should be created");
        let user_root = tempfile::tempdir().expect("user root should be created");
        let workspace_root = tempfile::tempdir().expect("workspace root should be created");

        write_skill(
            builtin_root.path(),
            "builtin-review",
            "code-review",
            "Built-in review skill",
            "Built-in body\n",
        );
        write_skill(
            user_root.path(),
            "user-review",
            "code-review",
            "User review skill",
            "User body\n",
        );
        let workspace_skill_path = workspace_root
            .path()
            .join("workspace-review")
            .join("SKILL.md");
        write_skill(
            workspace_root.path(),
            "workspace-review",
            "code-review",
            "Workspace review skill",
            "Workspace body\n",
        );

        let registry = FileSkillRegistry::discover(vec![
            SkillRoot::new(SkillSourceKind::Builtin, builtin_root.path()),
            SkillRoot::new(SkillSourceKind::User, user_root.path()),
            SkillRoot::new(SkillSourceKind::Workspace, workspace_root.path()),
        ])
        .await
        .expect("registry should be discovered");

        let metadata = registry
            .get(&SkillId::new("code-review"))
            .expect("effective skill should exist");

        assert_eq!(metadata.id, SkillId::new("code-review"));
        assert_eq!(metadata.description, "Workspace review skill");
        assert_eq!(metadata.source.kind, SkillSourceKind::Workspace);
        assert_eq!(metadata.source.root, workspace_root.path());
        assert_eq!(metadata.source.path, workspace_skill_path);
        assert_eq!(registry.list(), vec![metadata]);
    }

    #[tokio::test]
    async fn skips_invalid_skill_documents_during_discovery() {
        let user_root = tempfile::tempdir().expect("user root should be created");
        let invalid_skill_directory = user_root.path().join("invalid-skill");
        fs::create_dir_all(&invalid_skill_directory).expect("invalid skill directory should exist");
        fs::write(
            invalid_skill_directory.join("SKILL.md"),
            "---\nname: broken-skill\n---\n# Missing description\n",
        )
        .expect("invalid skill should be written");
        write_skill(
            user_root.path(),
            "valid-skill",
            "valid-skill",
            "Valid skill description",
            "Valid body\n",
        );

        let registry = FileSkillRegistry::discover(vec![SkillRoot::new(
            SkillSourceKind::User,
            user_root.path(),
        )])
        .await
        .expect("invalid skill documents should not abort discovery");

        assert_eq!(registry.list().len(), 1);
        assert!(registry.get(&SkillId::new("broken-skill")).is_none());
        assert!(registry.get(&SkillId::new("valid-skill")).is_some());
    }

    #[tokio::test]
    async fn load_document_returns_body_for_effective_skill() {
        let builtin_root = tempfile::tempdir().expect("built-in root should be created");
        let user_root = tempfile::tempdir().expect("user root should be created");
        let workspace_root = tempfile::tempdir().expect("workspace root should be created");

        write_skill(
            builtin_root.path(),
            "builtin-loader",
            "loader",
            "Built-in loader skill",
            "Built-in body\n",
        );
        write_skill(
            user_root.path(),
            "user-loader",
            "loader",
            "User loader skill",
            "User body\n",
        );
        write_skill(
            workspace_root.path(),
            "workspace-loader",
            "loader",
            "Workspace loader skill",
            "Workspace body\n",
        );

        let registry = FileSkillRegistry::discover(vec![
            SkillRoot::new(SkillSourceKind::Builtin, builtin_root.path()),
            SkillRoot::new(SkillSourceKind::User, user_root.path()),
            SkillRoot::new(SkillSourceKind::Workspace, workspace_root.path()),
        ])
        .await
        .expect("registry should be discovered");

        let document = registry
            .load_document(&SkillId::new("loader"))
            .await
            .expect("effective skill document should load");

        assert_eq!(document.metadata.description, "Workspace loader skill");
        assert_eq!(document.metadata.source.kind, SkillSourceKind::Workspace);
        assert_eq!(document.body_markdown, "Workspace body\n");
    }

    #[tokio::test]
    async fn list_returns_empty_when_no_roots() {
        let registry = FileSkillRegistry::discover(vec![])
            .await
            .expect("empty discover should succeed");
        assert!(registry.list().is_empty());
    }

    #[tokio::test]
    async fn get_unknown_skill_returns_none() {
        let root = tempfile::tempdir().expect("root should exist");
        write_skill(root.path(), "test", "test", "A test skill", "Body\n");

        let registry =
            FileSkillRegistry::discover(vec![SkillRoot::new(SkillSourceKind::User, root.path())])
                .await
                .expect("discover should succeed");

        assert!(registry.get(&SkillId::new("nonexistent")).is_none());
    }

    #[tokio::test]
    async fn load_document_unknown_skill_returns_error() {
        let root = tempfile::tempdir().expect("root should exist");
        write_skill(root.path(), "test", "test", "A test skill", "Body\n");

        let registry =
            FileSkillRegistry::discover(vec![SkillRoot::new(SkillSourceKind::User, root.path())])
                .await
                .expect("discover should succeed");

        let result = registry.load_document(&SkillId::new("nonexistent")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn discover_skips_missing_root_directories() {
        let missing_path = std::path::PathBuf::from("/tmp/nonexistent-skills-root-12345");
        let valid_root = tempfile::tempdir().expect("valid root should exist");
        write_skill(valid_root.path(), "valid", "valid", "Valid skill", "Body\n");

        let registry = FileSkillRegistry::discover(vec![
            SkillRoot::new(SkillSourceKind::User, &missing_path),
            SkillRoot::new(SkillSourceKind::Builtin, valid_root.path()),
        ])
        .await
        .expect("discover should succeed despite missing root");

        let list = registry.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, SkillId::new("valid"));
    }
}
