//! Unit tests for the MCP Registry mapping layer.

use super::parser::{first_sentence, is_latest, map_mcp_to_entry};
use super::types::McpServerWrapper;
use crate::catalog::{InstallSpec, RuntimeKind, TrustLevel};
use serde_json::json;

fn sample_wrapper(name: &str, is_latest: bool) -> serde_json::Value {
    json!({
        "server": {
            "name": name,
            "description": "A test server.",
            "title": "Test Server",
            "version": "1.0.0",
            "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
        },
        "_meta": {
            "io.modelcontextprotocol.registry/official": {
                "status": "active",
                "isLatest": is_latest
            }
        }
    })
}

fn parse_wrapper(val: &serde_json::Value) -> McpServerWrapper {
    serde_json::from_value(val.clone()).unwrap()
}

#[test]
fn maps_remote_server_to_streamable_http() {
    let val = sample_wrapper("com.example/my-server", true);
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.id, "com.example/my-server");
    assert_eq!(entry.display_name, "Test Server");
    assert_eq!(entry.summary, "A test server");
    assert_eq!(entry.source, "mcp-registry");
    match &entry.install {
        InstallSpec::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://example.com/mcp");
        }
        _ => panic!("expected StreamableHttp install"),
    }
}

#[test]
fn maps_remote_with_headers() {
    let val = json!({
        "server": {
            "name": "ai.example/app",
            "description": "API server.",
            "remotes": [{
                "type": "sse",
                "url": "https://api.example.com/mcp",
                "headers": [
                    {"name": "Authorization", "description": "Bearer token", "isRequired": true, "isSecret": true}
                ]
            }]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.id, "ai.example/app");
    match &entry.install {
        InstallSpec::Sse { url, headers } => {
            assert_eq!(url, "https://api.example.com/mcp");
            assert!(headers.contains_key("Authorization"));
        }
        _ => panic!("expected SSE install"),
    }
    assert!(entry
        .default_env
        .iter()
        .any(|e| e.key == "Authorization" && e.required && e.secret));
}

#[test]
fn maps_npm_package_to_stdio() {
    let val = json!({
        "server": {
            "name": "org.example/cli-tool",
            "description": "CLI tool.",
            "version": "2.0.0",
            "packages": [{
                "registryType": "npm",
                "identifier": "@example/cli-tool",
                "version": "2.0.0",
                "transport": {"type": "stdio"},
                "environmentVariables": [
                    {"name": "API_KEY", "description": "key", "isRequired": true, "isSecret": true}
                ]
            }]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    match &entry.install {
        InstallSpec::Stdio { command, args, .. } => {
            assert_eq!(command, "npx");
            assert_eq!(args, &["-y", "@example/cli-tool"]);
        }
        _ => panic!("expected Stdio install"),
    }
    assert_eq!(entry.requirements.len(), 1);
    assert_eq!(entry.requirements[0].kind, RuntimeKind::Node);
    let env = &entry.default_env;
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].key, "API_KEY");
    assert!(env[0].required);
    assert!(env[0].secret);
}

#[test]
fn maps_pypi_package_to_uvx() {
    let val = json!({
        "server": {
            "name": "org.example/py-tool",
            "description": "Python tool.",
            "version": "1.0.0",
            "packages": [{
                "registryType": "pypi",
                "identifier": "py-tool",
                "version": "1.0.0",
                "transport": {"type": "stdio"}
            }]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    match &entry.install {
        InstallSpec::Stdio { command, args, .. } => {
            assert_eq!(command, "uvx");
            assert_eq!(args, &["py-tool"]);
        }
        _ => panic!("expected Stdio install"),
    }
    assert_eq!(entry.requirements[0].kind, RuntimeKind::Python);
}

#[test]
fn filters_non_latest_entries() {
    let old = sample_wrapper("com.example/server", false);
    let wrapper = parse_wrapper(&old);
    assert!(!is_latest(&wrapper.meta));

    let latest = sample_wrapper("com.example/server", true);
    let wrapper = parse_wrapper(&latest);
    assert!(is_latest(&wrapper.meta));
}

#[test]
fn missing_meta_defaults_to_latest() {
    let val = json!({
        "server": {
            "name": "com.example/no-meta",
            "version": "1.0.0"
        }
    });
    let wrapper = parse_wrapper(&val);
    assert!(is_latest(&wrapper.meta));
}

#[test]
fn trust_ceiling_is_applied() {
    let val = sample_wrapper("com.example/server", true);
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.trust, TrustLevel::Community);
}

#[test]
fn title_fallback_to_name_suffix() {
    let val = json!({
        "server": {
            "name": "com.example/cool-server",
            "version": "1.0.0"
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.display_name, "cool-server");
}

#[test]
fn homepage_from_website_url() {
    let val = json!({
        "server": {
            "name": "com.example/srv",
            "version": "1.0.0",
            "websiteUrl": "https://example.com"
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.homepage.as_deref(), Some("https://example.com"));
}

#[test]
fn homepage_fallback_to_repository() {
    let val = json!({
        "server": {
            "name": "com.example/srv",
            "version": "1.0.0",
            "repository": {"url": "https://github.com/example/srv"}
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(
        entry.homepage.as_deref(),
        Some("https://github.com/example/srv")
    );
}

// ── Edge-case tests ─────────────────────────────────────────────────

#[test]
fn is_latest_meta_missing_official_key() {
    let meta = json!({"other": "value"});
    assert!(is_latest(&Some(meta)));
}

#[test]
fn is_latest_meta_official_missing_is_latest() {
    let meta = json!({
        "io.modelcontextprotocol.registry/official": {
            "status": "active"
        }
    });
    assert!(is_latest(&Some(meta)));
}

#[test]
fn is_latest_meta_none() {
    assert!(is_latest(&None));
}

#[test]
fn is_latest_explicit_false() {
    let meta = json!({
        "io.modelcontextprotocol.registry/official": {
            "isLatest": false
        }
    });
    assert!(!is_latest(&Some(meta)));
}

#[test]
fn remote_takes_precedence_over_package() {
    let val = json!({
        "server": {
            "name": "ai.example/hybrid",
            "description": "Has both remote and package.",
            "remotes": [{"type": "sse", "url": "https://remote.example.com/mcp"}],
            "packages": [{
                "registryType": "npm",
                "identifier": "@example/hybrid",
                "version": "1.0.0",
                "transport": {"type": "stdio"}
            }]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    // Remote wins.
    match &entry.install {
        InstallSpec::Sse { url, .. } => {
            assert_eq!(url, "https://remote.example.com/mcp");
        }
        _ => panic!("expected SSE from remote, not package"),
    }
}

#[test]
fn package_only_without_remote() {
    let val = json!({
        "server": {
            "name": "ai.example/pkg-only",
            "description": "Only a package.",
            "packages": [{
                "registryType": "npm",
                "identifier": "@example/pkg-only",
                "version": "1.0.0",
                "transport": {"type": "stdio"}
            }]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    match &entry.install {
        InstallSpec::Stdio { command, args, .. } => {
            assert_eq!(command, "npx");
            assert_eq!(args, &["-y", "@example/pkg-only"]);
        }
        _ => panic!("expected Stdio from package"),
    }
}

#[test]
fn no_remote_no_package_placeholder() {
    let val = json!({
        "server": {
            "name": "com.example/bare",
            "version": "1.0.0"
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    match &entry.install {
        InstallSpec::Stdio { command, args, .. } => {
            assert_eq!(command, "com.example/bare");
            assert!(args.is_empty());
        }
        _ => panic!("expected placeholder Stdio"),
    }
}

#[test]
fn deduplicates_runtime_requirements() {
    let val = json!({
        "server": {
            "name": "ai.example/dual-pkg",
            "description": "Two npm packages.",
            "packages": [
                {
                    "registryType": "npm",
                    "identifier": "@example/a",
                    "version": "1.0.0",
                    "transport": {"type": "stdio"}
                },
                {
                    "registryType": "npm",
                    "identifier": "@example/b",
                    "version": "2.0.0",
                    "transport": {"type": "stdio"}
                }
            ]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    // Both are npm → only one Node requirement.
    assert_eq!(entry.requirements.len(), 1);
    assert_eq!(entry.requirements[0].kind, RuntimeKind::Node);
}

#[test]
fn mixed_package_runtimes() {
    let val = json!({
        "server": {
            "name": "ai.example/mixed",
            "description": "npm + pypi packages.",
            "packages": [
                {
                    "registryType": "npm",
                    "identifier": "@example/js",
                    "version": "1.0.0",
                    "transport": {"type": "stdio"}
                },
                {
                    "registryType": "pypi",
                    "identifier": "py-part",
                    "version": "1.0.0",
                    "transport": {"type": "stdio"}
                }
            ]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.requirements.len(), 2);
    let kinds: Vec<_> = entry.requirements.iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RuntimeKind::Node));
    assert!(kinds.contains(&RuntimeKind::Python));
}

#[test]
fn deduplicates_env_vars_from_multiple_packages() {
    let val = json!({
        "server": {
            "name": "ai.example/dup-env",
            "description": "Duplicate env vars.",
            "packages": [
                {
                    "registryType": "npm",
                    "identifier": "@example/a",
                    "transport": {"type": "stdio"},
                    "environmentVariables": [
                        {"name": "API_KEY", "description": "key A", "isRequired": true}
                    ]
                },
                {
                    "registryType": "npm",
                    "identifier": "@example/b",
                    "transport": {"type": "stdio"},
                    "environmentVariables": [
                        {"name": "API_KEY", "description": "key B", "isRequired": false}
                    ]
                }
            ]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    // API_KEY appears only once (first-package-wins).
    assert_eq!(entry.default_env.len(), 1);
    assert_eq!(entry.default_env[0].key, "API_KEY");
}

#[test]
fn env_var_with_empty_name_skipped() {
    let val = json!({
        "server": {
            "name": "ai.example/empty-env",
            "description": "Env var with empty name.",
            "packages": [{
                "registryType": "npm",
                "identifier": "@example/x",
                "transport": {"type": "stdio"},
                "environmentVariables": [
                    {"name": "", "description": "should be skipped"}
                ]
            }]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert!(entry.default_env.is_empty());
}

#[test]
fn unknown_registry_type_no_runtime() {
    let val = json!({
        "server": {
            "name": "ai.example/unknown",
            "description": "Unknown registry type.",
            "packages": [{
                "registryType": "cargo",
                "identifier": "some-crate",
                "transport": {"type": "stdio"}
            }]
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert!(entry.requirements.is_empty());
    // Falls back to raw identifier as command.
    match &entry.install {
        InstallSpec::Stdio { command, .. } => {
            assert_eq!(command, "some-crate");
        }
        _ => panic!("expected Stdio"),
    }
}

#[test]
fn trust_ceiling_clamps_verified_down_to_community() {
    let val = sample_wrapper("com.example/server", true);
    let wrapper = parse_wrapper(&val);
    // Even with Verified ceiling, entry trust is clamped to Community.
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Verified).unwrap();
    assert_eq!(entry.trust, TrustLevel::Community);
}

#[test]
fn trust_ceiling_unverified_clamps_down() {
    let val = sample_wrapper("com.example/server", true);
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Unverified).unwrap();
    assert_eq!(entry.trust, TrustLevel::Unverified);
}

#[test]
fn first_sentence_cuts_at_period() {
    assert_eq!(first_sentence("Hello. World", 200), "Hello");
}

#[test]
fn first_sentence_cuts_at_newline() {
    assert_eq!(first_sentence("Line one\nLine two", 200), "Line one");
}

#[test]
fn first_sentence_truncates_long() {
    let long = "A".repeat(150);
    let result = first_sentence(&long, 50);
    assert!(result.ends_with('…'));
    assert!(result.chars().count() <= 51); // 50 chars + ellipsis
}

#[test]
fn empty_description_produces_display_name_summary() {
    let val = json!({
        "server": {
            "name": "com.example/no-desc",
            "title": "No Desc",
            "version": "1.0.0"
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.description, "");
    assert_eq!(entry.summary, "No Desc");
}

#[test]
fn website_url_beats_repository_for_homepage() {
    let val = json!({
        "server": {
            "name": "com.example/srv",
            "version": "1.0.0",
            "websiteUrl": "https://example.com",
            "repository": {"url": "https://github.com/example/srv"}
        }
    });
    let wrapper = parse_wrapper(&val);
    let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.homepage.as_deref(), Some("https://example.com"));
}
