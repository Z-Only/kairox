//! Installer for marketplace catalog entries.
//!
//! Validates env vars, expands `${VAR}` placeholders in `args`, probes host
//! runtimes, atomically writes a `mcp_servers.toml`, and (optionally) marks
//! the entry as trusted.

use crate::catalog::{
    EnvVarSpec, InstallRequest, InstallSpec, RuntimeKind, RuntimeRequirement, ServerEntry,
};
use async_trait::async_trait;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Outcome of an [`Installer::install`] call. Surfaced to the GUI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallOutcomeView {
    Installed { server_id: String, started: bool },
    RuntimeMissing { missing: Vec<RuntimeRequirement> },
    AlreadyInstalled { server_id: String },
    InvalidEnv { missing_keys: Vec<String> },
}

/// Detects whether a host runtime is available.
#[async_trait]
pub trait RuntimeProbe: Send + Sync {
    async fn is_available(&self, kind: RuntimeKind) -> bool;
}

/// Default probe using the `which` crate to look up binaries on PATH.
pub struct OsRuntimeProbe;

#[async_trait]
impl RuntimeProbe for OsRuntimeProbe {
    async fn is_available(&self, kind: RuntimeKind) -> bool {
        let bin = match kind {
            RuntimeKind::Node => "node",
            RuntimeKind::Python => "python3",
            RuntimeKind::Uvx => "uvx",
            RuntimeKind::Docker => "docker",
            RuntimeKind::Bun => "bun",
            RuntimeKind::Deno => "deno",
            RuntimeKind::Other => return true,
        };
        which::which(bin).is_ok()
    }
}

/// Errors emitted by the installer when filesystem or TOML operations fail.
#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error("io: {0}")]
    Io(String),
    #[error("toml: {0}")]
    Toml(String),
    #[error("invalid: {0}")]
    Invalid(String),
}

/// Persists marketplace installations into a managed `mcp_servers.toml`.
pub struct Installer {
    toml_path: PathBuf,
    probe: Arc<dyn RuntimeProbe>,
    write_lock: Mutex<()>,
}

impl Installer {
    pub fn new(toml_path: PathBuf, probe: Arc<dyn RuntimeProbe>) -> Self {
        Self {
            toml_path,
            probe,
            write_lock: Mutex::new(()),
        }
    }

    /// Returns the list of runtime requirements that are not satisfied on the
    /// host. An empty vector means everything is available.
    pub async fn check_requirements(&self, entry: &ServerEntry) -> Vec<RuntimeRequirement> {
        let mut missing = Vec::new();
        for req in &entry.requirements {
            if !self.probe.is_available(req.kind).await {
                missing.push(req.clone());
            }
        }
        missing
    }

    pub async fn install(
        &self,
        entry: &ServerEntry,
        req: &InstallRequest,
    ) -> Result<InstallOutcomeView, InstallerError> {
        let _guard = self.write_lock.lock().await;
        let server_id = req
            .server_id_override
            .clone()
            .unwrap_or_else(|| entry.id.clone());

        // 1. Validate env first; cheapest failure.
        let resolved = match resolve_env(&entry.default_env, &req.env_overrides) {
            Ok(v) => v,
            Err(missing_keys) => {
                return Ok(InstallOutcomeView::InvalidEnv { missing_keys });
            }
        };

        // 2. Probe runtimes.
        let missing = self.check_requirements(entry).await;
        if !missing.is_empty() {
            return Ok(InstallOutcomeView::RuntimeMissing { missing });
        }

        // 3. Read current toml document (if any).
        let mut doc = self.read_doc()?;
        if doc_contains_server(&doc, &server_id) {
            return Ok(InstallOutcomeView::AlreadyInstalled { server_id });
        }

        // 4. Build the new section and insert it.
        let section = build_section(entry, &resolved);
        ensure_table(&mut doc, "mcp_servers");
        doc["mcp_servers"][&server_id] = toml_edit::Item::Table(section);

        // 5. Trust grant.
        if req.trust_grant {
            add_trusted(&mut doc, &server_id);
        }

        // 6. Atomic write.
        self.atomic_write(&doc.to_string())?;

        Ok(InstallOutcomeView::Installed {
            server_id,
            started: req.auto_start,
        })
    }

    pub async fn uninstall(&self, server_id: &str) -> Result<(), InstallerError> {
        let _guard = self.write_lock.lock().await;
        let mut doc = self.read_doc()?;
        if let Some(table) = doc.get_mut("mcp_servers").and_then(|i| i.as_table_mut()) {
            table.remove(server_id);
        }
        if let Some(arr) = doc
            .get_mut("trusted_servers")
            .and_then(|i| i.as_array_mut())
        {
            arr.retain(|v| v.as_str() != Some(server_id));
        }
        self.atomic_write(&doc.to_string())?;
        Ok(())
    }

    pub fn list_installed_ids(&self) -> Result<Vec<String>, InstallerError> {
        let doc = self.read_doc()?;
        let mut ids = Vec::new();
        if let Some(t) = doc.get("mcp_servers").and_then(|i| i.as_table()) {
            for (k, _) in t.iter() {
                ids.push(k.to_string());
            }
        }
        Ok(ids)
    }

    fn read_doc(&self) -> Result<toml_edit::DocumentMut, InstallerError> {
        if !self.toml_path.exists() {
            return Ok(toml_edit::DocumentMut::new());
        }
        let content = std::fs::read_to_string(&self.toml_path)
            .map_err(|e| InstallerError::Io(e.to_string()))?;
        content
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| InstallerError::Toml(e.to_string()))
    }

    fn atomic_write(&self, body: &str) -> Result<(), InstallerError> {
        if let Some(parent) = self.toml_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e: std::io::Error| InstallerError::Io(e.to_string()))?;
        }
        let parent = self
            .toml_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut tmp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e: std::io::Error| InstallerError::Io(e.to_string()))?;
        use std::io::Write;
        let header = "# Managed by Kairox marketplace, schema=1\n# Edit at your own risk; entries here may be rewritten by the marketplace UI.\n\n";
        tmp.write_all(header.as_bytes())
            .map_err(|e: std::io::Error| InstallerError::Io(e.to_string()))?;
        tmp.write_all(body.as_bytes())
            .map_err(|e: std::io::Error| InstallerError::Io(e.to_string()))?;
        tmp.persist(&self.toml_path)
            .map_err(|e: tempfile::PersistError| InstallerError::Io(e.to_string()))?;
        Ok(())
    }
}

/// Resolves the final env map by overlaying `overrides` on top of `default_env`
/// defaults. Returns the keys that are still missing if any required entry has
/// neither an override nor a default.
fn resolve_env(
    default_env: &[EnvVarSpec],
    overrides: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, Vec<String>> {
    let mut out = BTreeMap::new();
    let mut missing = Vec::new();
    for spec in default_env {
        let value = overrides
            .get(&spec.key)
            .cloned()
            .or_else(|| spec.default.clone());
        match value {
            Some(v) => {
                out.insert(spec.key.clone(), v);
            }
            None if spec.required => missing.push(spec.key.clone()),
            None => {}
        }
    }
    if missing.is_empty() {
        Ok(out)
    } else {
        Err(missing)
    }
}

/// Expands `${VAR}` placeholders in `s` using `env`. Unknown vars expand to
/// the empty string. Unterminated `${` sequences are written as-is.
fn expand(s: &str, env: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
            if let Some(end) = s[i + 2..].find('}') {
                let key = &s[i + 2..i + 2 + end];
                out.push_str(env.get(key).map(String::as_str).unwrap_or(""));
                i = i + 2 + end + 1;
                continue;
            }
        }
        // Walk one UTF-8 char at a time so we never split a codepoint.
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn build_section(entry: &ServerEntry, env: &BTreeMap<String, String>) -> toml_edit::Table {
    use toml_edit::{value, Array, Table};
    let mut t = Table::new();
    match &entry.install {
        InstallSpec::Stdio {
            command,
            args,
            env: extra_env,
            cwd,
        } => {
            t["type"] = value("stdio");
            t["command"] = value(expand(command, env));
            let mut a = Array::new();
            for arg in args {
                a.push(expand(arg, env));
            }
            t["args"] = value(a);
            let mut env_table = Table::new();
            for (k, v) in env.iter().chain(extra_env.iter()) {
                env_table[k] = value(expand(v, env));
            }
            if !env_table.is_empty() {
                t["env"] = toml_edit::Item::Table(env_table);
            }
            if let Some(c) = cwd {
                t["cwd"] = value(expand(c, env));
            }
        }
        InstallSpec::Sse { url, headers } => {
            t["type"] = value("sse");
            t["url"] = value(expand(url, env));
            if !headers.is_empty() {
                let mut h = Table::new();
                for (k, v) in headers {
                    h[k] = value(expand(v, env));
                }
                t["headers"] = toml_edit::Item::Table(h);
            }
        }
    }
    // Marketplace bookkeeping for later round-trips.
    t["__catalog_id"] = value(entry.id.as_str());
    t["__source"] = value(entry.source.as_str());
    t
}

fn ensure_table(doc: &mut toml_edit::DocumentMut, key: &str) {
    if doc.get(key).is_none() {
        doc[key] = toml_edit::Item::Table(toml_edit::Table::new());
    }
}

fn doc_contains_server(doc: &toml_edit::DocumentMut, id: &str) -> bool {
    doc.get("mcp_servers")
        .and_then(|i| i.as_table())
        .map(|t| t.contains_key(id))
        .unwrap_or(false)
}

fn add_trusted(doc: &mut toml_edit::DocumentMut, id: &str) {
    use toml_edit::{value, Array};
    let mut existing: BTreeSet<String> = doc
        .get("trusted_servers")
        .and_then(|i| i.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    existing.insert(id.to_string());
    let mut arr = Array::new();
    for s in existing {
        arr.push(s);
    }
    doc["trusted_servers"] = value(arr);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::TrustLevel;

    struct StaticProbe {
        available: Vec<RuntimeKind>,
    }

    #[async_trait::async_trait]
    impl RuntimeProbe for StaticProbe {
        async fn is_available(&self, kind: RuntimeKind) -> bool {
            self.available.contains(&kind)
        }
    }

    fn sample_entry() -> ServerEntry {
        ServerEntry {
            id: "test-server".into(),
            source: "builtin".into(),
            display_name: "Test Server".into(),
            summary: "A test server".into(),
            description: "A server for testing installer behaviour.".into(),
            categories: vec!["test".into()],
            tags: vec![],
            author: None,
            homepage: None,
            version: None,
            install: InstallSpec::Stdio {
                command: "echo".into(),
                args: vec!["hello".into()],
                env: BTreeMap::new(),
                cwd: None,
            },
            requirements: vec![],
            trust: TrustLevel::Community,
            default_env: vec![],
            icon: None,
        }
    }

    fn install_request(entry: &ServerEntry) -> InstallRequest {
        InstallRequest {
            catalog_id: entry.id.clone(),
            source: entry.source.clone(),
            server_id_override: None,
            env_overrides: BTreeMap::new(),
            trust_grant: false,
            auto_start: false,
        }
    }

    #[test]
    fn installer_new_is_not_installed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let toml_path = dir.path().join("mcp_servers.toml");
        let probe = Arc::new(StaticProbe { available: vec![] });
        let installer = Installer::new(toml_path, probe);

        let ids = installer
            .list_installed_ids()
            .expect("list_installed_ids should succeed");
        assert!(
            ids.is_empty(),
            "new installer should report zero installed servers"
        );
    }

    #[tokio::test]
    async fn installer_can_start_install() {
        let dir = tempfile::tempdir().expect("tempdir");
        let toml_path = dir.path().join("mcp_servers.toml");
        let probe = Arc::new(StaticProbe { available: vec![] });
        let installer = Installer::new(toml_path.clone(), probe);

        let entry = sample_entry();
        let req = install_request(&entry);

        // Before install, no servers installed.
        let before = installer.list_installed_ids().unwrap();
        assert!(
            before.is_empty(),
            "should have no installed servers before install"
        );

        // Install the entry (no runtime requirements, so it should succeed).
        let outcome = installer.install(&entry, &req).await.unwrap();
        assert!(
            matches!(outcome, InstallOutcomeView::Installed { ref started, .. } if !started),
            "expected Installed with started=false, got {:?}",
            outcome
        );

        // After install, the server should appear in the list.
        let after = installer.list_installed_ids().unwrap();
        assert_eq!(after, vec!["test-server"]);

        // The TOML file should now exist.
        assert!(toml_path.exists(), "TOML file should be created on install");
    }
}
