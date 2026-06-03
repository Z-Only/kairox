use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};

use super::{
    format_runtime_instance_summary, process_is_running, RuntimeInstanceKind,
    RuntimeInstanceRecord, RuntimeInstanceRegistration, RuntimeInstanceRegistry,
};

fn workspace_path() -> PathBuf {
    std::env::temp_dir().join("kairox-instance-registry-workspace")
}

fn unused_pid() -> u32 {
    let current = std::process::id();
    let candidates = [
        current.saturating_add(10_000)..current.saturating_add(11_000),
        100_000..101_000,
        2_000_000..2_001_000,
    ];

    candidates
        .into_iter()
        .flatten()
        .find(|pid| *pid != current && !process_is_running(*pid))
        .expect("find an unused pid for stale registry record test")
}

#[test]
fn register_lists_and_drop_cleans_current_instance() {
    let temp = tempfile::tempdir().expect("temp data dir");
    let registry = RuntimeInstanceRegistry::new(temp.path());
    let workspace_root = workspace_path();

    let guard = registry
        .register(RuntimeInstanceRegistration {
            kind: RuntimeInstanceKind::Gui,
            database_filename: "kairox-gui.sqlite".to_string(),
            workspace_root: Some(workspace_root.clone()),
        })
        .expect("register instance");

    let records = registry.list().expect("list records");
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.id, guard.id());
    assert_eq!(record.pid, std::process::id());
    assert_eq!(record.kind, RuntimeInstanceKind::Gui);
    assert_eq!(record.database_filename, "kairox-gui.sqlite");
    assert_eq!(record.data_dir, temp.path());
    assert_eq!(
        record.workspace_root.as_deref(),
        Some(workspace_root.as_path())
    );

    drop(guard);

    assert!(registry.list().expect("list after drop").is_empty());
}

#[test]
fn explicit_cleanup_is_idempotent() {
    let temp = tempfile::tempdir().expect("temp data dir");
    let registry = RuntimeInstanceRegistry::new(temp.path());

    let guard = registry
        .register(RuntimeInstanceRegistration {
            kind: RuntimeInstanceKind::Gui,
            database_filename: "kairox-gui.sqlite".to_string(),
            workspace_root: None,
        })
        .expect("register instance");

    guard.cleanup().expect("cleanup instance");
    guard.cleanup().expect("cleanup instance again");

    assert!(registry.list().expect("list after cleanup").is_empty());
}

#[test]
fn stale_records_are_pruned_before_listing_other_instances() {
    let temp = tempfile::tempdir().expect("temp data dir");
    let registry = RuntimeInstanceRegistry::new(temp.path());
    fs::create_dir_all(registry.records_dir()).expect("records dir");
    let stale_record = RuntimeInstanceRecord {
        id: "stale-record".to_string(),
        pid: unused_pid(),
        kind: RuntimeInstanceKind::Tui,
        database_filename: "kairox.sqlite".to_string(),
        data_dir: temp.path().to_path_buf(),
        workspace_root: None,
        started_at: Utc.timestamp_opt(1, 0).single().expect("timestamp"),
        executable: None,
    };
    let stale_record_path = registry.records_dir().join("stale-record.json");
    fs::write(
        &stale_record_path,
        serde_json::to_vec_pretty(&stale_record).expect("serialize stale record"),
    )
    .expect("write stale record");

    let records = registry
        .list_other_instances()
        .expect("other instances prunes stale records");

    assert!(records.is_empty());
    assert!(!stale_record_path.exists());
}

#[test]
fn summary_includes_kind_pid_database_and_workspace() {
    let record = RuntimeInstanceRecord {
        id: "record".to_string(),
        pid: 42,
        kind: RuntimeInstanceKind::Gui,
        database_filename: "kairox-gui.sqlite".to_string(),
        data_dir: PathBuf::from("/tmp/kairox"),
        workspace_root: Some(PathBuf::from("/repo")),
        started_at: Utc.timestamp_opt(1, 0).single().expect("timestamp"),
        executable: Some(PathBuf::from("/bin/kairox")),
    };

    let summary = format_runtime_instance_summary(&[record]);

    assert_eq!(summary, "gui pid=42 db=kairox-gui.sqlite workspace=/repo");
}
