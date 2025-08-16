//! DuckDuckGo search client implementation
//!
//! This module provides a client for performing web searches using DuckDuckGo's HTML search interface.
//! Based on the proven approach from the ddgs Python library (https://github.com/deedy5/ddgs).
//!
//! Key features:
//! - Simple POST requests to DuckDuckGo HTML endpoint without VQD tokens
//! - XPath-based HTML parsing for reliable result extraction
//! - CAPTCHA detection and graceful error handling
//! - Configurable scoring for search result ranking

use crate::mcp::tools::web_search::privacy::PrivacyManager;
use crate::mcp::tools::web_search::types::{ScoringConfig, *};
use reqwest::{Client, Error as ReqwestError};
use std::collections::HashMap;
use std::time::Duration;

/// DuckDuckGo search client using simple HTML scraping
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
    pub fn new() -> Self {
        Self::with_scoring_config(ScoringConfig::default())
    }

    /// Creates a new DuckDuckGo client with custom scoring configuration
    pub fn with_scoring_config(scoring_config: ScoringConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            base_url: "https://html.duckduckgo.com".to_string(),
            scoring_config,
        }
    }

    /// Performs a web search using DuckDuckGo HTML interface
    pub async fn search(
        &mut self,
        request: &WebSearchRequest,
        privacy_manager: &PrivacyManager,
    ) -> Result<Vec<SearchResult>, DuckDuckGoError> {
        tracing::debug!("Starting DuckDuckGo search for: '{}'", request.query);

        // Apply request jitter for privacy
        privacy_manager.apply_jitter().await;

        // Use DuckDuckGo's HTML search endpoint with POST (based on ddgs approach)
        let search_url = format!("{}/html/", self.base_url);

        // Build form parameters (simplified from ddgs implementation)
        let mut params = HashMap::new();
        params.insert("q", request.query.as_str());
        params.insert("b", ""); // Empty value as in ddgs
        
        // Add region parameter
        let region = if let Some(ref language) = request.language {
            match language.as_str() {
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
                _ => "us-en",
            }
        } else {
            "us-en"
        };
        params.insert("l", region);

        // Add time range parameter if specified
        if let Some(ref time_range) = request.time_range {
            let time_param = match time_range {
                TimeRange::Day => Some("d"),
                TimeRange::Week => Some("w"),
                TimeRange::Month => Some("m"),
                TimeRange::Year => Some("y"),
                TimeRange::All => None,
            };
            if let Some(time) = time_param {
                params.insert("df", time);
            }
        }

        let mut request_builder = self.client.post(&search_url).form(&params);

        // Apply User-Agent and privacy headers
        if let Some(user_agent) = privacy_manager.get_user_agent() {
            request_builder = request_builder.header("User-Agent", user_agent);
        }
        request_builder = privacy_manager.apply_privacy_headers(request_builder);

        let response = request_builder
            .send()
            .await
            .map_err(DuckDuckGoError::Network)?;

        if !response.status().is_success() {
            return Err(DuckDuckGoError::InvalidRequest(format!(
                "Search request failed with status: {}",
                response.status()
            )));
        }

        let response_text = response.text().await.map_err(DuckDuckGoError::Network)?;

        // Check for CAPTCHA challenges
        if self.is_captcha_challenge(&response_text) {
            privacy_manager.record_captcha_challenge();
            return Err(DuckDuckGoError::CaptchaRequired);
        }

        // Parse results using XPath-based approach (similar to ddgs)
        let results = self.parse_html_results(&response_text, request.results_count.unwrap_or(10))?;

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
            || html_content.contains("captcha")
            || html_content.contains("human verification")
    }

    /// Parses HTML content to extract search results (simplified based on ddgs approach)
    fn parse_html_results(
        &self,
        html_content: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>, DuckDuckGoError> {
        use scraper::{Html, Selector};

        tracing::debug!(
            "Parsing HTML content of {} characters for search results",
            html_content.len()
        );

        // Save HTML to a debug file if debug logging is enabled
        if tracing::enabled!(tracing::Level::DEBUG) {
            let debug_file_path = std::env::temp_dir().join("duckduckgo_response.html");
            if let Ok(mut file) = std::fs::File::create(&debug_file_path) {
                use std::io::Write;
                let _ = file.write_all(html_content.as_bytes());
                tracing::debug!("HTML response saved to {:?}", debug_file_path);
            }
        }

        // Parse the HTML document
        let document = Html::parse_document(html_content);

        // Use ddgs-based selectors (translated from XPath to CSS)
        // Original ddgs: items_xpath = "//div[contains(@class, 'body')]"
        let result_selectors = vec![
            "div[class*='body']",          // Primary selector from ddgs
            "div[data-testid='web-result']", // Modern DuckDuckGo
            "div.result",                    // Classic DuckDuckGo
        ];

        for result_selector_str in &result_selectors {
            let result_selector = Selector::parse(result_selector_str).map_err(|e| {
                DuckDuckGoError::Parse(format!("Invalid CSS selector '{result_selector_str}': {e}"))
            })?;

            let result_elements: Vec<_> = document.select(&result_selector).collect();

            if result_elements.is_empty() {
                tracing::debug!("No results found with selector: {}", result_selector_str);
                continue; // Try next selector
            }

            tracing::debug!("Found {} potential results with selector: {}", result_elements.len(), result_selector_str);

            let mut results = Vec::new();
            for (index, result_element) in result_elements.iter().enumerate() {
                if index >= max_results {
                    break;
                }

                // Extract title and URL (based on ddgs: title from h2, href from a)
                let (title, url) = self.extract_title_and_url_simple(result_element)?;

                if title.is_empty() || url.is_empty() || !url.starts_with("http") {
                    tracing::debug!("Skipping invalid result {}: title='{}', url='{}'", index, title, url);
                    continue; // Skip invalid results
                }

                // Extract description (based on ddgs: body from a text)
                let description = self.extract_description_simple(result_element);

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
                tracing::debug!("Successfully parsed {} results", results.len());
                return Ok(results);
            }
        }

        tracing::warn!("No search results found with any selector");
        Err(DuckDuckGoError::NoResults)
    }

    /// Extract title and URL from result element (simplified ddgs approach)
    fn extract_title_and_url_simple(
        &self,
        element: &scraper::ElementRef,
    ) -> Result<(String, String), DuckDuckGoError> {
        use scraper::Selector;

        // ddgs approach: title from h2, href from a
        let title_selectors = vec!["h2 a", "h3 a", "h2", "h3", "a"];
        let url_selectors = vec!["a"];

        let mut title = String::new();
        let mut url = String::new();

        // Extract title and URL from the same <a> element if possible
        for selector_str in &title_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(link_element) = element.select(&selector).next() {
                    if let Some(href) = link_element.value().attr("href") {
                        url = href.to_string();
                    }
                    title = link_element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();
                    if !title.is_empty() && !url.is_empty() {
                        break;
                    }
                }
            }
        }

        // If we didn't find both title and URL, try to find URL separately
        if url.is_empty() {
            for selector_str in &url_selectors {
                if let Ok(selector) = Selector::parse(selector_str) {
                    if let Some(link_element) = element.select(&selector).next() {
                        if let Some(href) = link_element.value().attr("href") {
                            url = href.to_string();
                            break;
                        }
                    }
                }
            }
        }

        Ok((title, url))
    }

    /// Extract description from result element (simplified ddgs approach)
    fn extract_description_simple(&self, element: &scraper::ElementRef) -> String {
        use scraper::Selector;

        // ddgs approach: body text from a elements
        let description_selectors = vec![
            "a.result__snippet",
            ".result__snippet",
            "a[class*='snippet']",
            "[class*='snippet']",
            "a", // Fallback to any link text
        ];

        for selector_str in &description_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for desc_element in element.select(&selector) {
                    let description = desc_element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();
                    if !description.is_empty() {
                        // Avoid using the same text as title
                        if description.len() > 10 {
                            return description;
                        }
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
