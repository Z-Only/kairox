use super::*;

#[test]
fn overlay_ignores_marketplace_mcp_servers() {
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
    assert!(!names.contains(&"brave-search".to_string()));
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
    assert!(
        cfg.mcp_servers.is_empty(),
        "MCP server definitions are only loaded from config.toml"
    );
}
