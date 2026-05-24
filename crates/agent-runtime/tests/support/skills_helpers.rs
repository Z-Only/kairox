#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use async_trait::async_trait;
use futures::{stream, stream::BoxStream};
use tokio::sync::Mutex as AsyncMutex;

use agent_core::facade::RemoteSkillSearchResult;
use agent_memory::ContextBudget;
use agent_models::{FakeModelClient, ModelClient, ModelEvent, ModelRequest};
use agent_runtime::skill_package::SkillPackageManager;
use agent_runtime::LocalRuntime;
use agent_skills::{
    SkillActivation, SkillDocument, SkillId, SkillMetadata, SkillPermissionDeclaration,
    SkillRegistry, SkillSource, SkillSourceKind,
};
use agent_store::SqliteEventStore;

pub fn context_budget() -> ContextBudget {
    ContextBudget {
        context_window: 8_000,
        output_reservation: 1_000,
        source_caps: vec![],
    }
}

pub fn write_test_skill(root: &std::path::Path, name: &str, description: &str, body: &str) {
    let skill_directory = root.join(name);
    std::fs::create_dir_all(&skill_directory).expect("skill directory should be created");
    std::fs::write(
        skill_directory.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}"),
    )
    .expect("skill file should be written");
}

pub fn write_plugin_skill(
    root: &std::path::Path,
    _namespace: &str,
    skill_name: &str,
    description: &str,
    body: &str,
) {
    let skill_directory = root.join(skill_name);
    std::fs::create_dir_all(&skill_directory).expect("skill directory should be created");
    std::fs::write(
        skill_directory.join("SKILL.md"),
        format!("---\nname: {skill_name}\ndescription: {description}\n---\n{body}"),
    )
    .expect("skill file should be written");
}

pub async fn build_runtime_with_skill_registry(
    registry: Arc<dyn agent_skills::SkillRegistry>,
) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    LocalRuntime::new(store, model).with_skill_registry(registry)
}

pub async fn build_runtime_with_package_manager(
    manager: Arc<dyn SkillPackageManager>,
) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    LocalRuntime::new(store, model).with_skill_package_manager(manager)
}

pub fn remote_result(
    name: &str,
    description: &str,
    repository: &str,
    install_count: u64,
) -> RemoteSkillSearchResult {
    RemoteSkillSearchResult {
        name: name.into(),
        description: description.into(),
        repository: Some(repository.into()),
        install_count: Some(install_count),
        source_url: repository.into(),
        package: format!("@skills/{name}"),
    }
}

#[derive(Clone, Debug)]
pub struct RecordingModelClient {
    requests: Arc<AsyncMutex<Vec<ModelRequest>>>,
}

impl RecordingModelClient {
    pub fn new(requests: Arc<AsyncMutex<Vec<ModelRequest>>>) -> Self {
        Self { requests }
    }
}

#[async_trait]
impl ModelClient for RecordingModelClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        self.requests.lock().await.push(request);
        Ok(Box::pin(stream::iter(vec![
            Ok(ModelEvent::TokenDelta("ok".into())),
            Ok(ModelEvent::Completed { usage: None }),
        ])))
    }
}

#[derive(Debug)]
pub struct ToggleSkillRegistry {
    enabled: Arc<AtomicBool>,
    metadata: SkillMetadata,
}

impl ToggleSkillRegistry {
    pub fn new(enabled: Arc<AtomicBool>, metadata: SkillMetadata) -> Self {
        Self { enabled, metadata }
    }

    pub fn plugin_review(enabled: Arc<AtomicBool>) -> Self {
        Self::new(
            enabled,
            SkillMetadata {
                id: SkillId::new("my-plugin:review"),
                name: "review".into(),
                description: "Plugin review skill".into(),
                version: None,
                source: SkillSource {
                    kind: SkillSourceKind::Plugin,
                    root: PathBuf::from("/plugin"),
                    path: PathBuf::from("/plugin/skills/review/SKILL.md"),
                },
                activation: SkillActivation::default(),
                permissions: SkillPermissionDeclaration::default(),
            },
        )
    }
}

#[async_trait]
impl SkillRegistry for ToggleSkillRegistry {
    fn list(&self) -> Vec<SkillMetadata> {
        if self.enabled.load(Ordering::SeqCst) {
            vec![self.metadata.clone()]
        } else {
            Vec::new()
        }
    }

    fn get(&self, id: &SkillId) -> Option<SkillMetadata> {
        if self.enabled.load(Ordering::SeqCst) && id == &self.metadata.id {
            Some(self.metadata.clone())
        } else {
            None
        }
    }

    async fn load_document(&self, id: &SkillId) -> agent_skills::Result<SkillDocument> {
        self.get(id)
            .map(|metadata| SkillDocument {
                metadata,
                body_markdown: "Review carefully.".into(),
            })
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("skill not found: {id}"),
                )
                .into()
            })
    }
}
