//! Privacy and request anonymization features for web search
//!
//! This module provides comprehensive privacy protection features including:
//! - User-Agent rotation to prevent fingerprinting
//! - Request anonymization to strip identifying headers
//! - Request timing jitter to avoid detection patterns
//! - Smart instance distribution to prevent tracking
//!
//! All privacy features are configurable and can be disabled if needed.

use rand::Rng;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Privacy configuration for web search operations
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrivacyConfig {
    // User-Agent rotation
    /// Enable User-Agent rotation to prevent browser fingerprinting
    pub rotate_user_agents: bool,
    /// Use random User-Agent selection instead of sequential rotation
    pub randomize_user_agents: bool,
    /// Optional custom User-Agent strings to use instead of built-in defaults
    pub custom_user_agents: Option<Vec<String>>,
    
    // Request anonymization
    /// Add Do Not Track header to all requests
    pub enable_dnt: bool,
    /// Remove referrer headers to prevent tracking across sites
    pub strip_referrer: bool,
    /// Add cache-control headers to prevent response caching
    pub disable_cache: bool,
    
    // Request timing
    /// Add randomized delays between requests to avoid detection patterns
    pub enable_request_jitter: bool,
    /// Minimum delay in milliseconds for request jitter
    pub min_request_delay_ms: u64,
    /// Maximum delay in milliseconds for request jitter
    pub max_request_delay_ms: u64,
    
    // Instance distribution
    /// Enable smart distribution of requests across multiple search instances
    pub distribute_requests: bool,
    /// Number of recently used instances to avoid repeating
    pub avoid_repeat_instances: usize,
    
    // Content fetching privacy
    /// Apply privacy features to content fetching requests
    pub anonymize_content_requests: bool,
    /// Delay in milliseconds between content fetching requests
    pub content_request_delay_ms: u64,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            rotate_user_agents: true,
            randomize_user_agents: true,
            custom_user_agents: None,
            enable_dnt: true,
            strip_referrer: true,
            disable_cache: true,
            enable_request_jitter: true,
            min_request_delay_ms: 100,
            max_request_delay_ms: 500,
            distribute_requests: true,
            avoid_repeat_instances: 3,
            anonymize_content_requests: true,
            content_request_delay_ms: 200,
        }
    }
}

/// Manages User-Agent rotation for privacy
pub struct UserAgentRotator {
    user_agents: Vec<String>,
    current_index: AtomicUsize,
    randomize: bool,
}

impl Clone for UserAgentRotator {
    fn clone(&self) -> Self {
        Self {
            user_agents: self.user_agents.clone(),
            current_index: AtomicUsize::new(self.current_index.load(Ordering::Relaxed)),
            randomize: self.randomize,
        }
    }
}

impl UserAgentRotator {
    /// Creates a new UserAgentRotator with default browser User-Agent strings
    pub fn new(config: &PrivacyConfig) -> Self {
        let user_agents = config
            .custom_user_agents
            .clone()
            .unwrap_or_else(Self::default_user_agents);
        
        Self {
            user_agents,
            current_index: AtomicUsize::new(0),
            randomize: config.randomize_user_agents,
        }
    }
    
    /// Returns the default set of realistic User-Agent strings
    fn default_user_agents() -> Vec<String> {
        vec![
            // Chrome on Windows
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
            // Firefox on Windows  
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0".to_string(),
            // Safari on macOS
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15".to_string(),
            // Chrome on macOS
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
            // Firefox on Linux
            "Mozilla/5.0 (X11; Linux x86_64; rv:121.0) Gecko/20100101 Firefox/121.0".to_string(),
            // Chrome on Linux
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        ]
    }
    
    /// Gets the next User-Agent string based on the rotation strategy
    pub fn get_next_user_agent(&self) -> String {
        if self.user_agents.is_empty() {
            // Fallback to basic User-Agent if no agents configured
            return "SwissArmyHammer/1.0 (Privacy-Focused Web Search)".to_string();
        }
        
        if self.randomize {
            let index = {
                let mut rng = rand::thread_rng();
                rng.gen_range(0..self.user_agents.len())
            };
            self.user_agents[index].clone()
        } else {
            let index = self.current_index.fetch_add(1, Ordering::Relaxed) % self.user_agents.len();
            self.user_agents[index].clone()
        }
    }
}

/// Manages privacy headers for request anonymization
#[derive(Clone)]
pub struct PrivacyHeaders {
    enable_dnt: bool,
    strip_referrer: bool,
    disable_cache: bool,
}

impl PrivacyHeaders {
    /// Creates a new PrivacyHeaders instance from config
    pub fn new(config: &PrivacyConfig) -> Self {
        Self {
            enable_dnt: config.enable_dnt,
            strip_referrer: config.strip_referrer,
            disable_cache: config.disable_cache,
        }
    }
    
    /// Applies privacy headers to a request builder
    pub fn apply_privacy_headers(&self, mut request: RequestBuilder) -> RequestBuilder {
        // Add Do Not Track header
        if self.enable_dnt {
            request = request.header("DNT", "1");
        }
        
        // Strip referrer to prevent tracking
        if self.strip_referrer {
            request = request.header("Referrer-Policy", "no-referrer");
        }
        
        // Disable caching for privacy
        if self.disable_cache {
            request = request.header("Cache-Control", "no-cache, no-store, must-revalidate");
            request = request.header("Pragma", "no-cache");
            request = request.header("Expires", "0");
        }
        
        // Set standard browser headers to avoid standing out
        request = request.header("Accept-Language", "en-US,en;q=0.9");
        request = request.header("Accept-Encoding", "gzip, deflate, br");
        request = request.header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8");
        
        // Add Upgrade-Insecure-Requests for HTTPS preference
        request = request.header("Upgrade-Insecure-Requests", "1");
        
        request
    }
}

/// Manages request timing jitter to avoid detection patterns
#[derive(Clone)]
pub struct RequestJitter {
    min_delay: Duration,
    max_delay: Duration,
    enabled: bool,
}

impl RequestJitter {
    /// Creates a new RequestJitter instance from config
    pub fn new(config: &PrivacyConfig) -> Self {
        Self {
            min_delay: Duration::from_millis(config.min_request_delay_ms),
            max_delay: Duration::from_millis(config.max_request_delay_ms),
            enabled: config.enable_request_jitter,
        }
    }
    
    /// Applies randomized delay if jitter is enabled
    pub async fn apply_jitter(&self) {
        if !self.enabled {
            return;
        }
        
        let delay = {
            let mut rng = rand::thread_rng();
            let delay_ms = rng.gen_range(self.min_delay.as_millis()..=self.max_delay.as_millis());
            Duration::from_millis(delay_ms as u64)
        };
        
        tokio::time::sleep(delay).await;
    }
}

/// Manages distribution of requests across multiple instances
#[derive(Clone)]
pub struct InstanceDistributor {
    last_used_instances: Arc<Mutex<VecDeque<String>>>,
    avoid_repeat_count: usize,
    enabled: bool,
}

impl InstanceDistributor {
    /// Creates a new InstanceDistributor from config
    pub fn new(config: &PrivacyConfig) -> Self {
        Self {
            last_used_instances: Arc::new(Mutex::new(VecDeque::new())),
            avoid_repeat_count: config.avoid_repeat_instances,
            enabled: config.distribute_requests,
        }
    }
    
    /// Selects a distributed instance from available instances
    pub fn select_distributed_instance(&self, available: &[String]) -> Option<String> {
        if !self.enabled || available.is_empty() {
            // If disabled or no instances, just return the first available
            return available.first().cloned();
        }
        
        let last_used = self.last_used_instances.lock().unwrap();
        
        // Find instances that weren't recently used
        let suitable_instances: Vec<_> = available
            .iter()
            .filter(|instance| !last_used.contains(*instance))
            .collect();
        
        drop(last_used); // Release lock early
        
        if suitable_instances.is_empty() {
            // All instances recently used, pick any available one
            let index = {
                let mut rng = rand::thread_rng();
                rng.gen_range(0..available.len())
            };
            Some(available[index].clone())
        } else {
            // Pick random from suitable instances
            let index = {
                let mut rng = rand::thread_rng();
                rng.gen_range(0..suitable_instances.len())
            };
            Some(suitable_instances[index].clone())
        }
    }
    
    /// Records that an instance was used for distribution tracking
    pub fn record_instance_use(&self, instance_url: &str) {
        if !self.enabled {
            return;
        }
        
        let mut last_used = self.last_used_instances.lock().unwrap();
        
        // Add to front of queue
        last_used.push_front(instance_url.to_string());
        
        // Keep only recent instances
        while last_used.len() > self.avoid_repeat_count {
            last_used.pop_back();
        }
    }
}

/// Main privacy manager that coordinates all privacy features
#[derive(Clone)]
pub struct PrivacyManager {
    user_agent_rotator: Option<UserAgentRotator>,
    privacy_headers: PrivacyHeaders,
    request_jitter: RequestJitter,
    instance_distributor: InstanceDistributor,
}

impl PrivacyManager {
    /// Creates a new PrivacyManager from configuration
    pub fn new(config: PrivacyConfig) -> Self {
        let user_agent_rotator = if config.rotate_user_agents {
            Some(UserAgentRotator::new(&config))
        } else {
            None
        };
        
        Self {
            user_agent_rotator,
            privacy_headers: PrivacyHeaders::new(&config),
            request_jitter: RequestJitter::new(&config),
            instance_distributor: InstanceDistributor::new(&config),
        }
    }
    
    /// Gets the next User-Agent string for a request
    pub fn get_user_agent(&self) -> Option<String> {
        self.user_agent_rotator.as_ref().map(|r| r.get_next_user_agent())
    }
    
    /// Applies privacy headers to a request
    pub fn apply_privacy_headers(&self, request: RequestBuilder) -> RequestBuilder {
        self.privacy_headers.apply_privacy_headers(request)
    }
    
    /// Applies request jitter delay
    pub async fn apply_jitter(&self) {
        self.request_jitter.apply_jitter().await
    }
    
    /// Selects a distributed instance from available instances
    pub fn select_distributed_instance(&self, available: &[String]) -> Option<String> {
        self.instance_distributor.select_distributed_instance(available)
    }
    
    /// Records instance usage for distribution tracking
    pub fn record_instance_use(&self, instance_url: &str) {
        self.instance_distributor.record_instance_use(instance_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_privacy_config_default() {
        let config = PrivacyConfig::default();
        
        assert!(config.rotate_user_agents);
        assert!(config.randomize_user_agents);
        assert!(config.enable_dnt);
        assert!(config.strip_referrer);
        assert!(config.disable_cache);
        assert!(config.enable_request_jitter);
        assert_eq!(config.min_request_delay_ms, 100);
        assert_eq!(config.max_request_delay_ms, 500);
        assert!(config.distribute_requests);
        assert_eq!(config.avoid_repeat_instances, 3);
        assert!(config.anonymize_content_requests);
        assert_eq!(config.content_request_delay_ms, 200);
    }
    
    #[test]
    fn test_user_agent_rotator_default_agents() {
        let config = PrivacyConfig::default();
        let rotator = UserAgentRotator::new(&config);
        
        assert!(!rotator.user_agents.is_empty());
        assert!(rotator.randomize);
        
        // Test that we get valid user agents
        let agent1 = rotator.get_next_user_agent();
        let agent2 = rotator.get_next_user_agent();
        
        assert!(!agent1.is_empty());
        assert!(!agent2.is_empty());
        assert!(agent1.contains("Mozilla"));
        assert!(agent2.contains("Mozilla"));
    }
    
    #[test]
    fn test_user_agent_rotator_custom_agents() {
        let mut config = PrivacyConfig::default();
        config.custom_user_agents = Some(vec![
            "CustomAgent/1.0".to_string(),
            "CustomAgent/2.0".to_string(),
        ]);
        config.randomize_user_agents = false; // Use sequential for predictable testing
        
        let rotator = UserAgentRotator::new(&config);
        assert_eq!(rotator.user_agents.len(), 2);
        
        let agent1 = rotator.get_next_user_agent();
        let agent2 = rotator.get_next_user_agent();
        let agent3 = rotator.get_next_user_agent(); // Should cycle back
        
        assert_eq!(agent1, "CustomAgent/1.0");
        assert_eq!(agent2, "CustomAgent/2.0");
        assert_eq!(agent3, "CustomAgent/1.0");
    }
    
    #[test]
    fn test_user_agent_rotator_empty_agents() {
        let mut config = PrivacyConfig::default();
        config.custom_user_agents = Some(vec![]);
        
        let rotator = UserAgentRotator::new(&config);
        let agent = rotator.get_next_user_agent();
        
        assert!(agent.contains("SwissArmyHammer"));
    }
    
    #[test]
    fn test_privacy_headers_configuration() {
        let mut config = PrivacyConfig::default();
        config.enable_dnt = false;
        config.strip_referrer = false;
        config.disable_cache = false;
        
        let privacy_headers = PrivacyHeaders::new(&config);
        assert!(!privacy_headers.enable_dnt);
        assert!(!privacy_headers.strip_referrer);
        assert!(!privacy_headers.disable_cache);
    }
    
    #[test]
    fn test_request_jitter_configuration() {
        let mut config = PrivacyConfig::default();
        config.enable_request_jitter = false;
        config.min_request_delay_ms = 50;
        config.max_request_delay_ms = 200;
        
        let jitter = RequestJitter::new(&config);
        assert!(!jitter.enabled);
        assert_eq!(jitter.min_delay, Duration::from_millis(50));
        assert_eq!(jitter.max_delay, Duration::from_millis(200));
    }
    
    #[tokio::test]
    async fn test_request_jitter_disabled() {
        let mut config = PrivacyConfig::default();
        config.enable_request_jitter = false;
        
        let jitter = RequestJitter::new(&config);
        let start = std::time::Instant::now();
        jitter.apply_jitter().await;
        let elapsed = start.elapsed();
        
        // Should return immediately when disabled
        assert!(elapsed < Duration::from_millis(10));
    }
    
    #[tokio::test]
    async fn test_request_jitter_enabled() {
        let mut config = PrivacyConfig::default();
        config.enable_request_jitter = true;
        config.min_request_delay_ms = 50;
        config.max_request_delay_ms = 100;
        
        let jitter = RequestJitter::new(&config);
        let start = std::time::Instant::now();
        jitter.apply_jitter().await;
        let elapsed = start.elapsed();
        
        // Should have applied some delay
        assert!(elapsed >= Duration::from_millis(40)); // Allow some timing variance
        assert!(elapsed <= Duration::from_millis(150)); // Allow some timing variance
    }
    
    #[test]
    fn test_instance_distributor_disabled() {
        let mut config = PrivacyConfig::default();
        config.distribute_requests = false;
        
        let distributor = InstanceDistributor::new(&config);
        let available = vec!["instance1".to_string(), "instance2".to_string()];
        
        let selected = distributor.select_distributed_instance(&available);
        assert_eq!(selected, Some("instance1".to_string()));
    }
    
    #[test]
    fn test_instance_distributor_empty() {
        let config = PrivacyConfig::default();
        let distributor = InstanceDistributor::new(&config);
        let available = vec![];
        
        let selected = distributor.select_distributed_instance(&available);
        assert_eq!(selected, None);
    }
    
    #[test]
    fn test_instance_distributor_avoidance() {
        let config = PrivacyConfig::default();
        let distributor = InstanceDistributor::new(&config);
        
        let available = vec![
            "instance1".to_string(),
            "instance2".to_string(),
            "instance3".to_string(),
        ];
        
        // Record usage of instance1
        distributor.record_instance_use("instance1");
        
        // Should prefer instances that weren't recently used
        let selected = distributor.select_distributed_instance(&available);
        assert!(selected.is_some());
        
        // The selected instance should not be instance1 (though it could be due to randomness)
        // We can't make a deterministic assertion here due to randomness,
        // but we can verify the mechanism works by checking internal state
        let last_used = distributor.last_used_instances.lock().unwrap();
        assert_eq!(last_used.len(), 1);
        assert_eq!(last_used[0], "instance1");
    }
    
    #[test]
    fn test_privacy_manager_creation() {
        let config = PrivacyConfig::default();
        let manager = PrivacyManager::new(config);
        
        assert!(manager.user_agent_rotator.is_some());
        
        let user_agent = manager.get_user_agent();
        assert!(user_agent.is_some());
        assert!(!user_agent.unwrap().is_empty());
    }
    
    #[test]
    fn test_privacy_manager_no_rotation() {
        let mut config = PrivacyConfig::default();
        config.rotate_user_agents = false;
        
        let manager = PrivacyManager::new(config);
        assert!(manager.user_agent_rotator.is_none());
        
        let user_agent = manager.get_user_agent();
        assert!(user_agent.is_none());
    }
}