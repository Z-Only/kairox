use serde::Deserialize;
use serde_json::Value;

use crate::{PluginError, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketplaceFile {
    pub name: String,
    pub display_name: String,
    pub plugins: Vec<MarketplacePluginEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketplacePluginEntry {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub source: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub keywords: Vec<String>,
    pub category: Option<String>,
    pub trust: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawMarketplaceFile {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    plugins: Vec<RawMarketplacePluginEntry>,
}

#[derive(Debug, Deserialize)]
struct RawMarketplacePluginEntry {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    repository: Option<Value>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    trust: Option<String>,
    source: Value,
}

pub fn parse_marketplace(raw: &str) -> Result<MarketplaceFile> {
    let parsed: RawMarketplaceFile = serde_json::from_str(raw)
        .map_err(|error| PluginError::InvalidManifest(error.to_string()))?;
    Ok(MarketplaceFile {
        display_name: parsed.display_name.unwrap_or_else(|| parsed.name.clone()),
        name: parsed.name,
        plugins: parsed
            .plugins
            .into_iter()
            .map(|plugin| MarketplacePluginEntry {
                name: plugin.name,
                description: plugin.description.unwrap_or_default(),
                version: plugin.version,
                source: normalize_source(plugin.source),
                homepage: plugin.homepage,
                repository: plugin.repository.and_then(normalize_optional_url),
                keywords: plugin.keywords,
                category: plugin.category,
                trust: plugin.trust,
            })
            .collect(),
    })
}

fn normalize_optional_url(value: Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value),
        Value::Object(object) => object
            .get("url")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        _ => None,
    }
}

fn normalize_source(source: Value) -> String {
    match source {
        Value::String(value) => value,
        Value::Object(object) => serde_json::to_string(&object).unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_marketplace_json() {
        let marketplace = parse_marketplace(
            r#"{
              "name": "my-plugins",
              "owner": {"name": "Team"},
              "plugins": [
                {
                  "name": "quality-review",
                  "source": "./plugins/quality-review",
                  "description": "Review code",
                  "version": "1.0.0",
                  "homepage": "https://example.com/quality-review",
                  "repository": {"url": "https://github.com/example/quality-review"},
                  "keywords": ["review", "coding"],
                  "category": "Coding",
                  "trust": "verified"
                }
              ]
            }"#,
        )
        .expect("marketplace");

        assert_eq!(marketplace.name, "my-plugins");
        assert_eq!(marketplace.plugins.len(), 1);
        assert_eq!(marketplace.plugins[0].source, "./plugins/quality-review");
        assert_eq!(
            marketplace.plugins[0].homepage.as_deref(),
            Some("https://example.com/quality-review")
        );
        assert_eq!(
            marketplace.plugins[0].repository.as_deref(),
            Some("https://github.com/example/quality-review")
        );
        assert_eq!(marketplace.plugins[0].keywords, vec!["review", "coding"]);
        assert_eq!(marketplace.plugins[0].category.as_deref(), Some("Coding"));
        assert_eq!(marketplace.plugins[0].trust.as_deref(), Some("verified"));
    }
}
