//! Rate limiting for API calls
//!
//! Token bucket rate limiter to prevent API throttling.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Token bucket rate limiter
///
/// Allows bursts up to bucket size while maintaining average rate.
#[derive(Debug)]
pub struct RateLimiter {
    inner: Arc<Mutex<RateLimiterInner>>,
}

#[derive(Debug)]
struct RateLimiterInner {
    /// Current number of tokens
    tokens: f64,
    /// Maximum bucket size (burst capacity)
    max_tokens: f64,
    /// Tokens added per second
    refill_rate: f64,
    /// Last time tokens were refilled
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    ///
    /// * `requests_per_second` - Average requests allowed per second
    /// * `burst_size` - Maximum burst size
    ///
    /// # Example
    ///
    /// ```
    /// use axon::utils::rate_limiter::RateLimiter;
    ///
    /// // Allow 10 requests/sec with burst of 20
    /// let limiter = RateLimiter::new(10.0, 20);
    /// ```
    pub fn new(requests_per_second: f64, burst_size: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateLimiterInner {
                tokens: burst_size as f64,
                max_tokens: burst_size as f64,
                refill_rate: requests_per_second,
                last_refill: Instant::now(),
            })),
        }
    }

    /// Create a rate limiter for Claude API (default limits)
    pub fn for_claude() -> Self {
        // Claude has ~60 requests/minute = 1 req/sec with burst of 10
        Self::new(1.0, 10)
    }

    /// Create a rate limiter for OpenAI API
    pub fn for_openai() -> Self {
        // OpenAI varies by tier, conservative defaults
        Self::new(3.0, 20)
    }

    /// Create a rate limiter for Gemini API
    pub fn for_gemini() -> Self {
        // Gemini free tier: 60 requests/minute
        Self::new(1.0, 15)
    }

    /// Create a rate limiter for local Ollama (no real limits)
    pub fn for_ollama() -> Self {
        // Local, so high limits
        Self::new(100.0, 100)
    }

    /// Acquire a token, waiting if necessary
    ///
    /// Returns immediately if tokens are available, otherwise waits.
    pub async fn acquire(&self) {
        loop {
            let wait_time = {
                let mut inner = self.inner.lock().await;
                inner.refill();

                if inner.tokens >= 1.0 {
                    inner.tokens -= 1.0;
                    return;
                }

                // Calculate wait time for next token
                let tokens_needed = 1.0 - inner.tokens;
                Duration::from_secs_f64(tokens_needed / inner.refill_rate)
            };

            // Wait outside the lock
            tokio::time::sleep(wait_time).await;
        }
    }

    /// Try to acquire a token without waiting
    ///
    /// Returns true if token was acquired, false otherwise.
    pub async fn try_acquire(&self) -> bool {
        let mut inner = self.inner.lock().await;
        inner.refill();

        if inner.tokens >= 1.0 {
            inner.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Get current number of available tokens
    pub async fn available(&self) -> u32 {
        let mut inner = self.inner.lock().await;
        inner.refill();
        inner.tokens as u32
    }

    /// Get time until next token is available
    pub async fn time_until_available(&self) -> Duration {
        let inner = self.inner.lock().await;
        if inner.tokens >= 1.0 {
            Duration::ZERO
        } else {
            let tokens_needed = 1.0 - inner.tokens;
            Duration::from_secs_f64(tokens_needed / inner.refill_rate)
        }
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl RateLimiterInner {
    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let new_tokens = elapsed.as_secs_f64() * self.refill_rate;

        self.tokens = (self.tokens + new_tokens).min(self.max_tokens);
        self.last_refill = now;
    }
}

/// Per-provider rate limiter registry
#[derive(Debug, Clone)]
pub struct RateLimiterRegistry {
    claude: RateLimiter,
    openai: RateLimiter,
    gemini: RateLimiter,
    ollama: RateLimiter,
}

impl RateLimiterRegistry {
    /// Create a new registry with default limiters
    pub fn new() -> Self {
        Self {
            claude: RateLimiter::for_claude(),
            openai: RateLimiter::for_openai(),
            gemini: RateLimiter::for_gemini(),
            ollama: RateLimiter::for_ollama(),
        }
    }

    /// Get rate limiter for a provider
    pub fn for_provider(&self, provider: &crate::protocol::Provider) -> &RateLimiter {
        match provider {
            crate::protocol::Provider::Anthropic => &self.claude,
            crate::protocol::Provider::OpenAi => &self.openai,
            crate::protocol::Provider::Google => &self.gemini,
            crate::protocol::Provider::Ollama => &self.ollama,
        }
    }
}

impl Default for RateLimiterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_acquire() {
        let limiter = RateLimiter::new(100.0, 10);

        // Should have 10 tokens available
        assert!(limiter.try_acquire().await);
        assert!(limiter.try_acquire().await);

        // Should still have tokens
        assert!(limiter.available().await > 0);
    }

    #[tokio::test]
    async fn test_burst() {
        let limiter = RateLimiter::new(1.0, 5);

        // Should be able to acquire 5 quickly (burst)
        for _ in 0..5 {
            assert!(limiter.try_acquire().await);
        }

        // 6th should fail (no tokens left)
        assert!(!limiter.try_acquire().await);
    }

    #[tokio::test]
    async fn test_refill() {
        let limiter = RateLimiter::new(100.0, 10);

        // Drain all tokens
        for _ in 0..10 {
            limiter.try_acquire().await;
        }
        assert!(!limiter.try_acquire().await);

        // Wait for refill (100 tokens/sec = 1 token per 10ms)
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should have some tokens now
        assert!(limiter.try_acquire().await);
    }

    #[tokio::test]
    async fn test_wait_for_token() {
        let limiter = RateLimiter::new(100.0, 1);

        // Use the only token
        limiter.acquire().await;

        // Next acquire should wait
        let start = Instant::now();
        limiter.acquire().await;
        let elapsed = start.elapsed();

        // Should have waited ~10ms (1 token at 100/sec)
        assert!(elapsed >= Duration::from_millis(5));
    }

    #[tokio::test]
    async fn test_clone_shares_state() {
        let limiter1 = RateLimiter::new(100.0, 10);
        let limiter2 = limiter1.clone();

        // Acquire from limiter1
        limiter1.try_acquire().await;

        // Should affect limiter2
        let available = limiter2.available().await;
        assert!(available < 10);
    }

    #[test]
    fn test_provider_limiters() {
        let registry = RateLimiterRegistry::new();

        // Each provider should have a limiter
        let _ = registry.for_provider(&crate::protocol::Provider::Anthropic);
        let _ = registry.for_provider(&crate::protocol::Provider::OpenAi);
        let _ = registry.for_provider(&crate::protocol::Provider::Google);
        let _ = registry.for_provider(&crate::protocol::Provider::Ollama);
    }
}
