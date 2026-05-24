#![cfg(feature = "live-model-tests")]

//! Live integration test against a real model API (GitHub Models / gpt-4o-mini).
//!
//! Gated behind the `live-model-tests` cargo feature so that the regular
//! `cargo test --workspace` stays hermetic. When the feature is enabled but
//! `GITHUB_TOKEN` is not set in the environment, the test prints a skip
//! message and returns early — it never panics. This lets developers run
//! `just test-live` locally without configuring a token.

use std::time::Duration;

use agent_config::{build_router, loader};
use agent_models::{ModelClient, ModelEvent, ModelRequest};
use futures::StreamExt;
use tokio::time::timeout;

/// Embed the fixture at compile time so the test does not depend on the
/// process current working directory at runtime. `CARGO_MANIFEST_DIR` points
/// at `crates/agent-runtime`, so the fixture lives two directories up.
const GITHUB_MODELS_PROFILE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/test-profiles/github-models.toml"
));

const PROFILE_ALIAS: &str = "github-gpt4o-mini";
const TOKEN_ENV: &str = "GITHUB_TOKEN";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

fn is_rate_limit_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("http 429") || lower.contains("too many requests")
}

#[tokio::test]
async fn github_models_responds_to_simple_prompt() {
    if std::env::var(TOKEN_ENV).is_err() {
        eprintln!(
            "[live-model-tests] skipping: {} not set; live integration test requires a GitHub Models token",
            TOKEN_ENV
        );
        return;
    }

    // Parse the embedded fixture, resolve env-backed API keys, validate, then
    // build the router that owns one ModelClient per profile.
    let mut config = loader::load_from_str(
        GITHUB_MODELS_PROFILE,
        "fixtures/test-profiles/github-models.toml",
    )
    .expect("github-models.toml fixture should parse");
    loader::resolve_api_keys(&mut config);
    loader::validate(&config).expect("github-models.toml fixture should validate");

    let router = build_router(&config);
    assert!(
        router.get_profile(PROFILE_ALIAS).is_some(),
        "expected profile '{}' to be registered",
        PROFILE_ALIAS
    );

    // ModelRouter implements ModelClient; routing is by `request.model_profile`.
    let request = ModelRequest::user_text(PROFILE_ALIAS, "Say hello in one word")
        .with_system_prompt("You are a terse assistant. Reply with a single word.");

    let open_result = timeout(REQUEST_TIMEOUT, router.stream(request))
        .await
        .expect("opening the model stream timed out");

    let stream_result = match open_result {
        Ok(stream) => stream,
        Err(err) => {
            let message = err.to_string();
            if is_rate_limit_error(&message) {
                eprintln!(
                    "[live-model-tests] skipping: GitHub Models rate limit hit while opening stream: {}",
                    message
                );
                return;
            }
            panic!("model stream should open successfully: {}", message);
        }
    };

    let mut stream = stream_result;
    let mut response = String::new();
    let mut completed = false;

    let collected: Result<(), String> = timeout(REQUEST_TIMEOUT, async {
        while let Some(event) = stream.next().await {
            match event {
                Ok(ModelEvent::TokenDelta(delta)) => response.push_str(&delta),
                Ok(ModelEvent::Completed { .. }) => {
                    completed = true;
                    return Ok(());
                }
                Ok(ModelEvent::Failed { message }) => {
                    return Err(format!("model reported failure: {}", message));
                }
                Ok(ModelEvent::ToolCallRequested { .. }) => {
                    // No tools were offered; ignore if the model emits one.
                }
                Err(err) => return Err(format!("stream error: {}", err)),
            }
        }
        Ok(())
    })
    .await
    .expect("draining the model stream timed out");

    if let Err(message) = collected {
        if is_rate_limit_error(&message) {
            eprintln!(
                "[live-model-tests] skipping: GitHub Models rate limit hit mid-stream: {}",
                message
            );
            return;
        }
        panic!("model stream should complete without error: {}", message);
    }

    eprintln!(
        "[live-model-tests] received response ({} chars, completed={}): {:?}",
        response.len(),
        completed,
        response
    );

    assert!(
        completed,
        "expected the model stream to emit a Completed event before ending; got {:?}",
        response
    );
    assert!(
        !response.trim().is_empty(),
        "expected non-empty response from GitHub Models; got {:?}",
        response
    );
}
