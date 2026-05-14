//! On-disk + in-memory cache for remote catalog HTTP responses.
//!
//! Each source ID maps to a single [`CachedResponse`] persisted to a JSON
//! file under `cache_dir`. Writes go through a temp file + rename for
//! crash-safety. A per-key [`tokio::sync::Mutex`] protects against
//! concurrent refetch (single-flight).

use crate::catalog::remote::RemoteError;
use crate::catalog::ServerEntry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    pub fetched_at_unix: u64,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub entries: Vec<ServerEntry>,
}

pub struct HttpResponseCache {
    cache_dir: PathBuf,
    in_memory: Mutex<HashMap<String, CachedResponse>>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl HttpResponseCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        // Ensure cache directory exists at initialization time to avoid
        // "No such file or directory" errors on first access.
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            tracing::warn!(
                error = %e,
                path = ?cache_dir,
                "failed to create catalog cache directory"
            );
        }
        Self {
            cache_dir,
            in_memory: Mutex::new(HashMap::new()),
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, key: &str) -> Option<CachedResponse> {
        if let Some(v) = self.in_memory.lock().await.get(key) {
            return Some(v.clone());
        }
        let path = self.cache_dir.join(format!("{key}.json"));
        let bytes = tokio::fs::read(&path).await.ok()?;
        let value: CachedResponse = serde_json::from_slice(&bytes).ok()?;
        self.in_memory
            .lock()
            .await
            .insert(key.to_string(), value.clone());
        Some(value)
    }

    pub async fn put(&self, key: &str, value: CachedResponse) -> Result<(), RemoteError> {
        tokio::fs::create_dir_all(&self.cache_dir).await?;
        let bytes = serde_json::to_vec(&value)
            .map_err(|e| RemoteError::Decode(format!("encode cache: {e}")))?;
        let final_path = self.cache_dir.join(format!("{key}.json"));
        let tmp_path = self.cache_dir.join(format!("{key}.json.tmp"));
        tokio::fs::write(&tmp_path, &bytes).await?;
        tokio::fs::rename(&tmp_path, &final_path).await?;
        self.in_memory.lock().await.insert(key.to_string(), value);
        Ok(())
    }

    pub fn is_fresh(value: &CachedResponse, ttl_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now.saturating_sub(value.fetched_at_unix) < ttl_seconds
    }

    pub async fn lock_for(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().await;
        locks
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

#[cfg(test)]
mod tests {
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
}
