# MCP WebSearch Tool Specification

## Overview

This specification defines a new MCP tool `web_search` that enables LLMs to perform web searches using SearXNG metasearch engines. The tool provides privacy-respecting search capabilities with automatic result fetching and content processing for comprehensive AI-assisted research workflows.

## Problem Statement

LLMs often need to search the web for:
1. Current information beyond their training data cutoff
2. Real-time data and recent developments
3. Technical documentation and resources
4. Fact-checking and verification
5. Research and analysis tasks
6. Finding relevant sources and references

Currently, there's no standardized way for MCP tools to perform web searches with privacy protection and structured result processing.

## Solution: MCP WebSearch Tool

### Tool Definition

**Tool Name**: `web_search`  
**Purpose**: Perform web searches using SearXNG and fetch result content  
**Usage Context**: Available to LLMs for research, fact-checking, and information retrieval

### Parameters

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "The search query string",
      "minLength": 1,
      "maxLength": 500
    },
    "category": {
      "type": "string",
      "enum": ["general", "images", "videos", "news", "map", "music", "it", "science", "files"],
      "description": "Search category (optional, defaults to general)",
      "default": "general"
    },
    "language": {
      "type": "string",
      "description": "Search language code (optional, defaults to 'en')",
      "pattern": "^[a-z]{2}(-[A-Z]{2})?$",
      "default": "en"
    },
    "results_count": {
      "type": "integer",
      "description": "Number of search results to return (optional, defaults to 10)",
      "minimum": 1,
      "maximum": 50,
      "default": 10
    },
    "fetch_content": {
      "type": "boolean",
      "description": "Whether to fetch and process content from result URLs (optional, defaults to true)",
      "default": true
    },
    "safe_search": {
      "type": "integer",
      "enum": [0, 1, 2],
      "description": "Safe search level: 0=off, 1=moderate, 2=strict (optional, defaults to 1)",
      "default": 1
    },
    "time_range": {
      "type": "string",
      "enum": ["", "day", "week", "month", "year"],
      "description": "Time range filter for results (optional, empty means all time)",
      "default": ""
    }
  },
  "required": ["query"]
}
```

### Implementation Requirements

#### SearXNG Integration
- Use SearXNG search API from high-quality public instances (A+ grade from searx.space)
- Implement instance health checking and failover
- Support multiple search categories and filters
- Handle rate limiting and API quotas gracefully

#### Search Result Processing
- Parse JSON responses from SearXNG API
- Extract titles, URLs, descriptions, and metadata
- Filter and rank results by relevance and quality
- Remove duplicate and low-quality results

#### Content Fetching
- Use `markdowndown` crate to fetch and convert result page content
- Process multiple URLs concurrently with proper rate limiting
- Convert HTML content to clean markdown for analysis
- Extract key information and summaries from each result

#### Privacy and Security
- Rotate between multiple SearXNG instances to avoid tracking
- Use random User-Agent strings and headers
- Implement request distribution to prevent overloading instances
- Respect robots.txt and rate limits

## Response Format

### Successful Search with Content
```json
{
  "content": [{
    "type": "text",
    "text": "Found 10 search results for query 'rust async programming'"
  }],
  "is_error": false,
  "metadata": {
    "query": "rust async programming",
    "category": "general",
    "language": "en",
    "results_count": 10,
    "search_time_ms": 1250,
    "instance_used": "https://search.example.org",
    "results": [
      {
        "title": "Async Programming in Rust - The Rust Book",
        "url": "https://doc.rust-lang.org/book/ch16-00-concurrency.html",
        "description": "Learn about asynchronous programming in Rust with async/await syntax...",
        "score": 0.95,
        "engine": "duckduckgo",
        "content": {
          "markdown": "# Async Programming in Rust\n\nRust's approach to async programming...",
          "word_count": 2840,
          "fetch_time_ms": 340,
          "summary": "Comprehensive guide to async programming concepts in Rust including futures, async/await, and runtime considerations."
        }
      },
      {
        "title": "Tokio - An asynchronous Rust runtime",
        "url": "https://tokio.rs/",
        "description": "Tokio is an asynchronous runtime for the Rust programming language...",
        "score": 0.92,
        "engine": "google",
        "content": {
          "markdown": "# Tokio\n\nA runtime for writing reliable, asynchronous...",
          "word_count": 1560,
          "fetch_time_ms": 280,
          "summary": "Tokio provides the runtime and libraries needed for async Rust applications."
        }
      }
    ],
    "total_results": 8450,
    "engines_used": ["duckduckgo", "google", "bing"],
    "content_fetch_stats": {
      "attempted": 10,
      "successful": 8,
      "failed": 2,
      "total_time_ms": 2840
    }
  }
}
```

### Search Results Only (No Content Fetch)
```json
{
  "content": [{
    "type": "text", 
    "text": "Found 5 search results for query 'rust memory management'"
  }],
  "is_error": false,
  "metadata": {
    "query": "rust memory management",
    "results_count": 5,
    "fetch_content": false,
    "results": [
      {
        "title": "Understanding Ownership - The Rust Programming Language",
        "url": "https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html", 
        "description": "Learn about Rust's unique approach to memory management through ownership...",
        "score": 0.98,
        "engine": "duckduckgo"
      }
    ]
  }
}
```

### Error Response
```json
{
  "content": [{
    "type": "text",
    "text": "Search failed: No available SearXNG instances"
  }],
  "is_error": true,
  "metadata": {
    "query": "rust programming",
    "error_type": "no_instances_available",
    "error_details": "All configured SearXNG instances are unavailable or rate limited",
    "attempted_instances": ["https://search.example1.org", "https://search.example2.org"],
    "retry_after": 300
  }
}
```

## SearXNG Instance Management

### Instance Discovery
- Use searx.space API to discover high-quality instances (A+ grade)
- Filter instances by API availability, response time, and reliability
- Maintain a curated list of verified instances with monitoring

### Health Monitoring
```rust
struct SearxInstance {
    url: String,
    grade: String,      // A+, A, B, etc.
    uptime: f32,        // Percentage uptime
    response_time: u64, // Average response time in ms
    last_checked: DateTime<Utc>,
    rate_limited_until: Option<DateTime<Utc>>,
    consecutive_failures: u32,
}
```

### Load Balancing Strategy
- Round-robin distribution across healthy instances
- Prefer higher-grade instances (A+ > A > B)
- Implement exponential backoff for failed instances
- Track and respect rate limits per instance

### Fallback Mechanism
- Primary: A+ grade instances
- Secondary: A grade instances with good uptime
- Tertiary: B grade instances as last resort
- Failure: Return error with retry information

## Content Processing Pipeline

### Concurrent Fetching
```rust
async fn fetch_search_results(
    results: Vec<SearchResult>,
    markdowndown_client: &MarkdownDownClient,
    max_concurrent: usize,
) -> Vec<ProcessedResult> {
    // Process results concurrently with rate limiting
    // Use semaphore to control concurrent requests
    // Handle individual failures gracefully
}
```

### Content Quality Assessment
- Filter out low-quality or spam content
- Prefer authoritative sources (documentation, official sites)
- Remove paywall or login-protected content
- Score content by relevance and information density

### Markdown Processing
- Convert HTML to clean, structured markdown
- Extract main content and remove boilerplate
- Preserve important formatting and structure
- Generate content summaries and key points

## Use Cases

### Technical Research
```json
{
  "query": "rust async sqlx database connection pooling",
  "category": "it",
  "results_count": 15,
  "fetch_content": true,
  "time_range": "year"
}
```

### Current Events and News
```json
{
  "query": "latest rust language updates 2024",
  "category": "news", 
  "results_count": 10,
  "time_range": "month"
}
```

### Documentation Discovery
```json
{
  "query": "site:docs.rs serde deserialize custom format",
  "category": "general",
  "results_count": 8,
  "fetch_content": true
}
```

### Quick Fact Checking
```json
{
  "query": "rust 1.75 release date new features",
  "results_count": 5,
  "fetch_content": false
}
```

## Privacy and Security Features

### Request Anonymization
- Rotate User-Agent strings to avoid fingerprinting
- Use different SearXNG instances to distribute requests
- Add random delays between requests to avoid detection
- Strip identifying headers and metadata

### Data Protection
- No search history storage or logging
- Temporary caching only for performance
- Encrypted communication with all instances
- Respect Do Not Track preferences

### Instance Security
- Verify HTTPS certificates for all instances
- Validate instance authenticity and safety
- Monitor for malicious or compromised instances
- Automatic blacklisting of problematic instances

## Configuration Options

### Global Settings
```toml
[web_search]
default_results_count = 10
max_results_count = 50
max_concurrent_fetches = 5
default_timeout = 30          # seconds
content_fetch_timeout = 45    # seconds per URL
max_content_size = "2MB"      # per result

# Instance management
instance_health_check_interval = 300  # seconds
max_consecutive_failures = 3
rate_limit_backoff_base = 60         # seconds

# Privacy settings
rotate_user_agents = true
request_delay_range = [100, 500]     # milliseconds
use_random_instances = true

# Content processing
summarize_long_content = true
max_summary_length = 500             # characters
extract_code_blocks = true
```

### Runtime Configuration
- Dynamic instance list updates
- Per-query timeout and result count overrides
- Category and language preferences
- Content fetching enable/disable toggle

## Error Handling

### Search API Errors
- Instance unavailability or timeouts
- Rate limiting and quota exceeded
- Invalid search parameters or queries
- API response parsing failures

### Content Fetching Errors
- Network connectivity issues for result URLs
- Content size limits exceeded
- Malformed or invalid HTML content
- Timeout during content processing

### Recovery Strategies
- Automatic failover to backup instances
- Graceful degradation (return results without content)
- Partial result delivery when some content fails
- Clear error reporting with actionable suggestions

## Performance Optimization

### Caching Strategy
- Search result caching with TTL (short-lived for freshness)
- Content caching for frequently accessed URLs
- Instance health status caching
- Negative caching for failed instances

### Resource Management
- Connection pooling for HTTP requests
- Memory-efficient streaming for large content
- Concurrent processing with backpressure control
- Automatic cleanup of cached data

### Response Time Optimization
- Parallel instance health checking
- Concurrent content fetching with limits
- Early termination for slow responses
- Result ranking and filtering optimization

## Integration Points

### Workflow Integration
- Search results stored in workflow variables
- Multi-step research workflows with result chaining
- Conditional searching based on previous results
- Integration with content analysis and summarization tools

### MCP Tool Ecosystem
- Results can be processed by memo and note-taking tools
- Integration with content indexing and search tools
- Support for follow-up searches and refinement
- Content can be fed into analysis and review workflows

### External Services
- Optional integration with fact-checking services
- Content quality scoring and verification
- Source credibility assessment
- Duplicate detection across searches

## Implementation Strategy

### Phase 1: Core Search
- Basic SearXNG API integration
- Simple result formatting and display
- Instance discovery and health checking
- Essential error handling

### Phase 2: Content Processing
- MarkdownDown integration for content fetching
- Concurrent processing and rate limiting
- Result quality filtering and ranking
- Content summarization and extraction

### Phase 3: Advanced Features
- Instance load balancing and failover
- Advanced privacy protection features
- Performance optimization and caching
- Integration with external quality services

## Security Considerations

### API Security
- Secure communication with all SearXNG instances
- Input validation and sanitization
- Protection against injection attacks
- Rate limiting and abuse prevention

### Content Security
- Safe handling of fetched content
- Protection against malicious websites
- Content sanitization and validation
- Sandboxed content processing

### Privacy Protection
- No user tracking or profiling
- Minimal logging and data retention
- Anonymous request routing
- Compliance with privacy regulations

## Testing Strategy

### Unit Tests
- SearXNG API response parsing
- Instance health checking logic
- Content processing and markdown conversion
- Error handling and recovery mechanisms

### Integration Tests
- Real search scenarios with live instances
- Content fetching and processing validation
- Instance failover and load balancing
- Performance and timeout handling

### Quality Assurance
- Search result relevance and quality testing
- Content extraction accuracy verification
- Privacy and security audit
- Performance benchmarking and optimization

## Future Enhancements

### Advanced Search Features
- Image and video search capabilities
- Specialized search for code and documentation
- Academic and research paper search
- Real-time and social media search

### AI Integration
- Search query optimization and expansion
- Result ranking using AI models
- Content summarization and key point extraction
- Semantic search and similarity matching

### User Experience
- Search history and bookmarking (with privacy controls)
- Search refinement and suggestion systems
- Custom search profiles and preferences
- Interactive search result exploration

## Conclusion

The MCP WebSearch tool provides comprehensive web search capabilities while maintaining privacy, security, and performance. The integration with SearXNG ensures access to diverse search engines without tracking, while the `markdowndown` integration enables high-quality content processing for thorough research and analysis workflows.