# WEB_SEARCH_000005: Content Fetching with markdowndown

Refer to /Users/wballard/github/sah-search/ideas/web_search.md

## Overview
Integrate markdowndown for fetching and processing content from search result URLs, with concurrent processing and rate limiting.

## Goals
- Integrate with existing markdowndown crate for content fetching
- Implement concurrent content fetching with proper rate limiting
- Add content quality assessment and filtering
- Generate content summaries and extract key information
- Handle various content types and edge cases gracefully

## Tasks
1. **markdowndown Integration**: Use existing markdowndown client for content fetching
2. **Concurrent Processing**: Implement semaphore-based concurrent fetching
3. **Rate Limiting**: Add per-domain rate limiting and backpressure control
4. **Content Quality**: Filter low-quality, paywall, or spam content
5. **Content Processing**: Extract summaries, key points, and metadata

## Implementation Details

### Content Fetcher Integration
```rust
use markdowndown::MarkdownDownClient;

pub struct ContentFetcher {
    markdowndown_client: MarkdownDownClient,
    semaphore: Arc<Semaphore>,
    rate_limiter: DomainRateLimiter,
    quality_filter: ContentQualityFilter,
}

impl ContentFetcher {
    pub async fn fetch_search_results(
        &self,
        results: Vec<SearchResult>,
        max_concurrent: usize,
    ) -> Vec<ProcessedResult> {
        // Use semaphore to limit concurrent requests
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        
        let tasks: Vec<_> = results
            .into_iter()
            .map(|result| {
                let semaphore = semaphore.clone();
                let fetcher = self.clone();
                tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    fetcher.fetch_single_result(result).await
                })
            })
            .collect();
        
        // Collect results, handling failures gracefully
        let mut processed_results = Vec::new();
        for task in tasks {
            if let Ok(Ok(result)) = task.await {
                processed_results.push(result);
            }
        }
        
        processed_results
    }
    
    async fn fetch_single_result(&self, result: SearchResult) -> Result<ProcessedResult> {
        // Apply rate limiting per domain
        self.rate_limiter.wait_for_domain(&result.url).await?;
        
        // Fetch content using markdowndown
        let content = self.markdowndown_client
            .fetch_and_convert(&result.url)
            .await?;
        
        // Assess content quality
        if !self.quality_filter.is_quality_content(&content) {
            return Err(ContentError::LowQuality);
        }
        
        // Process and summarize content
        let processed_content = self.process_content(content, &result).await?;
        
        Ok(ProcessedResult {
            original_result: result,
            content: Some(processed_content),
            fetch_time_ms: /* ... */,
            fetch_status: FetchStatus::Success,
        })
    }
}
```

### Domain Rate Limiting
```rust
pub struct DomainRateLimiter {
    domain_trackers: Arc<DashMap<String, RateLimitState>>,
    default_delay: Duration,
    respect_robots_txt: bool,
}

struct RateLimitState {
    last_request: Instant,
    delay: Duration,
    consecutive_requests: u32,
}

impl DomainRateLimiter {
    pub async fn wait_for_domain(&self, url: &str) -> Result<()> {
        let domain = extract_domain(url)?;
        
        let mut state = self.domain_trackers
            .entry(domain.clone())
            .or_insert_with(|| RateLimitState {
                last_request: Instant::now() - Duration::from_secs(60),
                delay: self.default_delay,
                consecutive_requests: 0,
            });
        
        // Calculate required delay
        let elapsed = state.last_request.elapsed();
        if elapsed < state.delay {
            let wait_time = state.delay - elapsed;
            tokio::time::sleep(wait_time).await;
        }
        
        // Update state
        state.last_request = Instant::now();
        state.consecutive_requests += 1;
        
        // Increase delay for frequent requests to same domain
        if state.consecutive_requests > 5 {
            state.delay = (state.delay * 2).min(Duration::from_secs(30));
        }
        
        Ok(())
    }
}
```

### Content Quality Assessment
```rust
pub struct ContentQualityFilter {
    min_content_length: usize,
    max_content_length: usize,
    spam_indicators: Vec<String>,
    paywall_indicators: Vec<String>,
}

impl ContentQualityFilter {
    pub fn is_quality_content(&self, content: &ProcessedContent) -> bool {
        // Check content length
        if content.word_count < self.min_content_length 
            || content.word_count > self.max_content_length {
            return false;
        }
        
        // Check for spam indicators
        let content_lower = content.markdown.to_lowercase();
        for indicator in &self.spam_indicators {
            if content_lower.contains(indicator) {
                return false;
            }
        }
        
        // Check for paywall indicators
        for indicator in &self.paywall_indicators {
            if content_lower.contains(indicator) {
                return false;
            }
        }
        
        // Additional quality checks
        self.check_content_structure(content)
            && self.check_information_density(content)
    }
    
    fn check_content_structure(&self, content: &ProcessedContent) -> bool {
        // Check for reasonable text structure
        // Ensure it's not just navigation/boilerplate
        // Look for meaningful paragraphs and sentences
        true // Placeholder implementation
    }
    
    fn check_information_density(&self, content: &ProcessedContent) -> bool {
        // Calculate information density metrics
        // Ratio of meaningful words to total words
        // Presence of technical terms or specific information
        true // Placeholder implementation
    }
}
```

### Content Processing and Summarization
```rust
#[derive(Debug, Clone)]
pub struct ProcessedContent {
    pub markdown: String,
    pub word_count: usize,
    pub summary: Option<String>,
    pub key_points: Vec<String>,
    pub code_blocks: Vec<CodeBlock>,
    pub metadata: ContentMetadata,
}

#[derive(Debug, Clone)]
pub struct ContentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub published_date: Option<String>,
    pub content_type: ContentType,
    pub language: Option<String>,
}

impl ContentFetcher {
    async fn process_content(&self, raw_content: String, result: &SearchResult) -> Result<ProcessedContent> {
        let word_count = count_words(&raw_content);
        
        // Generate summary for long content
        let summary = if word_count > 1000 {
            Some(self.generate_summary(&raw_content, 500).await?)
        } else {
            None
        };
        
        // Extract code blocks if present
        let code_blocks = self.extract_code_blocks(&raw_content);
        
        // Extract key points and metadata
        let key_points = self.extract_key_points(&raw_content).await?;
        let metadata = self.extract_metadata(&raw_content, result).await?;
        
        Ok(ProcessedContent {
            markdown: raw_content,
            word_count,
            summary,
            key_points,
            code_blocks,
            metadata,
        })
    }
    
    async fn generate_summary(&self, content: &str, max_length: usize) -> Result<String> {
        // Simple extractive summarization
        // Take first few sentences up to max_length
        // Could be enhanced with more sophisticated summarization later
        let sentences: Vec<&str> = content.split('.').collect();
        let mut summary = String::new();
        
        for sentence in sentences {
            if summary.len() + sentence.len() > max_length {
                break;
            }
            if !sentence.trim().is_empty() {
                summary.push_str(sentence.trim());
                summary.push('.');
                summary.push(' ');
            }
        }
        
        Ok(summary.trim().to_string())
    }
}
```

## Success Criteria
- [x] Successfully integrates with existing markdowndown client
- [x] Implements concurrent fetching with proper rate limiting
- [x] Correctly filters low-quality and spam content
- [x] Generates useful summaries for long content
- [x] Handles fetch failures gracefully without breaking search
- [x] Respects rate limits and doesn't overload target servers

## Testing Strategy
- Mock markdowndown client for controlled content responses
- Concurrent processing tests with various result counts
- Rate limiting tests with same-domain URLs
- Content quality filter tests with spam/paywall samples
- Summarization tests with long content samples
- Error handling tests for network failures and timeouts

## Integration Points
- Uses existing markdowndown client and configuration
- Integrates with search results from previous steps
- Adds content fetching option to MCP tool parameters
- Updates response format to include fetched content
- Follows existing async patterns and error handling

## Configuration Options
```toml
[web_search.content_fetching]
# Concurrent processing
max_concurrent_fetches = 5
content_fetch_timeout = 45     # seconds per URL
max_content_size = "2MB"       # per result

# Rate limiting
default_domain_delay = 1000    # milliseconds between requests
respect_robots_txt = true
max_domain_requests_per_minute = 30

# Content quality
min_content_length = 100       # minimum word count
max_content_length = 50000     # maximum word count  
max_summary_length = 500       # characters

# Content processing
extract_code_blocks = true
generate_summaries = true
extract_metadata = true
```

## Error Handling
- Network timeouts during content fetching (skip result with error logged)
- Content size limits exceeded (truncate or skip with warning)
- Malformed HTML/content (use markdowndown error handling)
- Rate limiting violations (exponential backoff and retry)
- Domain blocking or 403 errors (skip with appropriate error message)

## Sample Response Enhancement
With content fetching enabled, search results now include:
```json
{
  "title": "Async Programming in Rust - The Rust Book",
  "url": "https://doc.rust-lang.org/book/ch16-00-concurrency.html",
  "description": "Learn about asynchronous programming in Rust...",
  "score": 0.95,
  "engine": "duckduckgo",
  "content": {
    "markdown": "# Async Programming in Rust\n\nRust's approach to async programming...",
    "word_count": 2840,
    "fetch_time_ms": 340,
    "summary": "Comprehensive guide to async programming concepts in Rust including futures, async/await, and runtime considerations."
  }
}
```