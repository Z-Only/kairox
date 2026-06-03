use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Result, RuntimeError};

const INSTANCE_DIR: &str = "runtime/instances";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstanceKind {
    Gui,
    Tui,
}

impl fmt::Display for RuntimeInstanceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeInstanceKind::Gui => f.write_str("gui"),
            RuntimeInstanceKind::Tui => f.write_str("tui"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeInstanceRegistration {
    pub kind: RuntimeInstanceKind,
    pub database_filename: String,
    pub workspace_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInstanceRecord {
    pub id: String,
    pub pid: u32,
    pub kind: RuntimeInstanceKind,
    pub database_filename: String,
    pub data_dir: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<PathBuf>,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RuntimeInstanceRegistry {
    data_dir: PathBuf,
    records_dir: PathBuf,
}

#[derive(Debug)]
pub struct RuntimeInstanceGuard {
    id: String,
    path: PathBuf,
    pid: u32,
}

impl RuntimeInstanceRegistry {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        let data_dir = data_dir.as_ref().to_path_buf();
        let records_dir = data_dir.join(INSTANCE_DIR);
        Self {
            data_dir,
            records_dir,
        }
    }

    pub fn records_dir(&self) -> &Path {
        &self.records_dir
    }

    pub fn register(
        &self,
        registration: RuntimeInstanceRegistration,
    ) -> Result<RuntimeInstanceGuard> {
        fs::create_dir_all(&self.records_dir).map_err(|error| {
            RuntimeError::Other(format!(
                "create runtime instance registry {}: {error}",
                self.records_dir.display()
            ))
        })?;

        let pid = std::process::id();
        let id = format!("{}-{pid}-{}", registration.kind, Uuid::new_v4());
        let path = self.records_dir.join(format!("{id}.json"));
        let record = RuntimeInstanceRecord {
            id: id.clone(),
            pid,
            kind: registration.kind,
            database_filename: registration.database_filename,
            data_dir: self.data_dir.clone(),
            workspace_root: registration.workspace_root,
            started_at: Utc::now(),
            executable: std::env::current_exe().ok(),
        };
        self.write_record_atomic(&path, &record)?;

        Ok(RuntimeInstanceGuard { id, path, pid })
    }

    pub fn list(&self) -> Result<Vec<RuntimeInstanceRecord>> {
        let entries = match fs::read_dir(&self.records_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(RuntimeError::Other(format!(
                    "read runtime instance registry {}: {error}",
                    self.records_dir.display()
                )))
            }
        };

        let mut records = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            match serde_json::from_slice::<RuntimeInstanceRecord>(&bytes) {
                Ok(record) => records.push(record),
                Err(error) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %error,
                        "ignored invalid runtime instance record"
                    );
                }
            }
        }
        records.sort_by(|a, b| {
            a.started_at
                .cmp(&b.started_at)
                .then_with(|| a.pid.cmp(&b.pid))
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(records)
    }

    pub fn list_other_instances(&self) -> Result<Vec<RuntimeInstanceRecord>> {
        self.prune_stale()?;
        let current_pid = std::process::id();
        Ok(self
            .list()?
            .into_iter()
            .filter(|record| record.pid != current_pid)
            .collect())
    }

    pub fn prune_stale(&self) -> Result<usize> {
        let entries = match fs::read_dir(&self.records_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(error) => {
                return Err(RuntimeError::Other(format!(
                    "read runtime instance registry {}: {error}",
                    self.records_dir.display()
                )))
            }
        };

        let mut removed = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let should_remove = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<RuntimeInstanceRecord>(&bytes).ok())
                .is_none_or(|record| !process_is_running(record.pid));
            if should_remove && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn write_record_atomic(&self, path: &Path, record: &RuntimeInstanceRecord) -> Result<()> {
        let mut temp_file =
            tempfile::NamedTempFile::new_in(&self.records_dir).map_err(|error| {
                RuntimeError::Other(format!(
                    "create runtime instance record temp file in {}: {error}",
                    self.records_dir.display()
                ))
            })?;
        serde_json::to_writer_pretty(&mut temp_file, record)
            .map_err(|error| RuntimeError::Other(format!("serialize runtime instance: {error}")))?;
        temp_file
            .write_all(b"\n")
            .map_err(|error| RuntimeError::Other(format!("write runtime instance: {error}")))?;
        temp_file
            .flush()
            .map_err(|error| RuntimeError::Other(format!("flush runtime instance: {error}")))?;
        temp_file
            .persist(path)
            .map_err(|error| RuntimeError::Other(format!("persist runtime instance: {error}")))?;
        Ok(())
    }
}

impl RuntimeInstanceGuard {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Remove this process' registry record.
    pub fn cleanup(&self) -> io::Result<()> {
        let should_remove = fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<RuntimeInstanceRecord>(&bytes).ok())
            .is_some_and(|record| record.id == self.id && record.pid == self.pid);
        if should_remove {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

impl Drop for RuntimeInstanceGuard {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub fn format_runtime_instance_summary(records: &[RuntimeInstanceRecord]) -> String {
    records
        .iter()
        .map(|record| {
            let workspace = record
                .workspace_root
                .as_ref()
                .map(|path| format!(" workspace={}", path.display()))
                .unwrap_or_default();
            format!(
                "{} pid={} db={}{}",
                record.kind, record.pid, record.database_filename, workspace
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn process_is_running(pid: u32) -> bool {
    pid == std::process::id() || platform_process_is_running(pid)
}

#[cfg(unix)]
fn platform_process_is_running(pid: u32) -> bool {
    let output = Command::new("kill").args(["-0", &pid.to_string()]).output();
    match output {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
            stderr.contains("operation not permitted") || stderr.contains("not permitted")
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
fn platform_process_is_running(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    let output = Command::new("tasklist")
        .args(["/FI", &filter, "/FO", "CSV", "/NH"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(&pid.to_string()) && !stdout.to_ascii_lowercase().contains("no tasks")
        }
        _ => false,
    }
}

#[cfg(not(any(unix, windows)))]
fn platform_process_is_running(_pid: u32) -> bool {
    false
}

#[cfg(test)]
#[path = "instance_registry_tests.rs"]
mod tests;
