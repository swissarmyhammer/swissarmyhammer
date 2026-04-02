---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffea80
title: Add tests for ContentFetcher fetch pipeline and rate limiting
---
src/search/content_fetcher.rs:256-462\n\nCoverage: 0% for fetch pipeline methods\n\nUncovered lines: 256-312, 315-419, 436-462\n\n```rust\npub async fn fetch_search_results(&self, results: Vec<SearchResult>) -> (...)\nasync fn fetch_single_result(&self, result: SearchResult) -> Result<...>\nasync fn wait_for_domain(&self, domain: &str) -> Result<...>\n```\n\nThese are async methods requiring tokio and HTTP mocking:\n1. fetch_search_results — concurrent orchestration with stats tracking\n2. fetch_single_result — full pipeline: rate limit → fetch → quality check → process\n3. wait_for_domain — rate limiting with max_domain_delay exceeded → RateLimited error\n\nNote: Requires mockable HTTP client or integration test approach. #coverage-gap