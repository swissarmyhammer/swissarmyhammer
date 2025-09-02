//! Rate limiting utilities for preventing denial of service attacks
//!
//! This module provides configurable rate limiting for MCP operations and other API endpoints
//! using a token bucket algorithm with per-operation and per-client limits.

use crate::{Result, SwissArmyHammerError};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Trait for rate limiting functionality
///
/// This trait allows for dependency injection of rate limiting behavior,
/// enabling easier testing with mock implementations.
pub trait RateLimitChecker: Send + Sync {
    /// Check if an operation is allowed for a client
    ///
    /// # Arguments
    ///
    /// * `client_id` - Unique identifier for the client (IP, session ID, etc.)
    /// * `operation` - The operation being performed
    /// * `cost` - Token cost of the operation (default: 1, expensive operations: 2-5)
    ///
    /// # Returns
    ///
    /// * `Ok(())` if operation is allowed
    /// * `Err(SwissArmyHammerError)` if rate limit exceeded
    fn check_rate_limit(&self, client_id: &str, operation: &str, cost: u32) -> Result<()>;
}

/// Default rate limits for different operation types
pub const DEFAULT_GLOBAL_RATE_LIMIT: u32 = 100; // requests per minute
/// Default rate limit per client (requests per minute)
pub const DEFAULT_PER_CLIENT_RATE_LIMIT: u32 = 10; // requests per minute
/// Default rate limit for expensive operations (requests per minute)
pub const DEFAULT_EXPENSIVE_OPERATION_LIMIT: u32 = 5; // requests per minute

/// Rate limiter using token bucket algorithm
#[derive(Debug)]
pub struct RateLimiter {
    /// Global rate limits by operation type
    global_limits: DashMap<String, TokenBucket>,
    /// Per-client rate limits
    client_limits: DashMap<String, TokenBucket>,
    /// Configuration for operation limits
    config: RateLimiterConfig,
}

/// Configuration for rate limiter
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Global requests per minute across all clients
    pub global_limit: u32,
    /// Requests per minute per client
    pub per_client_limit: u32,
    /// Limit for expensive operations (search, complex workflows)
    pub expensive_operation_limit: u32,
    /// Time window for rate limiting (default: 1 minute)
    pub window_duration: Duration,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            global_limit: DEFAULT_GLOBAL_RATE_LIMIT,
            per_client_limit: DEFAULT_PER_CLIENT_RATE_LIMIT,
            expensive_operation_limit: DEFAULT_EXPENSIVE_OPERATION_LIMIT,
            window_duration: Duration::from_secs(60),
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
struct TokenBucket {
    /// Maximum tokens in the bucket
    capacity: u32,
    /// Current number of tokens
    tokens: u32,
    /// Last time tokens were added
    last_refill: Instant,
    /// Rate at which tokens are added (tokens per second)
    refill_rate: f64,
}

impl TokenBucket {
    /// Create a new token bucket
    fn new(capacity: u32, window_duration: Duration) -> Self {
        let refill_rate = capacity as f64 / window_duration.as_secs_f64();
        Self {
            capacity,
            tokens: capacity, // Start with full bucket
            last_refill: Instant::now(),
            refill_rate,
        }
    }

    /// Try to consume a token from the bucket
    fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();

        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on time elapsed
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        if elapsed > 0.0 {
            let tokens_to_add = (elapsed * self.refill_rate) as u32;
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
            self.last_refill = now;
        }
    }

    /// Get time until next token is available
    fn time_until_token(&mut self) -> Duration {
        self.refill();

        if self.tokens > 0 {
            Duration::from_secs(0)
        } else {
            Duration::from_secs_f64(1.0 / self.refill_rate)
        }
    }
}

impl RateLimiter {
    /// Create a new rate limiter with default configuration
    pub fn new() -> Self {
        Self::with_config(RateLimiterConfig::default())
    }

    /// Create a new rate limiter with custom configuration
    pub fn with_config(config: RateLimiterConfig) -> Self {
        Self {
            global_limits: DashMap::new(),
            client_limits: DashMap::new(),
            config,
        }
    }

    /// Check if an operation is allowed for a client
    ///
    /// # Arguments
    ///
    /// * `client_id` - Unique identifier for the client (IP, session ID, etc.)
    /// * `operation` - The operation being performed
    /// * `cost` - Token cost of the operation (default: 1, expensive operations: 2-5)
    ///
    /// # Returns
    ///
    /// * `Ok(())` if operation is allowed
    /// * `Err(SwissArmyHammerError)` if rate limit exceeded
    pub fn check_rate_limit(&self, client_id: &str, operation: &str, cost: u32) -> Result<()> {
        // Check global rate limit for this operation type
        let global_key = format!("global:{operation}");
        let mut global_bucket = self.global_limits.entry(global_key).or_insert_with(|| {
            let limit = self.operation_limit(operation);
            TokenBucket::new(limit, self.config.window_duration)
        });

        if !global_bucket.try_consume(cost) {
            let wait_time = global_bucket.time_until_token();
            return Err(SwissArmyHammerError::Other(format!(
                "Global rate limit exceeded for operation '{}'. Retry after {}ms",
                operation,
                wait_time.as_millis()
            )));
        }

        // Check per-client rate limit
        let client_key = format!("client:{client_id}");
        let mut client_bucket = self.client_limits.entry(client_key).or_insert_with(|| {
            TokenBucket::new(self.config.per_client_limit, self.config.window_duration)
        });

        if !client_bucket.try_consume(cost) {
            let wait_time = client_bucket.time_until_token();
            return Err(SwissArmyHammerError::Other(format!(
                "Client rate limit exceeded for '{}'. Retry after {}ms",
                client_id,
                wait_time.as_millis()
            )));
        }

        Ok(())
    }

    /// Get the rate limit for a specific operation
    fn operation_limit(&self, operation: &str) -> u32 {
        match operation {
            // Expensive operations that require more resources
            "search" | "workflow_run" | "complex_query" => self.config.expensive_operation_limit,
            // Standard operations
            _ => self.config.global_limit,
        }
    }

    /// Get current status of rate limits for monitoring
    pub fn get_rate_limit_status(&self, client_id: &str) -> RateLimitStatus {
        let global_remaining = self
            .global_limits
            .iter()
            .map(|entry| {
                let mut bucket = entry.value().clone();
                bucket.refill();
                bucket.tokens
            })
            .min()
            .unwrap_or(self.config.global_limit);

        let client_key = format!("client:{client_id}");
        let client_remaining = self
            .client_limits
            .get(&client_key)
            .map(|bucket_ref| {
                let mut bucket = bucket_ref.clone();
                bucket.refill();
                bucket.tokens
            })
            .unwrap_or(self.config.per_client_limit);

        RateLimitStatus {
            global_remaining,
            client_remaining,
            global_limit: self.config.global_limit,
            client_limit: self.config.per_client_limit,
            window_seconds: self.config.window_duration.as_secs(),
        }
    }

    /// Clean up old entries to prevent memory leaks
    pub fn cleanup_old_entries(&self) {
        let cutoff = Instant::now() - self.config.window_duration * 2;

        self.client_limits
            .retain(|_, bucket| bucket.last_refill > cutoff);

        self.global_limits
            .retain(|_, bucket| bucket.last_refill > cutoff);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitChecker for RateLimiter {
    fn check_rate_limit(&self, client_id: &str, operation: &str, cost: u32) -> Result<()> {
        self.check_rate_limit(client_id, operation, cost)
    }
}

/// Rate limit status for monitoring and headers
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    /// Remaining requests in global bucket
    pub global_remaining: u32,
    /// Remaining requests in client bucket  
    pub client_remaining: u32,
    /// Global rate limit
    pub global_limit: u32,
    /// Per-client rate limit
    pub client_limit: u32,
    /// Time window in seconds
    pub window_seconds: u64,
}

/// Shared rate limiter instance
static RATE_LIMITER: std::sync::OnceLock<Arc<RateLimiter>> = std::sync::OnceLock::new();

/// Get the global rate limiter instance
pub fn get_rate_limiter() -> &'static Arc<RateLimiter> {
    RATE_LIMITER.get_or_init(|| Arc::new(RateLimiter::new()))
}

/// Initialize rate limiter with custom configuration
pub fn init_rate_limiter(config: RateLimiterConfig) {
    RATE_LIMITER
        .set(Arc::new(RateLimiter::with_config(config)))
        .map_err(|_| "Rate limiter already initialized")
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_creation() {
        let bucket = TokenBucket::new(10, Duration::from_secs(60));
        assert_eq!(bucket.capacity, 10);
        assert_eq!(bucket.tokens, 10);
    }

    #[test]
    fn test_token_bucket_consume() {
        let mut bucket = TokenBucket::new(5, Duration::from_secs(60));

        assert!(bucket.try_consume(3));
        assert_eq!(bucket.tokens, 2);

        assert!(bucket.try_consume(2));
        assert_eq!(bucket.tokens, 0);

        assert!(!bucket.try_consume(1)); // Should fail
    }

    #[test]
    fn test_rate_limiter_basic() {
        let limiter = RateLimiter::with_config(RateLimiterConfig {
            per_client_limit: 2,
            global_limit: 5,
            expensive_operation_limit: 1,
            window_duration: Duration::from_secs(60),
        });

        // Should succeed
        assert!(limiter.check_rate_limit("client1", "test_op", 1).is_ok());
        assert!(limiter.check_rate_limit("client1", "test_op", 1).is_ok());

        // Should fail - client limit exceeded
        assert!(limiter.check_rate_limit("client1", "test_op", 1).is_err());

        // Different client should still work
        assert!(limiter.check_rate_limit("client2", "test_op", 1).is_ok());
    }

    #[test]
    fn test_rate_limiter_expensive_operations() {
        let limiter = RateLimiter::with_config(RateLimiterConfig {
            per_client_limit: 10,
            global_limit: 10,
            expensive_operation_limit: 1,
            window_duration: Duration::from_secs(60),
        });

        // First expensive operation should succeed
        assert!(limiter.check_rate_limit("client1", "search", 1).is_ok());

        // Second should fail due to expensive operation limit
        assert!(limiter.check_rate_limit("client1", "search", 1).is_err());

        // Regular operations should still work
        assert!(limiter.check_rate_limit("client1", "regular_op", 1).is_ok());
    }

    #[test]
    fn test_rate_limit_status() {
        let limiter = RateLimiter::with_config(RateLimiterConfig {
            per_client_limit: 5,
            global_limit: 10,
            expensive_operation_limit: 2,
            window_duration: Duration::from_secs(60),
        });

        let status = limiter.get_rate_limit_status("client1");
        assert_eq!(status.client_limit, 5);
        assert_eq!(status.global_limit, 10);
        assert_eq!(status.client_remaining, 5);
    }

    #[test]
    fn test_real_rate_limiting_behavior_replaces_mock() {
        // This test verifies that real rate limiting works correctly
        // and demonstrates the replacement of MockRateLimiter functionality
        let limiter = RateLimiter::with_config(RateLimiterConfig {
            per_client_limit: 3,
            global_limit: 10,
            expensive_operation_limit: 2,
            window_duration: Duration::from_secs(60),
        });

        // Test basic rate limiting with real enforcement
        assert!(limiter
            .check_rate_limit("test_client", "operation", 1)
            .is_ok());
        assert!(limiter
            .check_rate_limit("test_client", "operation", 1)
            .is_ok());
        assert!(limiter
            .check_rate_limit("test_client", "operation", 1)
            .is_ok());

        // Should be rate limited now - this is real rate limiting, not a mock
        let result = limiter.check_rate_limit("test_client", "operation", 1);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Client rate limit exceeded"));

        // Different client should still work
        assert!(limiter
            .check_rate_limit("different_client", "operation", 1)
            .is_ok());

        // Test expensive operations are really limited
        assert!(limiter.check_rate_limit("new_client", "search", 1).is_ok());
        assert!(limiter.check_rate_limit("new_client", "search", 1).is_ok());

        // Should fail on expensive operation limit
        let expensive_result = limiter.check_rate_limit("new_client", "search", 1);
        assert!(expensive_result.is_err());
        assert!(expensive_result
            .unwrap_err()
            .to_string()
            .contains("Global rate limit exceeded"));
    }
}
