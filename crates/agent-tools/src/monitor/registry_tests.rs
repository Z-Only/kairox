use super::*;
use agent_core::events::EventPayload;

fn test_registry(workspace: PathBuf) -> MonitorRegistry {
    let (tx, _) = tokio::sync::broadcast::channel(64);
    MonitorRegistry::new(workspace, tx)
}

#[tokio::test]
async fn start_and_list_monitor() {
    let registry = test_registry(PathBuf::from("/tmp"));
    let id = registry
        .start(
            "test".into(),
            "echo hello".into(),
            false,
            Some(5_000),
            WorkspaceId::new(),
            SessionId::new(),
        )
        .await
        .unwrap();
    assert!(id.starts_with("mon_"));

    let list = registry.list().await;
    assert!(list.len() <= 1); // may have already finished
}

#[tokio::test]
async fn stop_unknown_returns_error() {
    let registry = test_registry(PathBuf::from("/tmp"));
    let result = registry.stop("mon_999").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn stop_all_clears_monitors() {
    let registry = test_registry(PathBuf::from("/tmp"));
    registry.stop_all().await;
    assert!(registry.list().await.is_empty());
}

#[tokio::test]
async fn monitor_emits_events_for_echo() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(64);
    let registry = MonitorRegistry::new(PathBuf::from("/tmp"), tx);
    let wid = WorkspaceId::new();
    let sid = SessionId::new();

    let _id = registry
        .start(
            "echo test".into(),
            "echo hello".into(),
            false,
            Some(5_000),
            wid,
            sid,
        )
        .await
        .unwrap();

    // Collect events for up to 2 seconds
    let mut events = vec![];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(ev)) => events.push(ev),
            _ => break,
        }
    }

    let has_started = events
        .iter()
        .any(|e| matches!(&e.payload, EventPayload::MonitorStarted { .. }));
    assert!(has_started, "should emit MonitorStarted");

    let has_event = events
        .iter()
        .any(|e| matches!(&e.payload, EventPayload::MonitorEvent { line, .. } if line == "hello"));
    assert!(has_event, "should emit MonitorEvent with 'hello'");
}

#[tokio::test]
async fn stop_individual_monitor_keeps_others() {
    let registry = test_registry(PathBuf::from("/tmp"));
    let wid = WorkspaceId::new();
    let sid = SessionId::new();

    let id1 = registry
        .start(
            "mon-a".into(),
            "sleep 60".into(),
            false,
            Some(60_000),
            wid.clone(),
            sid.clone(),
        )
        .await
        .unwrap();
    let _id2 = registry
        .start(
            "mon-b".into(),
            "sleep 60".into(),
            false,
            Some(60_000),
            wid,
            sid,
        )
        .await
        .unwrap();
    assert_eq!(registry.list().await.len(), 2);

    registry.stop(&id1).await.unwrap();
    assert_eq!(registry.list().await.len(), 1);

    registry.stop_all().await;
}

#[tokio::test]
async fn monitor_ids_are_unique() {
    let registry = test_registry(PathBuf::from("/tmp"));
    let wid = WorkspaceId::new();
    let sid = SessionId::new();

    let id1 = registry
        .start(
            "a".into(),
            "sleep 60".into(),
            false,
            Some(60_000),
            wid.clone(),
            sid.clone(),
        )
        .await
        .unwrap();
    let id2 = registry
        .start("b".into(), "sleep 60".into(), false, Some(60_000), wid, sid)
        .await
        .unwrap();
    assert_ne!(id1, id2);

    registry.stop_all().await;
}

#[tokio::test]
async fn monitor_auto_removes_after_exit() {
    let registry = test_registry(PathBuf::from("/tmp"));
    let wid = WorkspaceId::new();
    let sid = SessionId::new();

    registry
        .start(
            "auto-exit".into(),
            "echo done".into(),
            false,
            Some(5_000),
            wid,
            sid,
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        registry.list().await.is_empty(),
        "monitor should auto-remove after process exits"
    );
}

#[tokio::test]
async fn monitor_emits_stopped_event_on_normal_exit() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(64);
    let registry = MonitorRegistry::new(PathBuf::from("/tmp"), tx);

    registry
        .start(
            "exit-test".into(),
            "echo bye".into(),
            false,
            Some(5_000),
            WorkspaceId::new(),
            SessionId::new(),
        )
        .await
        .unwrap();

    let mut events = vec![];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(ev)) => events.push(ev),
            _ => break,
        }
    }

    let has_stopped = events.iter().any(|e| {
        matches!(
            &e.payload,
            EventPayload::MonitorStopped {
                reason: MonitorStopReason::ExitCode { code: 0 },
                ..
            }
        )
    });
    assert!(has_stopped, "should emit MonitorStopped with exit code 0");
}

#[tokio::test]
async fn monitor_emits_multiple_lines() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(64);
    let registry = MonitorRegistry::new(PathBuf::from("/tmp"), tx);

    registry
        .start(
            "multi-line".into(),
            "printf 'alpha\\nbeta\\n'".into(),
            false,
            Some(5_000),
            WorkspaceId::new(),
            SessionId::new(),
        )
        .await
        .unwrap();

    let mut lines = vec![];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(ev)) => {
                if let EventPayload::MonitorEvent { line, .. } = &ev.payload {
                    lines.push(line.clone());
                }
            }
            _ => break,
        }
    }

    assert!(lines.contains(&"alpha".to_string()), "should emit 'alpha'");
    assert!(lines.contains(&"beta".to_string()), "should emit 'beta'");
}

#[tokio::test]
async fn spawn_failure_emits_failed_event() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(64);
    let registry = MonitorRegistry::new(PathBuf::from("/nonexistent/dir"), tx);

    registry
        .start(
            "bad-dir".into(),
            "echo nope".into(),
            false,
            Some(5_000),
            WorkspaceId::new(),
            SessionId::new(),
        )
        .await
        .unwrap();

    let mut events = vec![];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(ev)) => events.push(ev),
            _ => break,
        }
    }

    let has_failed = events
        .iter()
        .any(|e| matches!(&e.payload, EventPayload::MonitorFailed { .. }));
    assert!(
        has_failed,
        "should emit MonitorFailed for bad working directory"
    );
}

#[tokio::test]
async fn max_monitors_enforced() {
    let registry = test_registry(PathBuf::from("/tmp"));
    let wid = WorkspaceId::new();
    let sid = SessionId::new();

    // Start MAX_MONITORS monitors using sleep so they stay alive
    for i in 0..MAX_MONITORS {
        registry
            .start(
                format!("test {i}"),
                "sleep 60".into(),
                false,
                Some(60_000),
                wid.clone(),
                sid.clone(),
            )
            .await
            .unwrap();
    }

    let result = registry
        .start(
            "overflow".into(),
            "echo x".into(),
            false,
            Some(1_000),
            wid,
            sid,
        )
        .await;
    assert!(result.is_err());

    registry.stop_all().await;
}
