//! Read/write user-level plugin marketplace sources in `config.toml`.

use agent_core::facade::PluginMarketplaceSourceView;
use std::path::{Path, PathBuf};
use toml_edit::{value, DocumentMut, Item};

pub struct PluginSourcesToml {
    path: PathBuf,
}

impl PluginSourcesToml {
    pub fn new(config_dir: &Path) -> Self {
        std::fs::create_dir_all(config_dir).ok();
        Self {
            path: config_dir.join("config.toml"),
        }
    }

    pub fn read(&self) -> Vec<PluginMarketplaceSourceView> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(_) => return Vec::new(),
        };
        let doc: DocumentMut = match text.parse() {
            Ok(doc) => doc,
            Err(_) => return Vec::new(),
        };
        let Some(Item::ArrayOfTables(arr)) = doc.get("plugin_marketplaces") else {
            return Vec::new();
        };
        let mut sources = Vec::new();
        for item in arr.iter() {
            let id = item
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if id.is_empty() {
                continue;
            }
            sources.push(PluginMarketplaceSourceView {
                id: id.to_string(),
                display_name: item
                    .get("display_name")
                    .and_then(|value| value.as_str())
                    .unwrap_or(id)
                    .to_string(),
                source: item
                    .get("source")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                enabled: item
                    .get("enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                builtin: false,
            });
        }
        sources
    }

    pub fn write(&self, sources: &[PluginMarketplaceSourceView]) -> std::io::Result<()> {
        let mut doc = self.read_doc()?;
        doc.remove("plugin_marketplaces");
        for source in sources {
            let mut table = toml_edit::Table::new();
            table.insert("id", value(&source.id));
            table.insert("display_name", value(&source.display_name));
            table.insert("source", value(&source.source));
            table.insert("enabled", value(source.enabled));
            if !doc.contains_key("plugin_marketplaces") {
                doc["plugin_marketplaces"] = Item::ArrayOfTables(Default::default());
            }
            doc["plugin_marketplaces"]
                .as_array_of_tables_mut()
                .expect("array of tables")
                .push(table);
        }
        std::fs::write(&self.path, doc.to_string())
    }

    fn read_doc(&self) -> std::io::Result<DocumentMut> {
        if !self.path.exists() {
            return Ok(DocumentMut::new());
        }
        let text = std::fs::read_to_string(&self.path)?;
        text.parse::<DocumentMut>()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }

    pub fn merged_sources(&self) -> Vec<PluginMarketplaceSourceView> {
        merge_with_defaults(self.read())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> std::io::Result<bool> {
        let mut sources = self.read();
        if let Some(source) = sources.iter_mut().find(|source| source.id == id) {
            source.enabled = enabled;
            self.write(&sources)?;
            return Ok(true);
        }
        if let Some(mut default_source) = default_plugin_marketplace_sources()
            .into_iter()
            .find(|source| source.id == id)
        {
            default_source.enabled = enabled;
            default_source.builtin = false;
            sources.push(default_source);
            self.write(&sources)?;
            return Ok(true);
        }
        Ok(false)
    }
}

pub fn default_plugin_marketplace_sources() -> Vec<PluginMarketplaceSourceView> {
    vec![
        PluginMarketplaceSourceView {
            id: "claude-plugins-official".into(),
            display_name: "Claude Plugins Official".into(),
            source: "anthropics/claude-plugins-official".into(),
            enabled: true,
            builtin: true,
        },
        PluginMarketplaceSourceView {
            id: "anthropics-claude-code".into(),
            display_name: "Anthropic Claude Code".into(),
            source: "anthropics/claude-code".into(),
            enabled: true,
            builtin: true,
        },
    ]
}

fn merge_with_defaults(
    user_sources: Vec<PluginMarketplaceSourceView>,
) -> Vec<PluginMarketplaceSourceView> {
    let mut merged = user_sources;
    let existing = merged
        .iter()
        .map(|source| source.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for source in default_plugin_marketplace_sources() {
        if !existing.contains(&source.id) {
            merged.push(source);
        }
    }
    merged
}

#[cfg(test)]
#[path = "plugin_sources_toml_tests.rs"]
mod tests;
