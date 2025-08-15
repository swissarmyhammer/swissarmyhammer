//! Content fetcher for web search results with concurrent processing and rate limiting
//!
//! This module provides advanced content fetching capabilities using html2md for high-quality
//! HTML to markdown conversion, with comprehensive rate limiting, content quality assessment,
//! and concurrent processing support.

use crate::mcp::tools::web_search::types::{
    CodeBlock, ContentMetadata, ContentType, SearchResult, SearchResultContent,
};
use dashmap::DashMap;
use html2md;
use regex::Regex;
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, warn};
use url::Url;

/// Error types for content fetching operations
#[derive(Debug, thiserror::Error)]
pub enum ContentFetchError {
    /// HTTP error response from server
    #[error("HTTP error {status}: {message}")]
    HttpError {
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
    },

    /// Network connection or communication error
    #[error("Network error: {message}")]
    NetworkError {
        /// Error message describing the network issue
        message: String,
    },

    /// Error during content processing
    #[error("Content processing error: {message}")]
    ProcessingError {
        /// Error message describing the processing issue
        message: String,
    },

    /// Rate limiting exceeded for a domain
    #[error("Rate limit exceeded for domain: {domain}")]
    RateLimited {
        /// Domain that has been rate limited
        domain: String,
    },

    /// Content failed quality assessment
    #[error("Content quality check failed: {reason}")]
    QualityCheckFailed {
        /// Reason for quality check failure
        reason: String,
    },

    /// Request timeout
    #[error("Timeout after {seconds}s")]
    Timeout {
        /// Number of seconds before timeout
        seconds: u64,
    },

    /// Invalid URL format
    #[error("Invalid URL: {url}")]
    InvalidUrl {
        /// The invalid URL string
        url: String,
    },
}

/// Configuration for content fetching operations
#[derive(Debug, Clone)]
pub struct ContentFetchConfig {
    /// Maximum number of concurrent fetches
    pub max_concurrent_fetches: usize,
    /// Timeout for each fetch operation
    pub fetch_timeout: Duration,
    /// Maximum content size to process
    pub max_content_size: usize,
    /// Default delay between requests to the same domain
    pub default_domain_delay: Duration,
    /// Maximum delay for domain rate limiting
    pub max_domain_delay: Duration,
    /// Content quality settings
    pub quality_config: ContentQualityConfig,
    /// Processing settings
    pub processing_config: ContentProcessingConfig,
}

impl Default for ContentFetchConfig {
    fn default() -> Self {
        Self {
            max_concurrent_fetches: 5,
            fetch_timeout: Duration::from_secs(45),
            max_content_size: 2 * 1024 * 1024, // 2MB
            default_domain_delay: Duration::from_millis(1000),
            max_domain_delay: Duration::from_secs(30),
            quality_config: ContentQualityConfig::default(),
            processing_config: ContentProcessingConfig::default(),
        }
    }
}

/// Configuration for content quality filtering
#[derive(Debug, Clone)]
pub struct ContentQualityConfig {
    /// Minimum word count for quality content
    pub min_content_length: usize,
    /// Maximum word count for quality content
    pub max_content_length: usize,
    /// Spam indicators to filter out
    pub spam_indicators: Vec<String>,
    /// Paywall indicators to filter out
    pub paywall_indicators: Vec<String>,
}

impl Default for ContentQualityConfig {
    fn default() -> Self {
        Self {
            min_content_length: 100,
            max_content_length: 50_000,
            spam_indicators: vec![
                "click here to continue".to_string(),
                "advertisement".to_string(),
                "sponsored content".to_string(),
                "this site uses cookies".to_string(),
            ],
            paywall_indicators: vec![
                "subscribe to continue".to_string(),
                "sign up for free".to_string(),
                "paywall".to_string(),
                "subscription required".to_string(),
                "login to view".to_string(),
            ],
        }
    }
}

/// Configuration for content processing
#[derive(Debug, Clone)]
pub struct ContentProcessingConfig {
    /// Maximum length for content summary
    pub max_summary_length: usize,
    /// Whether to extract code blocks
    pub extract_code_blocks: bool,
    /// Whether to generate summaries
    pub generate_summaries: bool,
    /// Whether to extract metadata
    pub extract_metadata: bool,
}

impl Default for ContentProcessingConfig {
    fn default() -> Self {
        Self {
            max_summary_length: 500,
            extract_code_blocks: true,
            generate_summaries: true,
            extract_metadata: true,
        }
    }
}

/// State tracking for domain rate limiting
#[derive(Debug)]
struct RateLimitState {
    last_request: Instant,
    delay: Duration,
    consecutive_requests: u32,
}

/// Statistics for content fetching operations
#[derive(Debug, Clone)]
pub struct ContentFetchStats {
    /// Number of URLs attempted to fetch
    pub attempted: usize,
    /// Number of URLs successfully fetched
    pub successful: usize,
    /// Number of URLs that failed to fetch
    pub failed: usize,
    /// Total time taken for all fetch operations in milliseconds
    pub total_time_ms: u64,
    /// Number of URLs that were rate limited
    pub rate_limited: usize,
    /// Number of URLs that were filtered due to quality issues
    pub quality_filtered: usize,
}

impl ContentFetchStats {
    fn new() -> Self {
        Self {
            attempted: 0,
            successful: 0,
            failed: 0,
            total_time_ms: 0,
            rate_limited: 0,
            quality_filtered: 0,
        }
    }
}

/// Advanced content fetcher with concurrent processing and rate limiting
pub struct ContentFetcher {
    client: Client,
    config: ContentFetchConfig,
    semaphore: Arc<Semaphore>,
    domain_trackers: Arc<DashMap<String, RateLimitState>>,
}

impl ContentFetcher {
    /// Create a new ContentFetcher with the given configuration
    pub fn new(config: ContentFetchConfig) -> Self {
        let client = Client::builder()
            .timeout(config.fetch_timeout)
            .user_agent("SwissArmyHammer/1.0 (Privacy-Focused Content Fetcher)")
            .build()
            .unwrap_or_else(|_| Client::new());

        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_fetches));
        let domain_trackers = Arc::new(DashMap::new());

        Self {
            client,
            config,
            semaphore,
            domain_trackers,
        }
    }

    /// Create a new ContentFetcher with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ContentFetchConfig::default())
    }

    /// Fetch content from multiple search results concurrently
    pub async fn fetch_search_results(
        &self,
        results: Vec<SearchResult>,
    ) -> (Vec<SearchResult>, ContentFetchStats) {
        let start_time = Instant::now();
        let mut stats = ContentFetchStats::new();
        stats.attempted = results.len();

        let tasks = results
            .into_iter()
            .map(|result| {
                let semaphore = self.semaphore.clone();
                let fetcher = self.clone_for_concurrent_use();
                tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    fetcher.fetch_single_result(result).await
                })
            })
            .collect::<Vec<_>>();

        let mut processed_results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(result)) => {
                    if result.content.is_some() {
                        stats.successful += 1;
                    } else {
                        stats.failed += 1;
                    }
                    processed_results.push(result);
                }
                Ok(Err(ContentFetchError::RateLimited { .. })) => {
                    stats.rate_limited += 1;
                    stats.failed += 1;
                }
                Ok(Err(ContentFetchError::QualityCheckFailed { .. })) => {
                    stats.quality_filtered += 1;
                    stats.failed += 1;
                }
                Ok(Err(_)) => {
                    stats.failed += 1;
                }
                Err(_) => {
                    stats.failed += 1;
                }
            }
        }

        stats.total_time_ms = start_time.elapsed().as_millis() as u64;

        debug!(
            "Content fetching completed: {} successful, {} failed, {} rate limited, {} quality filtered in {}ms",
            stats.successful, stats.failed, stats.rate_limited, stats.quality_filtered, stats.total_time_ms
        );

        (processed_results, stats)
    }

    /// Fetch content from a single search result
    async fn fetch_single_result(
        &self,
        mut result: SearchResult,
    ) -> Result<SearchResult, ContentFetchError> {
        let domain = self.extract_domain(&result.url)?;

        // Apply rate limiting
        self.wait_for_domain(&domain).await?;

        let start_time = Instant::now();

        // Fetch content
        let response = self.client.get(&result.url).send().await.map_err(|e| {
            if e.is_timeout() {
                ContentFetchError::Timeout {
                    seconds: self.config.fetch_timeout.as_secs(),
                }
            } else if e.is_connect() {
                ContentFetchError::NetworkError {
                    message: format!("Connection failed: {e}"),
                }
            } else {
                ContentFetchError::NetworkError {
                    message: format!("Network error: {e}"),
                }
            }
        })?;

        if !response.status().is_success() {
            return Err(ContentFetchError::HttpError {
                status: response.status().as_u16(),
                message: response
                    .status()
                    .canonical_reason()
                    .unwrap_or("Unknown")
                    .to_string(),
            });
        }

        let html = response
            .text()
            .await
            .map_err(|e| ContentFetchError::NetworkError {
                message: format!("Failed to read response body: {e}"),
            })?;

        // Check content size
        if html.len() > self.config.max_content_size {
            warn!("Content too large: {} bytes, truncating", html.len());
        }

        let html_to_process = if html.len() > self.config.max_content_size {
            &html[..self.config.max_content_size]
        } else {
            &html
        };

        // Convert HTML to markdown
        let markdown = html2md::parse_html(html_to_process);
        let word_count = markdown.split_whitespace().count();
        let fetch_time_ms = start_time.elapsed().as_millis() as u64;

        // Assess content quality
        if !self.is_quality_content(&markdown, word_count) {
            return Err(ContentFetchError::QualityCheckFailed {
                reason: "Content failed quality assessment".to_string(),
            });
        }

        // Process content
        let summary = if self.config.processing_config.generate_summaries {
            self.generate_summary(&markdown)
        } else {
            String::new()
        };

        let key_points = self.extract_key_points(&markdown);
        let code_blocks = if self.config.processing_config.extract_code_blocks {
            self.extract_code_blocks(&markdown)
        } else {
            Vec::new()
        };
        let metadata = if self.config.processing_config.extract_metadata {
            self.extract_metadata(&markdown, &result)
        } else {
            ContentMetadata::default()
        };

        let content = SearchResultContent {
            markdown,
            word_count,
            fetch_time_ms,
            summary,
            key_points,
            code_blocks,
            metadata,
        };

        result.content = Some(content);

        // Update domain tracking
        self.update_domain_tracking(&domain);

        Ok(result)
    }

    /// Extract domain from URL
    fn extract_domain(&self, url: &str) -> Result<String, ContentFetchError> {
        let parsed_url = Url::parse(url).map_err(|_| ContentFetchError::InvalidUrl {
            url: url.to_string(),
        })?;

        parsed_url
            .host_str()
            .map(|host| host.to_string())
            .ok_or_else(|| ContentFetchError::InvalidUrl {
                url: url.to_string(),
            })
    }

    /// Wait for domain rate limiting
    async fn wait_for_domain(&self, domain: &str) -> Result<(), ContentFetchError> {
        let mut should_wait = false;
        let mut wait_duration = Duration::from_millis(0);

        // Check if we need to wait
        if let Some(state) = self.domain_trackers.get_mut(domain) {
            let elapsed = state.last_request.elapsed();
            if elapsed < state.delay {
                should_wait = true;
                wait_duration = state.delay - elapsed;
            }
        }

        if should_wait {
            if wait_duration > self.config.max_domain_delay {
                return Err(ContentFetchError::RateLimited {
                    domain: domain.to_string(),
                });
            }
            debug!(
                "Rate limiting domain {}: waiting {:?}",
                domain, wait_duration
            );
            tokio::time::sleep(wait_duration).await;
        }

        Ok(())
    }

    /// Update domain tracking after successful request
    fn update_domain_tracking(&self, domain: &str) {
        let now = Instant::now();

        self.domain_trackers
            .entry(domain.to_string())
            .and_modify(|state| {
                state.last_request = now;
                state.consecutive_requests += 1;

                // Increase delay for frequent requests to same domain
                if state.consecutive_requests > 5 {
                    state.delay = (state.delay * 2).min(self.config.max_domain_delay);
                }
            })
            .or_insert(RateLimitState {
                last_request: now,
                delay: self.config.default_domain_delay,
                consecutive_requests: 1,
            });
    }

    /// Assess content quality
    fn is_quality_content(&self, content: &str, word_count: usize) -> bool {
        let config = &self.config.quality_config;

        // Check content length
        if word_count < config.min_content_length || word_count > config.max_content_length {
            debug!(
                "Content failed length check: {} words (min: {}, max: {})",
                word_count, config.min_content_length, config.max_content_length
            );
            return false;
        }

        let content_lower = content.to_lowercase();

        // Check for spam indicators
        for indicator in &config.spam_indicators {
            if content_lower.contains(indicator) {
                debug!("Content failed spam check: contains '{}'", indicator);
                return false;
            }
        }

        // Check for paywall indicators
        for indicator in &config.paywall_indicators {
            if content_lower.contains(indicator) {
                debug!("Content failed paywall check: contains '{}'", indicator);
                return false;
            }
        }

        true
    }

    /// Generate summary for content
    fn generate_summary(&self, content: &str) -> String {
        let max_length = self.config.processing_config.max_summary_length;

        if content.len() <= max_length {
            return content.to_string();
        }

        // Simple extractive summarization - take first few sentences
        let sentences: Vec<&str> = content.split('.').collect();
        let mut summary = String::new();

        for sentence in sentences {
            let sentence = sentence.trim();
            if sentence.is_empty() {
                continue;
            }

            if summary.len() + sentence.len() + 2 > max_length {
                break;
            }

            if !summary.is_empty() {
                summary.push_str(". ");
            }
            summary.push_str(sentence);
        }

        if !summary.ends_with('.') && !summary.is_empty() {
            summary.push('.');
        }

        summary
    }

    /// Extract key points from content using basic heuristics
    fn extract_key_points(&self, content: &str) -> Vec<String> {
        let mut key_points = Vec::new();

        // Look for bullet points and numbered lists
        let bullet_regex = Regex::new(r"(?m)^[\s]*[•\-\*\+]\s+(.+)$").unwrap();
        for cap in bullet_regex.captures_iter(content) {
            if let Some(point) = cap.get(1) {
                let point = point.as_str().trim();
                if point.len() > 10 && point.len() < 200 {
                    key_points.push(point.to_string());
                }
            }
        }

        // Look for numbered points
        let numbered_regex = Regex::new(r"(?m)^[\s]*\d+[\.\)]\s+(.+)$").unwrap();
        for cap in numbered_regex.captures_iter(content) {
            if let Some(point) = cap.get(1) {
                let point = point.as_str().trim();
                if point.len() > 10 && point.len() < 200 && !key_points.contains(&point.to_string())
                {
                    key_points.push(point.to_string());
                }
            }
        }

        // Look for sentences that start with strong indicator words
        let indicator_words = [
            "Key",
            "Important",
            "Note",
            "Remember",
            "Conclusion",
            "Summary",
        ];
        let lines: Vec<&str> = content.lines().collect();

        for line in lines {
            let line = line.trim();
            for &indicator in &indicator_words {
                if line.starts_with(indicator)
                    && line.len() > 20
                    && line.len() < 200
                    && !key_points.iter().any(|p| p.contains(line))
                {
                    key_points.push(line.to_string());
                }
            }
        }

        // Limit to top key points
        key_points.truncate(10);
        key_points
    }

    /// Extract code blocks from markdown content
    fn extract_code_blocks(&self, content: &str) -> Vec<CodeBlock> {
        let mut code_blocks = Vec::new();

        // Match fenced code blocks with optional language specification
        let code_regex = Regex::new(r"(?ms)^```(\w+)?\n?(.*?)^```").unwrap();

        for cap in code_regex.captures_iter(content) {
            let language = cap.get(1).map(|m| m.as_str().to_string());
            let code = cap
                .get(2)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            if !code.is_empty() && code.len() > 5 {
                code_blocks.push(CodeBlock {
                    language,
                    code,
                    start_line: None, // Could be enhanced to track line numbers
                });
            }
        }

        // Also look for inline code blocks if no fenced blocks found
        if code_blocks.is_empty() {
            let inline_regex = Regex::new(r"`([^`]+)`").unwrap();
            for cap in inline_regex.captures_iter(content) {
                if let Some(code_match) = cap.get(1) {
                    let code = code_match.as_str();
                    if code.len() > 10 && code.contains(|c: char| c.is_ascii_punctuation()) {
                        code_blocks.push(CodeBlock {
                            language: None,
                            code: code.to_string(),
                            start_line: None,
                        });
                    }
                }
            }
            // Limit inline code blocks
            code_blocks.truncate(5);
        }

        code_blocks
    }

    /// Extract metadata from content and search result
    fn extract_metadata(&self, content: &str, result: &SearchResult) -> ContentMetadata {
        let word_count = content.split_whitespace().count();

        // Estimate reading time (average 200 words per minute)
        let reading_time_minutes = if word_count > 0 {
            Some(((word_count as f64 / 200.0).ceil() as u32).max(1))
        } else {
            None
        };

        // Detect content type based on URL and content patterns
        let content_type = self.classify_content_type(&result.url, content);

        // Extract basic metadata from content
        let title = self.extract_title_from_content(content);
        let language = self.detect_language(content);
        let tags = self.extract_tags(content);

        ContentMetadata {
            title,
            author: None,         // Could be enhanced with author extraction
            published_date: None, // Could be enhanced with date extraction
            content_type,
            language,
            reading_time_minutes,
            tags,
        }
    }

    /// Classify content type based on URL and content patterns
    fn classify_content_type(&self, url: &str, content: &str) -> ContentType {
        let url_lower = url.to_lowercase();
        let content_lower = content.to_lowercase();

        // Check URL patterns
        if url_lower.contains("/docs/") || url_lower.contains("/documentation/") {
            return ContentType::Documentation;
        }

        if url_lower.contains("/news/") || url_lower.contains("/blog/") {
            return ContentType::News;
        }

        if url_lower.contains("/tutorial") || url_lower.contains("/guide") {
            return ContentType::Tutorial;
        }

        if url_lower.contains("/forum") || url_lower.contains("/discussion") {
            return ContentType::Forum;
        }

        // Check content patterns
        if content_lower.contains("tutorial") || content_lower.contains("how to") {
            return ContentType::Tutorial;
        }

        if content_lower.contains("abstract") && content_lower.contains("references") {
            return ContentType::Academic;
        }

        if content_lower.contains("documentation") || content_lower.contains("api reference") {
            return ContentType::Documentation;
        }

        ContentType::Article
    }

    /// Extract title from content (look for first heading)
    fn extract_title_from_content(&self, content: &str) -> Option<String> {
        // Look for markdown headings
        let heading_regex = Regex::new(r"(?m)^#+\s+(.+)$").unwrap();
        if let Some(cap) = heading_regex.captures(content) {
            if let Some(title) = cap.get(1) {
                let title = title.as_str().trim();
                if !title.is_empty() && title.len() < 200 {
                    return Some(title.to_string());
                }
            }
        }

        None
    }

    /// Basic language detection (very simple heuristics)
    fn detect_language(&self, content: &str) -> Option<String> {
        let content_lower = content.to_lowercase();

        // Simple keyword-based detection for major languages
        let common_words_en = ["the", "and", "for", "are", "but", "not", "you", "all"];
        let common_words_es = ["que", "para", "con", "por", "los", "las", "del"];
        let common_words_fr = ["que", "pour", "avec", "par", "les", "des", "une"];

        let en_score = common_words_en
            .iter()
            .filter(|&&word| content_lower.contains(word))
            .count();

        let es_score = common_words_es
            .iter()
            .filter(|&&word| content_lower.contains(word))
            .count();

        let fr_score = common_words_fr
            .iter()
            .filter(|&&word| content_lower.contains(word))
            .count();

        if en_score > es_score && en_score > fr_score && en_score > 2 {
            Some("en".to_string())
        } else if es_score > en_score && es_score > fr_score && es_score > 2 {
            Some("es".to_string())
        } else if fr_score > en_score && fr_score > es_score && fr_score > 2 {
            Some("fr".to_string())
        } else {
            None
        }
    }

    /// Extract tags/topics from content
    fn extract_tags(&self, content: &str) -> Vec<String> {
        let mut tags = Vec::new();

        // Look for common tech keywords
        let tech_keywords = [
            "rust",
            "python",
            "javascript",
            "typescript",
            "react",
            "vue",
            "angular",
            "docker",
            "kubernetes",
            "aws",
            "database",
            "api",
            "rest",
            "graphql",
            "machine learning",
            "ai",
            "blockchain",
            "security",
            "testing",
        ];

        let content_lower = content.to_lowercase();
        for &keyword in &tech_keywords {
            if content_lower.contains(keyword) {
                tags.push(keyword.to_string());
            }
        }

        // Look for hashtags
        let hashtag_regex = Regex::new(r"#(\w+)").unwrap();
        for cap in hashtag_regex.captures_iter(content) {
            if let Some(tag) = cap.get(1) {
                let tag = tag.as_str().to_lowercase();
                if tag.len() > 2 && !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
        }

        tags.truncate(10);
        tags
    }

    /// Clone the fetcher for concurrent use (shares client and domain trackers)
    fn clone_for_concurrent_use(&self) -> Self {
        Self {
            client: self.client.clone(),
            config: self.config.clone(),
            semaphore: self.semaphore.clone(),
            domain_trackers: self.domain_trackers.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_content_fetcher_creation() {
        let fetcher = ContentFetcher::with_defaults();
        assert_eq!(fetcher.config.max_concurrent_fetches, 5);
        assert_eq!(fetcher.config.fetch_timeout, Duration::from_secs(45));
    }

    #[test]
    fn test_extract_domain() {
        let fetcher = ContentFetcher::with_defaults();

        let domain = fetcher.extract_domain("https://example.com/path").unwrap();
        assert_eq!(domain, "example.com");

        let domain = fetcher
            .extract_domain("http://subdomain.example.com")
            .unwrap();
        assert_eq!(domain, "subdomain.example.com");

        let result = fetcher.extract_domain("invalid-url");
        assert!(result.is_err());
    }

    #[test]
    fn test_quality_assessment() {
        let fetcher = ContentFetcher::with_defaults();

        // Good content - need enough words to meet the 100 word minimum
        let good_content = "This is a high quality article with meaningful content and useful information for readers. ".repeat(10);
        assert!(fetcher.is_quality_content(&good_content, good_content.split_whitespace().count()));

        // Too short
        let short_content = "Short content";
        assert!(
            !fetcher.is_quality_content(short_content, short_content.split_whitespace().count())
        );

        // Contains spam
        let spam_content = "This article contains advertisement content. ".repeat(20);
        assert!(!fetcher.is_quality_content(&spam_content, spam_content.split_whitespace().count()));

        // Contains paywall
        let paywall_content =
            "Great content but you need to subscribe to continue reading. ".repeat(20);
        assert!(!fetcher
            .is_quality_content(&paywall_content, paywall_content.split_whitespace().count()));
    }

    #[test]
    fn test_summary_generation() {
        let fetcher = ContentFetcher::with_defaults();

        let content = "First sentence. Second sentence. Third sentence.";
        let summary = fetcher.generate_summary(content);
        assert!(summary.contains("First sentence"));
        assert!(summary.ends_with('.'));

        // Test with content shorter than max length
        let short_content = "Short content without periods";
        let short_summary = fetcher.generate_summary(short_content);
        assert_eq!(short_summary, short_content);
    }

    #[test]
    fn test_domain_tracking() {
        let fetcher = ContentFetcher::with_defaults();

        // First request should set up tracking
        fetcher.update_domain_tracking("example.com");
        assert!(fetcher.domain_trackers.contains_key("example.com"));

        // Subsequent requests should increment counter
        fetcher.update_domain_tracking("example.com");
        let state = fetcher.domain_trackers.get("example.com").unwrap();
        assert_eq!(state.consecutive_requests, 2);
    }

    #[test]
    fn test_config_defaults() {
        let config = ContentFetchConfig::default();
        assert_eq!(config.max_concurrent_fetches, 5);
        assert_eq!(config.fetch_timeout, Duration::from_secs(45));
        assert_eq!(config.max_content_size, 2 * 1024 * 1024);

        let quality_config = ContentQualityConfig::default();
        assert_eq!(quality_config.min_content_length, 100);
        assert_eq!(quality_config.max_content_length, 50_000);
        assert!(!quality_config.spam_indicators.is_empty());
        assert!(!quality_config.paywall_indicators.is_empty());
    }
}
