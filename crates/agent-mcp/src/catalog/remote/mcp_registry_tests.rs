use super::*;
use crate::catalog::remote::RemoteSourceKind;
use crate::catalog::TrustLevel;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn end_to_end_list_fetches_and_filters() {
    let server = MockServer::start().await;
    let body = json!({
        "servers": [
            {
                "server": {
                    "name": "com.example/a",
                    "description": "A test server.",
                    "title": "Test Server",
                    "version": "1.0.0",
                    "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                },
                "_meta": {
                    "io.modelcontextprotocol.registry/official": {
                        "status": "active",
                        "isLatest": true
                    }
                }
            },
            {
                "server": {
                    "name": "com.example/b",
                    "description": "A test server.",
                    "title": "Test Server",
                    "version": "1.0.0",
                    "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                },
                "_meta": {
                    "io.modelcontextprotocol.registry/official": {
                        "status": "active",
                        "isLatest": false
                    }
                }
            },
            {
                "server": {
                    "name": "com.example/c",
                    "description": "A test server.",
                    "title": "Test Server",
                    "version": "1.0.0",
                    "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                },
                "_meta": {
                    "io.modelcontextprotocol.registry/official": {
                        "status": "active",
                        "isLatest": true
                    }
                }
            }
        ],
        "metadata": {"count": 3}
    });
    Mock::given(method("GET"))
        .and(path("/v0.1/servers"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_string()))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
    let provider = McpRegistryProvider::new(
        RemoteSourceConfig {
            id: "mcp-registry".into(),
            display_name: "MCP Servers".into(),
            kind: RemoteSourceKind::McpRegistry,
            url: server.uri(),
            api_key_env: None,
            priority: 50,
            default_trust: TrustLevel::Community,
            enabled: true,
            cache_ttl_seconds: None,
        },
        SharedHttpClient::new().unwrap(),
        cache,
    );
    let entries = provider.list(&CatalogQuery::default()).await.unwrap();
    // Only isLatest==true entries are returned (a and c).
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "com.example/a");
    assert_eq!(entries[1].id, "com.example/c");
    assert_eq!(entries[0].source, "mcp-registry");
}

#[tokio::test]
async fn cached_entries_served_on_second_list() {
    let server = MockServer::start().await;
    let body = json!({
        "servers": [
            {
                "server": {
                    "name": "com.example/warm",
                    "description": "Cache warm test.",
                    "title": "Warm",
                    "version": "1.0.0",
                    "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                },
                "_meta": {
                    "io.modelcontextprotocol.registry/official": {
                        "isLatest": true
                    }
                }
            }
        ],
        "metadata": {"count": 1}
    });
    Mock::given(method("GET"))
        .and(path("/v0.1/servers"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_string()))
        .expect(1) // only one HTTP call; second list() hits cache
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
    let provider = McpRegistryProvider::new(
        RemoteSourceConfig {
            id: "mcp-registry".into(),
            display_name: "MCP Servers".into(),
            kind: RemoteSourceKind::McpRegistry,
            url: server.uri(),
            api_key_env: None,
            priority: 50,
            default_trust: TrustLevel::Community,
            enabled: true,
            cache_ttl_seconds: None,
        },
        SharedHttpClient::new().unwrap(),
        cache,
    );

    // First call populates cache.
    let first = provider.list(&CatalogQuery::default()).await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].id, "com.example/warm");

    // Second call serves from in-memory cache — no HTTP call.
    let second = provider.list(&CatalogQuery::default()).await.unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].id, "com.example/warm");
}

#[tokio::test]
async fn refresh_clears_cache_and_refetches() {
    let server = MockServer::start().await;
    // First response.
    let body1 = json!({
        "servers": [
            {
                "server": {
                    "name": "com.example/v1",
                    "description": "First version.",
                    "title": "V1",
                    "version": "1.0.0",
                    "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                },
                "_meta": {
                    "io.modelcontextprotocol.registry/official": {
                        "isLatest": true
                    }
                }
            }
        ],
        "metadata": {"count": 1}
    });
    Mock::given(method("GET"))
        .and(path("/v0.1/servers"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body1.to_string()))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Second response (after refresh).
    let body2 = json!({
        "servers": [
            {
                "server": {
                    "name": "com.example/v1",
                    "description": "First version.",
                    "title": "V1",
                    "version": "1.0.0",
                    "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                },
                "_meta": {
                    "io.modelcontextprotocol.registry/official": {
                        "isLatest": true
                    }
                }
            },
            {
                "server": {
                    "name": "com.example/v2",
                    "description": "Second version.",
                    "title": "V2",
                    "version": "2.0.0",
                    "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                },
                "_meta": {
                    "io.modelcontextprotocol.registry/official": {
                        "isLatest": true
                    }
                }
            }
        ],
        "metadata": {"count": 2}
    });
    Mock::given(method("GET"))
        .and(path("/v0.1/servers"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body2.to_string()))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
    let provider = McpRegistryProvider::new(
        RemoteSourceConfig {
            id: "mcp-registry".into(),
            display_name: "MCP Servers".into(),
            kind: RemoteSourceKind::McpRegistry,
            url: server.uri(),
            api_key_env: None,
            priority: 50,
            default_trust: TrustLevel::Community,
            enabled: true,
            cache_ttl_seconds: None,
        },
        SharedHttpClient::new().unwrap(),
        cache,
    );

    let first = provider.list(&CatalogQuery::default()).await.unwrap();
    assert_eq!(first.len(), 1);

    provider.refresh().await.unwrap();
    let second = provider.list(&CatalogQuery::default()).await.unwrap();
    assert_eq!(second.len(), 2);
}
