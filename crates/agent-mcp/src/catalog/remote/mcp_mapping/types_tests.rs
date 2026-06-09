use super::*;

#[test]
fn deserialize_minimal_response() {
    let json = r#"{"servers": []}"#;
    let resp: McpListResponse = serde_json::from_str(json).unwrap();
    assert!(resp.servers.is_empty());
    assert!(resp.metadata.is_none());
}

#[test]
fn deserialize_response_with_metadata() {
    let json = r#"{
        "servers": [],
        "metadata": {"nextCursor": "abc123"}
    }"#;
    let resp: McpListResponse = serde_json::from_str(json).unwrap();
    assert_eq!(
        resp.metadata.unwrap().next_cursor,
        Some("abc123".to_string())
    );
}

#[test]
fn deserialize_server_with_all_fields() {
    let json = r#"{
        "server": {
            "name": "com.example/my-server",
            "title": "My Server",
            "description": "A test server",
            "version": "1.0.0",
            "websiteUrl": "https://example.com",
            "remotes": [{
                "type": "streamable-http",
                "url": "https://api.example.com/mcp",
                "headers": [{
                    "name": "Authorization",
                    "description": "Bearer token",
                    "isRequired": true,
                    "isSecret": true
                }]
            }],
            "packages": [{
                "registryType": "npm",
                "identifier": "@example/my-server",
                "environmentVariables": [{
                    "name": "API_KEY",
                    "description": "API key",
                    "isRequired": true,
                    "isSecret": true
                }]
            }],
            "repository": {"url": "https://github.com/example/my-server"}
        }
    }"#;
    let wrapper: McpServerWrapper = serde_json::from_str(json).unwrap();
    let srv = &wrapper.server;

    assert_eq!(srv.name, "com.example/my-server");
    assert_eq!(srv.title.as_deref(), Some("My Server"));
    assert_eq!(srv.description.as_deref(), Some("A test server"));
    assert_eq!(srv.version.as_deref(), Some("1.0.0"));
    assert_eq!(srv.website_url.as_deref(), Some("https://example.com"));
    assert_eq!(srv.remotes.len(), 1);
    assert_eq!(srv.remotes[0].transport_type, "streamable-http");
    assert_eq!(srv.remotes[0].headers.len(), 1);
    assert_eq!(srv.packages.len(), 1);
    assert_eq!(srv.packages[0].registry_type, "npm");
    assert_eq!(srv.packages[0].environment_variables.len(), 1);
    assert!(srv.repository.is_some());
}

#[test]
fn deserialize_server_minimal_fields() {
    let json = r#"{"server": {"name": "minimal"}}"#;
    let wrapper: McpServerWrapper = serde_json::from_str(json).unwrap();
    assert_eq!(wrapper.server.name, "minimal");
    assert!(wrapper.server.title.is_none());
    assert!(wrapper.server.description.is_none());
    assert!(wrapper.server.remotes.is_empty());
    assert!(wrapper.server.packages.is_empty());
    assert!(wrapper.meta.is_none());
}

#[test]
fn deserialize_env_var_defaults() {
    let json = r#"{"name": "KEY"}"#;
    let ev: McpEnvVar = serde_json::from_str(json).unwrap();
    assert_eq!(ev.name.as_deref(), Some("KEY"));
    assert!(ev.description.is_none());
    assert!(ev.is_required.is_none());
    assert!(ev.is_secret.is_none());
}

#[test]
fn deserialize_remote_header_defaults() {
    let json = r#"{}"#;
    let header: McpRemoteHeader = serde_json::from_str(json).unwrap();
    assert!(header.name.is_none());
    assert!(header.description.is_none());
    assert!(header.is_required.is_none());
    assert!(header.is_secret.is_none());
}

#[test]
fn deserialize_package_transport() {
    let json = r#"{
        "registryType": "pypi",
        "identifier": "my-server",
        "transport": {"type": "stdio"},
        "environmentVariables": []
    }"#;
    let pkg: McpPackage = serde_json::from_str(json).unwrap();
    assert_eq!(pkg.registry_type, "pypi");
    assert_eq!(pkg.identifier, "my-server");
    assert_eq!(pkg.transport.unwrap().transport_type, "stdio");
}

#[test]
fn wrapper_with_meta_block() {
    let json = r#"{
        "server": {"name": "test"},
        "_meta": {
            "io.modelcontextprotocol.registry/official": {
                "isLatest": false
            }
        }
    }"#;
    let wrapper: McpServerWrapper = serde_json::from_str(json).unwrap();
    assert!(wrapper.meta.is_some());
    let is_latest = wrapper.meta.as_ref().unwrap()["io.modelcontextprotocol.registry/official"]
        ["isLatest"]
        .as_bool()
        .unwrap();
    assert!(!is_latest);
}
