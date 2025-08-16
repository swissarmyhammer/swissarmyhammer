//! Core types for web search functionality
//!
//! This module defines the data structures used for web search requests and responses,
//! following the specification in /ideas/web_search.md

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Search category for filtering results
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchCategory {
    /// General web search across all content types
    #[default]
    General,
    /// Search specifically for images
    Images,
    /// Search specifically for videos
    Videos,
    /// Search specifically for news articles
    News,
    /// Search for map and location-based results
    Map,
    /// Search specifically for music content
    Music,
    /// Search for IT and technology-related content
    It,
    /// Search for scientific and academic content
    Science,
    /// Search specifically for files and documents
    Files,
}

/// Safe search level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
pub enum SafeSearchLevel {
    /// Safe search disabled, show all results
    Off = 0,
    /// Moderate safe search filtering
    #[default]
    Moderate = 1,
    /// Strict safe search filtering
    Strict = 2,
}

/// Time range filter for search results
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimeRange {
    /// Search results from all time periods
    #[serde(rename = "")]
    #[default]
    All,
    /// Search results from the last day
    Day,
    /// Search results from the last week
    Week,
    /// Search results from the last month
    Month,
    /// Search results from the last year
    Year,
}

/// Request structure for web search operations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchRequest {
    /// The search query string
    #[schemars(length(min = 1, max = 500))]
    pub query: String,

    /// Search category (optional, defaults to general)
    #[serde(default)]
    pub category: Option<SearchCategory>,

    /// Search language code (optional, defaults to 'en')
    #[serde(default)]
    #[schemars(regex(pattern = r"^[a-z]{2}(-[A-Z]{2})?$"))]
    pub language: Option<String>,

    /// Number of search results to return (optional, defaults to 10)
    #[serde(default = "default_results_count")]
    #[schemars(range(min = 1, max = 50))]
    pub results_count: Option<usize>,

    /// Whether to fetch and process content from result URLs (optional, defaults to true)
    #[serde(default = "default_fetch_content")]
    pub fetch_content: Option<bool>,

    /// Safe search level: 0=off, 1=moderate, 2=strict (optional, defaults to 1)
    #[serde(default)]
    pub safe_search: Option<SafeSearchLevel>,

    /// Time range filter for results (optional, empty means all time)
    #[serde(default)]
    pub time_range: Option<TimeRange>,
}

fn default_results_count() -> Option<usize> {
    Some(10)
}

fn default_fetch_content() -> Option<bool> {
    Some(true)
}

/// Individual search result
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResult {
    /// Page title
    pub title: String,

    /// Page URL
    pub url: String,

    /// Page description/snippet
    pub description: String,

    /// Relevance score (0.0 to 1.0)
    pub score: f64,

    /// Search engine that provided this result
    pub engine: String,

    /// Fetched content (if fetch_content was true)
    pub content: Option<SearchResultContent>,
}

/// Content fetched from a search result URL
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResultContent {
    /// Content converted to markdown
    pub markdown: String,

    /// Word count of the content
    pub word_count: usize,

    /// Time taken to fetch content in milliseconds
    pub fetch_time_ms: u64,

    /// Summary of the content
    pub summary: String,

    /// Key points extracted from the content
    #[serde(default)]
    pub key_points: Vec<String>,

    /// Code blocks found in the content
    #[serde(default)]
    pub code_blocks: Vec<CodeBlock>,

    /// Metadata extracted from the content
    #[serde(default)]
    pub metadata: ContentMetadata,
}

/// Code block extracted from content
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CodeBlock {
    /// Programming language (if detected)
    pub language: Option<String>,

    /// The code content
    pub code: String,

    /// Line number where the code block starts (if available)
    pub start_line: Option<usize>,
}

/// Content type classification
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    /// Article or blog post
    Article,
    /// Documentation
    Documentation,
    /// News article
    News,
    /// Academic or research paper
    Academic,
    /// Tutorial or how-to guide
    Tutorial,
    /// Reference material
    Reference,
    /// Forum post or discussion
    Forum,
    /// Product or service page
    Product,
    /// Unknown or other content type
    #[default]
    Other,
}

/// Metadata extracted from content
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ContentMetadata {
    /// Page title (if different from search result title)
    pub title: Option<String>,

    /// Author information
    pub author: Option<String>,

    /// Published date
    pub published_date: Option<String>,

    /// Content type classification
    pub content_type: ContentType,

    /// Language of the content
    pub language: Option<String>,

    /// Reading time estimate in minutes
    pub reading_time_minutes: Option<u32>,

    /// Tags or topics identified in the content
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Statistics for content fetching operations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentFetchStats {
    /// Number of URLs attempted to fetch
    pub attempted: usize,

    /// Number of URLs successfully fetched
    pub successful: usize,

    /// Number of URLs that failed to fetch
    pub failed: usize,

    /// Total time taken for all fetch operations in milliseconds
    pub total_time_ms: u64,
}

/// Metadata about the search operation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchMetadata {
    /// The search query that was executed
    pub query: String,

    /// Search category used
    pub category: SearchCategory,

    /// Language code used
    pub language: String,

    /// Number of results returned
    pub results_count: usize,

    /// Time taken for search operation in milliseconds
    pub search_time_ms: u64,

    /// Search service instance that was used
    pub instance_used: String,

    /// Total number of results found by search engines
    pub total_results: usize,

    /// Search engines that provided results
    pub engines_used: Vec<String>,

    /// Content fetching statistics (if fetch_content was true)
    pub content_fetch_stats: Option<ContentFetchStats>,

    /// Whether content fetching was enabled
    pub fetch_content: bool,
}

/// Response structure for web search operations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchResponse {
    /// Search results
    pub results: Vec<SearchResult>,

    /// Metadata about the search operation
    pub metadata: SearchMetadata,
}

/// Error information for failed search operations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchError {
    /// Error type classification
    pub error_type: String,

    /// Detailed error message
    pub error_details: String,

    /// Search service instances that were attempted
    pub attempted_instances: Vec<String>,

    /// Recommended retry delay in seconds
    pub retry_after: Option<u64>,
}

/// Configuration for result scoring algorithms
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScoringConfig {
    /// Base score for the first result (default: 1.0)
    pub base_score: f64,

    /// Score reduction per position (default: 0.05 = 5% per position)
    pub position_penalty: f64,

    /// Minimum score threshold (default: 0.05 = 5%)
    pub min_score: f64,

    /// Whether to apply exponential decay instead of linear (default: false)
    pub exponential_decay: bool,

    /// Decay rate for exponential scoring (default: 0.1, only used if exponential_decay is true)
    pub decay_rate: f64,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            base_score: 1.0,
            position_penalty: 0.05,
            min_score: 0.05,
            exponential_decay: false,
            decay_rate: 0.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_search_request_defaults() {
        let request = WebSearchRequest {
            query: "test query".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        assert_eq!(request.query, "test query");
        assert!(request.category.is_none());
        assert!(request.language.is_none());
        assert!(request.results_count.is_none());
        assert!(request.fetch_content.is_none());
        assert!(request.safe_search.is_none());
        assert!(request.time_range.is_none());
    }

    #[test]
    fn test_search_category_default() {
        let category = SearchCategory::default();
        matches!(category, SearchCategory::General);
    }

    #[test]
    fn test_safe_search_level_default() {
        let level = SafeSearchLevel::default();
        matches!(level, SafeSearchLevel::Moderate);
    }

    #[test]
    fn test_time_range_default() {
        let range = TimeRange::default();
        matches!(range, TimeRange::All);
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            title: "Test Title".to_string(),
            url: "https://example.com".to_string(),
            description: "Test description".to_string(),
            score: 0.95,
            engine: "duckduckgo".to_string(),
            content: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.title, deserialized.title);
        assert_eq!(result.url, deserialized.url);
        assert_eq!(result.description, deserialized.description);
        assert_eq!(result.score, deserialized.score);
        assert_eq!(result.engine, deserialized.engine);
        assert!(deserialized.content.is_none());
    }

    #[test]
    fn test_web_search_response_serialization() {
        let response = WebSearchResponse {
            results: vec![],
            metadata: SearchMetadata {
                query: "test".to_string(),
                category: SearchCategory::General,
                language: "en".to_string(),
                results_count: 0,
                search_time_ms: 100,
                instance_used: "https://duckduckgo.com".to_string(),
                total_results: 0,
                engines_used: vec![],
                content_fetch_stats: None,
                fetch_content: false,
            },
        };

        let json = serde_json::to_string_pretty(&response).unwrap();
        let deserialized: WebSearchResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.metadata.query, deserialized.metadata.query);
        assert_eq!(response.results.len(), deserialized.results.len());
    }
}
