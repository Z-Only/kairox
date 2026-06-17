use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use agent_mcp::catalog::skills::{
    aggregate::AggregateSkillCatalogProvider,
    remote::{build_skill_provider, RemoteSkillSourceConfig, SkillSourceKind},
    SkillCatalogProvider,
};
use agent_mcp::catalog::{AggregateCatalogProvider, BuiltinCatalogProvider, CatalogProvider};
use agent_mcp::installer::{Installer, OsRuntimeProbe};
use agent_mcp::{build_remote_catalog_provider, HttpResponseCache, SharedHttpClient};

use super::parse_trust_str;
use crate::catalog_sink::CatalogEventSink;
use crate::facade_runtime::LocalRuntime;
use agent_store::EventStore;

// ── Builder methods ─────────────────────────────────────────────────────────

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    /// Configure the skill catalog with a cache directory. Creates an internal
    /// HTTP client automatically.
    pub fn with_skill_catalog(mut self, dir: Option<PathBuf>) -> Self {
        if let Some(ref d) = dir {
            self.skill_sources_toml = Some(crate::skill_sources_toml::SkillSourcesToml::new(d));
        }
        self.skill_catalog_http = SharedHttpClient::new().ok();
        self.skill_catalog_cache_dir = dir;
        self
    }

    /// Wire the MCP marketplace: built-in catalog provider + on-disk installer
    /// targeting `<config_dir>/config.toml`.
    ///
    /// Without this, the catalog-related [`AppFacade`] methods return errors
    /// (or empty results) because they have nowhere to read from or write to.
    pub fn with_marketplace(self, config_dir: PathBuf) -> crate::Result<Self> {
        self.with_marketplace_loaded(config_dir, &[])
    }

    /// Phase 2: like [`with_marketplace`] but also registers user-configured
    /// remote catalog sources. The runtime stores the marketplace directory
    /// for future atomic toml mutations + reloads.
    pub fn with_marketplace_loaded(
        mut self,
        config_dir: PathBuf,
        sources: &[agent_config::CatalogSourceConfig],
    ) -> crate::Result<Self> {
        let cache_dir = config_dir.join("catalog-cache");
        let event_tx = self.event_tx.clone();
        let aggregate = build_catalog_provider(sources, cache_dir.clone(), event_tx)
            .map_err(|e| crate::RuntimeError::Other(format!("catalog provider: {e}")))?;
        let aggregate_arc = Arc::new(aggregate);
        let dyn_arc: Arc<dyn CatalogProvider> = aggregate_arc.clone();
        self.aggregate_handle = Some(aggregate_arc);
        self.catalog = Some(dyn_arc);

        let toml_path = config_dir.join("config.toml");
        self.installer = Some(Arc::new(Installer::new(
            toml_path,
            Arc::new(OsRuntimeProbe),
        )));
        self.catalog_http = Some(
            SharedHttpClient::new()
                .map_err(|e| crate::RuntimeError::Other(format!("http client: {e}")))?,
        );
        self.catalog_cache = Some(Arc::new(HttpResponseCache::new(cache_dir)));
        self.marketplace_dir = Some(config_dir);
        Ok(self)
    }

    /// Rebuild the skill catalog aggregate from `skill_sources.toml` and
    /// re-create providers. Called after every toml mutation so the runtime
    /// always reflects the latest persisted configuration.
    pub(crate) fn rebuild_skill_aggregate(&self) -> agent_core::Result<()> {
        let Some(toml) = &self.skill_sources_toml else {
            return Ok(());
        };
        let http = self.skill_catalog_http.clone().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill catalog http not configured".into())
        })?;
        let sources = toml.merge_with_defaults(&toml.read());
        let providers: Vec<(u32, Arc<dyn SkillCatalogProvider>)> = sources
            .into_iter()
            .filter(|s| s.enabled)
            .filter_map(|s| {
                let kind = SkillSourceKind::from_str(&s.kind).ok()?;
                let cfg = RemoteSkillSourceConfig {
                    id: s.id.clone(),
                    display_name: s.display_name.clone(),
                    kind,
                    url: s.url.clone(),
                    search_template: s.search_template.clone(),
                    download_template: s.download_template.clone(),
                    list_template: s.list_template.clone(),
                    detail_template: s.detail_template.clone(),
                    enabled: s.enabled,
                    priority: s.priority,
                    cache_ttl_seconds: s.cache_ttl_seconds,
                };
                Some((s.priority, build_skill_provider(cfg, http.clone())))
            })
            .collect();
        if let Some(catalog) = self.skill_catalog.get() {
            catalog.reload(providers);
        } else {
            let agg = Arc::new(AggregateSkillCatalogProvider::new(providers));
            let _ = self.skill_catalog.set(agg);
        }
        Ok(())
    }

    /// Get (or lazily build) the skill catalog aggregate. Returns `None`
    /// only when the catalog has never been configured.
    pub(crate) fn ensure_skill_catalog(&self) -> Option<Arc<AggregateSkillCatalogProvider>> {
        if let Some(c) = self.skill_catalog.get() {
            return Some(c.clone());
        }
        let _ = self.rebuild_skill_aggregate();
        self.skill_catalog.get().cloned()
    }
}

// ── Catalog provider builder ────────────────────────────────────────────────

/// Build the aggregate catalog provider: builtin (priority 0) plus every
/// enabled remote source. Wires a [`CatalogEventSink`] for failure
/// observability.
fn build_catalog_provider(
    sources: &[agent_config::CatalogSourceConfig],
    cache_dir: PathBuf,
    event_tx: tokio::sync::broadcast::Sender<agent_core::DomainEvent>,
) -> anyhow::Result<AggregateCatalogProvider> {
    let http = SharedHttpClient::new()?;
    let cache = Arc::new(HttpResponseCache::new(cache_dir));

    let mut providers: Vec<(u32, Arc<dyn CatalogProvider>)> = Vec::new();
    let builtin = Arc::new(BuiltinCatalogProvider::new()?);
    providers.push((0, builtin));

    for s in sources.iter().filter(|s| s.enabled) {
        let cfg = agent_mcp::RemoteSourceConfig {
            id: s.id.clone(),
            display_name: s.display_name.clone(),
            kind: match s.kind {
                agent_config::CatalogSourceKind::McpRegistry => {
                    agent_mcp::RemoteSourceKind::McpRegistry
                }
            },
            url: s.url.clone(),
            api_key_env: s.api_key_env.clone(),
            priority: s.priority,
            default_trust: parse_trust_str(&s.default_trust),
            enabled: true,
            cache_ttl_seconds: s.cache_ttl_seconds,
        };
        let provider = build_remote_catalog_provider(cfg, http.clone(), cache.clone());
        providers.push((s.priority, provider));
    }

    let sink: Arc<dyn agent_mcp::DomainEventSink> = CatalogEventSink::new(event_tx);
    Ok(AggregateCatalogProvider::new_with_priority(
        providers,
        Some(sink),
    ))
}

#[cfg(test)]
#[path = "skill_catalog_tests.rs"]
mod tests;
