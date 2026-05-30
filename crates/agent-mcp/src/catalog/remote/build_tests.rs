use super::*;
use crate::catalog::TrustLevel;

#[test]
fn build_provider_returns_correct_impl_per_kind() {
    let http = SharedHttpClient::new().unwrap();
    let cache = Arc::new(HttpResponseCache::new(
        std::env::temp_dir().join("kairox-test-cache"),
    ));
    let mcp = build_provider(
        RemoteSourceConfig {
            id: "m".into(),
            display_name: "m".into(),
            kind: RemoteSourceKind::McpRegistry,
            url: "https://registry.modelcontextprotocol.io".into(),
            api_key_env: None,
            priority: 50,
            default_trust: TrustLevel::Community,
            enabled: true,
            cache_ttl_seconds: None,
        },
        http,
        cache,
    );
    assert_eq!(mcp.source_id(), "m");
}
