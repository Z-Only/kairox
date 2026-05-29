use super::*;
use crate::loader::load_from_str;

#[test]
fn parse_stdio_mcp_server() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.filesystem]
type = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
keep_alive = true
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    assert_eq!(config.mcp_servers.len(), 1);
    let (id, server) = &config.mcp_servers[0];
    assert_eq!(id, "filesystem");
    assert_eq!(server.r#type, McpTransportType::Stdio);
    assert_eq!(server.command, Some("npx".to_string()));
    assert!(server.keep_alive);
}

#[test]
fn parse_sse_mcp_server() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.remote-search]
type = "sse"
url = "https://mcp.example.com/search"
api_key_env = "MCP_SEARCH_KEY"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    assert_eq!(config.mcp_servers.len(), 1);
    let (id, server) = &config.mcp_servers[0];
    assert_eq!(id, "remote-search");
    assert_eq!(server.r#type, McpTransportType::Sse);
    assert_eq!(
        server.url,
        Some("https://mcp.example.com/search".to_string())
    );
}

#[test]
fn parse_streamable_http_mcp_server() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.remote-search]
type = "streamable_http"
url = "https://mcp.example.com/mcp"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    assert_eq!(config.mcp_servers.len(), 1);
    let (id, server) = &config.mcp_servers[0];
    assert_eq!(id, "remote-search");
    assert_eq!(server.r#type, McpTransportType::StreamableHttp);
    assert_eq!(server.url, Some("https://mcp.example.com/mcp".to_string()));
}

#[test]
fn reject_stdio_without_command() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.bad]
type = "stdio"
"#;
    let result = load_from_str(toml, "test.toml");
    assert!(result.is_err());
}

#[test]
fn reject_sse_without_url() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.bad]
type = "sse"
"#;
    let result = load_from_str(toml, "test.toml");
    assert!(result.is_err());
}

#[test]
fn reject_streamable_http_without_url() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.bad]
type = "streamable_http"
"#;
    let result = load_from_str(toml, "test.toml");
    assert!(result.is_err());
}

#[test]
fn mcp_default_values() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.test]
type = "stdio"
command = "echo"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    let (_, server) = &config.mcp_servers[0];
    assert!(!server.keep_alive);
    assert_eq!(server.idle_timeout_secs, 300);
    assert!(server.auto_restart);
    assert_eq!(server.max_restart_attempts, 3);
}

#[test]
fn mcp_server_defs_converts_to_agent_mcp_types() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.fs]
type = "stdio"
command = "npx"
args = ["server-fs"]
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    let defs = config.mcp_server_defs();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "fs");
    assert!(matches!(
        defs[0].transport,
        agent_mcp::McpTransportDef::Stdio { .. }
    ));
    assert_eq!(defs[0].args, vec!["server-fs"]);
}

#[test]
fn config_parse_merges_multiple_mcp_servers() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.server1]
type = "stdio"
command = "npx"
args = ["server1"]

[mcp_servers.server2]
type = "sse"
url = "https://mcp2.example.com"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    assert_eq!(config.mcp_servers.len(), 2);
    let ids: Vec<&str> = config
        .mcp_servers
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    assert!(ids.contains(&"server1"));
    assert!(ids.contains(&"server2"));
}

#[test]
fn config_parse_stdio_mcp_with_all_fields() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.full-stdio]
type = "stdio"
command = "/usr/local/bin/node"
args = ["server.js", "--port", "3000"]
cwd = "/home/user/mcp-server"
keep_alive = true
idle_timeout_secs = 600
auto_restart = false
max_restart_attempts = 5

[mcp_servers.full-stdio.env]
NODE_ENV = "production"
DEBUG = "mcp:*"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    assert_eq!(config.mcp_servers.len(), 1);
    let (id, server) = &config.mcp_servers[0];
    assert_eq!(id, "full-stdio");
    assert_eq!(server.r#type, McpTransportType::Stdio);
    assert_eq!(server.command.as_deref(), Some("/usr/local/bin/node"));
    assert_eq!(
        server.args.as_deref(),
        Some(
            &vec![
                "server.js".to_string(),
                "--port".to_string(),
                "3000".to_string()
            ][..]
        )
    );
    assert_eq!(server.cwd.as_deref(), Some("/home/user/mcp-server"));
    assert!(server.keep_alive);
    assert_eq!(server.idle_timeout_secs, 600);
    assert!(!server.auto_restart);
    assert_eq!(server.max_restart_attempts, 5);
    let env = server.env.as_ref().unwrap();
    assert_eq!(env.get("NODE_ENV"), Some(&"production".to_string()));
    assert_eq!(env.get("DEBUG"), Some(&"mcp:*".to_string()));
}
