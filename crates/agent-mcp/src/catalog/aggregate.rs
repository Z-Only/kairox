//! Aggregates multiple [`CatalogProvider`]s into one logical view.

use crate::catalog::{CatalogProvider, CatalogQuery, CatalogResult, ServerEntry};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

pub struct AggregateCatalogProvider {
    inner: Vec<Arc<dyn CatalogProvider>>,
}

impl AggregateCatalogProvider {
    pub fn new(inner: Vec<Arc<dyn CatalogProvider>>) -> Self {
        Self { inner }
    }

    pub fn add(&mut self, provider: Arc<dyn CatalogProvider>) {
        self.inner.push(provider);
    }
}

#[async_trait]
impl CatalogProvider for AggregateCatalogProvider {
    fn source_id(&self) -> &str {
        "aggregate"
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let mut all: Vec<ServerEntry> = Vec::new();
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for provider in &self.inner {
            // Honour source filter cheaply.
            if let Some(src) = &query.source {
                if provider.source_id() != src {
                    continue;
                }
            }
            for entry in provider.list(query).await? {
                let key = (entry.source.clone(), entry.id.clone());
                if seen.insert(key) {
                    all.push(entry);
                }
            }
        }
        all.sort_by(|a, b| {
            b.trust
                .cmp(&a.trust)
                .then_with(|| a.source.cmp(&b.source))
                .then_with(|| a.display_name.cmp(&b.display_name))
        });
        if let Some(limit) = query.limit {
            all.truncate(limit);
        }
        Ok(all)
    }

    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
        for provider in &self.inner {
            if let Some(entry) = provider.get(id).await? {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    async fn refresh(&self) -> CatalogResult<()> {
        for provider in &self.inner {
            provider.refresh().await?;
        }
        Ok(())
    }
}
