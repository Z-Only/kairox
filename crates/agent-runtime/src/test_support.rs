//! Helpers for integration tests in `crates/agent-runtime/tests/`.
//!
//! These helpers are exposed via `pub mod test_support` under
//! `#[cfg(any(test, feature = "test-support"))]` so integration tests in the
//! `tests/` directory can construct a `LocalRuntime` wired up with the
//! marketplace catalog and installer.

use crate::LocalRuntime;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use tempfile::TempDir;

/// Build a `LocalRuntime` backed by an in-memory SQLite event store, a
/// `FakeModelClient`, and a fresh temporary `config_dir` wired through
/// `with_marketplace`.
///
/// Returns the runtime plus the owning `TempDir` so the caller can keep the
/// temp directory alive for the lifetime of the test.
pub async fn build_marketplace_runtime(
) -> (LocalRuntime<SqliteEventStore, FakeModelClient>, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    let rt = LocalRuntime::new(store, model)
        .with_marketplace(tmp.path().to_path_buf())
        .expect("with_marketplace");
    (rt, tmp)
}
