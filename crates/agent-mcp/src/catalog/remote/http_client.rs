//! Thin async HTTP wrapper used by every remote catalog provider.
//!
//! Centralises timeout, user-agent, bearer-token and conditional-GET
//! handling so individual provider adapters stay focused on
//! source-specific decoding.
//!
//! Some servers (notably the official MCP Registry behind Google Cloud
//! infrastructure) reject connections from Rust's `hyper`/`rustls` stack
//! due to TLS fingerprint filtering, producing "Broken pipe (os error
//! 32)" at connect time even though the same URL works from `curl`. When
//! the primary `reqwest` path fails with a connection-level error the
//! client transparently retries via the system `curl` binary.

use crate::catalog::remote::RemoteError;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, USER_AGENT};
use std::time::Duration;

const DEFAULT_USER_AGENT: &str = concat!("kairox-marketplace/", env!("CARGO_PKG_VERSION"));

/// Maximum time (seconds) for the curl fallback process.
const CURL_TIMEOUT_SECONDS: u64 = 30;

#[derive(Clone)]
pub struct SharedHttpClient {
    inner: reqwest::Client,
}

pub struct GetOpts<'a> {
    pub api_key_env: Option<&'a str>,
    pub if_none_match: Option<&'a str>,
}

#[derive(Debug)]
pub struct GetResponse {
    pub status: u16,
    pub etag: Option<String>,
    pub body: Vec<u8>,
}

impl SharedHttpClient {
    pub fn new() -> Result<Self, RemoteError> {
        let inner = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .map_err(|e| RemoteError::Http(e.to_string()))?;
        Ok(Self { inner })
    }

    pub async fn get_json(&self, url: &str, opts: GetOpts<'_>) -> Result<GetResponse, RemoteError> {
        match self.get_json_reqwest(url, &opts).await {
            Ok(resp) => Ok(resp),
            Err(primary_err) if is_connection_error(&primary_err) => {
                tracing::warn!(
                    url,
                    error = %primary_err,
                    "reqwest connect failed, falling back to curl"
                );
                self.get_json_curl(url, &opts).await.map_err(|curl_err| {
                    RemoteError::Http(format!(
                        "curl fallback also failed: {curl_err} (original: {primary_err})"
                    ))
                })
            }
            Err(err) => Err(err),
        }
    }

    /// Primary path: use the in-process `reqwest` client.
    async fn get_json_reqwest(
        &self,
        url: &str,
        opts: &GetOpts<'_>,
    ) -> Result<GetResponse, RemoteError> {
        let mut headers = HeaderMap::new();
        if let Some(env_key) = opts.api_key_env {
            let value = std::env::var(env_key)
                .map_err(|_| RemoteError::Config(format!("missing env var: {env_key}")))?;
            let header_val = HeaderValue::from_str(&format!("Bearer {value}"))
                .map_err(|e| RemoteError::Http(e.to_string()))?;
            headers.insert(AUTHORIZATION, header_val);
        }
        if let Some(etag) = opts.if_none_match {
            headers.insert(
                HeaderName::from_static("if-none-match"),
                HeaderValue::from_str(etag).map_err(|e| RemoteError::Http(e.to_string()))?,
            );
        }
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        let resp = self
            .inner
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| RemoteError::Http(reqwest_error_chain(&e)))?;
        let status = resp.status().as_u16();
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let body = resp
            .bytes()
            .await
            .map_err(|e| RemoteError::Http(e.to_string()))?
            .to_vec();
        Ok(GetResponse { status, etag, body })
    }

    /// Fallback: shell out to the system `curl` binary. This uses the
    /// operating system's native TLS stack and avoids TLS fingerprint
    /// filtering that blocks `hyper`/`rustls`.
    async fn get_json_curl(
        &self,
        url: &str,
        opts: &GetOpts<'_>,
    ) -> Result<GetResponse, RemoteError> {
        use tokio::process::Command;

        let mut cmd = Command::new("curl");
        cmd.args([
            "--silent",
            "--show-error",
            "--location", // follow redirects
            "--max-time",
            &CURL_TIMEOUT_SECONDS.to_string(),
            "--header",
            &format!("User-Agent: {DEFAULT_USER_AGENT}"),
            "--header",
            "Accept: application/json",
            // Write HTTP status code to stderr so we can parse it
            "--write-out",
            "\n%{http_code}",
        ]);

        if let Some(env_key) = opts.api_key_env {
            let value = std::env::var(env_key)
                .map_err(|_| RemoteError::Config(format!("missing env var: {env_key}")))?;
            cmd.args(["--header", &format!("Authorization: Bearer {value}")]);
        }
        if let Some(etag) = opts.if_none_match {
            cmd.args(["--header", &format!("If-None-Match: {etag}")]);
        }

        // Include response headers so we can extract ETag.
        cmd.args(["--dump-header", "-"]);
        cmd.arg(url);

        let output = cmd
            .output()
            .await
            .map_err(|e| RemoteError::Http(format!("curl spawn failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RemoteError::Http(format!(
                "curl exited {}: {stderr}",
                output.status
            )));
        }

        parse_curl_dump_header_output(&output.stdout)
    }
}

/// Flatten a `reqwest::Error` into a single string that preserves the
/// full source chain (e.g. "error sending request … : Broken pipe (os
/// error 32)"). The default `Display` impl only shows the top-level
/// message and hides the underlying OS error that we need for fallback
/// detection.
fn reqwest_error_chain(err: &reqwest::Error) -> String {
    use std::fmt::Write;
    let mut buf = err.to_string();
    let mut source = std::error::Error::source(err);
    while let Some(inner) = source {
        let _ = write!(buf, ": {inner}");
        source = inner.source();
    }
    buf
}

/// Returns `true` for errors that indicate the connection itself failed
/// (as opposed to a valid HTTP error response). These are candidates for
/// the curl fallback.
fn is_connection_error(err: &RemoteError) -> bool {
    match err {
        RemoteError::Http(msg) => {
            let lower = msg.to_lowercase();
            lower.contains("broken pipe")
                || lower.contains("connection reset")
                || lower.contains("connection refused")
                || lower.contains("error sending request")
                || lower.contains("connection closed before message completed")
                || lower.contains("connection closed")
        }
        _ => false,
    }
}

/// Parse curl output produced with `--dump-header -`.
///
/// The output format is:
/// ```text
/// HTTP/2 200\r\n
/// content-type: application/json\r\n
/// etag: "abc"\r\n
/// \r\n
/// {"body":"here"}
/// 123              <-- appended by --write-out "\n%{http_code}"
/// ```
fn parse_curl_dump_header_output(raw: &[u8]) -> Result<GetResponse, RemoteError> {
    let full = String::from_utf8_lossy(raw);

    // Split headers from body at the first blank line (\r\n\r\n).
    let (header_block, body_and_code) = full
        .split_once("\r\n\r\n")
        .ok_or_else(|| RemoteError::Http("curl: cannot find header/body boundary".into()))?;

    // The --write-out appends "\n<status_code>" after the body.
    let (body_str, status_str) = match body_and_code.rsplit_once('\n') {
        Some((b, s)) if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() => (b, s),
        _ => {
            // Fallback: try to parse status from the HTTP status line.
            let status_from_header = header_block
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("200");
            (body_and_code, status_from_header)
        }
    };

    let status: u16 = status_str.trim().parse().unwrap_or(200);

    // Extract ETag from response headers.
    let etag = header_block
        .lines()
        .find(|line| line.to_lowercase().starts_with("etag:"))
        .map(|line| line[5..].trim().to_string());

    Ok(GetResponse {
        status,
        etag,
        body: body_str.as_bytes().to_vec(),
    })
}

#[cfg(test)]
mod tests {
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
}
