use agent_core::events::{EventPayload, MonitorStopReason};
use agent_core::{AgentId, DomainEvent, PrivacyClassification, SessionId, WorkspaceId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use crate::shell::sandbox::ALLOWED_ENV_VARS;

const DEFAULT_MONITOR_TIMEOUT_MS: u64 = 300_000; // 5 min
const MAX_MONITORS: usize = 32;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub monitor_id: String,
    pub description: String,
    pub command: String,
    pub persistent: bool,
    pub timeout_ms: u64,
}

struct MonitorHandle {
    info: MonitorInfo,
    abort_handle: tokio::task::AbortHandle,
}

pub struct MonitorRegistry {
    workspace_root: PathBuf,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    monitors: Arc<Mutex<HashMap<String, MonitorHandle>>>,
    id_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl MonitorRegistry {
    pub fn new(
        workspace_root: PathBuf,
        event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    ) -> Self {
        Self {
            workspace_root,
            event_tx,
            monitors: Arc::new(Mutex::new(HashMap::new())),
            id_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    pub async fn start(
        &self,
        description: String,
        command: String,
        persistent: bool,
        timeout_ms: Option<u64>,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> crate::Result<String> {
        let monitors = self.monitors.lock().await;
        if monitors.len() >= MAX_MONITORS {
            return Err(crate::ToolError::ExecutionFailed(format!(
                "monitor limit reached ({MAX_MONITORS})"
            )));
        }
        drop(monitors);

        let timeout = timeout_ms.unwrap_or(DEFAULT_MONITOR_TIMEOUT_MS);
        let id_num = self
            .id_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let monitor_id = format!("mon_{id_num}");

        let info = MonitorInfo {
            monitor_id: monitor_id.clone(),
            description: description.clone(),
            command: command.clone(),
            persistent,
            timeout_ms: timeout,
        };

        let event_tx = self.event_tx.clone();
        let monitors = self.monitors.clone();
        let workspace_root = self.workspace_root.clone();
        let mid = monitor_id.clone();
        let wid = workspace_id.clone();
        let sid = session_id.clone();

        let join_handle = tokio::spawn(async move {
            run_monitor(RunMonitorArgs {
                monitor_id: mid,
                command,
                persistent,
                timeout_ms: timeout,
                workspace_root,
                event_tx,
                monitors,
                workspace_id: wid,
                session_id: sid,
            })
            .await;
        });

        let handle = MonitorHandle {
            info,
            abort_handle: join_handle.abort_handle(),
        };
        self.monitors
            .lock()
            .await
            .insert(monitor_id.clone(), handle);

        let _ = self.event_tx.send(DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::MonitorStarted {
                monitor_id: monitor_id.clone(),
                description,
                command: "(started)".into(),
                persistent,
                timeout_ms: timeout,
            },
        ));

        Ok(monitor_id)
    }

    pub async fn stop(&self, monitor_id: &str) -> crate::Result<()> {
        let mut monitors = self.monitors.lock().await;
        if let Some(handle) = monitors.remove(monitor_id) {
            handle.abort_handle.abort();
            Ok(())
        } else {
            Err(crate::ToolError::NotFound(format!("monitor {monitor_id}")))
        }
    }

    pub async fn list(&self) -> Vec<MonitorInfo> {
        self.monitors
            .lock()
            .await
            .values()
            .map(|h| h.info.clone())
            .collect()
    }

    pub async fn stop_all(&self) {
        let mut monitors = self.monitors.lock().await;
        for (_, handle) in monitors.drain() {
            handle.abort_handle.abort();
        }
    }
}

struct RunMonitorArgs {
    monitor_id: String,
    command: String,
    persistent: bool,
    timeout_ms: u64,
    workspace_root: PathBuf,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    monitors: Arc<Mutex<HashMap<String, MonitorHandle>>>,
    workspace_id: WorkspaceId,
    session_id: SessionId,
}

async fn run_monitor(args: RunMonitorArgs) {
    let RunMonitorArgs {
        monitor_id,
        command,
        persistent,
        timeout_ms,
        workspace_root,
        event_tx,
        monitors,
        workspace_id,
        session_id,
    } = args;

    let emit = |payload: EventPayload| {
        let _ = event_tx.send(DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        ));
    };

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut cmd = tokio::process::Command::new(&shell);
    cmd.args(["-c", &command])
        .current_dir(&workspace_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());

    cmd.env_clear();
    for var in ALLOWED_ENV_VARS {
        if let Ok(val) = std::env::var(var) {
            cmd.env(var, val);
        }
    }

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            emit(EventPayload::MonitorFailed {
                monitor_id: monitor_id.clone(),
                error: e.to_string(),
            });
            monitors.lock().await.remove(&monitor_id);
            return;
        }
    };

    let stdout = match child.stdout {
        Some(s) => s,
        None => {
            emit(EventPayload::MonitorFailed {
                monitor_id: monitor_id.clone(),
                error: "no stdout handle".into(),
            });
            monitors.lock().await.remove(&monitor_id);
            return;
        }
    };

    let mut reader = BufReader::new(stdout).lines();
    let timeout_duration = if persistent {
        None
    } else {
        Some(Duration::from_millis(timeout_ms))
    };

    let read_loop = async {
        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    emit(EventPayload::MonitorEvent {
                        monitor_id: monitor_id.clone(),
                        line,
                    });
                }
                Ok(None) => break,
                Err(e) => {
                    emit(EventPayload::MonitorFailed {
                        monitor_id: monitor_id.clone(),
                        error: e.to_string(),
                    });
                    monitors.lock().await.remove(&monitor_id);
                    return;
                }
            }
        }
        emit(EventPayload::MonitorStopped {
            monitor_id: monitor_id.clone(),
            reason: MonitorStopReason::ExitCode { code: 0 },
        });
        monitors.lock().await.remove(&monitor_id);
    };

    if let Some(timeout) = timeout_duration {
        if tokio::time::timeout(timeout, read_loop).await.is_err() {
            emit(EventPayload::MonitorStopped {
                monitor_id: monitor_id.clone(),
                reason: MonitorStopReason::Timeout,
            });
            monitors.lock().await.remove(&monitor_id);
        }
    } else {
        read_loop.await;
    }
}

#[cfg(test)]
mod tests {
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

        let has_event = events.iter().any(
            |e| matches!(&e.payload, EventPayload::MonitorEvent { line, .. } if line == "hello"),
        );
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
}
