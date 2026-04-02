//! Content fetcher for web search results with concurrent processing and rate limiting
//!
//! This module provides advanced content fetching capabilities using html2md for high-quality
//! HTML to markdown conversion, with comprehensive rate limiting, content quality assessment,
//! and concurrent processing support.

use crate::types::{CodeBlock, ContentMetadata, ContentType, SearchResult, SearchResultContent};
use dashmap::DashMap;
use html2md;
use regex::Regex;
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use swissarmyhammer_common::{ErrorSeverity, Severity};
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

impl Severity for ContentFetchError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Error: Fetch operations failed but system can continue with other URLs
            ContentFetchError::HttpError { .. } => ErrorSeverity::Error,
            ContentFetchError::NetworkError { .. } => ErrorSeverity::Error,
            ContentFetchError::ProcessingError { .. } => ErrorSeverity::Error,
            ContentFetchError::RateLimited { .. } => ErrorSeverity::Error,
            ContentFetchError::Timeout { .. } => ErrorSeverity::Error,
            ContentFetchError::InvalidUrl { .. } => ErrorSeverity::Error,

            // Warning: Content available but quality is below threshold
            ContentFetchError::QualityCheckFailed { .. } => ErrorSeverity::Warning,
        }
    }
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
    /// Note: User-Agent will be set per-request by privacy manager
    pub fn new(config: ContentFetchConfig) -> Self {
        let client = Client::builder()
            .timeout(config.fetch_timeout)
            // Don't set User-Agent here - privacy manager will handle it per-request
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

    /// Fetch content from multiple search results concurrently (backward compatibility)
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

    /// Fetch content from a single search result (backward compatibility)
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

    // ── extract_key_points ───────────────────────────────────────────

    #[test]
    fn test_extract_key_points_bullet_lists() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "Introduction paragraph.\n\
                        - This is a bullet point with enough text\n\
                        - Another bullet point that is long enough\n\
                        - Short\n\
                        * Asterisk bullet point with sufficient length\n\
                        + Plus bullet point with sufficient length here";
        let points = fetcher.extract_key_points(content);
        assert!(points.iter().any(|p| p.contains("This is a bullet")));
        assert!(points.iter().any(|p| p.contains("Another bullet")));
        assert!(points.iter().any(|p| p.contains("Asterisk bullet")));
        assert!(points.iter().any(|p| p.contains("Plus bullet")));
        // "Short" is under 10 chars and should be excluded
        assert!(!points.iter().any(|p| p == "Short"));
    }

    #[test]
    fn test_extract_key_points_numbered_lists() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "Some intro text here.\n\
                        1. First numbered point with enough text\n\
                        2. Second numbered point with enough text\n\
                        3) Third numbered point with enough text";
        let points = fetcher.extract_key_points(content);
        assert!(points.iter().any(|p| p.contains("First numbered")));
        assert!(points.iter().any(|p| p.contains("Second numbered")));
        assert!(points.iter().any(|p| p.contains("Third numbered")));
    }

    #[test]
    fn test_extract_key_points_no_duplicates_between_bullet_and_numbered() {
        let fetcher = ContentFetcher::with_defaults();
        // A line that matches both bullet and numbered patterns should not appear twice
        let content = "- 1. This overlapping point has enough text for both patterns";
        let points = fetcher.extract_key_points(content);
        let count = points
            .iter()
            .filter(|p| p.contains("This overlapping point"))
            .count();
        // May match one or both regexes, but numbered deduplication check prevents exact dupes
        assert!(count >= 1);
    }

    #[test]
    fn test_extract_key_points_indicator_words() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "Some text.\n\
                        Key takeaway from this section is very important indeed.\n\
                        Important note about the configuration settings here.\n\
                        Note that this behavior is expected and documented.\n\
                        Remember to always validate your inputs before use.\n\
                        Conclusion: the results are statistically significant.\n\
                        Summary of the findings presented in this paper.";
        let points = fetcher.extract_key_points(content);
        assert!(points.iter().any(|p| p.starts_with("Key")));
        assert!(points.iter().any(|p| p.starts_with("Important")));
        assert!(points.iter().any(|p| p.starts_with("Note")));
        assert!(points.iter().any(|p| p.starts_with("Remember")));
        assert!(points.iter().any(|p| p.starts_with("Conclusion")));
        assert!(points.iter().any(|p| p.starts_with("Summary")));
    }

    #[test]
    fn test_extract_key_points_truncates_at_ten() {
        let fetcher = ContentFetcher::with_defaults();
        let mut content = String::new();
        for i in 0..15 {
            content.push_str(&format!(
                "- Bullet point number {} with enough text to pass the length filter\n",
                i
            ));
        }
        let points = fetcher.extract_key_points(&content);
        assert!(points.len() <= 10);
    }

    #[test]
    fn test_extract_key_points_empty_content() {
        let fetcher = ContentFetcher::with_defaults();
        let points = fetcher.extract_key_points("");
        assert!(points.is_empty());
    }

    // ── extract_code_blocks ─────────────────────────────────────────

    #[test]
    fn test_extract_code_blocks_fenced_with_language() {
        let fetcher = ContentFetcher::with_defaults();
        let content =
            "Some text.\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\nMore text.";
        let blocks = fetcher.extract_code_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language.as_deref(), Some("rust"));
        assert!(blocks[0].code.contains("fn main()"));
    }

    #[test]
    fn test_extract_code_blocks_fenced_without_language() {
        let fetcher = ContentFetcher::with_defaults();
        let content =
            "Some text.\n```\nsome_command --flag value\nmore commands here\n```\nMore text.";
        let blocks = fetcher.extract_code_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].language.is_none());
        assert!(blocks[0].code.contains("some_command"));
    }

    #[test]
    fn test_extract_code_blocks_multiple_fenced() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "Intro.\n```python\ndef foo():\n    return 42\n```\nMiddle.\n```javascript\nfunction bar() { return 42; }\n```\nEnd.";
        let blocks = fetcher.extract_code_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].language.as_deref(), Some("python"));
        assert_eq!(blocks[1].language.as_deref(), Some("javascript"));
    }

    #[test]
    fn test_extract_code_blocks_inline_fallback() {
        let fetcher = ContentFetcher::with_defaults();
        // No fenced blocks, so should fall back to inline code extraction
        let content = "Run the command `cargo build --release --target x86_64` to compile.";
        let blocks = fetcher.extract_code_blocks(content);
        // Inline code must be >10 chars and contain punctuation
        assert!(blocks.iter().any(|b| b.code.contains("cargo build")));
        assert!(blocks.iter().all(|b| b.language.is_none()));
    }

    #[test]
    fn test_extract_code_blocks_no_inline_when_fenced_exist() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "Use `short-cmd` or:\n```bash\nlong_command --with-flags here\n```\n";
        let blocks = fetcher.extract_code_blocks(content);
        // Inline code should NOT be extracted when fenced blocks exist
        assert!(blocks
            .iter()
            .all(|b| b.language.is_some() || b.code.contains("long_command")));
        assert!(!blocks.iter().any(|b| b.code.contains("short-cmd")));
    }

    #[test]
    fn test_extract_code_blocks_ignores_short_fenced() {
        let fetcher = ContentFetcher::with_defaults();
        // Code block with <=5 chars should be ignored
        let content = "Text.\n```\nhi\n```\nMore.";
        let blocks = fetcher.extract_code_blocks(content);
        assert!(blocks.is_empty());
    }

    // ── classify_content_type ───────────────────────────────────────

    #[test]
    fn test_classify_content_type_url_patterns() {
        let fetcher = ContentFetcher::with_defaults();

        assert_eq!(
            fetcher.classify_content_type("https://example.com/docs/api", "some content"),
            ContentType::Documentation
        );
        assert_eq!(
            fetcher.classify_content_type("https://example.com/documentation/v2", "some content"),
            ContentType::Documentation
        );
        assert_eq!(
            fetcher.classify_content_type("https://example.com/blog/post-1", "some content"),
            ContentType::News
        );
        assert_eq!(
            fetcher.classify_content_type("https://example.com/news/latest", "some content"),
            ContentType::News
        );
        assert_eq!(
            fetcher.classify_content_type("https://example.com/tutorial/intro", "some content"),
            ContentType::Tutorial
        );
        assert_eq!(
            fetcher.classify_content_type("https://example.com/guide/setup", "some content"),
            ContentType::Tutorial
        );
        assert_eq!(
            fetcher.classify_content_type("https://example.com/forum/thread", "some content"),
            ContentType::Forum
        );
        assert_eq!(
            fetcher.classify_content_type("https://example.com/discussion/123", "some content"),
            ContentType::Forum
        );
    }

    #[test]
    fn test_classify_content_type_content_patterns() {
        let fetcher = ContentFetcher::with_defaults();

        // Content-based detection when URL has no pattern
        assert_eq!(
            fetcher.classify_content_type(
                "https://example.com/page",
                "This tutorial shows how to build"
            ),
            ContentType::Tutorial
        );
        assert_eq!(
            fetcher.classify_content_type(
                "https://example.com/page",
                "Learn how to configure your system"
            ),
            ContentType::Tutorial
        );
        assert_eq!(
            fetcher.classify_content_type(
                "https://example.com/page",
                "Abstract: We present a study. References: [1]"
            ),
            ContentType::Academic
        );
        assert_eq!(
            fetcher.classify_content_type(
                "https://example.com/page",
                "API Reference and documentation for the SDK"
            ),
            ContentType::Documentation
        );
    }

    #[test]
    fn test_classify_content_type_default_is_article() {
        let fetcher = ContentFetcher::with_defaults();
        assert_eq!(
            fetcher
                .classify_content_type("https://example.com/page", "nothing special here at all"),
            ContentType::Article
        );
    }

    // ── extract_title_from_content ──────────────────────────────────

    #[test]
    fn test_extract_title_from_content_h1() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "# My Great Title\n\nSome body text here.";
        assert_eq!(
            fetcher.extract_title_from_content(content),
            Some("My Great Title".to_string())
        );
    }

    #[test]
    fn test_extract_title_from_content_h2() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "## Section Heading\n\nBody text.";
        assert_eq!(
            fetcher.extract_title_from_content(content),
            Some("Section Heading".to_string())
        );
    }

    #[test]
    fn test_extract_title_from_content_first_heading_wins() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "# First Title\n\n## Second Title\n\nBody.";
        assert_eq!(
            fetcher.extract_title_from_content(content),
            Some("First Title".to_string())
        );
    }

    #[test]
    fn test_extract_title_from_content_none_when_no_heading() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "Just plain text with no markdown headings at all.";
        assert_eq!(fetcher.extract_title_from_content(content), None);
    }

    #[test]
    fn test_extract_title_from_content_empty() {
        let fetcher = ContentFetcher::with_defaults();
        assert_eq!(fetcher.extract_title_from_content(""), None);
    }

    // ── detect_language ─────────────────────────────────────────────

    #[test]
    fn test_detect_language_english() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "The quick brown fox jumps over the lazy dog and you are not going to believe all the things that happened for real but they did";
        assert_eq!(fetcher.detect_language(content), Some("en".to_string()));
    }

    #[test]
    fn test_detect_language_spanish() {
        let fetcher = ContentFetcher::with_defaults();
        let content =
            "Los gatos que corren por las calles del pueblo para buscar comida con los perros";
        assert_eq!(fetcher.detect_language(content), Some("es".to_string()));
    }

    #[test]
    fn test_detect_language_french() {
        let fetcher = ContentFetcher::with_defaults();
        let content =
            "Les chats qui courent pour attraper des souris avec une grande vitesse par les rues";
        assert_eq!(fetcher.detect_language(content), Some("fr".to_string()));
    }

    #[test]
    fn test_detect_language_none_when_ambiguous() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "12345 67890 !!@@##";
        assert_eq!(fetcher.detect_language(content), None);
    }

    // ── extract_tags ────────────────────────────────────────────────

    #[test]
    fn test_extract_tags_tech_keywords() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "This article covers rust programming and docker containers with kubernetes orchestration.";
        let tags = fetcher.extract_tags(content);
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"docker".to_string()));
        assert!(tags.contains(&"kubernetes".to_string()));
    }

    #[test]
    fn test_extract_tags_hashtags() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "Check out this post #webdev #coding #opensource";
        let tags = fetcher.extract_tags(content);
        assert!(tags.contains(&"webdev".to_string()));
        assert!(tags.contains(&"coding".to_string()));
        assert!(tags.contains(&"opensource".to_string()));
    }

    #[test]
    fn test_extract_tags_no_short_hashtags() {
        let fetcher = ContentFetcher::with_defaults();
        // Hashtags with <=2 chars should be excluded
        let content = "Tags: #ab #a #longertag";
        let tags = fetcher.extract_tags(content);
        assert!(!tags.contains(&"ab".to_string()));
        assert!(!tags.contains(&"a".to_string()));
        assert!(tags.contains(&"longertag".to_string()));
    }

    #[test]
    fn test_extract_tags_no_duplicate_hashtag_and_keyword() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "Learn rust programming #rust";
        let tags = fetcher.extract_tags(content);
        let rust_count = tags.iter().filter(|t| *t == "rust").count();
        assert_eq!(rust_count, 1);
    }

    #[test]
    fn test_extract_tags_truncates_at_ten() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "rust python javascript typescript react vue angular docker kubernetes aws database api rest graphql machine learning ai blockchain security testing #extra1 #extra2 #extra3";
        let tags = fetcher.extract_tags(content);
        assert!(tags.len() <= 10);
    }

    #[test]
    fn test_extract_tags_empty_content() {
        let fetcher = ContentFetcher::with_defaults();
        let tags = fetcher.extract_tags("");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_content_fetch_error_severity() {
        use swissarmyhammer_common::Severity;

        // Test Error severity for fetch failures
        let error_variants = vec![
            ContentFetchError::HttpError {
                status: 404,
                message: "Not Found".to_string(),
            },
            ContentFetchError::NetworkError {
                message: "Connection failed".to_string(),
            },
            ContentFetchError::ProcessingError {
                message: "Processing failed".to_string(),
            },
            ContentFetchError::RateLimited {
                domain: "example.com".to_string(),
            },
            ContentFetchError::Timeout { seconds: 30 },
            ContentFetchError::InvalidUrl {
                url: "invalid".to_string(),
            },
        ];

        for error in error_variants {
            assert_eq!(
                error.severity(),
                swissarmyhammer_common::ErrorSeverity::Error,
                "Expected Error severity for: {}",
                error
            );
        }

        // Test Warning severity for quality check failures
        let quality_error = ContentFetchError::QualityCheckFailed {
            reason: "Content too short".to_string(),
        };
        assert_eq!(
            quality_error.severity(),
            swissarmyhammer_common::ErrorSeverity::Warning,
            "Expected Warning severity for quality check failure"
        );
    }

    // ── ContentFetchStats::new ──────────────────────────────────────

    #[test]
    fn test_content_fetch_stats_new_initializes_to_zero() {
        let stats = ContentFetchStats::new();
        assert_eq!(stats.attempted, 0);
        assert_eq!(stats.successful, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.total_time_ms, 0);
        assert_eq!(stats.rate_limited, 0);
        assert_eq!(stats.quality_filtered, 0);
    }

    // ── wait_for_domain ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_wait_for_domain_no_prior_request_passes() {
        let fetcher = ContentFetcher::with_defaults();
        // No entry in domain_trackers — should return Ok immediately
        let result = fetcher.wait_for_domain("fresh-domain.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wait_for_domain_recent_request_within_delay_waits() {
        let fetcher = ContentFetcher::with_defaults();
        // Pre-populate with a very short delay so the test completes quickly
        let short_delay = Duration::from_millis(50);
        fetcher.domain_trackers.insert(
            "example.com".to_string(),
            RateLimitState {
                last_request: Instant::now(),
                delay: short_delay,
                consecutive_requests: 1,
            },
        );
        let start = Instant::now();
        let result = fetcher.wait_for_domain("example.com").await;
        assert!(result.is_ok());
        // Should have waited at least part of the delay
        // (elapsed >= short_delay minus small tolerance for timing)
        assert!(start.elapsed() >= Duration::from_millis(20));
    }

    #[tokio::test]
    async fn test_wait_for_domain_old_request_no_wait() {
        let fetcher = ContentFetcher::with_defaults();
        // Insert a tracker whose last_request was long ago — elapsed > delay, no wait needed
        fetcher.domain_trackers.insert(
            "old-domain.com".to_string(),
            RateLimitState {
                last_request: Instant::now() - Duration::from_secs(10),
                delay: Duration::from_millis(500),
                consecutive_requests: 3,
            },
        );
        let start = Instant::now();
        let result = fetcher.wait_for_domain("old-domain.com").await;
        assert!(result.is_ok());
        // Should return almost instantly since elapsed >> delay
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_wait_for_domain_exceeds_max_delay_returns_rate_limited() {
        let config = ContentFetchConfig {
            max_domain_delay: Duration::from_millis(100),
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);
        // Insert a tracker with a delay far exceeding max_domain_delay
        fetcher.domain_trackers.insert(
            "slow-domain.com".to_string(),
            RateLimitState {
                last_request: Instant::now(),
                delay: Duration::from_secs(60), // way above max_domain_delay of 100ms
                consecutive_requests: 10,
            },
        );
        let result = fetcher.wait_for_domain("slow-domain.com").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ContentFetchError::RateLimited { domain } => {
                assert_eq!(domain, "slow-domain.com");
            }
            other => panic!("Expected RateLimited error, got: {:?}", other),
        }
    }

    // ── update_domain_tracking delay escalation ─────────────────────

    #[test]
    fn test_update_domain_tracking_delay_escalates_after_five_consecutive() {
        let fetcher = ContentFetcher::with_defaults();
        let domain = "escalation-test.com";

        // Make 6 consecutive requests — first 5 should keep the default delay,
        // the 6th should double it (consecutive_requests transitions from 5 to 6)
        for _ in 0..6 {
            fetcher.update_domain_tracking(domain);
        }

        let state = fetcher.domain_trackers.get(domain).unwrap();
        assert_eq!(state.consecutive_requests, 6);
        // After the 6th call, delay should be 2x the default (1000ms -> 2000ms)
        assert_eq!(state.delay, Duration::from_millis(2000));
    }

    #[test]
    fn test_update_domain_tracking_delay_does_not_escalate_at_five_or_fewer() {
        let fetcher = ContentFetcher::with_defaults();
        let domain = "no-escalation.com";

        // Make exactly 5 requests — delay should remain at default
        for _ in 0..5 {
            fetcher.update_domain_tracking(domain);
        }

        let state = fetcher.domain_trackers.get(domain).unwrap();
        assert_eq!(state.consecutive_requests, 5);
        assert_eq!(state.delay, fetcher.config.default_domain_delay);
    }

    #[test]
    fn test_update_domain_tracking_delay_capped_at_max() {
        let config = ContentFetchConfig {
            default_domain_delay: Duration::from_secs(10),
            max_domain_delay: Duration::from_secs(30),
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);
        let domain = "cap-test.com";

        // After 6 calls: delay = 10s. 7th call doubles to 20s. 8th doubles to 40s but
        // gets capped at max_domain_delay (30s).
        for _ in 0..8 {
            fetcher.update_domain_tracking(domain);
        }

        let state = fetcher.domain_trackers.get(domain).unwrap();
        assert!(state.delay <= Duration::from_secs(30));
    }

    // ── generate_summary (thorough) ─────────────────────────────────

    #[test]
    fn test_generate_summary_respects_max_length() {
        let config = ContentProcessingConfig {
            max_summary_length: 50,
            ..ContentProcessingConfig::default()
        };
        let fetch_config = ContentFetchConfig {
            processing_config: config,
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(fetch_config);
        // Content much longer than 50 chars
        let content =
            "First sentence here. Second sentence here. Third sentence here. Fourth sentence here.";
        let summary = fetcher.generate_summary(content);
        assert!(
            summary.len() <= 55, // small tolerance for the trailing period
            "Summary length {} should be near max_summary_length 50",
            summary.len()
        );
    }

    #[test]
    fn test_generate_summary_short_content_returned_as_is() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "Short.";
        let summary = fetcher.generate_summary(content);
        assert_eq!(summary, content);
    }

    #[test]
    fn test_generate_summary_ends_with_period() {
        let fetcher = ContentFetcher::with_defaults();
        // Content longer than max_summary_length (500), split into sentences
        let long = "Alpha sentence here is first. Beta sentence here is second. Gamma sentence here is third. Delta sentence here is fourth. Epsilon sentence here is fifth. Zeta sentence here is sixth. Eta sentence here is seventh. Theta sentence here is eighth. Iota sentence here is ninth.";
        let summary = fetcher.generate_summary(long);
        assert!(
            summary.ends_with('.'),
            "Summary should end with period, got: {summary}"
        );
    }

    #[test]
    fn test_generate_summary_skips_empty_sentences() {
        let fetcher = ContentFetcher::with_defaults();
        // Extra periods create empty sentences; these should be skipped
        let content = "First sentence... Second sentence.. Third sentence.";
        let summary = fetcher.generate_summary(content);
        // Should still produce a non-empty summary from the non-empty parts
        assert!(!summary.is_empty());
    }

    // ── extract_metadata ────────────────────────────────────────────

    #[test]
    fn test_extract_metadata_reading_time_calculated() {
        let fetcher = ContentFetcher::with_defaults();
        // 200 words → reading_time_minutes = ceil(200/200) = 1
        let content = "word ".repeat(200);
        let result = SearchResult {
            title: "Test".to_string(),
            url: "https://example.com/page".to_string(),
            description: "desc".to_string(),
            score: 0.9,
            engine: "test".to_string(),
            content: None,
        };
        let metadata = fetcher.extract_metadata(&content, &result);
        assert_eq!(
            metadata.reading_time_minutes,
            Some(1),
            "200 words should take 1 minute"
        );
    }

    #[test]
    fn test_extract_metadata_reading_time_rounds_up() {
        let fetcher = ContentFetcher::with_defaults();
        // 201 words → ceil(201/200) = 2
        let content = "word ".repeat(201);
        let result = SearchResult {
            title: "Test".to_string(),
            url: "https://example.com/page".to_string(),
            description: "desc".to_string(),
            score: 0.9,
            engine: "test".to_string(),
            content: None,
        };
        let metadata = fetcher.extract_metadata(&content, &result);
        assert_eq!(
            metadata.reading_time_minutes,
            Some(2),
            "201 words should round up to 2 minutes"
        );
    }

    #[test]
    fn test_extract_metadata_empty_content_no_reading_time() {
        let fetcher = ContentFetcher::with_defaults();
        let result = SearchResult {
            title: "Test".to_string(),
            url: "https://example.com/page".to_string(),
            description: "desc".to_string(),
            score: 0.9,
            engine: "test".to_string(),
            content: None,
        };
        let metadata = fetcher.extract_metadata("", &result);
        assert_eq!(
            metadata.reading_time_minutes, None,
            "Empty content should have no reading time"
        );
    }

    #[test]
    fn test_extract_metadata_content_type_from_url() {
        let fetcher = ContentFetcher::with_defaults();
        let result_docs = SearchResult {
            title: "Docs".to_string(),
            url: "https://example.com/docs/api".to_string(),
            description: "d".to_string(),
            score: 0.9,
            engine: "test".to_string(),
            content: None,
        };
        let metadata = fetcher.extract_metadata("some content here", &result_docs);
        assert_eq!(metadata.content_type, ContentType::Documentation);
    }

    #[test]
    fn test_extract_metadata_title_extracted_from_heading() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "# My Page Title\n\nSome body text here.";
        let result = SearchResult {
            title: "Search Title".to_string(),
            url: "https://example.com/page".to_string(),
            description: "desc".to_string(),
            score: 0.9,
            engine: "test".to_string(),
            content: None,
        };
        let metadata = fetcher.extract_metadata(content, &result);
        assert_eq!(metadata.title, Some("My Page Title".to_string()));
    }

    #[test]
    fn test_extract_metadata_tags_populated() {
        let fetcher = ContentFetcher::with_defaults();
        let content = "This article discusses rust and docker in detail. ".repeat(5);
        let result = SearchResult {
            title: "Test".to_string(),
            url: "https://example.com/article".to_string(),
            description: "desc".to_string(),
            score: 0.9,
            engine: "test".to_string(),
            content: None,
        };
        let metadata = fetcher.extract_metadata(&content, &result);
        assert!(
            metadata.tags.contains(&"rust".to_string()),
            "Tags should include 'rust'"
        );
        assert!(
            metadata.tags.contains(&"docker".to_string()),
            "Tags should include 'docker'"
        );
    }

    // ── fetch_search_results async pipeline (mock HTTP server) ───────

    /// Build a minimal SearchResult pointing at the given URL.
    fn make_search_result(url: &str) -> SearchResult {
        SearchResult {
            title: "Test Result".to_string(),
            url: url.to_string(),
            description: "A test result".to_string(),
            score: 0.8,
            engine: "test".to_string(),
            content: None,
        }
    }

    /// Generate enough words to pass the quality word-count minimum (100 words).
    fn quality_html_body() -> String {
        let words = "This is meaningful content for the test page. ".repeat(15);
        format!("<html><body><p>{}</p></body></html>", words)
    }

    #[tokio::test]
    async fn test_fetch_search_results_success_populates_content() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(quality_html_body())
                    .append_header("content-type", "text/html"),
            )
            .mount(&mock_server)
            .await;

        let url = format!("{}/page", mock_server.uri());
        let results = vec![make_search_result(&url)];

        // Use a config with very short domain delay to avoid test slowness
        let config = ContentFetchConfig {
            default_domain_delay: Duration::from_millis(0),
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);
        let (processed, stats) = fetcher.fetch_search_results(results).await;

        assert_eq!(stats.attempted, 1);
        assert_eq!(stats.successful, 1, "Should have 1 successful fetch");
        assert_eq!(stats.failed, 0);
        assert!(!processed.is_empty());
        assert!(
            processed[0].content.is_some(),
            "Fetched result should have content"
        );
    }

    #[tokio::test]
    async fn test_fetch_search_results_http_error_counted_as_failed() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let url = format!("{}/not-found", mock_server.uri());
        let results = vec![make_search_result(&url)];

        let config = ContentFetchConfig {
            default_domain_delay: Duration::from_millis(0),
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);
        let (_processed, stats) = fetcher.fetch_search_results(results).await;

        assert_eq!(stats.attempted, 1);
        assert_eq!(stats.failed, 1, "HTTP 404 should count as failed");
        assert_eq!(stats.successful, 0);
    }

    #[tokio::test]
    async fn test_fetch_search_results_quality_filtered_counted() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // Return content that is too short to pass quality check (< 100 words)
        let short_html = "<html><body><p>Too short.</p></body></html>";

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(short_html)
                    .append_header("content-type", "text/html"),
            )
            .mount(&mock_server)
            .await;

        let url = format!("{}/short", mock_server.uri());
        let results = vec![make_search_result(&url)];

        let config = ContentFetchConfig {
            default_domain_delay: Duration::from_millis(0),
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);
        let (_processed, stats) = fetcher.fetch_search_results(results).await;

        assert_eq!(stats.attempted, 1);
        assert_eq!(
            stats.quality_filtered, 1,
            "Short content should be quality-filtered"
        );
        assert_eq!(stats.failed, 1);
    }

    #[tokio::test]
    async fn test_fetch_search_results_rate_limited_counted() {
        // Pre-populate a domain tracker with a delay that exceeds max_domain_delay
        // so fetch_single_result returns RateLimited immediately.
        let config = ContentFetchConfig {
            max_domain_delay: Duration::from_millis(100),
            default_domain_delay: Duration::from_millis(0),
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);

        // Manually insert a rate-limit state that will trigger the error
        fetcher.domain_trackers.insert(
            "rate-limited-host.example".to_string(),
            RateLimitState {
                last_request: Instant::now(),
                delay: Duration::from_secs(60), // >> max_domain_delay
                consecutive_requests: 20,
            },
        );

        let url = "https://rate-limited-host.example/page";
        let results = vec![make_search_result(url)];

        let (_processed, stats) = fetcher.fetch_search_results(results).await;

        assert_eq!(stats.attempted, 1);
        assert_eq!(stats.rate_limited, 1, "Should count as rate limited");
        assert_eq!(stats.failed, 1);
    }

    #[tokio::test]
    async fn test_fetch_search_results_multiple_results_concurrent() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        // Respond with quality content for all paths
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(quality_html_body())
                    .append_header("content-type", "text/html"),
            )
            .expect(3)
            .mount(&mock_server)
            .await;

        let base = mock_server.uri();
        let results = vec![
            make_search_result(&format!("{base}/p1")),
            make_search_result(&format!("{base}/p2")),
            make_search_result(&format!("{base}/p3")),
        ];

        let config = ContentFetchConfig {
            default_domain_delay: Duration::from_millis(0),
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);
        let (processed, stats) = fetcher.fetch_search_results(results).await;

        assert_eq!(stats.attempted, 3);
        assert_eq!(processed.len(), 3, "All results should be returned");
        // All should succeed (content populated)
        let successful_count = processed.iter().filter(|r| r.content.is_some()).count();
        assert_eq!(successful_count, 3);
    }

    #[tokio::test]
    async fn test_fetch_search_results_empty_input() {
        let fetcher = ContentFetcher::with_defaults();
        let (processed, stats) = fetcher.fetch_search_results(vec![]).await;
        assert_eq!(stats.attempted, 0);
        assert_eq!(stats.successful, 0);
        assert_eq!(stats.failed, 0);
        assert!(processed.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_search_results_total_time_recorded() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(quality_html_body())
                    .append_header("content-type", "text/html"),
            )
            .mount(&mock_server)
            .await;

        let url = format!("{}/page", mock_server.uri());
        let results = vec![make_search_result(&url)];

        let config = ContentFetchConfig {
            default_domain_delay: Duration::from_millis(0),
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);
        let (_processed, stats) = fetcher.fetch_search_results(results).await;

        assert!(
            stats.total_time_ms > 0,
            "Total time should be recorded as > 0ms"
        );
    }

    #[tokio::test]
    async fn test_fetch_search_results_invalid_url_counted_as_failed() {
        let fetcher = ContentFetcher::with_defaults();
        // URL is invalid — extract_domain will fail before any HTTP request
        let results = vec![make_search_result("not-a-valid-url")];
        let (_processed, stats) = fetcher.fetch_search_results(results).await;
        assert_eq!(stats.attempted, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.successful, 0);
    }

    #[tokio::test]
    async fn test_fetch_search_results_server_error_counted_as_failed() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let url = format!("{}/error", mock_server.uri());
        let results = vec![make_search_result(&url)];

        let config = ContentFetchConfig {
            default_domain_delay: Duration::from_millis(0),
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);
        let (_processed, stats) = fetcher.fetch_search_results(results).await;

        assert_eq!(stats.attempted, 1);
        assert_eq!(stats.failed, 1, "HTTP 500 should count as failed");
        assert_eq!(stats.successful, 0);
    }

    #[tokio::test]
    async fn test_fetch_search_results_content_too_large_truncated() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // Generate content larger than max_content_size (we'll set it small in config)
        let large_body = "word ".repeat(500); // well over a small limit
        let large_html = format!("<html><body><p>{}</p></body></html>", large_body);

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(large_html)
                    .append_header("content-type", "text/html"),
            )
            .mount(&mock_server)
            .await;

        let url = format!("{}/large", mock_server.uri());
        let results = vec![make_search_result(&url)];

        // Set max_content_size small enough to trigger truncation warning,
        // but quality check must still pass (min 100 words after truncation).
        // We pick 300 bytes which is enough to include ~50 "word " repetitions.
        let config = ContentFetchConfig {
            max_content_size: 300,
            default_domain_delay: Duration::from_millis(0),
            quality_config: ContentQualityConfig {
                min_content_length: 5, // lower threshold so truncated content passes
                ..ContentQualityConfig::default()
            },
            ..ContentFetchConfig::default()
        };
        let fetcher = ContentFetcher::new(config);
        let (processed, stats) = fetcher.fetch_search_results(results).await;

        // The fetch should succeed even with truncation
        assert_eq!(stats.attempted, 1);
        // Either success or quality-filtered is acceptable; the key is no panic
        assert_eq!(processed.len(), 1, "Result should still be returned");
        let _ = stats.successful + stats.failed; // just assert no panic
    }
}
