use super::*;
use crate::catalog::skills::SkillCatalogResult;
use async_trait::async_trait;

struct StaticSkillProvider {
    id: &'static str,
    entries: Vec<SkillCatalogEntry>,
}

#[async_trait]
impl SkillCatalogProvider for StaticSkillProvider {
    fn source_id(&self) -> &str {
        self.id
    }

    async fn search(&self, _q: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        Ok(self.entries.clone())
    }

    async fn list(&self, _q: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        Ok(self.entries.clone())
    }
}

fn make_entry(id: &str, source: &str) -> SkillCatalogEntry {
    SkillCatalogEntry {
        catalog_id: id.into(),
        name: id.into(),
        description: String::new(),
        source: source.into(),
        source_url: String::new(),
        install_count: None,
        github_stars: None,
        security_score: None,
        rating: None,
        package: id.into(),
        package_url: None,
    }
}

struct FailingSkillProvider {
    id: &'static str,
}

#[async_trait]
impl SkillCatalogProvider for FailingSkillProvider {
    fn source_id(&self) -> &str {
        self.id
    }
    async fn search(&self, _q: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        Err(SkillCatalogError::Provider("boom".into()))
    }
}

#[tokio::test]
async fn aggregates_multiple_sources() {
    let a = Arc::new(StaticSkillProvider {
        id: "a",
        entries: vec![make_entry("x", "a")],
    });
    let b = Arc::new(StaticSkillProvider {
        id: "b",
        entries: vec![make_entry("y", "b")],
    });
    let agg = AggregateSkillCatalogProvider::new(vec![(10, a), (20, b)]);
    let q = SkillCatalogQuery::default();
    let results = agg.search(&q).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn one_source_failure_does_not_fail_aggregate() {
    let ok = Arc::new(StaticSkillProvider {
        id: "ok",
        entries: vec![make_entry("x", "ok")],
    });
    let bad: Arc<dyn SkillCatalogProvider> = Arc::new(FailingSkillProvider { id: "bad" });
    let agg = AggregateSkillCatalogProvider::new(vec![(10, ok), (20, bad)]);
    let results = agg.search(&SkillCatalogQuery::default()).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, "ok");
}

#[tokio::test]
async fn deduplicates_by_source_and_catalog_id() {
    let p1 = Arc::new(StaticSkillProvider {
        id: "src",
        entries: vec![make_entry("dup", "src")],
    });
    let p2 = Arc::new(StaticSkillProvider {
        id: "src",
        entries: vec![make_entry("dup", "src"), make_entry("uniq", "src")],
    });
    let agg = AggregateSkillCatalogProvider::new(vec![(10, p1), (20, p2)]);
    let results = agg.search(&SkillCatalogQuery::default()).await.unwrap();
    assert_eq!(results.len(), 2, "should dedup by (source, catalog_id)");
}

#[tokio::test]
async fn respects_limit() {
    let p = Arc::new(StaticSkillProvider {
        id: "src",
        entries: vec![
            make_entry("a", "src"),
            make_entry("b", "src"),
            make_entry("c", "src"),
        ],
    });
    let agg = AggregateSkillCatalogProvider::new(vec![(10, p)]);
    let q = SkillCatalogQuery {
        limit: Some(2),
        ..SkillCatalogQuery::default()
    };
    let results = agg.search(&q).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn filters_by_source_ids() {
    let a = Arc::new(StaticSkillProvider {
        id: "a",
        entries: vec![make_entry("x", "a")],
    });
    let b = Arc::new(StaticSkillProvider {
        id: "b",
        entries: vec![make_entry("y", "b")],
    });
    let agg = AggregateSkillCatalogProvider::new(vec![(10, a), (20, b)]);
    let q = SkillCatalogQuery {
        sources: Some(vec!["a".into()]),
        ..SkillCatalogQuery::default()
    };
    let results = agg.search(&q).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, "a");
}

#[tokio::test]
async fn empty_providers_return_empty() {
    let agg = AggregateSkillCatalogProvider::new(vec![]);
    let results = agg.search(&SkillCatalogQuery::default()).await.unwrap();
    assert!(results.is_empty());
}
