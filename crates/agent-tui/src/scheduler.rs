//! Adaptive frame-rate throttling for the TUI render loop.

use std::time::{Duration, Instant};

/// Adaptive frame-rate throttling for the TUI render loop.
///
/// In non-streaming mode we render at ~60 fps (16 ms). During streaming,
/// the interval is adapted based on the number of tokens accumulated since
/// the last render to avoid burning CPU on rapid small updates.
#[derive(Debug)]
pub struct RenderScheduler {
    /// Base interval (16 ms ≈ 60 fps).
    base_interval: Duration,
    /// Current adaptive interval.
    interval: Duration,
    /// Whether state has changed since the last render.
    dirty: bool,
    /// Whether we are in streaming mode.
    streaming: bool,
    /// Number of tokens that arrived since the last render.
    tokens_since_render: usize,
    /// Time of the last render.
    last_render: Instant,
}

impl RenderScheduler {
    const BASE_INTERVAL_MS: u64 = 16;
    const STREAMING_FAST_TOKENS: usize = 5;
    const STREAMING_FAST_INTERVAL_MS: u64 = 60;
    const STREAMING_SLOW_TOKENS: usize = 20;
    const STREAMING_SLOW_INTERVAL_MS: u64 = 120;

    pub fn new() -> Self {
        Self {
            base_interval: Duration::from_millis(Self::BASE_INTERVAL_MS),
            interval: Duration::from_millis(Self::BASE_INTERVAL_MS),
            dirty: true,
            streaming: false,
            tokens_since_render: 0,
            last_render: Instant::now(),
        }
    }

    /// Mark state as changed — a render is needed.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Mark dirty and immediately boost to the fastest frame rate
    /// (used for key presses, resize events).
    pub fn mark_dirty_immediate(&mut self) {
        self.dirty = true;
        self.interval = Duration::from_millis(Self::BASE_INTERVAL_MS);
    }

    /// Check whether we are currently in streaming mode.
    pub fn is_streaming(&self) -> bool {
        self.streaming
    }

    /// Enter or exit streaming mode.
    pub fn set_streaming(&mut self, streaming: bool) {
        self.streaming = streaming;
        if !streaming {
            self.tokens_since_render = 0;
            self.interval = self.base_interval;
        }
    }

    /// Record that tokens have arrived. Call this from the token-delta handler.
    pub fn add_tokens(&mut self, count: usize) {
        self.tokens_since_render += count;
    }

    /// Check whether we should render now.
    ///
    /// Returns `true` when the state is dirty **and** enough time has elapsed
    /// according to the adaptive interval. After returning `true`, the caller
    /// should call [`RenderScheduler::did_render`] to reset the timer and
    /// counters.
    pub fn should_render(&mut self) -> bool {
        if !self.dirty {
            return false;
        }

        self.update_interval();

        let elapsed = self.last_render.elapsed();
        elapsed >= self.interval
    }

    /// Call after a render has been performed.
    pub fn did_render(&mut self) {
        self.dirty = false;
        self.tokens_since_render = 0;
        self.last_render = Instant::now();
    }

    /// Reset all counters and state.
    pub fn reset(&mut self) {
        self.interval = self.base_interval;
        self.dirty = true;
        self.streaming = false;
        self.tokens_since_render = 0;
        self.last_render = Instant::now();
    }

    fn update_interval(&mut self) {
        if !self.streaming {
            self.interval = self.base_interval;
            return;
        }

        // Adaptive: ≥20 tokens → 120 ms, ≥5 tokens → 60 ms, else 16 ms.
        self.interval = if self.tokens_since_render >= Self::STREAMING_SLOW_TOKENS {
            Duration::from_millis(Self::STREAMING_SLOW_INTERVAL_MS)
        } else if self.tokens_since_render >= Self::STREAMING_FAST_TOKENS {
            Duration::from_millis(Self::STREAMING_FAST_INTERVAL_MS)
        } else {
            self.base_interval
        };
    }
}

impl Default for RenderScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_scheduler_adapts_interval_during_streaming() {
        let mut rs = RenderScheduler::new();
        rs.set_streaming(true);

        rs.add_tokens(4);
        rs.mark_dirty();
        rs.last_render = Instant::now() - Duration::from_millis(200);
        let _ = rs.should_render();
        assert_eq!(rs.interval, Duration::from_millis(16));

        rs.add_tokens(2);
        rs.mark_dirty();
        rs.last_render = Instant::now() - Duration::from_millis(200);
        let _ = rs.should_render();
        assert_eq!(rs.interval, Duration::from_millis(60));

        rs.did_render();
        rs.set_streaming(true);
        rs.add_tokens(20);
        rs.mark_dirty();
        rs.last_render = Instant::now() - Duration::from_millis(200);
        let _ = rs.should_render();
        assert_eq!(rs.interval, Duration::from_millis(120));
    }

    #[test]
    fn render_scheduler_non_streaming_is_fast() {
        let mut rs = RenderScheduler::new();
        assert!(!rs.streaming);
        rs.add_tokens(100);
        rs.mark_dirty();
        rs.last_render = Instant::now() - Duration::from_millis(200);
        let _ = rs.should_render();
        assert_eq!(rs.interval, Duration::from_millis(16));
    }
}
