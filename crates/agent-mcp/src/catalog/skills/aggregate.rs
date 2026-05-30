//! Aggregates multiple [`SkillCatalogProvider`]s with priority ordering,
//! parallel querying, and failure isolation.

use crate::catalog::skills::{
    SkillCatalogEntry, SkillCatalogProvider, SkillCatalogQuery, SkillCatalogResult,
};

#[cfg(test)]
use crate::catalog::skills::SkillCatalogError;
use async_trait::async_trait;
use futures::future::join_all;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct PrioritisedProvider {
    priority: u32,
    inner: Arc<dyn SkillCatalogProvider>,
}

pub struct AggregateSkillCatalogProvider {
    inner: Mutex<Vec<PrioritisedProvider>>,
}

impl AggregateSkillCatalogProvider {
    pub fn new(providers: Vec<(u32, Arc<dyn SkillCatalogProvider>)>) -> Self {
        let mut inner: Vec<PrioritisedProvider> = providers
            .into_iter()
            .map(|(priority, inner)| PrioritisedProvider { priority, inner })
            .collect();
        inner.sort_by_key(|p| p.priority);
        Self {
            inner: Mutex::new(inner),
        }
    }

    pub fn reload(&self, providers: Vec<(u32, Arc<dyn SkillCatalogProvider>)>) {
        let mut inner: Vec<PrioritisedProvider> = providers
            .into_iter()
            .map(|(priority, inner)| PrioritisedProvider { priority, inner })
            .collect();
        inner.sort_by_key(|p| p.priority);
        match self.inner.try_lock() {
            Ok(mut guard) => *guard = inner,
            Err(_) => {
                let mut guard = self.inner.blocking_lock();
                *guard = inner;
            }
        }
    }

    async fn query(
        &self,
        query: &SkillCatalogQuery,
        use_search: bool,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        let active: Vec<PrioritisedProvider> = providers
            .into_iter()
            .filter(|p| {
                query
                    .sources
                    .as_ref()
                    .map(|srcs| srcs.contains(&p.inner.source_id().to_string()))
                    .unwrap_or(true)
            })
            .collect();

        let futures = active.iter().map(|p| {
            let q = query.clone();
            async move {
                let id = p.inner.source_id().to_string();
                let result = if use_search {
                    p.inner.search(&q).await
                } else {
                    p.inner.list(&q).await
                };
                (p.priority, id, result)
            }
        });
        let mut results = join_all(futures).await;
        results.sort_by_key(|(prio, _, _)| *prio);

        let mut all: Vec<SkillCatalogEntry> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for (_, source_id, res) in results {
            match res {
                Ok(entries) => {
                    for entry in entries {
                        let key = format!("{}:{}", source_id, entry.catalog_id);
                        if seen.insert(key) {
                            all.push(entry);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(source=%source_id, error=%e, "skill catalog source failed");
                }
            }
        }

        if let Some(limit) = query.limit {
            all.truncate(limit);
        }
        Ok(all)
    }
}

#[async_trait]
impl SkillCatalogProvider for AggregateSkillCatalogProvider {
    fn source_id(&self) -> &str {
        "aggregate"
    }

    async fn search(
        &self,
        query: &SkillCatalogQuery,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        self.query(query, true).await
    }

    async fn list(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        self.query(query, false).await
    }

    async fn refresh(&self) -> SkillCatalogResult<()> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        let futures = providers.iter().map(|p| async move {
            let id = p.inner.source_id().to_string();
            (id, p.inner.refresh().await)
        });
        let results = join_all(futures).await;
        for (source_id, res) in results {
            if let Err(e) = res {
                tracing::warn!(source=%source_id, error=%e, "skill catalog refresh failed");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "aggregate_tests.rs"]
mod tests;
