//! Atomic read/mutate/write for `[[catalog_sources]]` entries inside
//! `mcp_servers.toml`.
//!
//! Phase 2: this module owns the on-disk surface for catalog source
//! mutations. It uses `toml_edit::DocumentMut` so that other top-level
//! tables (notably `[mcp_servers.*]` written by the Phase 1 `Installer`)
//! are preserved verbatim across edits.

use agent_config::{CatalogSourceConfig, CatalogSourceKind};
use std::path::{Path, PathBuf};

/// Errors produced by [`MarketplaceToml`] operations.
#[allow(dead_code)] // Wired up in T11.S5 (facade methods).
#[derive(Debug, thiserror::Error)]
pub enum MarketplaceTomlError {
    #[error("io: {0}")]
    Io(String),
    #[error("toml parse: {0}")]
    Parse(String),
    #[error("source not found: {0}")]
    NotFound(String),
    #[error("source already exists: {0}")]
    AlreadyExists(String),
}

#[allow(dead_code)] // Wired up in T11.S5 (facade methods).
pub type Result<T> = std::result::Result<T, MarketplaceTomlError>;

/// Owns `<config_dir>/mcp_servers.toml` for catalog source mutations.
#[allow(dead_code)] // Wired up in T11.S5 (facade methods).
pub struct MarketplaceToml {
    path: PathBuf,
}

#[allow(dead_code)] // Wired up in T11.S5 (facade methods).
impl MarketplaceToml {
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        Self {
            path: config_dir.as_ref().join("mcp_servers.toml"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read the current `[[catalog_sources]]` array. Returns an empty
    /// vector when the file does not exist or contains no sources.
    pub fn read_sources(&self) -> Result<Vec<CatalogSourceConfig>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(&self.path)
            .map_err(|e| MarketplaceTomlError::Io(e.to_string()))?;
        agent_config::parse_catalog_sources(&raw)
            .map_err(|e| MarketplaceTomlError::Parse(e.to_string()))
    }

    /// Append a new source. Errors if a source with the same id already
    /// exists.
    pub fn add_source(&self, src: CatalogSourceConfig) -> Result<()> {
        self.mutate(|sources| {
            if sources.iter().any(|s| s.id == src.id) {
                return Err(MarketplaceTomlError::AlreadyExists(src.id.clone()));
            }
            sources.push(src);
            Ok(())
        })
    }

    /// Remove a source by id. Errors if the source is not present.
    pub fn remove_source(&self, id: &str) -> Result<()> {
        self.mutate(|sources| {
            let before = sources.len();
            sources.retain(|s| s.id != id);
            if sources.len() == before {
                return Err(MarketplaceTomlError::NotFound(id.to_string()));
            }
            Ok(())
        })
    }

    /// Toggle the `enabled` field. If the id is not yet present on disk
    /// but matches one of the shipped defaults
    /// (see [`agent_config::default_catalog_sources`]), the default is
    /// seeded into the file with the requested `enabled` value. This lets
    /// the GUI flip a built-in default to enabled without first having to
    /// "add" it. Errors only if the id is unknown to both the file and
    /// the defaults.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        self.mutate(|sources| {
            if let Some(existing) = sources.iter_mut().find(|s| s.id == id) {
                existing.enabled = enabled;
                return Ok(());
            }
            if let Some(mut seeded) = agent_config::default_catalog_sources()
                .into_iter()
                .find(|s| s.id == id)
            {
                seeded.enabled = enabled;
                sources.push(seeded);
                return Ok(());
            }
            Err(MarketplaceTomlError::NotFound(id.to_string()))
        })
    }

    fn mutate<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Vec<CatalogSourceConfig>) -> Result<()>,
    {
        let mut doc = self.read_doc()?;
        let mut sources = sources_from_doc(&doc)?;
        f(&mut sources)?;
        write_sources_to_doc(&mut doc, &sources);
        self.atomic_write(&doc.to_string())
    }

    fn read_doc(&self) -> Result<toml_edit::DocumentMut> {
        if !self.path.exists() {
            return Ok(toml_edit::DocumentMut::new());
        }
        let raw = std::fs::read_to_string(&self.path)
            .map_err(|e| MarketplaceTomlError::Io(e.to_string()))?;
        raw.parse::<toml_edit::DocumentMut>()
            .map_err(|e| MarketplaceTomlError::Parse(e.to_string()))
    }

    fn atomic_write(&self, body: &str) -> Result<()> {
        let parent = self.path.parent().ok_or_else(|| {
            MarketplaceTomlError::Io(format!("toml path has no parent: {}", self.path.display()))
        })?;
        std::fs::create_dir_all(parent).map_err(|e| MarketplaceTomlError::Io(e.to_string()))?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| MarketplaceTomlError::Io(e.to_string()))?;
        use std::io::Write;
        tmp.write_all(body.as_bytes())
            .map_err(|e| MarketplaceTomlError::Io(e.to_string()))?;
        tmp.flush()
            .map_err(|e| MarketplaceTomlError::Io(e.to_string()))?;
        tmp.persist(&self.path)
            .map_err(|e: tempfile::PersistError| MarketplaceTomlError::Io(e.to_string()))?;
        Ok(())
    }
}

#[allow(dead_code)] // Wired up in T11.S5 (facade methods).
fn sources_from_doc(doc: &toml_edit::DocumentMut) -> Result<Vec<CatalogSourceConfig>> {
    // Re-parse the rendered string so we reuse `agent_config`'s validation.
    agent_config::parse_catalog_sources(&doc.to_string())
        .map_err(|e| MarketplaceTomlError::Parse(e.to_string()))
}

#[allow(dead_code)] // Wired up in T11.S5 (facade methods).
fn write_sources_to_doc(doc: &mut toml_edit::DocumentMut, sources: &[CatalogSourceConfig]) {
    use toml_edit::{value, Array, ArrayOfTables, Item, Table};
    // Replace the entire `catalog_sources` array-of-tables in place.
    let mut aot = ArrayOfTables::new();
    for s in sources {
        let mut t = Table::new();
        t["id"] = value(s.id.as_str());
        t["display_name"] = value(s.display_name.as_str());
        t["kind"] = value(match s.kind {
            CatalogSourceKind::McpRegistry => "mcp_registry",
        });
        t["url"] = value(s.url.as_str());
        if let Some(env) = &s.api_key_env {
            t["api_key_env"] = value(env.as_str());
        }
        t["priority"] = value(s.priority as i64);
        t["default_trust"] = value(s.default_trust.as_str());
        t["enabled"] = value(s.enabled);
        if let Some(ttl) = s.cache_ttl_seconds {
            t["cache_ttl_seconds"] = value(ttl as i64);
        }
        aot.push(t);
    }
    if sources.is_empty() {
        doc.remove("catalog_sources");
    } else {
        doc["catalog_sources"] = Item::ArrayOfTables(aot);
    }
    // Suppress unused import warnings if no source uses Array.
    let _ = Array::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_source(id: &str) -> CatalogSourceConfig {
        CatalogSourceConfig {
            id: id.into(),
            display_name: format!("Display {id}"),
            kind: CatalogSourceKind::McpRegistry,
            url: "https://registry.modelcontextprotocol.io".into(),
            api_key_env: None,
            priority: 100,
            default_trust: "community".into(),
            enabled: true,
            cache_ttl_seconds: None,
        }
    }

    #[test]
    fn read_sources_returns_empty_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let mt = MarketplaceToml::new(tmp.path());
        assert!(mt.read_sources().unwrap().is_empty());
    }

    #[test]
    fn add_then_read_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let mt = MarketplaceToml::new(tmp.path());
        mt.add_source(sample_source("mcp-registry")).unwrap();
        let got = mt.read_sources().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "mcp-registry");
        assert_eq!(got[0].priority, 100);
    }

    #[test]
    fn add_duplicate_id_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let mt = MarketplaceToml::new(tmp.path());
        mt.add_source(sample_source("a")).unwrap();
        let err = mt.add_source(sample_source("a")).unwrap_err();
        assert!(matches!(err, MarketplaceTomlError::AlreadyExists(_)));
    }

    #[test]
    fn remove_existing_then_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let mt = MarketplaceToml::new(tmp.path());
        mt.add_source(sample_source("a")).unwrap();
        mt.remove_source("a").unwrap();
        assert!(mt.read_sources().unwrap().is_empty());
        let err = mt.remove_source("a").unwrap_err();
        assert!(matches!(err, MarketplaceTomlError::NotFound(_)));
    }

    #[test]
    fn set_enabled_toggles_field() {
        let tmp = tempfile::tempdir().unwrap();
        let mt = MarketplaceToml::new(tmp.path());
        mt.add_source(sample_source("a")).unwrap();
        mt.set_enabled("a", false).unwrap();
        let got = mt.read_sources().unwrap();
        assert!(!got[0].enabled);
        mt.set_enabled("a", true).unwrap();
        assert!(mt.read_sources().unwrap()[0].enabled);
    }

    #[test]
    fn mutations_preserve_other_top_level_tables() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("mcp_servers.toml");
        std::fs::write(
            &path,
            r#"
[mcp_servers.fs]
type = "stdio"
command = "fs-server"
args = []
"#,
        )
        .unwrap();
        let mt = MarketplaceToml::new(tmp.path());
        mt.add_source(sample_source("mcp-registry")).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        // mcp_servers.fs must survive verbatim.
        assert!(raw.contains("[mcp_servers.fs]"));
        assert!(raw.contains("command = \"fs-server\""));
        // catalog_sources entry must be present.
        assert!(raw.contains("[[catalog_sources]]"));
        assert!(raw.contains("id = \"mcp-registry\""));
    }

    #[test]
    fn write_then_remove_strips_array_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let mt = MarketplaceToml::new(tmp.path());
        mt.add_source(sample_source("a")).unwrap();
        mt.remove_source("a").unwrap();
        let raw = std::fs::read_to_string(mt.path()).unwrap();
        assert!(!raw.contains("catalog_sources"));
    }
}
