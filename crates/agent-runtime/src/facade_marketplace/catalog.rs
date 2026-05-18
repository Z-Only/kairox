use std::sync::Arc;

use agent_core::{CatalogQuery as CoreCatalogQuery, EventPayload, ServerEntry as CoreServerEntry};
use agent_mcp::catalog::{
    AggregateCatalogProvider, BuiltinCatalogProvider, CatalogProvider, CatalogQuery, ServerEntry,
    TrustLevel,
};

use super::emit_marketplace_event;
use crate::facade_runtime::LocalRuntime;
use agent_store::EventStore;

// ── Marketplace catalog queries ─────────────────────────────────────────────

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn list_catalog(
        &self,
        query: CoreCatalogQuery,
    ) -> agent_core::Result<Vec<CoreServerEntry>> {
        let inner_query = map_query(query);
        let entries = match self.catalog.as_ref() {
            Some(catalog) => catalog
                .list(&inner_query)
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog list: {e}")))?,
            None => {
                let builtin = builtin_only_provider()?;
                builtin.list(&inner_query).await.map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("catalog list: {e}"))
                })?
            }
        };
        Ok(entries.into_iter().map(map_entry_to_core).collect())
    }

    pub(crate) async fn get_catalog_entry(
        &self,
        id: String,
        _source: Option<String>,
    ) -> agent_core::Result<Option<CoreServerEntry>> {
        let entry =
            match self.catalog.as_ref() {
                Some(catalog) => catalog.get(&id).await.map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("catalog get: {e}"))
                })?,
                None => {
                    let builtin = builtin_only_provider()?;
                    builtin.get(&id).await.map_err(|e| {
                        agent_core::CoreError::InvalidState(format!("catalog get: {e}"))
                    })?
                }
            };
        Ok(entry.map(map_entry_to_core))
    }

    pub(crate) async fn refresh_catalog(&self, _source: Option<String>) -> agent_core::Result<()> {
        let Some(catalog) = self.catalog.as_ref() else {
            return Ok(());
        };

        catalog
            .refresh()
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog refresh: {e}")))?;
        let entry_count = catalog
            .list(&CatalogQuery::default())
            .await
            .map(|v| v.len())
            .unwrap_or(0);
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogRefreshed {
                source: "aggregate".into(),
                entry_count,
            },
        );
        Ok(())
    }
}

// ── Catalog mapping helpers ─────────────────────────────────────────────────

fn map_query(q: CoreCatalogQuery) -> CatalogQuery {
    CatalogQuery {
        keyword: q.keyword,
        category: q.category,
        trust_min: q.trust_min.as_deref().and_then(parse_trust),
        source: q.source,
        limit: q.limit,
    }
}

fn parse_trust(s: &str) -> Option<TrustLevel> {
    match s {
        "unverified" => Some(TrustLevel::Unverified),
        "community" => Some(TrustLevel::Community),
        "verified" => Some(TrustLevel::Verified),
        _ => None,
    }
}

fn trust_to_str(t: TrustLevel) -> &'static str {
    match t {
        TrustLevel::Unverified => "unverified",
        TrustLevel::Community => "community",
        TrustLevel::Verified => "verified",
    }
}

fn map_entry_to_core(e: ServerEntry) -> CoreServerEntry {
    let install_spec_json = serde_json::to_string(&e.install).unwrap_or_else(|_| "{}".into());
    let requirements_json = serde_json::to_string(&e.requirements).unwrap_or_else(|_| "[]".into());
    let default_env_json = serde_json::to_string(&e.default_env).unwrap_or_else(|_| "[]".into());
    CoreServerEntry {
        id: e.id,
        source: e.source,
        display_name: e.display_name,
        summary: e.summary,
        description: e.description,
        categories: e.categories,
        tags: e.tags,
        author: e.author,
        homepage: e.homepage,
        version: e.version,
        trust: trust_to_str(e.trust).into(),
        verified: e.verified,
        icon: e.icon,
        install_spec_json,
        requirements_json,
        default_env_json,
    }
}

fn builtin_only_provider() -> agent_core::Result<Arc<AggregateCatalogProvider>> {
    static BUILTIN_AGGREGATE: std::sync::OnceLock<Arc<AggregateCatalogProvider>> =
        std::sync::OnceLock::new();
    let agg = BUILTIN_AGGREGATE.get_or_init(|| {
        let builtin = Arc::new(
            BuiltinCatalogProvider::new()
                .expect("BUILTIN_JSON must parse; this is a build-time invariant"),
        );
        let providers: Vec<Arc<dyn CatalogProvider>> = vec![builtin];
        Arc::new(AggregateCatalogProvider::new(providers))
    });
    Ok(Arc::clone(agg))
}
