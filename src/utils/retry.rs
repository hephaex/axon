//! Retry logic with exponential backoff
//!
//! Provides retry functionality for transient failures.

use std::future::Future;
use std::time::Duration;

use crate::error::AxonError;
use crate::Result;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Add jitter to prevent thundering herd
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Create a new retry config with custom max retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Create a new retry config with custom initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Create a new retry config with custom max delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Create a config optimized for API calls
    pub fn for_api() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }

    /// Create a config optimized for local services (Ollama)
    pub fn for_local() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 1.5,
            jitter: false,
        }
    }

    /// Calculate delay for a given attempt number
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay =
            self.initial_delay.as_millis() as f64 * self.backoff_multiplier.powi(attempt as i32);

        let delay_ms = base_delay.min(self.max_delay.as_millis() as f64);

        let final_delay = if self.jitter {
            // Add up to 25% jitter
            let jitter_range = delay_ms * 0.25;
            let jitter = (rand_simple() * jitter_range * 2.0) - jitter_range;
            (delay_ms + jitter).max(0.0)
        } else {
            delay_ms
        };

        Duration::from_millis(final_delay as u64)
    }
}

/// Simple random number generator for jitter (0.0 to 1.0)
fn rand_simple() -> f64 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // Simple LCG
    ((seed.wrapping_mul(1103515245).wrapping_add(12345) >> 16) & 0x7fff) as f64 / 32767.0
}

/// Retry an async operation with exponential backoff
///
/// # Arguments
///
/// * `config` - Retry configuration
/// * `operation` - Async closure that returns Result<T>
///
/// # Returns
///
/// The successful result or the last error if all retries failed
///
/// # Example
///
/// ```ignore
/// let result = retry_with_backoff(RetryConfig::default(), || async {
///     make_api_call().await
/// }).await;
/// ```
pub async fn retry_with_backoff<T, F, Fut>(config: RetryConfig, mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error: Option<AxonError> = None;

    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                // Check if error is retryable
                if !err.is_retryable() || attempt == config.max_retries {
                    return Err(err);
                }

                // Calculate delay - use server-specified delay for rate limiting
                let delay = if let Some(retry_after) = err.retry_after() {
                    Duration::from_secs(retry_after)
                } else {
                    config.calculate_delay(attempt)
                };

                // Log retry attempt (in real code, use proper logging)
                #[cfg(debug_assertions)]
                eprintln!(
                    "Retry attempt {}/{} after {:?}: {}",
                    attempt + 1,
                    config.max_retries,
                    delay,
                    err
                );

                // Wait before retry
                tokio::time::sleep(delay).await;

                last_error = Some(err);
            }
        }
    }

    // Should not reach here, but just in case
    Err(last_error.unwrap_or_else(|| AxonError::Internal("Retry failed".into())))
}

/// Retry context for tracking retry state
#[derive(Debug, Clone)]
pub struct RetryContext {
    pub attempt: u32,
    pub total_elapsed: Duration,
    pub last_error: Option<String>,
}

impl RetryContext {
    pub fn new() -> Self {
        Self {
            attempt: 0,
            total_elapsed: Duration::ZERO,
            last_error: None,
        }
    }
}

impl Default for RetryContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_default_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert!(config.jitter);
    }

    #[test]
    fn test_api_config() {
        let config = RetryConfig::for_api();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_secs(1));
    }

    #[test]
    fn test_local_config() {
        let config = RetryConfig::for_local();
        assert_eq!(config.max_retries, 5);
        assert!(!config.jitter);
    }

    #[test]
    fn test_delay_calculation() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        // Attempt 0: 100ms
        let delay0 = config.calculate_delay(0);
        assert_eq!(delay0, Duration::from_millis(100));

        // Attempt 1: 200ms
        let delay1 = config.calculate_delay(1);
        assert_eq!(delay1, Duration::from_millis(200));

        // Attempt 2: 400ms
        let delay2 = config.calculate_delay(2);
        assert_eq!(delay2, Duration::from_millis(400));
    }

    #[test]
    fn test_delay_respects_max() {
        let config = RetryConfig {
            max_retries: 10,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 10.0,
            jitter: false,
        };

        // Should be capped at max_delay
        let delay = config.calculate_delay(5);
        assert_eq!(delay, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_retry_success_first_try() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(RetryConfig::default(), || {
            let c = counter_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok::<_, AxonError>(42)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let result = retry_with_backoff(config, || {
            let c = counter_clone.clone();
            async move {
                let count = c.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(AxonError::Timeout("timeout".into()))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32> = retry_with_backoff(RetryConfig::default(), || {
            let c = counter_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(AxonError::Config("not retryable".into()))
            }
        })
        .await;

        assert!(result.is_err());
        // Should only try once for non-retryable errors
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig {
            max_retries: 2,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(50),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let result: Result<i32> = retry_with_backoff(config, || {
            let c = counter_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(AxonError::Timeout("always fails".into()))
            }
        })
        .await;

        assert!(result.is_err());
        // Initial attempt + 2 retries = 3 total
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
