use super::*;
use agent_core::events::EventPayload;

fn test_registry(workspace: PathBuf) -> MonitorRegistry {
    let (tx, _) = tokio::sync::broadcast::channel(64);
    MonitorRegistry::new(workspace, tx)
}

async fn collect_events_until<F>(
    rx: &mut tokio::sync::broadcast::Receiver<DomainEvent>,
    timeout: Duration,
    mut done: F,
) -> Vec<DomainEvent>
where
    F: FnMut(&[DomainEvent]) -> bool,
{
    let mut events = vec![];
    let deadline = tokio::time::Instant::now() + timeout;

    while !done(&events) {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }

        match tokio::time::timeout(deadline - now, rx.recv()).await {
            Ok(Ok(ev)) => events.push(ev),
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) | Err(_) => break,
        }
    }

    events
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

    let events = collect_events_until(&mut rx, Duration::from_secs(2), |events| {
        events.iter().any(
            |e| matches!(&e.payload, EventPayload::MonitorEvent { line, .. } if line == "hello"),
        )
    })
    .await;

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
async fn stop_individual_monitor_emits_user_stopped_for_start_session() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(64);
    let registry = MonitorRegistry::new(PathBuf::from("/tmp"), tx);
    let wid = WorkspaceId::new();
    let sid = SessionId::new();

    let monitor_id = registry
        .start(
            "user-stop".into(),
            "sleep 60".into(),
            true,
            None,
            wid,
            sid.clone(),
        )
        .await
        .unwrap();

    let _started = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    registry.stop(&monitor_id).await.unwrap();
    let stopped = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(stopped.session_id, sid);
    assert!(matches!(
        stopped.payload,
        EventPayload::MonitorStopped {
            monitor_id: ref stopped_id,
            reason: MonitorStopReason::UserStopped,
        } if stopped_id == &monitor_id
    ));
    assert!(registry.list().await.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn stop_kills_monitor_process_group_children() {
    let temp = tempfile::tempdir().unwrap();
    let pid_path = temp.path().join("sleep.pid");
    let command = format!(
        "sleep 60 & echo $! > '{}'; printf 'ready\\n'; wait",
        pid_path.display()
    );
    let (tx, mut rx) = tokio::sync::broadcast::channel(64);
    let registry = MonitorRegistry::new(PathBuf::from("/tmp"), tx);

    let monitor_id = registry
        .start(
            "process-group".into(),
            command,
            false,
            Some(60_000),
            WorkspaceId::new(),
            SessionId::new(),
        )
        .await
        .unwrap();

    let events = collect_events_until(&mut rx, Duration::from_secs(2), |events| {
        events.iter().any(
            |event| matches!(&event.payload, EventPayload::MonitorEvent { line, .. } if line == "ready"),
        )
    })
    .await;
    assert!(
        events.iter().any(
            |event| matches!(&event.payload, EventPayload::MonitorEvent { line, .. } if line == "ready"),
        ),
        "monitor command did not report readiness"
    );

    let sleep_pid: libc::pid_t = std::fs::read_to_string(&pid_path)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        process_is_running(sleep_pid),
        "sleep process should be running before stop"
    );

    registry.stop(&monitor_id).await.unwrap();
    let stopped = wait_for_process_exit(sleep_pid, Duration::from_secs(3)).await;
    if !stopped {
        unsafe {
            libc::kill(sleep_pid, libc::SIGKILL);
        }
    }
    assert!(stopped, "monitor.stop should terminate child process group");
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

#[cfg(unix)]
fn process_is_running(pid: libc::pid_t) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(unix)]
async fn wait_for_process_exit(pid: libc::pid_t, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    !process_is_running(pid)
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

    let events = collect_events_until(&mut rx, Duration::from_secs(2), |events| {
        events.iter().any(|e| {
            matches!(
                &e.payload,
                EventPayload::MonitorStopped {
                    reason: MonitorStopReason::ExitCode { code: 0 },
                    ..
                }
            )
        })
    })
    .await;

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

    let events = collect_events_until(&mut rx, Duration::from_secs(2), |events| {
        let lines: Vec<_> = events
            .iter()
            .filter_map(|ev| {
                if let EventPayload::MonitorEvent { line, .. } = &ev.payload {
                    Some(line.as_str())
                } else {
                    None
                }
            })
            .collect();
        lines.contains(&"alpha") && lines.contains(&"beta")
    })
    .await;
    let lines: Vec<_> = events
        .iter()
        .filter_map(|ev| {
            if let EventPayload::MonitorEvent { line, .. } = &ev.payload {
                Some(line.clone())
            } else {
                None
            }
        })
        .collect();

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

    let events = collect_events_until(&mut rx, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|e| matches!(&e.payload, EventPayload::MonitorFailed { .. }))
    })
    .await;

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
