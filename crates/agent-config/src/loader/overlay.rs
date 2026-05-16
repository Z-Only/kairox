use crate::{Config, ConfigError};

/// Load main config plus an optional marketplace `mcp_servers.toml` overlay.
///
/// Both sources contribute to `mcp_servers`. On id conflict, the main file
/// wins. Profiles, base config, etc. come solely from the main file.
pub fn load_with_marketplace_overlay(
    main_content: &str,
    marketplace_content: Option<&str>,
    main_path: &str,
    marketplace_path: &str,
) -> Result<Config, ConfigError> {
    let mut cfg = super::load_from_str(main_content, main_path)?;

    let Some(market) = marketplace_content else {
        return Ok(cfg);
    };

    let market_cfg = super::load_from_str(market, marketplace_path)?;
    let existing: std::collections::HashSet<String> =
        cfg.mcp_servers.iter().map(|(id, _)| id.clone()).collect();
    for (id, srv) in market_cfg.mcp_servers {
        if !existing.contains(&id) {
            cfg.mcp_servers.push((id, srv));
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_merges_marketplace_into_main_with_main_winning() {
        let main = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"

[mcp_servers.filesystem]
type = "stdio"
command = "main-fs"
args = []
"#;
        let market = r#"
[mcp_servers.filesystem]
type = "stdio"
command = "marketplace-fs"
args = []

[mcp_servers.brave-search]
type = "stdio"
command = "npx"
args = ["-y", "@mcp/brave"]
"#;
        let cfg = load_with_marketplace_overlay(main, Some(market), "kairox.toml", "mcp.toml")
            .expect("merge ok");
        let names: Vec<_> = cfg.mcp_servers.iter().map(|(id, _)| id.clone()).collect();
        assert!(names.contains(&"filesystem".to_string()));
        assert!(names.contains(&"brave-search".to_string()));
        let fs = cfg
            .mcp_servers
            .iter()
            .find(|(id, _)| id == "filesystem")
            .unwrap();
        assert_eq!(
            fs.1.command.as_deref(),
            Some("main-fs"),
            "main file wins on id conflict"
        );
    }

    #[test]
    fn overlay_with_no_marketplace_is_just_main() {
        let main = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"
"#;
        let cfg = load_with_marketplace_overlay(main, None, "k.toml", "m.toml").unwrap();
        assert!(cfg.mcp_servers.is_empty());
    }

    #[test]
    fn overlay_marketplace_only_servers_section_parses() {
        // Marketplace file has no [profiles.*] section because ConfigToml
        // defaults profiles.
        let main = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"
"#;
        let market = r#"
[mcp_servers.foo]
type = "stdio"
command = "foo"
args = []
"#;
        let cfg = load_with_marketplace_overlay(main, Some(market), "k.toml", "m.toml").unwrap();
        assert_eq!(cfg.mcp_servers.len(), 1);
        assert_eq!(cfg.mcp_servers[0].0, "foo");
    }
}
