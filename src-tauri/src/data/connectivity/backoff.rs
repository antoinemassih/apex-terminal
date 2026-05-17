//! Exponential backoff with jitter and a hard attempt ceiling.
//!
//! Designed to be cheap, allocation-free, and embeddable in a hot reconnect
//! loop. The previous behavior — fixed 2/3/5-second sleeps without jitter —
//! caused thundering-herd reconnects after a network blip and burned tokens
//! on stale-credential loops. This type fixes both:
//!
//! - Doubling delay (configurable factor) up to a hard `max`
//! - ±`jitter` (default 25 %) randomization so N peer apps don't all hit at once
//! - Optional `max_attempts` ceiling — `next_delay` returns `None` once exhausted
//!
//! Usage:
//! ```ignore
//! let mut bo = Backoff::new();
//! loop {
//!     match connect().await {
//!         Ok(_) => { bo.reset(); break }
//!         Err(_) => match bo.next_delay() {
//!             Some(d) => tokio::time::sleep(d).await,
//!             None    => break, // give up, surface ConnectionError::MaxRetriesExceeded
//!         }
//!     }
//! }
//! ```

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Backoff {
    base: Duration,
    max: Duration,
    factor: f32,
    jitter: f32,
    max_attempts: Option<u32>,
    attempt: u32,
}

impl Default for Backoff {
    fn default() -> Self { Self::new() }
}

impl Backoff {
    /// Defaults: base=100ms, max=32s, factor=2.0, jitter=±25%, max_attempts=Some(8).
    pub fn new() -> Self {
        Self {
            base: Duration::from_millis(100),
            max: Duration::from_secs(32),
            factor: 2.0,
            jitter: 0.25,
            max_attempts: Some(8),
            attempt: 0,
        }
    }

    pub fn with_base(mut self, base: Duration) -> Self { self.base = base; self }
    pub fn with_max(mut self, max: Duration) -> Self { self.max = max; self }
    pub fn with_factor(mut self, factor: f32) -> Self { self.factor = factor; self }
    pub fn with_jitter(mut self, jitter: f32) -> Self { self.jitter = jitter; self }
    pub fn with_max_attempts(mut self, max_attempts: Option<u32>) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Compute the next delay and increment the attempt counter.
    ///
    /// Returns `None` once `max_attempts` is exhausted — the caller should
    /// stop retrying and surface `ConnectionError::MaxRetriesExceeded`.
    pub fn next_delay(&mut self) -> Option<Duration> {
        if let Some(cap) = self.max_attempts {
            if self.attempt >= cap {
                return None;
            }
        }
        // Pre-jitter: base * factor^attempt, capped at max.
        let exp = self.factor.powi(self.attempt as i32);
        let nanos = (self.base.as_nanos() as f64) * (exp as f64);
        let nanos = nanos.min(self.max.as_nanos() as f64);

        // Apply ±jitter — symmetric so the mean equals the deterministic value.
        let j: f32 = if self.jitter > 0.0 {
            // rand::random::<f32>() ∈ [0,1); shift to [-1,1).
            rand::random::<f32>() * 2.0 - 1.0
        } else {
            0.0
        };
        let factor = 1.0 + (self.jitter * j) as f64;
        // Clamp so jitter can't push below zero.
        let jittered = (nanos * factor.max(0.0)).round() as u128;
        let jittered = jittered.min(self.max.as_nanos());

        self.attempt = self.attempt.saturating_add(1);
        Some(Duration::from_nanos(jittered as u64))
    }

    /// Reset attempt counter — call after a successful operation.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// How many delays have been handed out (i.e. attempt index for the next call).
    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_growth_until_max() {
        // Disable jitter for deterministic comparison.
        let mut bo = Backoff::new().with_jitter(0.0).with_max_attempts(None);
        let d0 = bo.next_delay().unwrap();
        let d1 = bo.next_delay().unwrap();
        let d2 = bo.next_delay().unwrap();
        assert!(d1 >= d0, "{:?} >= {:?}", d1, d0);
        assert!(d2 >= d1, "{:?} >= {:?}", d2, d1);
        // Pure-exp values without jitter: 100ms, 200ms, 400ms.
        assert_eq!(d0, Duration::from_millis(100));
        assert_eq!(d1, Duration::from_millis(200));
        assert_eq!(d2, Duration::from_millis(400));
    }

    #[test]
    fn max_caps_delay() {
        let mut bo = Backoff::new()
            .with_jitter(0.0)
            .with_max(Duration::from_secs(1))
            .with_max_attempts(None);
        // Run far past the cap.
        let mut last = Duration::ZERO;
        for _ in 0..20 {
            last = bo.next_delay().unwrap();
        }
        assert!(last <= Duration::from_secs(1));
    }

    #[test]
    fn max_attempts_returns_none() {
        let mut bo = Backoff::new().with_max_attempts(Some(3));
        assert!(bo.next_delay().is_some());
        assert!(bo.next_delay().is_some());
        assert!(bo.next_delay().is_some());
        assert!(bo.next_delay().is_none());
        assert!(bo.next_delay().is_none());
    }

    #[test]
    fn reset_restores_initial_delay() {
        let mut bo = Backoff::new().with_jitter(0.0);
        let d0 = bo.next_delay().unwrap();
        let _ = bo.next_delay();
        let _ = bo.next_delay();
        bo.reset();
        let d_after_reset = bo.next_delay().unwrap();
        assert_eq!(d_after_reset, d0);
        assert_eq!(bo.attempt(), 1);
    }

    #[test]
    fn jitter_within_band() {
        // With ±25% jitter, every sample at attempt 0 lies in [75ms, 125ms].
        let lo = Duration::from_millis(75);
        let hi = Duration::from_millis(125);
        for _ in 0..200 {
            let mut bo = Backoff::new(); // jitter = 0.25, base = 100ms
            let d = bo.next_delay().unwrap();
            assert!(d >= lo && d <= hi, "delay {:?} out of [{:?},{:?}]", d, lo, hi);
        }
    }
}
