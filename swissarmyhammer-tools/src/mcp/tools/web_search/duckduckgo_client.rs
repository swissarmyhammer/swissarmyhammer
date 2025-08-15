//! DuckDuckGo search client implementation
//!
//! This module provides a client for performing web searches using DuckDuckGo's search API.
//! It replaces the previous SearXNG implementation with a direct connection to DuckDuckGo.

use crate::mcp::tools::web_search::types::*;
use reqwest::{Client, Error as ReqwestError};
use std::time::Duration;
use url::Url;

/// DuckDuckGo search client
pub struct DuckDuckGoClient {
    client: Client,
    base_url: String,
}

/// Errors that can occur during DuckDuckGo search operations
#[derive(Debug, thiserror::Error)]
pub enum DuckDuckGoError {
    /// Network connectivity error
    #[error("Network error: {0}")]
    Network(#[from] ReqwestError),
    /// HTML parsing error
    #[error("Parse error: {0}")]
    Parse(String),
    /// Invalid search request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    /// Search completed but no results were found
    #[error("No results found")]
    NoResults,
    /// DuckDuckGo is requesting CAPTCHA verification
    #[error("DuckDuckGo is requesting CAPTCHA verification to confirm this search was made by a human. This is a bot protection measure. Please try again later or use the web interface directly.")]
    CaptchaRequired,
}

impl DuckDuckGoClient {
    /// Creates a new DuckDuckGo client
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            base_url: "https://html.duckduckgo.com".to_string(),
        }
    }

    /// Performs a web search using DuckDuckGo
    pub async fn search(
        &self,
        request: &WebSearchRequest,
    ) -> Result<Vec<SearchResult>, DuckDuckGoError> {
        tracing::debug!("Starting DuckDuckGo search for: '{}'", request.query);

        // Build the search URL
        let search_url = self.build_search_url(request)?;

        tracing::debug!("DuckDuckGo search URL: {}", search_url);

        // Make the request
        let response = self
            .client
            .get(&search_url)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            )
            .header("Accept-Language", "en-US,en;q=0.5")
            .header("Accept-Encoding", "gzip, deflate")
            .header("DNT", "1")
            .header("Connection", "keep-alive")
            .header("Upgrade-Insecure-Requests", "1")
            .send()
            .await
            .map_err(DuckDuckGoError::Network)?;

        if !response.status().is_success() {
            return Err(DuckDuckGoError::InvalidRequest(format!(
                "DuckDuckGo returned status: {}",
                response.status()
            )));
        }

        // Get the response body as text
        let html_content = response.text().await.map_err(DuckDuckGoError::Network)?;

        // Check for CAPTCHA challenges before attempting to parse results
        if self.is_captcha_challenge(&html_content) {
            return Err(DuckDuckGoError::CaptchaRequired);
        }

        // Parse the HTML to extract search results
        let results =
            self.parse_html_results(&html_content, request.results_count.unwrap_or(10))?;

        tracing::debug!("DuckDuckGo search found {} results", results.len());

        Ok(results)
    }

    /// Checks if the HTML response contains a CAPTCHA challenge
    fn is_captcha_challenge(&self, html_content: &str) -> bool {
        // Look for CAPTCHA-related elements in the HTML
        html_content.contains("anomaly-modal")
            || html_content.contains("Unfortunately, bots use DuckDuckGo too")
            || html_content.contains("challenge-form")
            || html_content.contains("Please complete the following challenge")
    }

    /// Builds the search URL with proper parameters
    fn build_search_url(&self, request: &WebSearchRequest) -> Result<String, DuckDuckGoError> {
        let mut url = Url::parse(&format!("{}/html/", self.base_url))
            .map_err(|e| DuckDuckGoError::InvalidRequest(format!("Invalid base URL: {e}")))?;

        {
            let mut query_pairs = url.query_pairs_mut();

            // Required search query
            query_pairs.append_pair("q", &request.query);

            // Safe search parameter
            let safe_search = match request.safe_search.unwrap_or(SafeSearchLevel::Moderate) {
                SafeSearchLevel::Off => "-1",
                SafeSearchLevel::Moderate => "1",
                SafeSearchLevel::Strict => "1",
            };
            query_pairs.append_pair("safe", safe_search);

            // Language parameter
            if let Some(ref language) = request.language {
                // DuckDuckGo uses region codes, map common language codes
                let region = match language.as_str() {
                    "en" => "us-en",
                    "es" => "es-es",
                    "fr" => "fr-fr",
                    "de" => "de-de",
                    "it" => "it-it",
                    "pt" => "pt-br",
                    "ru" => "ru-ru",
                    "ja" => "jp-jp",
                    "ko" => "kr-kr",
                    "zh" => "cn-zh",
                    _ => "us-en", // Default to US English
                };
                query_pairs.append_pair("kl", region);
            }

            // Time range parameter
            if let Some(ref time_range) = request.time_range {
                let time_param = match time_range {
                    TimeRange::All => None,
                    TimeRange::Day => Some("d"),
                    TimeRange::Week => Some("w"),
                    TimeRange::Month => Some("m"),
                    TimeRange::Year => Some("y"),
                };
                if let Some(time) = time_param {
                    query_pairs.append_pair("df", time);
                }
            }

            // Disable ads and tracking
            query_pairs.append_pair("t", "h_");
            query_pairs.append_pair("ia", "web");
        }

        Ok(url.to_string())
    }

    /// Parses HTML content to extract search results
    fn parse_html_results(
        &self,
        html_content: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>, DuckDuckGoError> {
        use regex::Regex;

        let mut results = Vec::new();

        // Parse the HTML using regex patterns to extract results
        // This is a simplified parser - in production you might want to use a proper HTML parser

        // Pattern to match result containers
        let result_pattern =
            Regex::new(r#"(?s)<div[^>]+class="[^"]*result[^"]*"[^>]*>.*?</div>\s*</div>"#)
                .map_err(|e| DuckDuckGoError::Parse(format!("Invalid regex: {e}")))?;

        // Pattern to extract title and URL
        let title_url_pattern =
            Regex::new(r#"<a[^>]+href="([^"]+)"[^>]*><span[^>]*>([^<]+)</span></a>"#)
                .map_err(|e| DuckDuckGoError::Parse(format!("Invalid regex: {e}")))?;

        // Pattern to extract description
        let description_pattern =
            Regex::new(r#"<a[^>]+class="[^"]*result__snippet[^"]*"[^>]*>([^<]+)</a>"#)
                .map_err(|e| DuckDuckGoError::Parse(format!("Invalid regex: {e}")))?;

        for (index, result_match) in result_pattern.find_iter(html_content).enumerate() {
            if index >= max_results {
                break;
            }

            let result_html = result_match.as_str();

            // Extract title and URL
            let (title, url) = if let Some(captures) = title_url_pattern.captures(result_html) {
                let url = captures
                    .get(1)
                    .map(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = captures
                    .get(2)
                    .map(|m| m.as_str())
                    .unwrap_or("Untitled")
                    .to_string();
                (title, url)
            } else {
                continue; // Skip results without valid title/URL
            };

            // Extract description
            let description = description_pattern
                .captures(result_html)
                .and_then(|captures| captures.get(1))
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();

            // Validate URL format
            if url.is_empty() || !url.starts_with("http") {
                continue; // Skip invalid URLs
            }

            results.push(SearchResult {
                title: html_escape::decode_html_entities(&title).to_string(),
                url,
                description: html_escape::decode_html_entities(&description).to_string(),
                score: 1.0 - (index as f64 * 0.05), // Simple scoring based on position
                engine: "duckduckgo".to_string(),
                content: None, // Will be populated by content fetcher if needed
            });
        }

        if results.is_empty() {
            // Try alternative parsing method for different DuckDuckGo layouts
            self.parse_html_results_alternative(html_content, max_results)
        } else {
            Ok(results)
        }
    }

    /// Alternative HTML parsing method for different DuckDuckGo result layouts
    fn parse_html_results_alternative(
        &self,
        html_content: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>, DuckDuckGoError> {
        use regex::Regex;

        let mut results = Vec::new();

        // Alternative patterns for different DuckDuckGo layouts
        let link_pattern = Regex::new(
            r#"(?s)<a[^>]+href="([^"]+)"[^>]*class="[^"]*result__a[^"]*"[^>]*>([^<]+)</a>"#,
        )
        .map_err(|e| DuckDuckGoError::Parse(format!("Invalid regex: {e}")))?;

        let snippet_pattern =
            Regex::new(r#"(?s)<a[^>]+class="[^"]*result__snippet[^"]*"[^>]*>([^<]+)</a>"#)
                .map_err(|e| DuckDuckGoError::Parse(format!("Invalid regex: {e}")))?;

        let links: Vec<_> = link_pattern.captures_iter(html_content).collect();
        let snippets: Vec<_> = snippet_pattern.captures_iter(html_content).collect();

        for (index, link_captures) in links.iter().enumerate().take(max_results) {
            let url = link_captures
                .get(1)
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let title = link_captures
                .get(2)
                .map(|m| m.as_str())
                .unwrap_or("Untitled")
                .to_string();

            // Get corresponding description if available
            let description = snippets
                .get(index)
                .and_then(|captures| captures.get(1))
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();

            // Validate URL format
            if url.is_empty() || !url.starts_with("http") {
                continue;
            }

            results.push(SearchResult {
                title: html_escape::decode_html_entities(&title).to_string(),
                url,
                description: html_escape::decode_html_entities(&description).to_string(),
                score: 1.0 - (index as f64 * 0.05),
                engine: "duckduckgo".to_string(),
                content: None,
            });
        }

        if results.is_empty() {
            Err(DuckDuckGoError::NoResults)
        } else {
            Ok(results)
        }
    }
}

impl Default for DuckDuckGoClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duckduckgo_client_new() {
        let client = DuckDuckGoClient::new();
        assert_eq!(client.base_url, "https://html.duckduckgo.com");
    }

    #[test]
    fn test_build_search_url_basic() {
        let client = DuckDuckGoClient::new();
        let request = WebSearchRequest {
            query: "test query".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        let url = client.build_search_url(&request).unwrap();
        assert!(url.contains("q=test+query"));
        assert!(url.contains("safe=1")); // Default moderate safe search
        assert!(url.contains("t=h_"));
        assert!(url.contains("ia=web"));
    }

    #[test]
    fn test_build_search_url_with_language() {
        let client = DuckDuckGoClient::new();
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: Some("es".to_string()),
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        let url = client.build_search_url(&request).unwrap();
        assert!(url.contains("kl=es-es"));
    }

    #[test]
    fn test_build_search_url_with_time_range() {
        let client = DuckDuckGoClient::new();
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: Some(TimeRange::Week),
        };

        let url = client.build_search_url(&request).unwrap();
        assert!(url.contains("df=w"));
    }

    #[test]
    fn test_build_search_url_with_safe_search() {
        let client = DuckDuckGoClient::new();
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: Some(SafeSearchLevel::Off),
            time_range: None,
        };

        let url = client.build_search_url(&request).unwrap();
        assert!(url.contains("safe=-1"));
    }

    #[tokio::test]
    async fn test_search_invalid_query() {
        let client = DuckDuckGoClient::new();
        let request = WebSearchRequest {
            query: "".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        // This should still build a valid URL, but DuckDuckGo will return no results
        let url_result = client.build_search_url(&request);
        assert!(url_result.is_ok());
    }

    #[test]
    fn test_is_captcha_challenge() {
        let client = DuckDuckGoClient::new();

        // Test with CAPTCHA HTML content
        let captcha_html = r#"
            <div class="anomaly-modal__title">Unfortunately, bots use DuckDuckGo too.</div>
            <div class="anomaly-modal__description">Please complete the following challenge to confirm this search was made by a human.</div>
        "#;
        assert!(client.is_captcha_challenge(captcha_html));

        // Test with normal HTML content
        let normal_html = r#"
            <div class="result">
                <a href="https://example.com">Example</a>
            </div>
        "#;
        assert!(!client.is_captcha_challenge(normal_html));

        // Test with challenge form
        let challenge_html = r#"<form id="challenge-form" action="/anomaly.js">"#;
        assert!(client.is_captcha_challenge(challenge_html));
    }
}
