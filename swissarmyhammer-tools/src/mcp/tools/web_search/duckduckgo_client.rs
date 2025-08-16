//! DuckDuckGo search client implementation
//!
//! This module provides a client for performing web searches using DuckDuckGo's search API.
//! It replaces the previous SearXNG implementation with a direct connection to DuckDuckGo.

use crate::mcp::tools::web_search::privacy::PrivacyManager;
use crate::mcp::tools::web_search::types::{ScoringConfig, *};
use reqwest::{Client, Error as ReqwestError};
use std::time::Duration;
use url::Url;

/// DuckDuckGo search client
pub struct DuckDuckGoClient {
    client: Client,
    base_url: String,
    scoring_config: ScoringConfig,
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
    /// Creates a new DuckDuckGo client with default scoring configuration
    /// Note: User-Agent will be set per-request by privacy manager
    pub fn new() -> Self {
        Self::with_scoring_config(ScoringConfig::default())
    }

    /// Creates a new DuckDuckGo client with custom scoring configuration
    pub fn with_scoring_config(scoring_config: ScoringConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            // Don't set User-Agent here - privacy manager will handle it per-request
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            base_url: "https://html.duckduckgo.com".to_string(),
            scoring_config,
        }
    }

    /// Performs a web search using DuckDuckGo
    pub async fn search(
        &self,
        request: &WebSearchRequest,
        privacy_manager: &PrivacyManager,
    ) -> Result<Vec<SearchResult>, DuckDuckGoError> {
        tracing::debug!("Starting DuckDuckGo search for: '{}'", request.query);

        // Build the search URL
        let search_url = self.build_search_url(request)?;

        tracing::debug!("DuckDuckGo search URL: {}", search_url);

        // Apply request jitter for privacy
        privacy_manager.apply_jitter().await;

        // Build the request with privacy headers
        let mut request_builder = self.client.get(&search_url);

        // Apply User-Agent from privacy manager
        if let Some(user_agent) = privacy_manager.get_user_agent() {
            request_builder = request_builder.header("User-Agent", user_agent);
        } else {
            // Fallback User-Agent if privacy rotation is disabled
            request_builder = request_builder.header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
            );
        }

        // Apply privacy headers
        request_builder = privacy_manager.apply_privacy_headers(request_builder);

        let response = request_builder
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
            // Record CAPTCHA for adaptive rate limiting
            privacy_manager.record_captcha_challenge();
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

    /// Parses HTML content to extract search results using proper HTML parser
    fn parse_html_results(
        &self,
        html_content: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>, DuckDuckGoError> {
        use scraper::{Html, Selector};

        let mut results = Vec::new();

        // Parse the HTML document
        let document = Html::parse_document(html_content);

        // DuckDuckGo uses several different CSS selectors for results, try them in order
        let result_selectors = vec![
            // Main organic results
            "div[data-testid='web-result']",
            "div.result",
            "div[class*='result']",
            // Alternative result containers
            "div.web-result",
            "div[class*='web-result']",
        ];

        let title_selectors = vec![
            "h3 a",
            "a[data-testid='result-title-a']",
            "a.result__a",
            ".result__title a",
            "a[class*='result__a']",
        ];

        let description_selectors = vec![
            "[data-testid='result-snippet']",
            ".result__snippet",
            "a.result__snippet",
            "[class*='result__snippet']",
            ".result-snippet",
        ];

        // Try each result selector until we find results
        for result_selector_str in &result_selectors {
            let result_selector = Selector::parse(result_selector_str).map_err(|e| {
                DuckDuckGoError::Parse(format!("Invalid CSS selector '{result_selector_str}': {e}"))
            })?;

            let result_elements: Vec<_> = document.select(&result_selector).collect();

            if result_elements.is_empty() {
                continue; // Try next selector
            }

            for (index, result_element) in result_elements.iter().enumerate() {
                if index >= max_results {
                    break;
                }

                // Extract title and URL using multiple selector strategies
                let (title, url) = self.extract_title_and_url(result_element, &title_selectors)?;

                if title.is_empty() || url.is_empty() || !url.starts_with("http") {
                    continue; // Skip invalid results
                }

                // Extract description using multiple selector strategies
                let description = self.extract_description(result_element, &description_selectors);

                results.push(SearchResult {
                    title: html_escape::decode_html_entities(&title).to_string(),
                    url,
                    description: html_escape::decode_html_entities(&description).to_string(),
                    score: self.calculate_result_score(index),
                    engine: "duckduckgo".to_string(),
                    content: None, // Will be populated by content fetcher if needed
                });
            }

            // If we found results with this selector, we're done
            if !results.is_empty() {
                break;
            }
        }

        if results.is_empty() {
            // Try fallback regex parsing for edge cases
            self.parse_html_results_alternative(html_content, max_results)
        } else {
            Ok(results)
        }
    }

    /// Extracts title and URL from a result element using multiple selector strategies
    fn extract_title_and_url(
        &self,
        element: &scraper::ElementRef,
        title_selectors: &[&str],
    ) -> Result<(String, String), DuckDuckGoError> {
        use scraper::Selector;

        for selector_str in title_selectors {
            let selector = Selector::parse(selector_str).map_err(|e| {
                DuckDuckGoError::Parse(format!("Invalid title selector '{selector_str}': {e}"))
            })?;

            if let Some(title_element) = element.select(&selector).next() {
                let url = title_element.value().attr("href").unwrap_or("").to_string();
                let title = title_element
                    .text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string();

                if !title.is_empty() && !url.is_empty() {
                    return Ok((title, url));
                }
            }
        }

        Ok((String::new(), String::new()))
    }

    /// Extracts description from a result element using multiple selector strategies
    fn extract_description(
        &self,
        element: &scraper::ElementRef,
        description_selectors: &[&str],
    ) -> String {
        use scraper::Selector;

        for selector_str in description_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(desc_element) = element.select(&selector).next() {
                    let description = desc_element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();
                    if !description.is_empty() {
                        return description;
                    }
                }
            }
        }

        String::new()
    }

    /// Calculates result score based on position using configurable scoring algorithm
    fn calculate_result_score(&self, index: usize) -> f64 {
        let config = &self.scoring_config;

        if config.exponential_decay {
            // Exponential decay: score = base_score * e^(-decay_rate * index)
            let score = config.base_score * (-config.decay_rate * index as f64).exp();
            score.max(config.min_score)
        } else {
            // Linear decay: score = base_score - (position_penalty * index)
            let score = config.base_score - (config.position_penalty * index as f64);
            score.max(config.min_score)
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
                score: self.calculate_result_score(index),
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

    #[test]
    fn test_scoring_configuration() {
        // Test default linear scoring
        let client = DuckDuckGoClient::new();
        assert_eq!(client.calculate_result_score(0), 1.0); // First result gets full score
        assert_eq!(client.calculate_result_score(1), 0.95); // Second result gets 95%
        assert_eq!(client.calculate_result_score(2), 0.90); // Third result gets 90%

        // Test custom linear scoring
        let custom_config = ScoringConfig {
            base_score: 1.0,
            position_penalty: 0.1, // 10% penalty per position
            min_score: 0.1,
            exponential_decay: false,
            decay_rate: 0.0,
        };
        let custom_client = DuckDuckGoClient::with_scoring_config(custom_config);
        assert_eq!(custom_client.calculate_result_score(0), 1.0);
        assert_eq!(custom_client.calculate_result_score(1), 0.9);
        assert_eq!(custom_client.calculate_result_score(5), 0.5);
        assert_eq!(custom_client.calculate_result_score(10), 0.1); // Should hit min_score

        // Test exponential decay
        let exponential_config = ScoringConfig {
            base_score: 1.0,
            position_penalty: 0.0, // Not used for exponential
            min_score: 0.01,
            exponential_decay: true,
            decay_rate: 0.2,
        };
        let exp_client = DuckDuckGoClient::with_scoring_config(exponential_config);
        let score_0 = exp_client.calculate_result_score(0);
        let score_1 = exp_client.calculate_result_score(1);
        let score_2 = exp_client.calculate_result_score(2);

        assert_eq!(score_0, 1.0); // e^0 = 1
        assert!(score_1 < score_0); // Should decay
        assert!(score_2 < score_1); // Should decay further
        assert!(score_1 > 0.8); // Should be approximately e^(-0.2) ≈ 0.819
    }
}
