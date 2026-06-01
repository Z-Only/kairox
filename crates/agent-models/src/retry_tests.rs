use super::*;
use crate::ModelError;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn succeeds_on_first_try_returns_immediately() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let config = RetryConfig::default();
    let result = with_retry(&config, || {
        let cc = cc.clone();
        async move {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ModelError>(42)
        }
    })
    .await;

    assert_eq!(result.unwrap(), 42);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retries_on_recoverable_error_then_succeeds() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let config = RetryConfig {
        max_attempts: 3,
        initial_delay_ms: 10, // short delays for tests
        max_delay_ms: 100,
        backoff_factor: 2.0,
    };

    let result = with_retry(&config, || {
        let cc = cc.clone();
        async move {
            let count = cc.fetch_add(1, Ordering::SeqCst) + 1;
            if count < 2 {
                Err(ModelError::Api {
                    status: 429,
                    message: "rate limited".into(),
                })
            } else {
                Ok::<_, ModelError>("success")
            }
        }
    })
    .await;

    assert_eq!(result.unwrap(), "success");
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn unrecoverable_error_returns_immediately_without_retry() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let config = RetryConfig {
        max_attempts: 3,
        initial_delay_ms: 10,
        max_delay_ms: 100,
        backoff_factor: 2.0,
    };

    let result = with_retry(&config, || {
        let cc = cc.clone();
        async move {
            cc.fetch_add(1, Ordering::SeqCst);
            Err::<String, _>(ModelError::Api {
                status: 401,
                message: "unauthorized".into(),
            })
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
    match result.unwrap_err() {
        ModelError::Api { status, .. } => assert_eq!(status, 401),
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn exhausts_max_attempts_returns_last_error() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let config = RetryConfig {
        max_attempts: 3,
        initial_delay_ms: 10,
        max_delay_ms: 100,
        backoff_factor: 2.0,
    };

    let result = with_retry(&config, || {
        let cc = cc.clone();
        async move {
            let count = cc.fetch_add(1, Ordering::SeqCst) + 1;
            Err::<String, _>(ModelError::Api {
                status: 500,
                message: format!("attempt {count}"),
            })
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(call_count.load(Ordering::SeqCst), 3);
    match result.unwrap_err() {
        ModelError::Api { message, .. } => assert_eq!(message, "attempt 3"),
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn backoff_timing_approximately_doubles() {
    let config = RetryConfig {
        max_attempts: 3,
        initial_delay_ms: 50,
        max_delay_ms: 10_000,
        backoff_factor: 2.0,
    };

    let timestamps = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ts = timestamps.clone();

    let start = tokio::time::Instant::now();
    let _ = with_retry(&config, || {
        let ts = ts.clone();
        async move {
            ts.lock().unwrap().push(start.elapsed());
            Err::<(), _>(ModelError::Connection("timeout".into()))
        }
    })
    .await;

    let ts = timestamps.lock().unwrap();
    assert_eq!(ts.len(), 3);

    // First retry delay should be around initial_delay_ms (50ms)
    // Second retry delay should be around 2x that (100ms)
    let first_delay = (ts[1] - ts[0]).as_millis();
    let second_delay = (ts[2] - ts[1]).as_millis();

    // Allow generous margins for CI jitter: delay should be at least 40ms
    // and the second delay should be notably larger than the first
    assert!(
        first_delay >= 40,
        "first delay {first_delay}ms should be >= 40ms"
    );
    assert!(
        second_delay >= 80,
        "second delay {second_delay}ms should be >= 80ms"
    );
    assert!(
        second_delay > first_delay,
        "second delay {second_delay}ms should be > first delay {first_delay}ms"
    );
}

#[tokio::test]
async fn connection_error_is_retried() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let config = RetryConfig {
        max_attempts: 2,
        initial_delay_ms: 10,
        max_delay_ms: 100,
        backoff_factor: 2.0,
    };

    let result = with_retry(&config, || {
        let cc = cc.clone();
        async move {
            let count = cc.fetch_add(1, Ordering::SeqCst) + 1;
            if count < 2 {
                Err(ModelError::Connection("connection refused".into()))
            } else {
                Ok::<_, ModelError>("connected")
            }
        }
    })
    .await;

    assert_eq!(result.unwrap(), "connected");
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn stream_parse_error_is_not_retried() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let config = RetryConfig {
        max_attempts: 3,
        initial_delay_ms: 10,
        max_delay_ms: 100,
        backoff_factor: 2.0,
    };

    let result = with_retry(&config, || {
        let cc = cc.clone();
        async move {
            cc.fetch_add(1, Ordering::SeqCst);
            Err::<String, _>(ModelError::StreamParse("bad json".into()))
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn single_attempt_config_does_not_retry() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let config = RetryConfig {
        max_attempts: 1,
        initial_delay_ms: 10,
        max_delay_ms: 100,
        backoff_factor: 2.0,
    };

    let result = with_retry(&config, || {
        let cc = cc.clone();
        async move {
            cc.fetch_add(1, Ordering::SeqCst);
            Err::<String, _>(ModelError::Api {
                status: 500,
                message: "server error".into(),
            })
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[test]
fn retry_config_default_values() {
    let config = RetryConfig::default();
    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.initial_delay_ms, 1_000);
    assert_eq!(config.max_delay_ms, 30_000);
    assert!((config.backoff_factor - 2.0).abs() < f64::EPSILON);
}

#[test]
fn delay_for_attempt_respects_max_delay() {
    let config = RetryConfig {
        max_attempts: 10,
        initial_delay_ms: 1_000,
        max_delay_ms: 5_000,
        backoff_factor: 10.0,
    };

    // Attempt 3: base = 1000 * 10^3 = 1_000_000 — should be capped
    let delay = config.delay_for_attempt(3);
    // Max is 5000ms + up to 25% jitter = 6250ms
    assert!(
        delay.as_millis() <= 6250,
        "delay {}ms should be <= 6250ms",
        delay.as_millis()
    );
    assert!(
        delay.as_millis() >= 5000,
        "delay {}ms should be >= 5000ms (the cap)",
        delay.as_millis()
    );
}
