use super::*;
use crate::catalog::CatalogQuery;
use crate::TrustLevel;
use std::collections::HashSet;

#[test]
fn builtin_catalog_has_entries() {
    let provider =
        BuiltinCatalogProvider::new().expect("builtin catalog provider should be constructible");
    let entries = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(provider.list(&CatalogQuery::default()))
        .expect("list should succeed");
    assert!(!entries.is_empty(), "builtin catalog should have entries");
    assert!(
        entries.len() >= 6,
        "expected at least 6 core entries, got {}",
        entries.len()
    );
}

#[test]
fn each_entry_has_required_fields() {
    let provider = BuiltinCatalogProvider::new().unwrap();
    let entries = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(provider.list(&CatalogQuery::default()))
        .unwrap();

    for entry in &entries {
        assert!(
            !entry.id.is_empty(),
            "entry has empty id: display_name='{}'",
            entry.display_name
        );
        assert!(
            !entry.display_name.is_empty(),
            "entry '{}' has empty display_name",
            entry.id
        );
        assert!(
            !entry.summary.is_empty(),
            "entry '{}' has empty summary (description field)",
            entry.id
        );
        assert!(
            !entry.description.is_empty(),
            "entry '{}' has empty description",
            entry.id
        );
        // Verify trust is a valid value.
        match entry.trust {
            TrustLevel::Unverified | TrustLevel::Community | TrustLevel::Verified => {}
        }
    }
}

#[test]
fn entry_ids_are_unique() {
    let provider = BuiltinCatalogProvider::new().unwrap();
    let entries = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(provider.list(&CatalogQuery::default()))
        .unwrap();

    let mut seen = HashSet::new();
    for entry in &entries {
        assert!(
            seen.insert(entry.id.clone()),
            "duplicate entry id found: '{}'",
            entry.id
        );
    }
}
