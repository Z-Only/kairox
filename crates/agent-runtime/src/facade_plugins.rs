use std::path::{Path, PathBuf};

use agent_core::facade::{
    InstallPluginRequest, PluginCatalogEntry, PluginDetailView, PluginMarketplaceSourceView,
    PluginSettingsView, PluginsFacade,
};
use agent_store::EventStore;
use async_trait::async_trait;

use crate::facade_runtime::LocalRuntime;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn list_plugin_settings(&self) -> agent_core::Result<Vec<PluginSettingsView>> {
        crate::plugin_settings::list_plugin_settings(self.plugin_settings_roots()).await
    }

    pub(crate) async fn get_plugin_detail(
        &self,
        settings_id: String,
    ) -> agent_core::Result<Option<PluginDetailView>> {
        crate::plugin_settings::get_plugin_detail(self.plugin_settings_roots(), &settings_id).await
    }

    pub(crate) async fn set_plugin_enabled(
        &self,
        settings_id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        crate::plugin_settings::set_plugin_enabled(
            self.plugin_settings_roots(),
            &settings_id,
            enabled,
        )
        .await
    }

    pub(crate) async fn delete_plugin_settings(
        &self,
        settings_id: String,
    ) -> agent_core::Result<()> {
        crate::plugin_settings::delete_plugin(self.plugin_settings_roots(), &settings_id).await
    }

    pub(crate) async fn list_plugin_marketplace_sources(
        &self,
    ) -> agent_core::Result<Vec<PluginMarketplaceSourceView>> {
        let config_dir = self.plugin_marketplace_config_dir()?;
        Ok(crate::plugin_sources_toml::PluginSourcesToml::new(&config_dir).merged_sources())
    }

    pub(crate) async fn set_plugin_marketplace_source_enabled(
        &self,
        source_id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        let config_dir = self.plugin_marketplace_config_dir()?;
        let changed = crate::plugin_sources_toml::PluginSourcesToml::new(&config_dir)
            .set_enabled(&source_id, enabled)
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        if changed {
            Ok(())
        } else {
            Err(agent_core::CoreError::InvalidState(format!(
                "plugin marketplace source not found: {source_id}"
            )))
        }
    }

    fn plugin_marketplace_config_dir(&self) -> agent_core::Result<PathBuf> {
        if let Some(dir) = &self.marketplace_dir {
            return Ok(dir.clone());
        }
        crate::plugin_settings::user_config_dir(&self.plugin_settings_roots())
    }

    pub(crate) async fn list_plugin_catalog(
        &self,
        marketplace_id: Option<String>,
        keyword: Option<String>,
    ) -> agent_core::Result<Vec<PluginCatalogEntry>> {
        let mut entries = Vec::new();
        for source in self
            .list_plugin_marketplace_sources()
            .await?
            .into_iter()
            .filter(|source| source.enabled)
        {
            if marketplace_id.as_ref().is_some_and(|id| id != &source.id) {
                continue;
            }
            match read_catalog_source(&source).await {
                Ok(source_entries) => entries.extend(source_entries),
                Err(error) => {
                    tracing::warn!(
                        source_id = %source.id,
                        source = %source.source,
                        error = %error,
                        "skipping unreadable plugin marketplace source"
                    );
                }
            }
        }
        if let Some(keyword) = keyword.filter(|value| !value.trim().is_empty()) {
            let keyword = keyword.to_lowercase();
            entries.retain(|entry| {
                entry.name.to_lowercase().contains(&keyword)
                    || entry.description.to_lowercase().contains(&keyword)
            });
        }
        Ok(entries)
    }

    pub(crate) async fn install_plugin(
        &self,
        request: InstallPluginRequest,
    ) -> agent_core::Result<PluginSettingsView> {
        let catalog = self
            .list_plugin_catalog(Some(request.marketplace_id.clone()), None)
            .await?;
        let entry = catalog
            .into_iter()
            .find(|entry| entry.name == request.plugin_name)
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "plugin not found in marketplace: {}@{}",
                    request.plugin_name, request.marketplace_id
                ))
            })?;
        let source_path = PathBuf::from(&entry.source);
        if source_path.is_dir() {
            let install_root = crate::plugin_settings::install_root(
                &self.plugin_settings_roots(),
                request.target,
            )?;
            install_plugin_from_dir(&source_path, &install_root, &entry).await?;
        } else if let Some(github_source) = parse_github_plugin_source(&entry.source) {
            let install_root = crate::plugin_settings::install_root(
                &self.plugin_settings_roots(),
                request.target,
            )?;
            install_plugin_from_github(github_source, &install_root, &entry).await?;
        } else {
            return Err(agent_core::CoreError::InvalidState(format!(
                "unsupported plugin source: {}",
                entry.source
            )));
        }
        let settings_id = format!("{}:{}", request.target_label(), entry.name);
        crate::plugin_settings::list_plugin_settings(self.plugin_settings_roots())
            .await?
            .into_iter()
            .find(|view| view.settings_id == settings_id)
            .ok_or_else(|| agent_core::CoreError::InvalidState("installed plugin not found".into()))
    }
}

#[async_trait]
impl<S, M> PluginsFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn list_plugin_settings(&self) -> agent_core::Result<Vec<PluginSettingsView>> {
        self.list_plugin_settings().await
    }

    async fn get_plugin_detail(
        &self,
        settings_id: String,
    ) -> agent_core::Result<Option<PluginDetailView>> {
        self.get_plugin_detail(settings_id).await
    }

    async fn set_plugin_enabled(
        &self,
        settings_id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        self.set_plugin_enabled(settings_id, enabled).await
    }

    async fn delete_plugin_settings(&self, settings_id: String) -> agent_core::Result<()> {
        self.delete_plugin_settings(settings_id).await
    }

    async fn list_plugin_marketplace_sources(
        &self,
    ) -> agent_core::Result<Vec<PluginMarketplaceSourceView>> {
        self.list_plugin_marketplace_sources().await
    }

    async fn set_plugin_marketplace_source_enabled(
        &self,
        source_id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        self.set_plugin_marketplace_source_enabled(source_id, enabled)
            .await
    }

    async fn list_plugin_catalog(
        &self,
        marketplace_id: Option<String>,
        keyword: Option<String>,
    ) -> agent_core::Result<Vec<PluginCatalogEntry>> {
        self.list_plugin_catalog(marketplace_id, keyword).await
    }

    async fn install_plugin(
        &self,
        request: InstallPluginRequest,
    ) -> agent_core::Result<PluginSettingsView> {
        self.install_plugin(request).await
    }
}

trait InstallRequestTargetLabel {
    fn target_label(&self) -> &'static str;
}

impl InstallRequestTargetLabel for InstallPluginRequest {
    fn target_label(&self) -> &'static str {
        match self.target {
            agent_core::facade::PluginInstallTarget::User => "user",
            agent_core::facade::PluginInstallTarget::Project => "project",
        }
    }
}

async fn read_catalog_source(
    source: &PluginMarketplaceSourceView,
) -> agent_core::Result<Vec<PluginCatalogEntry>> {
    let source_path = PathBuf::from(&source.source);
    let (raw, catalog_root) = if source_path.exists() {
        let marketplace_path = if source_path.is_dir() {
            source_path.join(".claude-plugin/marketplace.json")
        } else {
            source_path
        };
        let catalog_root = marketplace_path
            .parent()
            .and_then(|path| path.parent())
            .map(PathBuf::from);
        (
            tokio::fs::read_to_string(&marketplace_path)
                .await
                .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?,
            catalog_root,
        )
    } else if let Some((owner, repo)) = parse_github_shorthand(&source.source) {
        let url = format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/main/.claude-plugin/marketplace.json"
        );
        (
            reqwest::get(url)
                .await
                .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
                .error_for_status()
                .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
                .text()
                .await
                .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?,
            Some(PathBuf::from(format!("github:{owner}/{repo}:"))),
        )
    } else {
        return Ok(Vec::new());
    };
    let parsed = agent_plugins::parse_marketplace(&raw)
        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
    Ok(parsed
        .plugins
        .into_iter()
        .map(|entry| PluginCatalogEntry {
            marketplace_id: source.id.clone(),
            name: entry.name,
            description: entry.description,
            version: entry.version,
            source: resolve_catalog_entry_source(catalog_root.as_ref(), &entry.source),
        })
        .collect())
}

async fn install_plugin_from_dir(
    source_path: &Path,
    install_root: &PathBuf,
    entry: &PluginCatalogEntry,
) -> agent_core::Result<()> {
    tokio::fs::create_dir_all(install_root)
        .await
        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
    let destination = install_root.join(&entry.name);
    if tokio::fs::try_exists(&destination)
        .await
        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
    {
        tokio::fs::remove_dir_all(&destination)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
    }
    copy_dir_recursive(source_path, &destination).await?;
    agent_plugins::write_plugin_state(
        install_root,
        &entry.name,
        true,
        Some("marketplace"),
        Some(&entry.marketplace_id),
    )
    .await
    .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
}

async fn install_plugin_from_github(
    source: GithubPluginSource,
    install_root: &Path,
    entry: &PluginCatalogEntry,
) -> agent_core::Result<()> {
    let install_root_for_clone = install_root.to_path_buf();
    let install_root_for_state = install_root.to_path_buf();
    let entry_for_clone = entry.clone();
    let entry_for_state = entry.clone();
    tokio::task::spawn_blocking(move || {
        let temp = tempfile::tempdir()
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let repo_dir = temp.path().join("repo");
        let url = format!("https://github.com/{}.git", source.repo);
        let status = std::process::Command::new("git")
            .args(["clone", "--depth", "1", &url])
            .arg(&repo_dir)
            .status()
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        if !status.success() {
            return Err(agent_core::CoreError::InvalidState(format!(
                "failed to clone plugin repository: {url}"
            )));
        }
        let source_dir = if source.path.is_empty() || source.path == "." {
            repo_dir
        } else {
            repo_dir.join(source.path)
        };
        if !source_dir.is_dir() {
            return Err(agent_core::CoreError::InvalidState(format!(
                "plugin source directory not found: {}",
                source_dir.display()
            )));
        }
        std::fs::create_dir_all(&install_root_for_clone)
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let destination = install_root_for_clone.join(&entry_for_clone.name);
        if destination.exists() {
            std::fs::remove_dir_all(&destination)
                .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        }
        copy_dir_recursive_sync(&source_dir, &destination)?;
        Ok(())
    })
    .await
    .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))??;
    agent_plugins::write_plugin_state(
        &install_root_for_state,
        &entry_for_state.name,
        true,
        Some("marketplace"),
        Some(&entry_for_state.marketplace_id),
    )
    .await
    .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
}

fn resolve_catalog_entry_source(catalog_root: Option<&PathBuf>, entry_source: &str) -> String {
    if let Some(relative) = entry_source.strip_prefix("./") {
        if let Some(root) = catalog_root {
            if root.to_string_lossy().starts_with("github:") {
                let repo = root
                    .to_string_lossy()
                    .trim_start_matches("github:")
                    .trim_end_matches(':')
                    .to_string();
                return format!("github:{repo}:{relative}");
            }
            return root.join(relative).to_string_lossy().to_string();
        }
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(entry_source) {
        if json.get("source").and_then(|value| value.as_str()) == Some("github") {
            if let Some(repo) = json.get("repo").and_then(|value| value.as_str()) {
                return format!("github:{repo}:.");
            }
        }
    }
    entry_source.to_string()
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct GithubPluginSource {
    repo: String,
    path: String,
}

fn parse_github_plugin_source(source: &str) -> Option<GithubPluginSource> {
    let source = source.strip_prefix("github:")?;
    let (repo, path) = source.split_once(':')?;
    if repo.is_empty() {
        return None;
    }
    Some(GithubPluginSource {
        repo: repo.to_string(),
        path: path.to_string(),
    })
}

fn parse_github_shorthand(source: &str) -> Option<(&str, &str)> {
    let mut parts = source.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((owner, repo))
}

async fn copy_dir_recursive(source: &Path, destination: &Path) -> agent_core::Result<()> {
    let source = source.to_path_buf();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || copy_dir_recursive_sync(&source, &destination))
        .await
        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
}

fn copy_dir_recursive_sync(source: &PathBuf, destination: &PathBuf) -> agent_core::Result<()> {
    std::fs::create_dir_all(destination)
        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
    for entry in std::fs::read_dir(source)
        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
    {
        let entry =
            entry.map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            copy_dir_recursive_sync(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)
                .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "facade_plugins_tests.rs"]
mod tests;
