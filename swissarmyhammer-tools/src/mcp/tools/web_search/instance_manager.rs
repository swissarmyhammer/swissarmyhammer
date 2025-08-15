//! Instance management for SearXNG instances
//!
//! This module provides centralized management of SearXNG instances including
//! discovery, health monitoring, selection strategies, and automatic failover.

use super::health_checker::HealthChecker;
use super::instance_discovery::{InstanceDiscovery, SearxInstance};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Selection strategy for choosing instances
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionStrategy {
    /// Round-robin selection through healthy instances
    RoundRobin,
    /// Weighted selection favoring higher grade instances
    WeightedByGrade,
    /// Weighted selection favoring faster responding instances
    WeightedByResponseTime,
    /// Random selection from healthy instances
    Random,
}

impl Default for SelectionStrategy {
    fn default() -> Self {
        Self::WeightedByGrade
    }
}

/// Configuration for instance manager
#[derive(Debug, Clone)]
pub struct InstanceManagerConfig {
    /// Discovery refresh interval
    pub discovery_refresh_interval: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Maximum consecutive failures before marking unhealthy
    pub max_consecutive_failures: u32,
    /// Selection strategy for instance choice
    pub selection_strategy: SelectionStrategy,
    /// Whether to enable instance discovery
    pub discovery_enabled: bool,
}

impl Default for InstanceManagerConfig {
    fn default() -> Self {
        Self {
            discovery_refresh_interval: Duration::from_secs(3600), // 1 hour
            health_check_interval: Duration::from_secs(300),       // 5 minutes
            max_consecutive_failures: 3,
            selection_strategy: SelectionStrategy::WeightedByGrade,
            discovery_enabled: true,
        }
    }
}

/// Central manager for SearXNG instances
#[derive(Debug)]
pub struct InstanceManager {
    instances: Arc<RwLock<Vec<SearxInstance>>>,
    current_index: AtomicUsize,
    last_discovery: Arc<RwLock<DateTime<Utc>>>,
    config: InstanceManagerConfig,
    discovery_client: InstanceDiscovery,
    health_checker: HealthChecker,
}

impl InstanceManager {
    /// Creates a new instance manager with default configuration
    pub async fn new() -> Self {
        let config = InstanceManagerConfig::default();
        Self::with_config(config).await
    }

    /// Creates a new instance manager with custom configuration
    pub async fn with_config(config: InstanceManagerConfig) -> Self {
        let manager = Self {
            instances: Arc::new(RwLock::new(Vec::new())),
            current_index: AtomicUsize::new(0),
            last_discovery: Arc::new(RwLock::new(Utc::now() - ChronoDuration::hours(24))), // Force initial discovery
            config,
            discovery_client: InstanceDiscovery::new(),
            health_checker: HealthChecker::new(),
        };

        // Initialize with fallback instances if discovery is disabled
        if !manager.config.discovery_enabled {
            let fallback_instances = Self::get_fallback_instances();
            *manager.instances.write().await = fallback_instances;
            // Set last discovery to now to avoid refreshing
            *manager.last_discovery.write().await = Utc::now();
        }

        manager
    }

    /// Gets the next available healthy instance using the configured selection strategy
    pub async fn get_next_instance(&self) -> Option<SearxInstance> {
        // Refresh instances if needed
        if self.should_refresh_instances().await {
            if let Err(e) = self.refresh_instances().await {
                warn!("Failed to refresh instances: {}", e);
            }
        }

        let instances = self.instances.read().await;
        let healthy_instances: Vec<&SearxInstance> = instances
            .iter()
            .filter(|instance| instance.is_healthy && !instance.is_rate_limited())
            .collect();

        if healthy_instances.is_empty() {
            warn!("No healthy instances available");
            return None;
        }

        let selected = match self.config.selection_strategy {
            SelectionStrategy::RoundRobin => self.select_round_robin(&healthy_instances),
            SelectionStrategy::WeightedByGrade => self.select_by_grade(&healthy_instances),
            SelectionStrategy::WeightedByResponseTime => {
                self.select_by_response_time(&healthy_instances)
            }
            SelectionStrategy::Random => self.select_random(&healthy_instances),
        };

        selected.cloned()
    }

    /// Marks an instance as failed and updates its health status
    pub async fn mark_instance_failed(&self, url: &str) {
        let mut instances = self.instances.write().await;

        if let Some(instance) = instances.iter_mut().find(|i| i.url == url) {
            instance.mark_failed();

            if instance.consecutive_failures >= self.config.max_consecutive_failures {
                warn!(
                    "Instance {} marked as unhealthy after {} consecutive failures",
                    url, instance.consecutive_failures
                );
            }
        }
    }

    /// Marks an instance as rate limited
    pub async fn mark_instance_rate_limited(&self, url: &str, duration: Duration) {
        let mut instances = self.instances.write().await;

        if let Some(instance) = instances.iter_mut().find(|i| i.url == url) {
            instance.set_rate_limited(duration);
            info!("Instance {} marked as rate limited for {:?}", url, duration);
        }
    }

    /// Refreshes the instance list from discovery and performs health checks
    pub async fn refresh_instances(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Starting instance refresh");

        let mut new_instances = if self.config.discovery_enabled {
            match self.discovery_client.discover_instances().await {
                Ok(discovered) => {
                    info!("Discovered {} instances from searx.space", discovered.len());
                    discovered
                }
                Err(e) => {
                    warn!("Instance discovery failed: {}", e);
                    // Fall back to existing instances or hardcoded list
                    let current_instances = self.instances.read().await;
                    if current_instances.is_empty() {
                        warn!("Using fallback instances due to discovery failure");
                        Self::get_fallback_instances()
                    } else {
                        current_instances.clone()
                    }
                }
            }
        } else {
            Self::get_fallback_instances()
        };

        // Perform health checks on all instances
        self.health_checker
            .bulk_health_check(&mut new_instances)
            .await;

        // Update the instance list
        *self.instances.write().await = new_instances;
        *self.last_discovery.write().await = Utc::now();

        let healthy_count = self
            .instances
            .read()
            .await
            .iter()
            .filter(|i| i.is_healthy)
            .count();

        info!(
            "Instance refresh completed: {} total, {} healthy",
            self.instances.read().await.len(),
            healthy_count
        );

        Ok(())
    }

    /// Performs health checks on all current instances
    pub async fn perform_health_checks(&self) {
        debug!("Starting health checks for all instances");

        let mut instances = self.instances.write().await;
        self.health_checker.bulk_health_check(&mut instances).await;

        let healthy_count = instances.iter().filter(|i| i.is_healthy).count();
        debug!(
            "Health checks completed: {}/{} healthy",
            healthy_count,
            instances.len()
        );
    }

    /// Gets the current list of instances (for debugging/monitoring)
    pub async fn get_instances(&self) -> Vec<SearxInstance> {
        self.instances.read().await.clone()
    }

    /// Gets the count of healthy instances
    pub async fn healthy_instance_count(&self) -> usize {
        self.instances
            .read()
            .await
            .iter()
            .filter(|i| i.is_healthy && !i.is_rate_limited())
            .count()
    }

    /// Gets the total count of instances
    pub async fn total_instance_count(&self) -> usize {
        self.instances.read().await.len()
    }

    /// Checks if instances should be refreshed based on the configured interval
    async fn should_refresh_instances(&self) -> bool {
        // Never refresh if discovery is disabled
        if !self.config.discovery_enabled {
            return false;
        }

        let last_discovery = *self.last_discovery.read().await;
        let next_discovery = last_discovery
            + ChronoDuration::from_std(self.config.discovery_refresh_interval).unwrap_or_default();
        Utc::now() >= next_discovery
    }

    /// Round-robin selection from healthy instances
    fn select_round_robin<'a>(
        &self,
        healthy_instances: &[&'a SearxInstance],
    ) -> Option<&'a SearxInstance> {
        let current = self.current_index.fetch_add(1, Ordering::SeqCst);
        let index = current % healthy_instances.len();
        healthy_instances.get(index).copied()
    }

    /// Selection weighted by instance grade (A+ > A > B)
    fn select_by_grade<'a>(
        &self,
        healthy_instances: &[&'a SearxInstance],
    ) -> Option<&'a SearxInstance> {
        // Find highest grade instances
        let max_score = healthy_instances.iter().map(|i| i.quality_score()).max()?;

        let top_instances: Vec<&SearxInstance> = healthy_instances
            .iter()
            .filter(|i| i.quality_score() == max_score)
            .copied()
            .collect();

        // Round-robin within the top grade
        if top_instances.is_empty() {
            return None;
        }

        let current = self.current_index.fetch_add(1, Ordering::SeqCst);
        let index = current % top_instances.len();
        top_instances.get(index).copied()
    }

    /// Selection weighted by response time (faster instances preferred)
    fn select_by_response_time<'a>(
        &self,
        healthy_instances: &[&'a SearxInstance],
    ) -> Option<&'a SearxInstance> {
        // Sort by response time and take the fastest
        healthy_instances
            .iter()
            .min_by_key(|i| i.response_time)
            .copied()
    }

    /// Random selection from healthy instances
    fn select_random<'a>(
        &self,
        healthy_instances: &[&'a SearxInstance],
    ) -> Option<&'a SearxInstance> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..healthy_instances.len());
        healthy_instances.get(index).copied()
    }

    /// Gets hardcoded fallback instances when discovery fails
    fn get_fallback_instances() -> Vec<SearxInstance> {
        let fallback_urls = [
            "https://search.bus-hit.me",
            "https://searx.tiekoetter.com",
            "https://search.projectsegfau.lt",
            "https://searx.work",
            "https://search.sapti.me",
        ];

        fallback_urls
            .iter()
            .map(|url| SearxInstance::new(url.to_string(), "B".to_string(), 95.0, 2000))
            .collect()
    }
}

impl Default for InstanceManager {
    fn default() -> Self {
        // Note: This will block, so prefer using new() in async contexts
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(Self::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_selection_strategy_default() {
        assert_eq!(
            SelectionStrategy::default(),
            SelectionStrategy::WeightedByGrade
        );
    }

    #[test]
    fn test_instance_manager_config_default() {
        let config = InstanceManagerConfig::default();

        assert_eq!(config.discovery_refresh_interval, Duration::from_secs(3600));
        assert_eq!(config.health_check_interval, Duration::from_secs(300));
        assert_eq!(config.max_consecutive_failures, 3);
        assert_eq!(
            config.selection_strategy,
            SelectionStrategy::WeightedByGrade
        );
        assert!(config.discovery_enabled);
    }

    #[tokio::test]
    async fn test_instance_manager_new() {
        let manager = InstanceManager::new().await;

        assert_eq!(
            manager.config.selection_strategy,
            SelectionStrategy::WeightedByGrade
        );
        assert!(manager.config.discovery_enabled);
    }

    #[tokio::test]
    async fn test_instance_manager_with_config() {
        let config = InstanceManagerConfig {
            discovery_enabled: false,
            selection_strategy: SelectionStrategy::RoundRobin,
            ..Default::default()
        };

        let manager = InstanceManager::with_config(config).await;

        assert_eq!(
            manager.config.selection_strategy,
            SelectionStrategy::RoundRobin
        );
        assert!(!manager.config.discovery_enabled);

        // Should have fallback instances when discovery is disabled
        let count = manager.total_instance_count().await;
        assert!(count > 0);
    }

    #[tokio::test]
    async fn test_mark_instance_failed() {
        let manager = InstanceManager::new().await;

        // Add a test instance
        let test_instance = SearxInstance::new(
            "https://test.example.com".to_string(),
            "A+".to_string(),
            95.0,
            1000,
        );
        manager.instances.write().await.push(test_instance);

        // Mark as failed
        manager
            .mark_instance_failed("https://test.example.com")
            .await;

        let instances = manager.instances.read().await;
        let test_instance = instances
            .iter()
            .find(|i| i.url == "https://test.example.com")
            .unwrap();
        assert_eq!(test_instance.consecutive_failures, 1);
        assert!(test_instance.is_healthy); // Still healthy after 1 failure
    }

    #[tokio::test]
    async fn test_mark_instance_rate_limited() {
        let manager = InstanceManager::new().await;

        // Add a test instance
        let test_instance = SearxInstance::new(
            "https://test.example.com".to_string(),
            "A+".to_string(),
            95.0,
            1000,
        );
        manager.instances.write().await.push(test_instance);

        // Mark as rate limited
        let duration = Duration::from_secs(300);
        manager
            .mark_instance_rate_limited("https://test.example.com", duration)
            .await;

        let instances = manager.instances.read().await;
        let test_instance = instances
            .iter()
            .find(|i| i.url == "https://test.example.com")
            .unwrap();
        assert!(test_instance.is_rate_limited());
    }

    #[test]
    fn test_get_fallback_instances() {
        let instances = InstanceManager::get_fallback_instances();

        assert!(!instances.is_empty());
        assert_eq!(instances.len(), 5);

        for instance in &instances {
            assert!(instance.url.starts_with("https://"));
            assert_eq!(instance.grade, "B");
            assert!(instance.is_healthy);
        }
    }

    #[tokio::test]
    async fn test_select_round_robin() {
        let manager = InstanceManager::new().await;

        let instances = vec![
            SearxInstance::new("https://a.com".to_string(), "A+".to_string(), 95.0, 1000),
            SearxInstance::new("https://b.com".to_string(), "A".to_string(), 95.0, 1000),
            SearxInstance::new("https://c.com".to_string(), "B".to_string(), 95.0, 1000),
        ];

        let healthy_refs: Vec<&SearxInstance> = instances.iter().collect();

        let first = manager.select_round_robin(&healthy_refs).unwrap();
        let second = manager.select_round_robin(&healthy_refs).unwrap();
        let third = manager.select_round_robin(&healthy_refs).unwrap();
        let fourth = manager.select_round_robin(&healthy_refs).unwrap(); // Should wrap around

        // Should cycle through instances
        assert_ne!(first.url, second.url);
        assert_ne!(second.url, third.url);
        assert_eq!(first.url, fourth.url); // Wrapped around
    }

    #[tokio::test]
    async fn test_select_by_grade() {
        let manager = InstanceManager::new().await;

        let instances = vec![
            SearxInstance::new("https://a.com".to_string(), "A+".to_string(), 95.0, 1000),
            SearxInstance::new("https://b.com".to_string(), "B".to_string(), 95.0, 1000),
            SearxInstance::new("https://c.com".to_string(), "A".to_string(), 95.0, 1000),
        ];

        let healthy_refs: Vec<&SearxInstance> = instances.iter().collect();

        let selected = manager.select_by_grade(&healthy_refs).unwrap();

        // Should select the A+ instance
        assert_eq!(selected.url, "https://a.com");
        assert_eq!(selected.grade, "A+");
    }

    #[tokio::test]
    async fn test_select_by_response_time() {
        let manager = InstanceManager::new().await;

        let instances = vec![
            SearxInstance::new("https://slow.com".to_string(), "A+".to_string(), 95.0, 3000),
            SearxInstance::new("https://fast.com".to_string(), "B".to_string(), 95.0, 500),
            SearxInstance::new(
                "https://medium.com".to_string(),
                "A".to_string(),
                95.0,
                1500,
            ),
        ];

        let healthy_refs: Vec<&SearxInstance> = instances.iter().collect();

        let selected = manager.select_by_response_time(&healthy_refs).unwrap();

        // Should select the fastest instance
        assert_eq!(selected.url, "https://fast.com");
        assert_eq!(selected.response_time, 500);
    }

    #[tokio::test]
    async fn test_healthy_instance_count() {
        let manager = InstanceManager::new().await;

        let mut instances = vec![
            SearxInstance::new(
                "https://healthy.com".to_string(),
                "A+".to_string(),
                95.0,
                1000,
            ),
            SearxInstance::new(
                "https://unhealthy.com".to_string(),
                "A".to_string(),
                95.0,
                1000,
            ),
        ];

        // Mark second instance as unhealthy
        instances[1].mark_failed();
        instances[1].mark_failed();
        instances[1].mark_failed(); // 3 failures = unhealthy

        *manager.instances.write().await = instances;

        assert_eq!(manager.healthy_instance_count().await, 1);
        assert_eq!(manager.total_instance_count().await, 2);
    }

    #[tokio::test]
    async fn test_get_next_instance_no_healthy() {
        let config = InstanceManagerConfig {
            discovery_enabled: false,
            ..Default::default()
        };
        let manager = InstanceManager::with_config(config).await;

        // Mark all instances as unhealthy
        let mut instances = manager.instances.write().await;
        for instance in instances.iter_mut() {
            instance.mark_failed();
            instance.mark_failed();
            instance.mark_failed(); // 3 failures = unhealthy
        }
        drop(instances);

        let result = manager.get_next_instance().await;
        assert!(result.is_none());
    }
}
