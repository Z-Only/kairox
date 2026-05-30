use super::*;
use crate::catalog::{InstallSpec, TrustLevel};
use std::collections::BTreeMap;

fn sample_entries() -> Vec<ServerEntry> {
    vec![ServerEntry {
        id: "s".into(),
        source: "x".into(),
        display_name: "S".into(),
        summary: "".into(),
        description: "".into(),
        categories: vec![],
        tags: vec![],
        author: None,
        homepage: None,
        version: None,
        install: InstallSpec::Stdio {
            command: "echo".into(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        },
        requirements: vec![],
        trust: TrustLevel::Community,
        default_env: vec![],
        icon: None,
        verified: false,
    }]
}

#[tokio::test]
async fn put_then_get_round_trips_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let cache = HttpResponseCache::new(dir.path().to_path_buf());
    let v = CachedResponse {
        fetched_at_unix: 100,
        etag: Some("W/\"abc\"".into()),
        last_modified: None,
        entries: sample_entries(),
    };
    cache.put("src1", v).await.unwrap();
    let got = cache.get("src1").await.unwrap();
    assert_eq!(got.entries.len(), 1);
    assert_eq!(got.etag.as_deref(), Some("W/\"abc\""));
}

#[tokio::test]
async fn put_persists_to_disk_and_reloads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let cache1 = HttpResponseCache::new(path.clone());
    let v = CachedResponse {
        fetched_at_unix: 200,
        etag: None,
        last_modified: None,
        entries: sample_entries(),
    };
    cache1.put("src2", v).await.unwrap();
    let cache2 = HttpResponseCache::new(path);
    let got = cache2.get("src2").await.unwrap();
    assert_eq!(got.fetched_at_unix, 200);
    assert_eq!(got.entries.len(), 1);
}

#[test]
fn is_fresh_within_ttl() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let v = CachedResponse {
        fetched_at_unix: now,
        etag: None,
        last_modified: None,
        entries: vec![],
    };
    assert!(HttpResponseCache::is_fresh(&v, 60));
}

#[test]
fn is_stale_after_ttl() {
    let v = CachedResponse {
        fetched_at_unix: 0,
        etag: None,
        last_modified: None,
        entries: vec![],
    };
    assert!(!HttpResponseCache::is_fresh(&v, 60));
}

#[tokio::test]
async fn lock_for_returns_same_mutex_across_calls() {
    let dir = tempfile::tempdir().unwrap();
    let cache = HttpResponseCache::new(dir.path().to_path_buf());
    let l1 = cache.lock_for("k").await;
    let l2 = cache.lock_for("k").await;
    assert!(Arc::ptr_eq(&l1, &l2));
}
