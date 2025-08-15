# WEB_SEARCH_000004: SearXNG Instance Discovery

Refer to /Users/wballard/github/sah-search/ideas/web_search.md

## Overview
Implement dynamic discovery and management of SearXNG instances using the searx.space API for high-availability and distributed searching.

## Goals
- Discover high-quality SearXNG instances from searx.space API
- Filter instances by quality grade (A+, A, B ratings)
- Implement instance health checking and monitoring
- Create instance selection and rotation logic
- Handle instance failures and automatic failover

## Tasks
1. **searx.space API Client**: Query searx.space for instance list
2. **Instance Filtering**: Filter by grade, uptime, and API availability
3. **Health Monitoring**: Periodic health checks for discovered instances
4. **Instance Manager**: Central management of instance pool
5. **Selection Strategy**: Implement round-robin and weighted selection

## Implementation Details

### searx.space API Integration
```rust
#[derive(Debug, Deserialize)]
struct SearxSpaceResponse {
    instances: HashMap<String, InstanceInfo>,
}

#[derive(Debug, Deserialize)]
struct InstanceInfo {
    grade: Option<String>,
    uptime: Option<f32>,
    response_time: Option<u64>,
    api: Option<bool>,
    // ... other fields
}

pub struct InstanceDiscovery {
    http_client: reqwest::Client,
    discovery_url: String,
}

impl InstanceDiscovery {
    pub async fn discover_instances(&self) -> Result<Vec<SearxInstance>> {
        let response: SearxSpaceResponse = self.http_client
            .get(&self.discovery_url)
            .send()
            .await?
            .json()
            .await?;
        
        // Filter and convert to our instance type
        Ok(self.filter_quality_instances(response.instances))
    }
}
```

### SearXNG Instance Management
```rust
#[derive(Debug, Clone)]
pub struct SearxInstance {
    pub url: String,
    pub grade: String,          // A+, A, B, etc.
    pub uptime: f32,            // Percentage uptime
    pub response_time: u64,     // Average response time in ms
    pub last_checked: DateTime<Utc>,
    pub rate_limited_until: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub is_healthy: bool,
}

pub struct InstanceManager {
    instances: Vec<SearxInstance>,
    current_index: AtomicUsize,
    last_discovery: DateTime<Utc>,
    discovery_interval: Duration,
}

impl InstanceManager {
    pub async fn get_next_instance(&self) -> Option<&SearxInstance> {
        // Round-robin selection of healthy instances
        // Skip rate-limited or unhealthy instances  
        // Return None if no instances available
    }
    
    pub async fn mark_instance_failed(&self, url: &str) {
        // Increment failure count
        // Mark as unhealthy if too many failures
        // Set rate limit if appropriate
    }
    
    pub async fn refresh_instances(&mut self) -> Result<()> {
        // Periodic refresh from searx.space
        // Update instance health status
        // Remove consistently failing instances
    }
}
```

### Instance Health Checking
```rust
pub struct HealthChecker {
    http_client: reqwest::Client,
    check_timeout: Duration,
}

impl HealthChecker {
    pub async fn check_instance_health(&self, instance: &SearxInstance) -> HealthStatus {
        // Perform lightweight health check
        // Test basic connectivity and API availability
        // Measure response time
        // Return health status with metrics
    }
    
    pub async fn bulk_health_check(&self, instances: &mut [SearxInstance]) {
        // Concurrent health checks for all instances
        // Update health status in place
        // Handle timeouts and failures gracefully
    }
}

#[derive(Debug)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub response_time: Duration,
    pub error: Option<String>,
}
```

### Instance Selection Strategy
```rust
pub enum SelectionStrategy {
    RoundRobin,
    WeightedByGrade,
    WeightedByResponseTime,
    Random,
}

impl InstanceManager {
    pub fn with_selection_strategy(strategy: SelectionStrategy) -> Self {
        // Configure selection strategy
    }
    
    fn select_by_grade(&self) -> Option<&SearxInstance> {
        // Prefer A+ > A > B grade instances
        // Within same grade, use round-robin
    }
    
    fn select_by_response_time(&self) -> Option<&SearxInstance> {
        // Prefer faster responding instances
        // Weight by inverse response time
    }
}
```

## Success Criteria
- [x] Successfully discovers instances from searx.space API
- [x] Properly filters instances by quality grades and API availability
- [x] Health checking accurately identifies working instances
- [x] Instance manager handles rotation and failover correctly
- [x] Failed instances are automatically excluded from rotation
- [x] Periodic refresh updates instance list and health status

## Testing Strategy
- Mock searx.space API responses for discovery testing
- Health check tests with mock HTTP responses
- Instance rotation tests with multiple mock instances
- Failover tests simulating instance failures
- Performance tests for concurrent health checking

## Integration Points
- Updates SearXNG client from WEB_SEARCH_000002 to use instance manager
- Integrates with existing error handling for network failures
- Prepares for load balancing improvements in future steps
- Uses existing async patterns and HTTP client configuration

## Configuration Options
```toml
[web_search.discovery]
# searx.space API endpoint
discovery_url = "https://searx.space/data/instances.json"

# Instance filtering criteria
min_grade = "B"              # Minimum quality grade (A+, A, B)
min_uptime = 90.0            # Minimum uptime percentage
max_response_time = 5000     # Maximum response time in ms
require_api = true           # Require API availability

# Health checking
health_check_interval = 300   # seconds
health_check_timeout = 10     # seconds
max_consecutive_failures = 3  # failures before marking unhealthy

# Instance management
discovery_refresh_interval = 3600  # seconds (1 hour)
selection_strategy = "weighted_by_grade"
```

## Error Handling
- Network failures during discovery (fallback to cached instances)
- Invalid or malformed searx.space responses (log and continue)
- All instances unhealthy (clear error message with suggestions)
- Health check timeouts (mark as temporarily unhealthy)
- Rate limiting detection (respect and track limits)

## Sample Usage
```rust
let mut manager = InstanceManager::new(SelectionStrategy::WeightedByGrade).await?;
manager.refresh_instances().await?;

if let Some(instance) = manager.get_next_instance().await {
    match client.search_with_instance(instance, &request).await {
        Ok(results) => return Ok(results),
        Err(e) => {
            manager.mark_instance_failed(&instance.url).await;
            // Try next instance...
        }
    }
}
```