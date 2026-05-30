//! Read/write `~/.kairox/skill_sources.toml` for skill catalog source
//! configuration persistence.

use agent_core::facade::{SkillFieldMappingView, SkillSourceView};
use std::path::{Path, PathBuf};
use toml_edit::{value, DocumentMut, Item};

pub struct SkillSourcesToml {
    path: PathBuf,
}

impl SkillSourcesToml {
    pub fn new(dir: &Path) -> Self {
        std::fs::create_dir_all(dir).ok();
        Self {
            path: dir.join("skill_sources.toml"),
        }
    }

    pub fn read(&self) -> Vec<SkillSourceView> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        let doc: DocumentMut = match text.parse() {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };

        let mut sources = Vec::new();
        let sources_array = match doc.get("skill_sources") {
            Some(Item::ArrayOfTables(arr)) => arr,
            _ => return sources,
        };

        for item in sources_array.iter() {
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() {
                continue;
            }
            sources.push(SkillSourceView {
                id: id.to_string(),
                display_name: item
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(id)
                    .to_string(),
                kind: item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("custom")
                    .to_string(),
                url: item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                search_template: item
                    .get("search_template")
                    .and_then(|v| v.as_str())
                    .unwrap_or(
                        "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc",
                    )
                    .to_string(),
                download_template: item
                    .get("download_template")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/api/v1/download?slug={{slug}}")
                    .to_string(),
                list_template: item
                    .get("list_template")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                detail_template: item
                    .get("detail_template")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                field_mapping: SkillFieldMappingView::default(),
                enabled: item
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                priority: item
                    .get("priority")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(100) as u32,
                cache_ttl_seconds: item
                    .get("cache_ttl_seconds")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(900) as u64,
                last_error: None,
            });
        }
        sources
    }

    pub fn write(&self, sources: &[SkillSourceView]) -> Result<(), std::io::Error> {
        let mut doc = DocumentMut::new();
        for src in sources {
            let mut tbl = toml_edit::Table::new();
            tbl.insert("id", value(&src.id));
            tbl.insert("display_name", value(&src.display_name));
            tbl.insert("kind", value(&src.kind));
            tbl.insert("url", value(&src.url));
            tbl.insert("search_template", value(&src.search_template));
            tbl.insert("download_template", value(&src.download_template));
            if let Some(ref lt) = src.list_template {
                tbl.insert("list_template", value(lt));
            }
            if let Some(ref dt) = src.detail_template {
                tbl.insert("detail_template", value(dt));
            }
            tbl.insert("enabled", value(src.enabled));
            tbl.insert("priority", value(src.priority as i64));
            tbl.insert("cache_ttl_seconds", value(src.cache_ttl_seconds as i64));
            if !doc.contains_key("skill_sources") {
                doc["skill_sources"] = Item::ArrayOfTables(Default::default());
            }
            doc["skill_sources"]
                .as_array_of_tables_mut()
                .unwrap()
                .push(tbl);
        }

        let text = doc.to_string();
        std::fs::write(&self.path, text)
    }

    pub fn merge_with_defaults(&self, user_sources: &[SkillSourceView]) -> Vec<SkillSourceView> {
        let defaults = default_skill_sources();
        let mut merged: Vec<SkillSourceView> = Vec::new();
        let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for src in user_sources {
            seen_ids.insert(src.id.clone());
            merged.push(migrate_builtin_skill_source(src));
        }

        for src in &defaults {
            if !seen_ids.contains(&src.id) {
                merged.push(src.clone());
            }
        }

        merged.sort_by_key(|s| s.priority);
        merged
    }
}

fn migrate_builtin_skill_source(src: &SkillSourceView) -> SkillSourceView {
    if src.id != "skillhub" || src.url != "https://skills.palebluedot.live" {
        return src.clone();
    }

    let mut migrated = default_skill_sources()
        .into_iter()
        .find(|default| default.id == "skillhub")
        .unwrap_or_else(|| src.clone());
    migrated.enabled = src.enabled;
    migrated.priority = src.priority;
    migrated.cache_ttl_seconds = src.cache_ttl_seconds;
    migrated
}

pub fn default_skill_sources() -> Vec<SkillSourceView> {
    vec![SkillSourceView {
        id: "skillhub".into(),
        display_name: "SkillHub".into(),
        kind: "skillhub".into(),
        url: "https://api.skillhub.cn".into(),
        search_template:
            "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc"
                .into(),
        download_template: "/api/v1/download?slug={{slug}}".into(),
        list_template: Some(
            "/api/skills?page=1&pageSize={{limit}}&sortBy=downloads&order=desc".into(),
        ),
        detail_template: Some("/api/v1/skills/{{slug}}".into()),
        field_mapping: SkillFieldMappingView::default(),
        enabled: true,
        priority: 1,
        cache_ttl_seconds: 900,
        last_error: None,
    }]
}

#[cfg(test)]
#[path = "skill_sources_toml_tests.rs"]
mod tests;
