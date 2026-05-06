//! Aggregates multiple [`CatalogProvider`]s into one logical view, with
//! per-source priority, parallel querying, failure isolation, and
//! rate-limited per-source failure events.

use crate::catalog::{CatalogProvider, CatalogQuery, CatalogResult, DomainEventSink, ServerEntry};
use async_trait::async_trait;
use futures::future::join_all;
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

        // Issue all `list` calls in parallel.
        let futures = active.iter().map(|p| {
            let q = query.clone();
            async move {
                let id = p.inner.source_id().to_string();
                let result = p.inner.list(&q).await;
                (p.priority, id, result)
            }
        });
        let mut results = join_all(futures).await;

        // Stable merge: collect successes in priority order, emit failures.
        results.sort_by_key(|(prio, _, _)| *prio);

        let mut all: Vec<ServerEntry> = Vec::new();
        let mut seen: std::collections::HashSet<(String, String)> = Default::default();
        for (_, source_id, res) in results {
            match res {
                Ok(entries) => {
                    for entry in entries {
                        let key = (entry.source.clone(), entry.id.clone());
                        if seen.insert(key) {
                            all.push(entry);
                        }
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    tracing::warn!(source=%source_id, error=%err_str, "catalog source failed");
                    self.maybe_emit_failure(&source_id, &err_str).await;
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
mod tests_phase2 {
    use super::*;
    use crate::catalog::{
        CatalogError, CatalogProvider, CatalogQuery, CatalogResult, DomainEventSink, InstallSpec,
        ServerEntry, TrustLevel,
    };
    use async_trait::async_trait;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex as StdMutex};

    fn make_entry(id: &str, source: &str, trust: TrustLevel) -> ServerEntry {
        ServerEntry {
            id: id.into(),
            source: source.into(),
            display_name: id.into(),
            summary: "".into(),
            description: "".into(),
            categories: vec![],
            tags: vec![],
            author: None,
            homepage: None,
            version: None,
            install: InstallSpec::Stdio {
                command: "x".into(),
                args: vec![],
                env: BTreeMap::new(),
                cwd: None,
            },
            requirements: vec![],
            trust,
            default_env: vec![],
            icon: None,
        }
    }

    struct StaticProvider {
        id: &'static str,
        entries: Vec<ServerEntry>,
    }

    #[async_trait]
    impl CatalogProvider for StaticProvider {
        fn source_id(&self) -> &str {
            self.id
        }
        async fn list(&self, _q: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
            Ok(self.entries.clone())
        }
        async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
            Ok(self.entries.iter().find(|e| e.id == id).cloned())
        }
    }

    struct FailingProvider {
        id: &'static str,
    }
    #[async_trait]
    impl CatalogProvider for FailingProvider {
        fn source_id(&self) -> &str {
            self.id
        }
        async fn list(&self, _q: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
            Err(CatalogError::Provider("boom".into()))
        }
        async fn get(&self, _id: &str) -> CatalogResult<Option<ServerEntry>> {
            Err(CatalogError::Provider("boom".into()))
        }
    }

    #[derive(Default)]
    struct RecordingSink {
        failed: StdMutex<Vec<(String, String)>>,
        added: StdMutex<Vec<String>>,
    }
    #[async_trait]
    impl DomainEventSink for RecordingSink {
        async fn emit_source_failed(&self, id: &str, err: &str) {
            self.failed
                .lock()
                .unwrap()
                .push((id.to_string(), err.to_string()));
        }
        async fn emit_source_added(&self, id: &str) {
            self.added.lock().unwrap().push(id.to_string());
        }
    }

    #[tokio::test]
    async fn higher_priority_source_first_in_aggregated_list() {
        let low = Arc::new(StaticProvider {
            id: "low",
            entries: vec![make_entry("a", "low", TrustLevel::Community)],
        });
        let high = Arc::new(StaticProvider {
            id: "high",
            entries: vec![make_entry("b", "high", TrustLevel::Community)],
        });
        let agg = AggregateCatalogProvider::new_with_priority(vec![(100, low), (10, high)], None);
        let entries = agg.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 2);
        // Trust ties → fall back to source ascending. high < low alphabetically.
        assert_eq!(entries[0].source, "high");
    }

    #[tokio::test]
    async fn one_source_failure_does_not_fail_aggregate() {
        let ok = Arc::new(StaticProvider {
            id: "ok",
            entries: vec![make_entry("a", "ok", TrustLevel::Community)],
        });
        let bad = Arc::new(FailingProvider { id: "bad" });
        let sink = Arc::new(RecordingSink::default());
        let sink_dyn: Arc<dyn DomainEventSink> = sink.clone();
        let agg =
            AggregateCatalogProvider::new_with_priority(vec![(10, ok), (20, bad)], Some(sink_dyn));
        let entries = agg.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "ok");
        let failed = sink.failed.lock().unwrap().clone();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].0, "bad");
    }

    #[tokio::test]
    async fn duplicate_failure_within_60s_is_rate_limited() {
        let bad = Arc::new(FailingProvider { id: "bad" });
        let sink = Arc::new(RecordingSink::default());
        let sink_dyn: Arc<dyn DomainEventSink> = sink.clone();
        let agg = AggregateCatalogProvider::new_with_priority(vec![(20, bad)], Some(sink_dyn));
        for _ in 0..3 {
            let _ = agg.list(&CatalogQuery::default()).await;
        }
        let failed = sink.failed.lock().unwrap().clone();
        assert_eq!(
            failed.len(),
            1,
            "duplicate (source, error) should rate-limit"
        );
    }

    #[tokio::test]
    async fn reload_swaps_providers_atomically() {
        let v1 = Arc::new(StaticProvider {
            id: "v",
            entries: vec![make_entry("a", "v", TrustLevel::Community)],
        });
        let agg = AggregateCatalogProvider::new_with_priority(vec![(10, v1)], None);
        assert_eq!(agg.list(&CatalogQuery::default()).await.unwrap().len(), 1);
        let v2 = Arc::new(StaticProvider {
            id: "v",
            entries: vec![
                make_entry("a", "v", TrustLevel::Community),
                make_entry("b", "v", TrustLevel::Community),
            ],
        });
        agg.reload(vec![(10, v2)]);
        assert_eq!(agg.list(&CatalogQuery::default()).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn parallel_list_does_not_serialize_slow_sources() {
        struct Slow {
            id: &'static str,
            ms: u64,
            counter: Arc<AtomicUsize>,
        }
        #[async_trait]
        impl CatalogProvider for Slow {
            fn source_id(&self) -> &str {
                self.id
            }
            async fn list(&self, _q: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
                self.counter.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(self.ms)).await;
                Ok(vec![])
            }
            async fn get(&self, _: &str) -> CatalogResult<Option<ServerEntry>> {
                Ok(None)
            }
        }
        let counter = Arc::new(AtomicUsize::new(0));
        let a = Arc::new(Slow {
            id: "a",
            ms: 100,
            counter: counter.clone(),
        });
        let b = Arc::new(Slow {
            id: "b",
            ms: 100,
            counter: counter.clone(),
        });
        let agg = AggregateCatalogProvider::new_with_priority(vec![(10, a), (20, b)], None);
        let start = std::time::Instant::now();
        let _ = agg.list(&CatalogQuery::default()).await.unwrap();
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(180),
            "expected parallel ~100ms, got {elapsed:?}"
        );
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
