use super::*;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_json_sends_user_agent_and_returns_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/c.json"))
        .and(header_exists("user-agent"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "W/\"v1\"")
                .set_body_string(r#"{"ok":true}"#),
        )
        .mount(&server)
        .await;
    let c = SharedHttpClient::new().unwrap();
    let resp = c
        .get_json(
            &format!("{}/c.json", server.uri()),
            GetOpts {
                api_key_env: None,
                if_none_match: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.etag.as_deref(), Some("W/\"v1\""));
    assert_eq!(resp.body, br#"{"ok":true}"#);
}

#[tokio::test]
async fn get_json_attaches_bearer_when_env_set() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .and(header("authorization", "Bearer SECRET-VALUE"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;
    // SAFETY: tests share env; use a unique key per test run to avoid collisions.
    std::env::set_var("KAIROX_TEST_BEARER_T3", "SECRET-VALUE");
    let c = SharedHttpClient::new().unwrap();
    let resp = c
        .get_json(
            &format!("{}/x", server.uri()),
            GetOpts {
                api_key_env: Some("KAIROX_TEST_BEARER_T3"),
                if_none_match: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.status, 200);
    std::env::remove_var("KAIROX_TEST_BEARER_T3");
}

#[tokio::test]
async fn get_json_returns_config_error_when_env_unset() {
    let c = SharedHttpClient::new().unwrap();
    std::env::remove_var("KAIROX_TEST_MISSING_T3");
    let err = c
        .get_json(
            "http://127.0.0.1:1/never",
            GetOpts {
                api_key_env: Some("KAIROX_TEST_MISSING_T3"),
                if_none_match: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, RemoteError::Config(ref k) if k.contains("KAIROX_TEST_MISSING_T3")));
}

#[tokio::test]
async fn get_json_returns_304_when_etag_matches() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/304"))
        .and(header("if-none-match", "W/\"v1\""))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;
    let c = SharedHttpClient::new().unwrap();
    let resp = c
        .get_json(
            &format!("{}/304", server.uri()),
            GetOpts {
                api_key_env: None,
                if_none_match: Some("W/\"v1\""),
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.status, 304);
}
