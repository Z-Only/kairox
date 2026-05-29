//! Aggregates multiple [`CatalogProvider`]s into one logical view, with
//! per-source priority, parallel querying, failure isolation, and
//! rate-limited per-source failure events.

use crate::catalog::{CatalogProvider, CatalogQuery, CatalogResult, DomainEventSink, ServerEntry};
use async_trait::async_trait;
use futures::future::join_all;
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Clone)]
struct PrioritisedProvider {
    priority: u32,
    inner: Arc<dyn CatalogProvider>,
}

pub struct AggregateCatalogProvider {
    inner: Mutex<Vec<PrioritisedProvider>>,
    event_sink: Option<Arc<dyn DomainEventSink>>,
    /// Last time we emitted `(source_id, error_signature)` → for rate limit.
    failure_emit_log: Mutex<HashMap<(String, String), Instant>>,
}

const FAILURE_RATE_LIMIT: Duration = Duration::from_secs(60);

impl AggregateCatalogProvider {
    /// Backward-compatible constructor: equal priority preserved by insertion
    /// order (each subsequent provider gets a higher priority value, i.e.
    /// lower precedence).
    pub fn new(inner: Vec<Arc<dyn CatalogProvider>>) -> Self {
        let providers = inner
            .into_iter()
            .enumerate()
            .map(|(i, p)| PrioritisedProvider {
                priority: 100 + i as u32,
                inner: p,
            })
            .collect();
        Self {
            inner: Mutex::new(providers),
            event_sink: None,
            failure_emit_log: Mutex::new(HashMap::new()),
        }
    }

    pub fn new_with_priority(
        providers: Vec<(u32, Arc<dyn CatalogProvider>)>,
        event_sink: Option<Arc<dyn DomainEventSink>>,
    ) -> Self {
        let mut inner: Vec<PrioritisedProvider> = providers
            .into_iter()
            .map(|(priority, inner)| PrioritisedProvider { priority, inner })
            .collect();
        inner.sort_by_key(|p| p.priority);
        Self {
            inner: Mutex::new(inner),
            event_sink,
            failure_emit_log: Mutex::new(HashMap::new()),
        }
    }

    pub fn reload(&self, providers: Vec<(u32, Arc<dyn CatalogProvider>)>) {
        let mut inner: Vec<PrioritisedProvider> = providers
            .into_iter()
            .map(|(priority, inner)| PrioritisedProvider { priority, inner })
            .collect();
        inner.sort_by_key(|p| p.priority);
        // try_lock is fine here: callers must serialise reload against
        // queries externally (e.g., during runtime startup or a settings
        // edit). If contention occurs we fall back to blocking_lock to
        // avoid losing the update.
        match self.inner.try_lock() {
            Ok(mut guard) => *guard = inner,
            Err(_) => {
                let mut guard = self.inner.blocking_lock();
                *guard = inner;
            }
        }
    }

    pub fn add(&self, provider: Arc<dyn CatalogProvider>) {
        let mut guard = self.inner.blocking_lock();
        let next_priority = guard.iter().map(|p| p.priority).max().unwrap_or(99) + 1;
        guard.push(PrioritisedProvider {
            priority: next_priority,
            inner: provider,
        });
        guard.sort_by_key(|p| p.priority);
    }

    async fn maybe_emit_failure(&self, source_id: &str, err: &str) {
        let key = (source_id.to_string(), err.to_string());
        let mut log = self.failure_emit_log.lock().await;
        let now = Instant::now();
        if let Some(prev) = log.get(&key) {
            if now.duration_since(*prev) < FAILURE_RATE_LIMIT {
                return;
            }
        }
        log.insert(key, now);
        drop(log);
        if let Some(sink) = &self.event_sink {
            sink.emit_source_failed(source_id, err).await;
        }
    }
}

#[async_trait]
impl CatalogProvider for AggregateCatalogProvider {
    fn source_id(&self) -> &str {
        "aggregate"
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        let active: Vec<PrioritisedProvider> = providers
            .into_iter()
            .filter(|p| {
                query
                    .source
                    .as_ref()
                    .map(|src| p.inner.source_id() == src)
                    .unwrap_or(true)
            })
            .collect();

        // Fire all provider queries concurrently. Process results as they
        // arrive so the frontend can update incrementally — no waiting for
        // the slowest source.
        let mut tasks: FuturesUnordered<_> = active
            .iter()
            .map(|p| {
                let q = query.clone();
                let id = p.inner.source_id().to_string();
                let provider = p.inner.clone();
                tokio::spawn(async move {
                    let result = provider.list(&q).await;
                    (id, result)
                })
            })
            .collect();

        let mut all: Vec<ServerEntry> = Vec::new();
        // Track (position in `all`, source) for each entry id so remote
        // sources can replace builtin entries for the same logical server.
        let mut index: HashMap<String, (usize, String)> = HashMap::new();

        while let Some(task_result) = tasks.next().await {
            let (source_id, res) = match task_result {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(error=%e, "catalog provider task panicked");
                    continue;
                }
            };
            match res {
                Ok(entries) => {
                    for entry in entries {
                        match index.get(&entry.id) {
                            Some((pos, existing_source)) => {
                                // Remote replaces builtin for same logical server.
                                if existing_source == "builtin" && entry.source != "builtin" {
                                    all[*pos] = entry.clone();
                                    index.insert(entry.id.clone(), (*pos, entry.source.clone()));
                                }
                            }
                            None => {
                                let pos = all.len();
                                all.push(entry.clone());
                                index.insert(entry.id.clone(), (pos, entry.source.clone()));
                            }
                        }
                    }
                    // Re-sort after each merge so incremental events are
                    // always consistent.
                    all.sort_by(|a, b| {
                        b.trust
                            .cmp(&a.trust)
                            .then_with(|| a.source.cmp(&b.source))
                            .then_with(|| a.display_name.cmp(&b.display_name))
                    });
                    if let Some(sink) = &self.event_sink {
                        sink.emit_source_results_arrived(&source_id, &all).await;
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    tracing::warn!(source=%source_id, error=%err_str, "catalog source failed");
                    self.maybe_emit_failure(&source_id, &err_str).await;
                }
            }
        }

        if let Some(limit) = query.limit {
            all.truncate(limit);
        }
        Ok(all)
    }

    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        for p in providers {
            match p.inner.get(id).await {
                Ok(Some(e)) => return Ok(Some(e)),
                Ok(None) => continue,
                Err(e) => {
                    self.maybe_emit_failure(p.inner.source_id(), &e.to_string())
                        .await;
                    continue;
                }
            }
        }
        Ok(None)
    }

    async fn refresh(&self) -> CatalogResult<()> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        let futures = providers.iter().map(|p| async move {
            let id = p.inner.source_id().to_string();
            (id, p.inner.refresh().await)
        });
        let results = join_all(futures).await;
        for (source_id, res) in results {
            if let Err(e) = res {
                self.maybe_emit_failure(&source_id, &e.to_string()).await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "aggregate_tests.rs"]
mod tests;
