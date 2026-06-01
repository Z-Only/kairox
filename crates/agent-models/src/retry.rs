use crate::Result;
use rand::Rng;
use std::future::Future;

/// Configuration for retry with exponential backoff and jitter.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the initial try).
    pub max_attempts: u32,
    /// Initial delay in milliseconds before the first retry.
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds (caps the exponential growth).
    pub max_delay_ms: u64,
    /// Multiplicative factor applied to delay after each retry.
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1_000,
            max_delay_ms: 30_000,
            backoff_factor: 2.0,
        }
    }
}

impl RetryConfig {
    /// Compute the delay for a given attempt (0-indexed), including jitter.
    fn delay_for_attempt(&self, attempt: u32) -> std::time::Duration {
        let base = self.initial_delay_ms as f64 * self.backoff_factor.powi(attempt as i32);
        let capped = base.min(self.max_delay_ms as f64);

        // Add random jitter: 0-25% of current delay
        let jitter = rand::thread_rng().gen_range(0.0..0.25) * capped;
        let total_ms = (capped + jitter) as u64;

        std::time::Duration::from_millis(total_ms)
    }
}

/// Execute an async operation with retry on recoverable errors.
///
/// On a recoverable error the function waits with exponential backoff
/// plus random jitter, then retries. Unrecoverable errors are returned
/// immediately. After `max_attempts` failures the last error is returned.
pub async fn with_retry<F, Fut, T>(config: &RetryConfig, operation: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                if !err.is_recoverable() {
                    return Err(err);
                }

                let remaining = config.max_attempts - attempt - 1;
                if remaining == 0 {
                    last_error = Some(err);
                    break;
                }

                let delay = config.delay_for_attempt(attempt);
                tracing::warn!(
                    attempt = attempt + 1,
                    max_attempts = config.max_attempts,
                    delay_ms = delay.as_millis() as u64,
                    error = %err,
                    "retrying after recoverable error"
                );

                tokio::time::sleep(delay).await;
                last_error = Some(err);
            }
        }
    }

    Err(last_error.expect("retry loop executed at least once"))
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
