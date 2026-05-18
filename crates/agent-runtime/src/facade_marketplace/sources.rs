use std::sync::Arc;

use agent_core::{AddCatalogSourceRequest, CatalogSourceView, EventPayload};
use agent_mcp::catalog::CatalogProvider;

use super::{emit_marketplace_event, parse_trust_str};
use crate::facade_runtime::LocalRuntime;
use agent_store::EventStore;

// ── Catalog source mutations ─────────────────────────────────────────────────

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn list_catalog_sources(&self) -> agent_core::Result<Vec<CatalogSourceView>> {
        let builtin_view = builtin_source_view();

        let user_sources = match self.marketplace_dir.as_ref() {
            Some(dir) => {
                let mt = crate::marketplace_toml::MarketplaceToml::new(dir);
                mt.read_sources()
                    .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?
            }
            None => Vec::new(),
        };
        let merged = agent_config::merge_with_defaults(user_sources);

        let mut out = Vec::with_capacity(merged.len() + 1);
        out.push(builtin_view);
        for s in merged {
            out.push(catalog_source_to_view(s));
        }
        Ok(out)
    }

    pub(crate) async fn add_catalog_source(
        &self,
        request: AddCatalogSourceRequest,
    ) -> agent_core::Result<()> {
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let cfg = request_to_source_config(request)?;
        let id = cfg.id.clone();
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.add_source(cfg)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await?;
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogSourceAdded { source: id },
        );
        Ok(())
    }

    pub(crate) async fn remove_catalog_source(&self, id: String) -> agent_core::Result<()> {
        if id == "builtin" {
            return Ok(());
        }
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.remove_source(&id)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await
    }

    pub(crate) async fn set_catalog_source_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        if id == "builtin" {
            return Ok(());
        }
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.set_enabled(&id, enabled)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await
    }

    /// Rebuild the aggregate's remote provider list from
    /// `<marketplace_dir>/config.toml`, calling
    /// [`AggregateCatalogProvider::reload`]. The builtin provider is
    /// always re-added at priority 0.
    pub(crate) async fn rebuild_aggregate_from_disk(&self) -> agent_core::Result<()> {
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let aggregate = self.aggregate_handle.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let http = self.catalog_http.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let cache = self.catalog_cache.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;

        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        let user_sources = mt
            .read_sources()
            .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?;
        let sources = agent_config::merge_with_defaults(user_sources);

        let mut providers: Vec<(u32, Arc<dyn CatalogProvider>)> = Vec::new();
        let builtin = Arc::new(
            agent_mcp::catalog::BuiltinCatalogProvider::new().map_err(|e| {
                agent_core::CoreError::InvalidState(format!("builtin catalog: {e}"))
            })?,
        );
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
            providers.push((
                s.priority,
                agent_mcp::build_remote_catalog_provider(cfg, http.clone(), cache.clone()),
            ));
        }
        aggregate.reload(providers);
        Ok(())
    }
}

// ── Source mapping helpers ───────────────────────────────────────────────────

fn builtin_source_view() -> CatalogSourceView {
    CatalogSourceView {
        id: "builtin".into(),
        display_name: "Built-in".into(),
        kind: "builtin".into(),
        url: String::new(),
        api_key_env: None,
        priority: 0,
        default_trust: "verified".into(),
        enabled: true,
        cache_ttl_seconds: None,
        last_error: None,
    }
}

fn catalog_source_to_view(s: agent_config::CatalogSourceConfig) -> CatalogSourceView {
    CatalogSourceView {
        id: s.id,
        display_name: s.display_name,
        kind: match s.kind {
            agent_config::CatalogSourceKind::McpRegistry => "mcp_registry".into(),
        },
        url: s.url,
        api_key_env: s.api_key_env,
        priority: s.priority,
        default_trust: s.default_trust,
        enabled: s.enabled,
        cache_ttl_seconds: s.cache_ttl_seconds,
        last_error: None,
    }
}

fn request_to_source_config(
    r: AddCatalogSourceRequest,
) -> agent_core::Result<agent_config::CatalogSourceConfig> {
    let kind = match r.kind.as_str() {
        "mcp_registry" => agent_config::CatalogSourceKind::McpRegistry,
        other => {
            return Err(agent_core::CoreError::InvalidState(format!(
                "unsupported catalog source kind: {other}"
            )));
        }
    };
    if !r.url.starts_with("http://") && !r.url.starts_with("https://") {
        return Err(agent_core::CoreError::InvalidState(
            "url must start with http:// or https://".into(),
        ));
    }
    Ok(agent_config::CatalogSourceConfig {
        id: r.id,
        display_name: r.display_name,
        kind,
        url: r.url,
        api_key_env: r.api_key_env,
        priority: r.priority.unwrap_or(100),
        default_trust: r.default_trust.unwrap_or_else(|| "community".into()),
        enabled: r.enabled.unwrap_or(true),
        cache_ttl_seconds: r.cache_ttl_seconds,
    })
}
