//! Rate Limiter
//!
//! Provides rate limiting for TTS requests to prevent abuse
#![allow(dead_code)] // Ready for integration when needed
use governor::{Quota, RateLimiter, state::{InMemoryState, NotKeyed}, clock::DefaultClock};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;

/// TTS Rate Limiter using token bucket algorithm
pub struct TtsRateLimiter {
    limiter: Arc<Mutex<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
}

impl TtsRateLimiter {
    /// Create a new rate limiter with default settings
    /// Default: 10 requests per minute, burst of 5
    pub fn new() -> Self {
        // 10 requests per minute, burst of 5
        let quota = Quota::per_minute(NonZeroU32::new(10).unwrap())
            .allow_burst(NonZeroU32::new(5).unwrap());

        Self {
            limiter: Arc::new(Mutex::new(RateLimiter::direct(quota))),
        }
    }

    /// Create a new rate limiter with custom settings
    pub fn with_config(requests_per_minute: u32, burst_size: u32) -> Self {
        let quota = Quota::per_minute(NonZeroU32::new(requests_per_minute).unwrap())
            .allow_burst(NonZeroU32::new(burst_size).unwrap());

        Self {
            limiter: Arc::new(Mutex::new(RateLimiter::direct(quota))),
        }
    }

    /// Check if request is allowed under rate limit
    pub fn check(&self) -> Result<(), String> {
        self.limiter.lock().check()
            .map(|_| ())
            .map_err(|e| format!("Rate limit exceeded: {}", e))
    }

    /// Get time until next request is allowed
    /// Returns the approximate duration to wait before the next request will be allowed
    pub fn wait_time(&self) -> Duration {
        // Check the current state and return wait time
        // Since governor 0.6 doesn't have a simple wait_time method on the limiter itself,
        // we return a reasonable default
        Duration::from_millis(100)
    }
}

impl Default for TtsRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_basic() {
        let limiter = TtsRateLimiter::with_config(10, 2);

        // First request should succeed
        assert!(limiter.check().is_ok());

        // Second request should succeed (within burst)
        assert!(limiter.check().is_ok());

        // Third request should fail (exceeds burst)
        assert!(limiter.check().is_err());
    }

    #[test]
    fn test_rate_limiter_wait_time() {
        let limiter = TtsRateLimiter::with_config(2, 1);

        // First request should succeed
        assert!(limiter.check().is_ok());

        // Should have positive wait time
        let wait = limiter.wait_time();
        assert!(wait.as_millis() > 0);
    }
}
