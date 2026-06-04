use agent_core::events::{EventPayload, MonitorStopReason};
use agent_core::{AgentId, DomainEvent, PrivacyClassification, SessionId, WorkspaceId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use crate::shell::sandbox::ALLOWED_ENV_VARS;

const DEFAULT_MONITOR_TIMEOUT_MS: u64 = 300_000; // 5 min
const MAX_MONITORS: usize = 32;

#[async_trait::async_trait]
pub trait MonitorEventSink: Send + Sync {
    async fn emit(&self, event: DomainEvent);
}

struct BroadcastMonitorEventSink {
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
}

#[async_trait::async_trait]
impl MonitorEventSink for BroadcastMonitorEventSink {
    async fn emit(&self, event: DomainEvent) {
        let _ = self.event_tx.send(event);
    }
}

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
    child_pid: Arc<AtomicU32>,
    workspace_id: WorkspaceId,
    session_id: SessionId,
}

pub struct MonitorRegistry {
    workspace_root: PathBuf,
    event_sink: Arc<dyn MonitorEventSink>,
    monitors: Arc<Mutex<HashMap<String, MonitorHandle>>>,
    id_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl MonitorRegistry {
    pub fn new(
        workspace_root: PathBuf,
        event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    ) -> Self {
        Self::new_with_event_sink(
            workspace_root,
            Arc::new(BroadcastMonitorEventSink { event_tx }),
        )
    }

    pub fn new_with_event_sink(
        workspace_root: PathBuf,
        event_sink: Arc<dyn MonitorEventSink>,
    ) -> Self {
        Self {
            workspace_root,
            event_sink,
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
        self.start_in_workspace(
            self.workspace_root.clone(),
            description,
            command,
            persistent,
            timeout_ms,
            workspace_id,
            session_id,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn start_in_workspace(
        &self,
        workspace_root: PathBuf,
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

        let event_sink = self.event_sink.clone();
        let monitors = self.monitors.clone();
        let child_pid = Arc::new(AtomicU32::new(0));
        let mid = monitor_id.clone();
        let wid = workspace_id.clone();
        let sid = session_id.clone();
        let child_pid_for_task = child_pid.clone();

        let join_handle = tokio::spawn(async move {
            run_monitor(RunMonitorArgs {
                monitor_id: mid,
                command,
                persistent,
                timeout_ms: timeout,
                workspace_root,
                event_sink,
                monitors,
                workspace_id: wid,
                session_id: sid,
                child_pid: child_pid_for_task,
            })
            .await;
        });

        let handle = MonitorHandle {
            info,
            abort_handle: join_handle.abort_handle(),
            child_pid,
            workspace_id: workspace_id.clone(),
            session_id: session_id.clone(),
        };
        self.monitors
            .lock()
            .await
            .insert(monitor_id.clone(), handle);

        self.event_sink
            .emit(DomainEvent::new(
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
            ))
            .await;

        Ok(monitor_id)
    }

    pub async fn stop(&self, monitor_id: &str) -> crate::Result<()> {
        let mut monitors = self.monitors.lock().await;
        if let Some(handle) = monitors.remove(monitor_id) {
            terminate_process_group(handle.child_pid.load(Ordering::Relaxed));
            handle.abort_handle.abort();
            self.event_sink
                .emit(DomainEvent::new(
                    handle.workspace_id,
                    handle.session_id,
                    AgentId::system(),
                    PrivacyClassification::FullTrace,
                    EventPayload::MonitorStopped {
                        monitor_id: monitor_id.to_string(),
                        reason: MonitorStopReason::UserStopped,
                    },
                ))
                .await;
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
            terminate_process_group(handle.child_pid.load(Ordering::Relaxed));
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
    event_sink: Arc<dyn MonitorEventSink>,
    monitors: Arc<Mutex<HashMap<String, MonitorHandle>>>,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    child_pid: Arc<AtomicU32>,
}

async fn emit_monitor_event(
    event_sink: Arc<dyn MonitorEventSink>,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    payload: EventPayload,
) {
    event_sink
        .emit(DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        ))
        .await;
}

async fn run_monitor(args: RunMonitorArgs) {
    let RunMonitorArgs {
        monitor_id,
        command,
        persistent,
        timeout_ms,
        workspace_root,
        event_sink,
        monitors,
        workspace_id,
        session_id,
        child_pid,
    } = args;

    let emit = |payload: EventPayload| {
        emit_monitor_event(
            event_sink.clone(),
            workspace_id.clone(),
            session_id.clone(),
            payload,
        )
    };

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut cmd = tokio::process::Command::new(&shell);
    cmd.args(["-c", &command])
        .current_dir(&workspace_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    cmd.kill_on_drop(true);
    configure_monitor_command(&mut cmd);

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
            })
            .await;
            monitors.lock().await.remove(&monitor_id);
            return;
        }
    };
    if let Some(pid) = child.id() {
        child_pid.store(pid, Ordering::Relaxed);
    }

    let stdout = match child.stdout {
        Some(s) => s,
        None => {
            emit(EventPayload::MonitorFailed {
                monitor_id: monitor_id.clone(),
                error: "no stdout handle".into(),
            })
            .await;
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
                    })
                    .await;
                }
                Ok(None) => break,
                Err(e) => {
                    emit(EventPayload::MonitorFailed {
                        monitor_id: monitor_id.clone(),
                        error: e.to_string(),
                    })
                    .await;
                    monitors.lock().await.remove(&monitor_id);
                    return;
                }
            }
        }
        emit(EventPayload::MonitorStopped {
            monitor_id: monitor_id.clone(),
            reason: MonitorStopReason::ExitCode { code: 0 },
        })
        .await;
        monitors.lock().await.remove(&monitor_id);
    };

    if let Some(timeout) = timeout_duration {
        if tokio::time::timeout(timeout, read_loop).await.is_err() {
            emit(EventPayload::MonitorStopped {
                monitor_id: monitor_id.clone(),
                reason: MonitorStopReason::Timeout,
            })
            .await;
            monitors.lock().await.remove(&monitor_id);
        }
    } else {
        read_loop.await;
    }
}

#[cfg(unix)]
fn configure_monitor_command(cmd: &mut tokio::process::Command) {
    cmd.process_group(0);
}

#[cfg(not(unix))]
fn configure_monitor_command(_cmd: &mut tokio::process::Command) {}

#[cfg(unix)]
fn terminate_process_group(pid: u32) {
    if pid == 0 {
        return;
    }
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGTERM);
    }
}

#[cfg(not(unix))]
fn terminate_process_group(_pid: u32) {}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod registry_tests;
