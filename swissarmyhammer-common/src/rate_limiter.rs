//! Rate limiting utilities for preventing denial of service attacks
//!
//! This module provides configurable rate limiting for MCP operations and other API endpoints
//! using the governor crate's token bucket algorithm with per-operation and per-client limits.

use crate::{Result, SwissArmyHammerError};
use dashmap::DashMap;
use governor::state::InMemoryState;
use governor::{clock::DefaultClock, Quota, RateLimiter as GovernorRateLimiter};
use std::num::NonZeroU32;
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
pub const DEFAULT_GLOBAL_RATE_LIMIT: u32 = 10000; // requests per minute
/// Default rate limit per client (requests per minute)
pub const DEFAULT_PER_CLIENT_RATE_LIMIT: u32 = 1000; // requests per minute
/// Default rate limit for expensive operations (requests per minute)
pub const DEFAULT_EXPENSIVE_OPERATION_LIMIT: u32 = 500; // requests per minute

/// Rate limiter using governor crate's token bucket algorithm
#[derive(Debug)]
pub struct RateLimiter {
    /// Global rate limiters by operation type
    global_limiters:
        DashMap<String, GovernorRateLimiter<String, DashMap<String, InMemoryState>, DefaultClock>>,
    /// Per-client rate limiter
    client_limiter: GovernorRateLimiter<String, DashMap<String, InMemoryState>, DefaultClock>,
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

impl RateLimiter {
    /// Create a new rate limiter with default configuration
    pub fn new() -> Self {
        Self::with_config(RateLimiterConfig::default())
    }

    /// Create a new rate limiter with custom configuration
    pub fn with_config(config: RateLimiterConfig) -> Self {
        // Create per-client rate limiter
        let client_quota = Quota::with_period(config.window_duration)
            .expect("Invalid duration for rate limiting")
            .allow_burst(
                NonZeroU32::new(config.per_client_limit).expect("Invalid per_client_limit"),
            );
        let client_limiter = GovernorRateLimiter::keyed(client_quota);

        Self {
            global_limiters: DashMap::new(),
            client_limiter,
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
        let global_limiter = self
            .global_limiters
            .entry(global_key.clone())
            .or_insert_with(|| {
                let limit = self.operation_limit(operation);
                let quota = Quota::with_period(self.config.window_duration)
                    .expect("Invalid duration for rate limiting")
                    .allow_burst(NonZeroU32::new(limit).expect("Invalid operation limit"));
                GovernorRateLimiter::keyed(quota)
            });

        // Try to consume tokens from global limiter
        let global_ok = if cost == 1 {
            global_limiter.check_key(&global_key).is_ok()
        } else {
            let cost_nonzero =
                NonZeroU32::new(cost).ok_or_else(|| SwissArmyHammerError::Other {
                    message: "Invalid cost value: must be greater than 0".to_string(),
                })?;
            matches!(
                global_limiter.check_key_n(&global_key, cost_nonzero),
                Ok(Ok(()))
            )
        };

        if !global_ok {
            return Err(SwissArmyHammerError::Other {
                message: format!(
                    "Global rate limit exceeded for operation '{}'. Retry after {}ms",
                    operation,
                    self.get_retry_after_ms()
                ),
            });
        }

        // Check per-client rate limit
        let client_id_string = client_id.to_string();
        let client_ok = if cost == 1 {
            self.client_limiter.check_key(&client_id_string).is_ok()
        } else {
            let cost_nonzero =
                NonZeroU32::new(cost).ok_or_else(|| SwissArmyHammerError::Other {
                    message: "Invalid cost value: must be greater than 0".to_string(),
                })?;
            matches!(
                self.client_limiter
                    .check_key_n(&client_id_string, cost_nonzero),
                Ok(Ok(()))
            )
        };

        if !client_ok {
            return Err(SwissArmyHammerError::Other {
                message: format!(
                    "Client rate limit exceeded for '{}'. Retry after {}ms",
                    client_id,
                    self.get_retry_after_ms()
                ),
            });
        }

        Ok(())
    }

    /// Get the rate limit for a specific operation
    fn operation_limit(&self, operation: &str) -> u32 {
        match operation {
            // Expensive operations that require more resources
            "search" | "workflow_run" | "complex_query" | "file_glob" | "file_grep" => {
                self.config.expensive_operation_limit
            }
            // Standard operations
            _ => self.config.global_limit,
        }
    }

    /// Get retry-after time in milliseconds
    fn get_retry_after_ms(&self) -> u64 {
        // For simplicity, return a fixed retry-after time based on window duration
        // In a real implementation, you might calculate this more precisely
        (self.config.window_duration.as_millis() / 10) as u64
    }

    /// Get current status of rate limits for monitoring
    pub fn get_rate_limit_status(&self, _client_id: &str) -> RateLimitStatus {
        // With governor, we can't easily inspect remaining tokens
        // So we'll provide best-effort estimates
        RateLimitStatus {
            global_remaining: self.config.global_limit, // Conservative estimate
            client_remaining: self.config.per_client_limit, // Conservative estimate
            global_limit: self.config.global_limit,
            client_limit: self.config.per_client_limit,
            window_seconds: self.config.window_duration.as_secs(),
        }
    }

    /// Clean up old entries to prevent memory leaks
    pub fn cleanup_old_entries(&self) {
        // Governor handles cleanup internally, but we can clean up our global limiters map
        // Remove entries that haven't been used recently
        let _cutoff = Instant::now() - self.config.window_duration * 2;

        // For simplicity in this implementation, we'll rely on governor's internal cleanup
        // In a production system, you might want to implement more sophisticated cleanup
        if self.global_limiters.len() > 1000 {
            // If we have too many operation types, clear the map
            // This is a simple heuristic - in practice you'd want more sophisticated logic
            self.global_limiters.clear();
        }
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

    #[test]
    fn test_high_cost_operations() {
        let limiter = RateLimiter::with_config(RateLimiterConfig {
            per_client_limit: 10,
            global_limit: 10,
            expensive_operation_limit: 5,
            window_duration: Duration::from_secs(60),
        });

        // Should succeed with high cost operation
        assert!(limiter.check_rate_limit("client1", "test_op", 5).is_ok());

        // Should fail - not enough tokens left
        assert!(limiter.check_rate_limit("client1", "test_op", 6).is_err());

        // Should succeed with lower cost
        assert!(limiter.check_rate_limit("client1", "test_op", 2).is_ok());
    }
}
