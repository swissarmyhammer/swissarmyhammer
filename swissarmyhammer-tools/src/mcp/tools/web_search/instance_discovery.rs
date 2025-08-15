//! SearXNG instance discovery and management
//!
//! This module provides functionality to discover high-quality SearXNG instances from the
//! searx.space API and manage their health status for distributed web search operations.

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Represents a SearXNG instance with health and quality information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearxInstance {
    /// Instance URL (e.g., "https://search.example.org")
    pub url: String,
    
    /// Quality grade from searx.space (A+, A, B, etc.)
    pub grade: String,
    
    /// Uptime percentage (0.0 to 100.0)
    pub uptime: f32,
    
    /// Average response time in milliseconds
    pub response_time: u64,
    
    /// Timestamp when this instance was last health checked
    pub last_checked: DateTime<Utc>,
    
    /// Timestamp until when this instance is rate limited (if any)
    pub rate_limited_until: Option<DateTime<Utc>>,
    
    /// Number of consecutive failures for this instance
    pub consecutive_failures: u32,
    
    /// Whether the instance is currently considered healthy
    pub is_healthy: bool,
}

impl SearxInstance {
    /// Creates a new SearxInstance from searx.space data
    pub fn new(
        url: String,
        grade: String,
        uptime: f32,
        response_time: u64,
    ) -> Self {
        Self {
            url,
            grade,
            uptime,
            response_time,
            last_checked: Utc::now(),
            rate_limited_until: None,
            consecutive_failures: 0,
            is_healthy: true,
        }
    }

    /// Checks if the instance is currently rate limited
    pub fn is_rate_limited(&self) -> bool {
        match self.rate_limited_until {
            Some(until) => Utc::now() < until,
            None => false,
        }
    }

    /// Marks this instance as having failed a health check
    pub fn mark_failed(&mut self) {
        self.consecutive_failures += 1;
        self.last_checked = Utc::now();
        
        if self.consecutive_failures >= 3 {
            self.is_healthy = false;
        }
    }

    /// Marks this instance as healthy after successful health check
    pub fn mark_healthy(&mut self, response_time: Duration) {
        self.consecutive_failures = 0;
        self.is_healthy = true;
        self.response_time = response_time.as_millis() as u64;
        self.last_checked = Utc::now();
    }

    /// Sets rate limit for this instance
    pub fn set_rate_limited(&mut self, duration: Duration) {
        self.rate_limited_until = Some(Utc::now() + chrono::Duration::from_std(duration).unwrap_or_default());
    }

    /// Gets the quality score for this instance (higher is better)
    /// A+ = 4, A = 3, B = 2, C = 1, D = 0
    pub fn quality_score(&self) -> u8 {
        match self.grade.as_str() {
            "A+" => 4,
            "A" => 3,
            "B" => 2,
            "C" => 1,
            "D" => 0,
            _ => 0,
        }
    }
}

/// Health status result from checking an instance
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// Whether the instance is healthy
    pub is_healthy: bool,
    
    /// Response time for the health check
    pub response_time: Duration,
    
    /// Error message if health check failed
    pub error: Option<String>,
}

/// Response from searx.space API
#[derive(Debug, Deserialize)]
struct SearxSpaceResponse {
    instances: HashMap<String, InstanceInfo>,
}

/// Instance information from searx.space API
#[derive(Debug, Deserialize)]
struct InstanceInfo {
    grade: Option<String>,
    uptime: Option<f32>,
    response_time: Option<u64>,
    api: Option<bool>,
    #[serde(rename = "type")]
    instance_type: Option<String>,
}

/// Client for discovering SearXNG instances from searx.space API
#[derive(Debug)]
pub struct InstanceDiscovery {
    http_client: Client,
    discovery_url: String,
}

impl InstanceDiscovery {
    /// Creates a new InstanceDiscovery client
    pub fn new() -> Self {
        Self {
            http_client: Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("SwissArmyHammer/1.0 (SearXNG Instance Discovery)")
                .build()
                .unwrap_or_else(|_| Client::new()),
            discovery_url: "https://searx.space/data/instances.json".to_string(),
        }
    }

    /// Creates a new InstanceDiscovery client with custom settings
    pub fn with_config(discovery_url: String, timeout: Duration) -> Self {
        Self {
            http_client: Client::builder()
                .timeout(timeout)
                .user_agent("SwissArmyHammer/1.0 (SearXNG Instance Discovery)")
                .build()
                .unwrap_or_else(|_| Client::new()),
            discovery_url,
        }
    }

    /// Discovers SearXNG instances from searx.space API
    pub async fn discover_instances(&self) -> Result<Vec<SearxInstance>, Box<dyn std::error::Error + Send + Sync>> {
        debug!("Fetching instance data from {}", self.discovery_url);
        
        let start_time = Instant::now();
        let response = self
            .http_client
            .get(&self.discovery_url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!(
                "searx.space API returned status: {}",
                response.status()
            ).into());
        }

        let searx_response: SearxSpaceResponse = response.json().await?;
        let fetch_time = start_time.elapsed();
        
        debug!("Fetched {} instances in {:?}", searx_response.instances.len(), fetch_time);
        
        let instances = self.filter_quality_instances(searx_response.instances);
        debug!("Filtered to {} high-quality instances", instances.len());
        
        Ok(instances)
    }

    /// Filters instances based on quality criteria
    fn filter_quality_instances(&self, instances: HashMap<String, InstanceInfo>) -> Vec<SearxInstance> {
        let mut quality_instances = Vec::new();
        
        for (url, info) in instances {
            // Skip instances without required fields
            let Some(grade) = info.grade else { continue; };
            let Some(uptime) = info.uptime else { continue; };
            let Some(response_time) = info.response_time else { continue; };
            let Some(has_api) = info.api else { continue; };
            
            // Skip instances without API support
            if !has_api {
                continue;
            }
            
            // Skip instances that are not public SearXNG instances
            if let Some(instance_type) = &info.instance_type {
                if instance_type != "searxng" {
                    continue;
                }
            }
            
            // Apply quality filters
            if !self.meets_quality_criteria(&grade, uptime, response_time) {
                continue;
            }
            
            // Ensure URL is valid and has HTTPS
            if !url.starts_with("https://") {
                warn!("Skipping non-HTTPS instance: {}", url);
                continue;
            }
            
            quality_instances.push(SearxInstance::new(
                url,
                grade,
                uptime,
                response_time,
            ));
        }
        
        // Sort by quality score (highest first), then by response time (lowest first)
        quality_instances.sort_by(|a, b| {
            let quality_cmp = b.quality_score().cmp(&a.quality_score());
            if quality_cmp == std::cmp::Ordering::Equal {
                a.response_time.cmp(&b.response_time)
            } else {
                quality_cmp
            }
        });
        
        quality_instances
    }
    
    /// Checks if an instance meets our quality criteria
    fn meets_quality_criteria(&self, grade: &str, uptime: f32, response_time: u64) -> bool {
        // Only accept A+, A, and B grade instances
        let acceptable_grades = ["A+", "A", "B"];
        if !acceptable_grades.contains(&grade) {
            return false;
        }
        
        // Minimum uptime requirement
        if uptime < 90.0 {
            return false;
        }
        
        // Maximum response time requirement (5 seconds)
        if response_time > 5000 {
            return false;
        }
        
        true
    }
}

impl Default for InstanceDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_searx_instance_new() {
        let instance = SearxInstance::new(
            "https://search.example.org".to_string(),
            "A+".to_string(),
            95.5,
            1200,
        );
        
        assert_eq!(instance.url, "https://search.example.org");
        assert_eq!(instance.grade, "A+");
        assert_eq!(instance.uptime, 95.5);
        assert_eq!(instance.response_time, 1200);
        assert_eq!(instance.consecutive_failures, 0);
        assert!(instance.is_healthy);
        assert!(!instance.is_rate_limited());
    }

    #[test]
    fn test_searx_instance_quality_score() {
        let instances = [
            SearxInstance::new("https://a.com".to_string(), "A+".to_string(), 95.0, 1000),
            SearxInstance::new("https://b.com".to_string(), "A".to_string(), 95.0, 1000),
            SearxInstance::new("https://c.com".to_string(), "B".to_string(), 95.0, 1000),
            SearxInstance::new("https://d.com".to_string(), "C".to_string(), 95.0, 1000),
            SearxInstance::new("https://e.com".to_string(), "D".to_string(), 95.0, 1000),
        ];
        
        assert_eq!(instances[0].quality_score(), 4); // A+
        assert_eq!(instances[1].quality_score(), 3); // A
        assert_eq!(instances[2].quality_score(), 2); // B
        assert_eq!(instances[3].quality_score(), 1); // C
        assert_eq!(instances[4].quality_score(), 0); // D
    }

    #[test]
    fn test_searx_instance_mark_failed() {
        let mut instance = SearxInstance::new(
            "https://search.example.org".to_string(),
            "A+".to_string(),
            95.5,
            1200,
        );
        
        assert_eq!(instance.consecutive_failures, 0);
        assert!(instance.is_healthy);
        
        // First failure
        instance.mark_failed();
        assert_eq!(instance.consecutive_failures, 1);
        assert!(instance.is_healthy);
        
        // Second failure
        instance.mark_failed();
        assert_eq!(instance.consecutive_failures, 2);
        assert!(instance.is_healthy);
        
        // Third failure should mark as unhealthy
        instance.mark_failed();
        assert_eq!(instance.consecutive_failures, 3);
        assert!(!instance.is_healthy);
    }

    #[test]
    fn test_searx_instance_mark_healthy() {
        let mut instance = SearxInstance::new(
            "https://search.example.org".to_string(),
            "A+".to_string(),
            95.5,
            1200,
        );
        
        // Mark as failed multiple times
        instance.mark_failed();
        instance.mark_failed();
        instance.mark_failed();
        assert!(!instance.is_healthy);
        assert_eq!(instance.consecutive_failures, 3);
        
        // Mark as healthy should reset everything
        let response_time = Duration::from_millis(800);
        instance.mark_healthy(response_time);
        
        assert!(instance.is_healthy);
        assert_eq!(instance.consecutive_failures, 0);
        assert_eq!(instance.response_time, 800);
    }

    #[test]
    fn test_searx_instance_rate_limiting() {
        let mut instance = SearxInstance::new(
            "https://search.example.org".to_string(),
            "A+".to_string(),
            95.5,
            1200,
        );
        
        assert!(!instance.is_rate_limited());
        
        // Set rate limit for 5 minutes
        instance.set_rate_limited(Duration::from_secs(300));
        assert!(instance.is_rate_limited());
    }

    #[test]
    fn test_instance_discovery_new() {
        let discovery = InstanceDiscovery::new();
        assert_eq!(discovery.discovery_url, "https://searx.space/data/instances.json");
    }

    #[test]
    fn test_instance_discovery_with_config() {
        let custom_url = "https://custom.searx.space/data/instances.json".to_string();
        let timeout = Duration::from_secs(60);
        
        let discovery = InstanceDiscovery::with_config(custom_url.clone(), timeout);
        assert_eq!(discovery.discovery_url, custom_url);
    }

    #[test]
    fn test_meets_quality_criteria() {
        let discovery = InstanceDiscovery::new();
        
        // Should pass - A+ grade, good uptime, fast response
        assert!(discovery.meets_quality_criteria("A+", 98.5, 1200));
        
        // Should pass - A grade, minimum uptime, acceptable response time
        assert!(discovery.meets_quality_criteria("A", 90.0, 5000));
        
        // Should pass - B grade, good stats
        assert!(discovery.meets_quality_criteria("B", 95.0, 2000));
        
        // Should fail - C grade (not acceptable)
        assert!(!discovery.meets_quality_criteria("C", 95.0, 1000));
        
        // Should fail - low uptime
        assert!(!discovery.meets_quality_criteria("A+", 85.0, 1000));
        
        // Should fail - slow response time
        assert!(!discovery.meets_quality_criteria("A+", 95.0, 6000));
    }

    #[test]
    fn test_filter_quality_instances() {
        let discovery = InstanceDiscovery::new();
        let mut instances = HashMap::new();
        
        // High quality instance
        instances.insert("https://good.example.org".to_string(), InstanceInfo {
            grade: Some("A+".to_string()),
            uptime: Some(98.5),
            response_time: Some(1200),
            api: Some(true),
            instance_type: Some("searxng".to_string()),
        });
        
        // Good quality instance
        instances.insert("https://decent.example.org".to_string(), InstanceInfo {
            grade: Some("A".to_string()),
            uptime: Some(92.0),
            response_time: Some(2000),
            api: Some(true),
            instance_type: Some("searxng".to_string()),
        });
        
        // Poor quality instance (should be filtered out)
        instances.insert("https://bad.example.org".to_string(), InstanceInfo {
            grade: Some("D".to_string()),
            uptime: Some(60.0),
            response_time: Some(8000),
            api: Some(true),
            instance_type: Some("searxng".to_string()),
        });
        
        // No API support (should be filtered out)
        instances.insert("https://noapi.example.org".to_string(), InstanceInfo {
            grade: Some("A+".to_string()),
            uptime: Some(98.0),
            response_time: Some(1000),
            api: Some(false),
            instance_type: Some("searxng".to_string()),
        });
        
        // HTTP instance (should be filtered out)
        instances.insert("http://insecure.example.org".to_string(), InstanceInfo {
            grade: Some("A+".to_string()),
            uptime: Some(98.0),
            response_time: Some(1000),
            api: Some(true),
            instance_type: Some("searxng".to_string()),
        });
        
        let filtered = discovery.filter_quality_instances(instances);
        
        // Should only have the 2 good instances
        assert_eq!(filtered.len(), 2);
        
        // Should be sorted by quality score (A+ first, then A)
        assert_eq!(filtered[0].url, "https://good.example.org");
        assert_eq!(filtered[0].grade, "A+");
        assert_eq!(filtered[1].url, "https://decent.example.org");
        assert_eq!(filtered[1].grade, "A");
    }

    #[test]
    fn test_health_status() {
        let healthy_status = HealthStatus {
            is_healthy: true,
            response_time: Duration::from_millis(500),
            error: None,
        };
        
        assert!(healthy_status.is_healthy);
        assert_eq!(healthy_status.response_time, Duration::from_millis(500));
        assert!(healthy_status.error.is_none());
        
        let unhealthy_status = HealthStatus {
            is_healthy: false,
            response_time: Duration::from_millis(0),
            error: Some("Connection timeout".to_string()),
        };
        
        assert!(!unhealthy_status.is_healthy);
        assert!(unhealthy_status.error.is_some());
    }
}