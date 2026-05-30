//! Built-in catalog backed by an embedded JSON file.

use crate::catalog::{CatalogError, CatalogProvider, CatalogQuery, CatalogResult, ServerEntry};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::OnceLock;

const BUILTIN_JSON: &str = include_str!("data/builtin-catalog.json");

#[derive(Debug, Deserialize)]
struct Doc {
    schema_version: String,
    #[serde(default, rename = "generated_at")]
    _generated_at: Option<String>,
    entries: Vec<ServerEntry>,
}

/// Lazily-parsed static lookup table for builtin catalog entries.
fn builtin_entries() -> &'static [ServerEntry] {
    static ENTRIES: OnceLock<Vec<ServerEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| {
            let doc: Doc =
                serde_json::from_str(BUILTIN_JSON).expect("builtin catalog should be valid JSON");
            doc.entries
        })
        .as_slice()
}

/// Return the `verified` flag for a builtin catalog entry, if it exists.
pub fn lookup_verified(server_id: &str) -> bool {
    builtin_entries()
        .iter()
        .any(|e| e.id == server_id && e.verified)
}

pub struct BuiltinCatalogProvider {
    entries: Vec<ServerEntry>,
}

impl BuiltinCatalogProvider {
    pub fn new() -> CatalogResult<Self> {
        let doc: Doc = serde_json::from_str(BUILTIN_JSON)
            .map_err(|e| CatalogError::InvalidData(format!("builtin catalog: {e}")))?;
        if doc.schema_version != "1" {
            return Err(CatalogError::InvalidData(format!(
                "unsupported builtin catalog schema_version: {}",
                doc.schema_version
            )));
        }
        Ok(Self {
            entries: doc.entries,
        })
    }
}

#[async_trait]
impl CatalogProvider for BuiltinCatalogProvider {
    fn source_id(&self) -> &str {
        "builtin"
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let kw = query.keyword.as_deref().map(str::to_lowercase);
        let mut out: Vec<ServerEntry> = self
            .entries
            .iter()
            .filter(|e| {
                if let Some(ref k) = kw {
                    let hay = format!(
                        "{} {} {}",
                        e.display_name.to_lowercase(),
                        e.summary.to_lowercase(),
                        e.tags.join(" ").to_lowercase()
                    );
                    if !hay.contains(k) {
                        return false;
                    }
                }
                if let Some(cat) = &query.category {
                    if !e.categories.iter().any(|c| c == cat) {
                        return false;
                    }
                }
                if let Some(min) = query.trust_min {
                    if e.trust < min {
                        return false;
                    }
                }
                if let Some(src) = &query.source {
                    if &e.source != src {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        out.sort_by(|a, b| {
            b.trust
                .cmp(&a.trust)
                .then_with(|| a.display_name.cmp(&b.display_name))
        });

        if let Some(limit) = query.limit {
            out.truncate(limit);
        }
        Ok(out)
    }

    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
        Ok(self.entries.iter().find(|e| e.id == id).cloned())
    }
}

#[cfg(test)]
#[path = "builtin_tests.rs"]
mod tests;
