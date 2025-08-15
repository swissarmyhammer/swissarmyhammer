# WEB_SEARCH_000006: Privacy and Request Anonymization

Refer to /Users/wballard/github/sah-search/ideas/web_search.md

## Overview
Implement privacy protection features including User-Agent rotation, request anonymization, and tracking prevention for web searches.

## Goals
- Rotate User-Agent strings to avoid fingerprinting
- Distribute requests across multiple SearXNG instances
- Add random delays and request jitter to avoid detection
- Strip identifying headers and metadata from requests
- Implement privacy-focused configuration options

## Tasks
1. **User-Agent Rotation**: Implement rotating User-Agent strings from common browsers
2. **Request Anonymization**: Strip identifying headers and add privacy headers
3. **Instance Distribution**: Smart distribution of requests across instances
4. **Request Timing**: Add randomized delays between requests
5. **Privacy Configuration**: Make privacy features configurable

## Implementation Details

### User-Agent Rotation
```rust
pub struct UserAgentRotator {
    user_agents: Vec<String>,
    current_index: AtomicUsize,
    randomize: bool,
}

impl UserAgentRotator {
    pub fn new() -> Self {
        Self {
            user_agents: Self::default_user_agents(),
            current_index: AtomicUsize::new(0),
            randomize: true,
        }
    }
    
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
    
    pub fn get_next_user_agent(&self) -> String {
        if self.randomize {
            let mut rng = rand::thread_rng();
            let index = rng.gen_range(0..self.user_agents.len());
            self.user_agents[index].clone()
        } else {
            let index = self.current_index.fetch_add(1, Ordering::Relaxed) % self.user_agents.len();
            self.user_agents[index].clone()
        }
    }
}
```

### Request Anonymization
```rust
pub struct PrivacyHeaders {
    enable_dnt: bool,           // Do Not Track
    strip_referrer: bool,       // Remove referrer header
    disable_cache: bool,        // Prevent caching
}

impl PrivacyHeaders {
    pub fn apply_privacy_headers(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut request = request;
        
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
        
        // Remove potentially identifying headers
        request = request.header("Accept-Language", "en-US,en;q=0.9");
        request = request.header("Accept-Encoding", "gzip, deflate, br");
        request = request.header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8");
        
        request
    }
}
```

### Request Distribution and Timing
```rust
pub struct PrivacyManager {
    user_agent_rotator: UserAgentRotator,
    privacy_headers: PrivacyHeaders,
    request_jitter: RequestJitter,
    instance_distributor: InstanceDistributor,
}

pub struct RequestJitter {
    min_delay: Duration,
    max_delay: Duration,
    enabled: bool,
}

impl RequestJitter {
    pub async fn apply_jitter(&self) {
        if !self.enabled {
            return;
        }
        
        let mut rng = rand::thread_rng();
        let delay_ms = rng.gen_range(self.min_delay.as_millis()..=self.max_delay.as_millis());
        let delay = Duration::from_millis(delay_ms as u64);
        
        tokio::time::sleep(delay).await;
    }
}

pub struct InstanceDistributor {
    last_used_instances: Arc<Mutex<VecDeque<String>>>,
    avoid_repeat_count: usize,
}

impl InstanceDistributor {
    pub fn select_distributed_instance(&self, available: &[SearxInstance]) -> Option<&SearxInstance> {
        let last_used = self.last_used_instances.lock().unwrap();
        
        // Find an instance that wasn't recently used
        let suitable_instances: Vec<_> = available
            .iter()
            .filter(|instance| !last_used.contains(&instance.url))
            .collect();
        
        if suitable_instances.is_empty() {
            // All instances recently used, pick any healthy one
            available.first()
        } else {
            // Pick random from suitable instances
            let mut rng = rand::thread_rng();
            let index = rng.gen_range(0..suitable_instances.len());
            Some(suitable_instances[index])
        }
    }
    
    pub fn record_instance_use(&self, instance_url: &str) {
        let mut last_used = self.last_used_instances.lock().unwrap();
        
        // Add to front of queue
        last_used.push_front(instance_url.to_string());
        
        // Keep only recent instances
        while last_used.len() > self.avoid_repeat_count {
            last_used.pop_back();
        }
    }
}
```

### Enhanced SearXNG Client with Privacy
```rust
impl SearXngClient {
    pub async fn search_with_privacy(
        &self,
        request: &WebSearchRequest,
        privacy_manager: &PrivacyManager,
    ) -> Result<SearXngResponse> {
        // Apply request jitter
        privacy_manager.request_jitter.apply_jitter().await;
        
        // Select distributed instance
        let instance = privacy_manager
            .instance_distributor
            .select_distributed_instance(&self.available_instances)
            .ok_or(SearchError::NoInstancesAvailable)?;
        
        // Build request with privacy features
        let user_agent = privacy_manager.user_agent_rotator.get_next_user_agent();
        let url = self.build_search_url(instance, request)?;
        
        let http_request = self.http_client
            .get(&url)
            .header("User-Agent", user_agent);
        
        let http_request = privacy_manager
            .privacy_headers
            .apply_privacy_headers(http_request);
        
        // Execute request
        let response = http_request.send().await?;
        
        // Record instance usage for distribution
        privacy_manager
            .instance_distributor
            .record_instance_use(&instance.url);
        
        // Parse response
        let search_response: SearXngResponse = response.json().await?;
        Ok(search_response)
    }
}
```

### Privacy Configuration
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct PrivacyConfig {
    // User-Agent rotation
    pub rotate_user_agents: bool,
    pub randomize_user_agents: bool,
    pub custom_user_agents: Option<Vec<String>>,
    
    // Request anonymization
    pub enable_dnt: bool,
    pub strip_referrer: bool,
    pub disable_cache: bool,
    
    // Request timing
    pub enable_request_jitter: bool,
    pub min_request_delay: u64,  // milliseconds
    pub max_request_delay: u64,  // milliseconds
    
    // Instance distribution
    pub distribute_requests: bool,
    pub avoid_repeat_instances: usize,
    
    // Content fetching privacy
    pub anonymize_content_requests: bool,
    pub content_request_delay: u64,  // milliseconds
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
            min_request_delay: 100,
            max_request_delay: 500,
            distribute_requests: true,
            avoid_repeat_instances: 3,
            anonymize_content_requests: true,
            content_request_delay: 200,
        }
    }
}
```

## Success Criteria
- [x] User-Agent strings rotate properly across requests
- [x] Identifying headers are stripped from all requests
- [x] Request timing includes appropriate jitter and delays
- [x] Requests are distributed across available instances
- [x] Privacy features are configurable and can be disabled
- [x] Content fetching also applies privacy protections

## Testing Strategy
- User-Agent rotation tests ensuring proper distribution
- Header anonymization tests checking for identifying information
- Request timing tests measuring actual delays
- Instance distribution tests verifying selection patterns
- Privacy configuration tests for all options
- Integration tests ensuring privacy doesn't break functionality

## Integration Points
- Integrates with SearXNG client from WEB_SEARCH_000002
- Uses instance management from WEB_SEARCH_000004  
- Applies to content fetching from WEB_SEARCH_000005
- Configurable through existing configuration system
- Maintains compatibility with existing search functionality

## Configuration Options
```toml
[web_search.privacy]
# User-Agent rotation
rotate_user_agents = true
randomize_user_agents = true
# custom_user_agents = ["Custom User Agent String"]

# Request anonymization  
enable_dnt = true           # Do Not Track header
strip_referrer = true       # Remove referrer information
disable_cache = true        # Prevent response caching

# Request timing and jitter
enable_request_jitter = true
min_request_delay = 100     # milliseconds
max_request_delay = 500     # milliseconds

# Instance distribution
distribute_requests = true
avoid_repeat_instances = 3   # number of recent instances to avoid

# Content fetching privacy
anonymize_content_requests = true
content_request_delay = 200  # milliseconds between content requests
```

## Security Considerations
- User-Agent strings should represent real browsers to avoid detection
- Request patterns should appear natural and human-like
- No logging of search queries or personally identifiable information
- Respect robots.txt and website terms of service
- Implement reasonable rate limiting to avoid overloading servers

## Privacy Compliance
- GDPR compliance through data minimization and user control
- No tracking or profiling of search behavior
- No storage of search history or user identification
- Respect Do Not Track preferences
- Clear privacy policy and data handling documentation

## Sample Enhanced Request Flow
```rust
let privacy_manager = PrivacyManager::from_config(&config.privacy)?;
let client = SearXngClient::new()?;

// Privacy-enhanced search
let response = client.search_with_privacy(&request, &privacy_manager).await?;

// Apply privacy to content fetching as well
let content_results = content_fetcher
    .fetch_with_privacy(&response.results, &privacy_manager)
    .await?;
```