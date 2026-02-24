//! SwissArmyHammer Web
//!
//! Core crate for web search and fetch functionality.
//! Provides DuckDuckGo search, URL fetching with HTML-to-markdown conversion,
//! security validation, and privacy-respecting request handling.
//!
//! This crate contains pure web domain logic with no MCP protocol dependency.
//! The MCP tool adapters live in `swissarmyhammer-tools`.

pub mod chrome;
pub mod fetch;
pub mod privacy;
pub mod search;
pub mod security;
pub mod types;

// Re-export key types
pub use chrome::{detect_chrome, get_chrome_path, is_chrome_available, ChromeDetectionResult};
pub use fetch::{FetchError, FetchResult, WebFetcher};
pub use privacy::{PrivacyConfig, PrivacyManager, UserAgentRotator};
pub use search::content_fetcher::{ContentFetchConfig, ContentFetcher};
pub use search::duckduckgo::{DuckDuckGoClient, DuckDuckGoError};
pub use search::WebSearcher;
pub use security::{SecurityError, SecurityPolicy, SecurityValidator};
pub use types::{
    CodeBlock, ContentFetchStats, ContentMetadata, ContentType, DuckDuckGoConfig,
    SafeSearchLevel, ScoringConfig, SearchCategory, SearchMetadata, SearchResult,
    SearchResultContent, TimeRange, WebFetchRequest, WebSearchError, WebSearchRequest,
    WebSearchResponse,
};
