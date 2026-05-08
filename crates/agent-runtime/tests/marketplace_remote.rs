//! End-to-end Phase 2 test: build a `LocalRuntime` with two remote catalog
//! sources backed by `wiremock`, verify `list_catalog` aggregates entries
//! across builtin + remotes, and verify a failing source is isolated +
//! emits a `CatalogSourceFailed` event without breaking the aggregate.

use agent_core::{AddCatalogSourceRequest, AppFacade, CatalogQuery, EventPayload};
use agent_runtime::test_support::build_marketplace_runtime;
use futures::StreamExt;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_catalog_aggregates_builtin_and_two_remote_sources() {
    let first_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v0.1/servers"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mcp_registry_doc("k1", "K1")))
        .mount(&first_server)
        .await;

    let second_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v0.1/servers"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mcp_registry_doc("s1", "S1")))
        .mount(&second_server)
        .await;

    let (runtime, _tmp) = build_marketplace_runtime().await;

    runtime
        .add_catalog_source(AddCatalogSourceRequest {
            id: "internal".into(),
            display_name: "Internal".into(),
            kind: "mcp_registry".into(),
            url: first_server.uri(),
            api_key_env: None,
            priority: Some(10),
            default_trust: Some("verified".into()),
            enabled: Some(true),
            cache_ttl_seconds: None,
        })
        .await
        .unwrap();

    runtime
        .add_catalog_source(AddCatalogSourceRequest {
            id: "second".into(),
            display_name: "Second".into(),
            kind: "mcp_registry".into(),
            url: second_server.uri(),
            api_key_env: None,
            priority: Some(50),
            default_trust: Some("community".into()),
            enabled: Some(true),
            cache_ttl_seconds: None,
        })
        .await
        .unwrap();

    let entries = runtime.list_catalog(CatalogQuery::default()).await.unwrap();

    // Builtin entries (>= 1) + 1 internal + 1 second.
    assert!(entries.len() >= 3, "got {}", entries.len());
    assert!(
        entries.iter().any(|e| e.source == "internal"),
        "expected internal source"
    );
    assert!(
        entries.iter().any(|e| e.source == "second"),
        "expected second source"
    );
    assert!(
        entries.iter().any(|e| e.source == "builtin"),
        "expected builtin source"
    );
}

#[tokio::test]
async fn failed_source_does_not_break_list_and_emits_event() {
    let dead_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v0.1/servers"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&dead_server)
        .await;

    let (runtime, _tmp) = build_marketplace_runtime().await;
    let mut events = runtime.subscribe_all();

    runtime
        .add_catalog_source(AddCatalogSourceRequest {
            id: "broken".into(),
            display_name: "Broken".into(),
            kind: "mcp_registry".into(),
            url: dead_server.uri(),
            api_key_env: None,
            priority: Some(10),
            default_trust: Some("community".into()),
            enabled: Some(true),
            cache_ttl_seconds: None,
        })
        .await
        .unwrap();

    let entries = runtime.list_catalog(CatalogQuery::default()).await.unwrap();
    // Builtin still works; "broken" returns nothing.
    assert!(
        entries.iter().all(|e| e.source != "broken"),
        "broken source must contribute no entries"
    );
    assert!(
        entries.iter().any(|e| e.source == "builtin"),
        "builtin must still appear"
    );

    // Drain events for up to 2s; one of them should be CatalogSourceFailed
    // for "broken".
    let mut saw_failed = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), events.next()).await {
            Ok(Some(ev)) => {
                if matches!(
                    ev.payload,
                    EventPayload::CatalogSourceFailed { ref source, .. }
                        if source == "broken"
                ) {
                    saw_failed = true;
                    break;
                }
            }
            _ => continue,
        }
    }
    assert!(
        saw_failed,
        "expected CatalogSourceFailed event for 'broken' within 2s"
    );
}

fn mcp_registry_doc(name: &str, title: &str) -> String {
    format!(
        r#"{{
      "servers": [{{
        "server": {{
          "name": "{name}",
          "title": "{title}",
          "description": "sample mcp server",
          "version": "1.0.0",
          "remotes": [],
          "packages": [{{
            "registryType": "npm",
            "identifier": "@example/{name}",
            "version": "1.0.0",
            "environmentVariables": []
          }}]
        }},
        "_meta": {{
          "isLatest": true
        }}
      }}],
      "metadata": {{}}
    }}"#
    )
}
