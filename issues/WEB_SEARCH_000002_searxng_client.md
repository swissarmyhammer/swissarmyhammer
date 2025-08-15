# WEB_SEARCH_000002: SearXNG HTTP Client Implementation

Refer to /Users/wballard/github/sah-search/ideas/web_search.md

## Overview
Implement the core HTTP client for communicating with SearXNG instances, including request formatting and response parsing.

## Goals
- Create SearXNG HTTP client with proper request formatting
- Implement JSON response parsing for SearXNG API
- Handle basic HTTP errors and timeouts
- Support all SearXNG search parameters from the specification

## Tasks
1. **SearXNG Client**: Implement HTTP client with proper URL construction
2. **Request Building**: Handle query parameters, categories, filters
3. **Response Parsing**: Parse SearXNG JSON responses into our types
4. **Error Mapping**: Map HTTP and parsing errors to our error types
5. **Parameter Validation**: Validate search parameters before sending requests

## Implementation Details

### SearXNG Client Structure
```rust
pub struct SearXngClient {
    http_client: reqwest::Client,
    base_url: String,
    timeout: Duration,
}

impl SearXngClient {
    pub async fn search(&self, request: &WebSearchRequest) -> Result<SearXngResponse> {
        // Build URL with query parameters
        // Send HTTP request
        // Parse JSON response
        // Map to our types
    }
}
```

### URL Construction
- Base URL: `{instance}/search`
- Query parameters: `q`, `categories`, `language`, `safesearch`, `time_range`
- Format: `format=json` for API responses
- Pagination: `pageno` for result paging

### Response Parsing
```rust
#[derive(Debug, Deserialize)]
struct SearXngResponse {
    query: String,
    results: Vec<SearXngResult>,
    // ... other metadata fields
}

#[derive(Debug, Deserialize)]  
struct SearXngResult {
    title: String,
    url: String,
    content: Option<String>,
    engine: String,
    score: Option<f32>,
    // ... other result fields
}
```

### Error Handling
- HTTP connection errors (network, DNS, etc.)
- HTTP status code errors (400, 429, 500, etc.)
- JSON parsing errors (invalid response format)
- Timeout errors (request or connection timeout)
- Invalid parameter errors (bad category, language, etc.)

## Success Criteria
- [x] Can send HTTP requests to SearXNG instances
- [x] Properly constructs query URLs with all parameters
- [x] Successfully parses JSON responses from SearXNG
- [x] Handles HTTP errors gracefully with proper error types
- [x] Validates input parameters before making requests

## Testing Strategy
- Mock HTTP client tests with sample SearXNG responses
- Parameter validation tests for all search options
- Error handling tests for various failure scenarios
- URL construction tests for different parameter combinations

## Integration Points
- Builds on the types and module structure from WEB_SEARCH_000001
- Uses reqwest client configured in previous step
- Integrates with error handling system
- Prepares for instance management in next step

## Configuration Options
- Default timeout values (30s for search, 45s for content)
- Default search parameters (results count, safe search level)
- HTTP client settings (keep-alive, connection pooling)
- User-Agent string for requests

## Sample Usage
```rust
let client = SearXngClient::new("https://search.example.org", Duration::from_secs(30))?;
let request = WebSearchRequest {
    query: "rust async programming".to_string(),
    category: Some(SearchCategory::General),
    results_count: Some(10),
    ..Default::default()
};
let response = client.search(&request).await?;
```