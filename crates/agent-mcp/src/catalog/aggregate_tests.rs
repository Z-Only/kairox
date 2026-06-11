use super::*;

#[cfg(test)]
mod resilience_tests {
    //! Contract guards for the "marketplace not configured" degraded path.
    //!
    //! `crates/agent-runtime/src/facade_runtime.rs` builds a builtin-only
    //! `AggregateCatalogProvider` when the user has no `[mcp_marketplace]`
    //! section in `kairox.toml`. These tests pin down the contracts that
    //! that fallback depends on:
    //!
    //!   1. An aggregator with **zero** providers must not error on `list`
    //!      / `get` / `refresh` — it must yield an empty/None result.
    //!   2. The `new(Vec)` ergonomic constructor preserves these contracts.
    use super::*;
    use crate::catalog::CatalogQuery;

    #[tokio::test]
    async fn empty_providers_yield_empty_list_no_error() {
        let agg = AggregateCatalogProvider::new(Vec::new());
        let entries = agg
            .list(&CatalogQuery::default())
            .await
            .expect("empty providers must not error on list");
        assert!(entries.is_empty(), "empty providers must yield empty list");
    }

    #[tokio::test]
    async fn empty_providers_yield_none_on_get_no_error() {
        let agg = AggregateCatalogProvider::new(Vec::new());
        let entry = agg
            .get("anything")
            .await
            .expect("empty providers must not error on get");
        assert!(entry.is_none(), "empty providers must yield None on get");
    }

    #[tokio::test]
    async fn empty_providers_refresh_is_noop_no_error() {
        let agg = AggregateCatalogProvider::new(Vec::new());
        agg.refresh()
            .await
            .expect("empty providers must not error on refresh");
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
            verified: false,
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
        results: StdMutex<Vec<(String, usize)>>,
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
        async fn emit_source_results_arrived(&self, source_id: &str, entries: &[ServerEntry]) {
            self.results
                .lock()
                .unwrap()
                .push((source_id.to_string(), entries.len()));
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
            elapsed < std::time::Duration::from_millis(350),
            "expected parallel ~100ms, got {elapsed:?}"
        );
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
