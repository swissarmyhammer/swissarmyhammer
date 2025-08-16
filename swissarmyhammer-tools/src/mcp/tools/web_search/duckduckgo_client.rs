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

        // Build search parameters based on ddgs approach
        let params = self.build_search_params(request);
        
        // Create the HTTP request with privacy headers
        let response = self.create_search_request(&params, privacy_manager).await?;
        
        // Validate response and check for CAPTCHA
        let response_text = self.validate_response(response, privacy_manager).await?;

        // Parse results using XPath-based approach (similar to ddgs)
        let results =
            self.parse_html_results(&response_text, request.results_count.unwrap_or(10))?;

        tracing::debug!("DuckDuckGo search found {} results", results.len());

        Ok(results)
    }

    /// Builds search parameters for DuckDuckGo request (based on ddgs implementation)
    fn build_search_params(&self, request: &WebSearchRequest) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("q".to_string(), request.query.clone());
        params.insert("b".to_string(), "".to_string()); // Empty value as in ddgs

        // Add region parameter based on language
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
        params.insert("l".to_string(), region.to_string());

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
                params.insert("df".to_string(), time.to_string());
            }
        }

        params
    }

    /// Creates and sends the HTTP request to DuckDuckGo
    async fn create_search_request(
        &self,
        params: &HashMap<String, String>,
        privacy_manager: &PrivacyManager,
    ) -> Result<reqwest::Response, DuckDuckGoError> {
        let search_url = format!("{}/html/", self.base_url);
        let mut request_builder = self.client.post(&search_url).form(params);

        // Apply User-Agent and privacy headers
        if let Some(user_agent) = privacy_manager.get_user_agent() {
            request_builder = request_builder.header("User-Agent", user_agent);
        }
        request_builder = privacy_manager.apply_privacy_headers(request_builder);

        let response = request_builder
            .send()
            .await
            .map_err(DuckDuckGoError::Network)?;

        Ok(response)
    }

    /// Validates response and checks for CAPTCHA challenges
    async fn validate_response(
        &self,
        response: reqwest::Response,
        privacy_manager: &PrivacyManager,
    ) -> Result<String, DuckDuckGoError> {
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

        Ok(response_text)
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

        // Use exact ddgs selectors (translated from XPath to CSS)
        // ddgs uses: items_xpath = "//div[contains(@class, 'body')]"
        let result_selectors = vec![
            "div[class*='body']",            // Exact ddgs primary selector
            "div.result",                    // Fallback classic DuckDuckGo
            "div[data-testid='web-result']", // Modern DuckDuckGo fallback
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

            tracing::debug!(
                "Found {} potential results with selector: {}",
                result_elements.len(),
                result_selector_str
            );

            let mut results = Vec::new();
            for (index, result_element) in result_elements.iter().enumerate() {
                if index >= max_results {
                    break;
                }

                // Extract title and URL (based on ddgs: title from h2, href from a)
                let (title, url) = self.extract_title_and_url_simple(result_element)?;

                if title.is_empty() || url.is_empty() || !url.starts_with("http") {
                    tracing::debug!(
                        "Skipping invalid result {}: title='{}', url='{}'",
                        index,
                        title,
                        url
                    );
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

        // Exact ddgs approach: title from ".//h2//text()", href from "./a/@href"
        // In CSS selector terms: h2 for title, a[href] for links
        let title_selectors = vec!["h2", "h3", "h2 a", "h3 a"];
        let url_selectors = vec!["a[href]", "a"];

        let mut title = String::new();
        let mut url = String::new();

        // ddgs: title from ".//h2//text()" - extract text from h2 elements first
        for selector_str in &title_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(title_element) = element.select(&selector).next() {
                    let extracted_title = title_element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();
                    
                    // If this is an 'a' element, also extract href
                    if title_element.value().name() == "a" {
                        if let Some(href) = title_element.value().attr("href") {
                            url = href.to_string();
                            title = extracted_title;
                            if !title.is_empty() && !url.is_empty() {
                                break;
                            }
                        }
                    } else if !extracted_title.is_empty() {
                        title = extracted_title;
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

        // Exact ddgs approach: body from "./a//text()"
        // In CSS selector terms: extract text from all <a> elements
        let description_selectors = vec![
            "a", // Primary ddgs approach: text from links
            "[class*='snippet']",
            ".result__snippet",
        ];

        // ddgs approach: body from "./a//text()" - extract text from all link elements
        for selector_str in &description_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                let mut all_texts = Vec::new();
                
                for desc_element in element.select(&selector) {
                    let text = desc_element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();
                    
                    if !text.is_empty() && text.len() > 10 {
                        all_texts.push(text);
                    }
                }
                
                if !all_texts.is_empty() {
                    // Join all extracted text and return first meaningful description
                    let combined = all_texts.join(" ").trim().to_string();
                    if combined.len() > 20 { // Ensure substantial content
                        return combined;
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

    #[test]
    fn test_html_parsing_with_sample_duckduckgo_response() {
        let client = DuckDuckGoClient::new();

        // Sample DuckDuckGo HTML response structure (based on ddgs approach)
        let sample_html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>DuckDuckGo Search</title></head>
        <body>
            <div class="results">
                <div class="result body">
                    <h2><a href="https://example.com/apple">What is an Apple? - Example.com</a></h2>
                    <a href="https://example.com/apple">An apple is a round fruit that grows on trees. Apples are commonly red, green, or yellow.</a>
                </div>
                <div class="result body">
                    <h2><a href="https://en.wikipedia.org/wiki/Apple">Apple - Wikipedia</a></h2>
                    <a href="https://en.wikipedia.org/wiki/Apple">An apple is an edible fruit produced by an apple tree. Apple trees are cultivated worldwide.</a>
                </div>
                <div class="result body">
                    <h2><a href="https://nutrition.org/apples">Apple Nutrition Facts</a></h2>
                    <a href="https://nutrition.org/apples">Apples are a great source of fiber and vitamin C. They make healthy snacks.</a>
                </div>
            </div>
        </body>
        </html>
        "#;

        let results = client.parse_html_results(sample_html, 10).unwrap();

        assert_eq!(results.len(), 3);

        // Test first result
        assert_eq!(results[0].title, "What is an Apple? - Example.com");
        assert_eq!(results[0].url, "https://example.com/apple");
        assert!(results[0].description.contains("apple is a round fruit"));
        assert_eq!(results[0].engine, "duckduckgo");
        assert_eq!(results[0].score, 1.0); // First result gets full score

        // Test second result
        assert_eq!(results[1].title, "Apple - Wikipedia");
        assert_eq!(results[1].url, "https://en.wikipedia.org/wiki/Apple");
        assert!(results[1].description.contains("edible fruit"));
        assert_eq!(results[1].score, 0.95); // Second result gets 95%

        // Test third result
        assert_eq!(results[2].title, "Apple Nutrition Facts");
        assert_eq!(results[2].url, "https://nutrition.org/apples");
        assert!(results[2].description.contains("great source of fiber"));
        assert_eq!(results[2].score, 0.90); // Third result gets 90%
    }

    #[test]
    fn test_html_parsing_with_complex_duckduckgo_structure() {
        let client = DuckDuckGoClient::new();

        // More complex HTML structure with nested elements
        let complex_html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <div class="main-results">
                <div class="search-result body">
                    <div class="result-header">
                        <h2><a href="https://apple.com">Apple Inc. Official Website</a></h2>
                    </div>
                    <div class="result-content">
                        <a href="https://apple.com">Discover the innovative world of Apple and shop everything iPhone, iPad, Mac, Apple Watch.</a>
                    </div>
                </div>
                <div class="search-result body">
                    <h3>
                        <a href="https://healthline.com/apple-benefits">
                            <span>Health Benefits of Apples</span>
                        </a>
                    </h3>
                    <a href="https://healthline.com/apple-benefits">
                        <span>Apples provide numerous health benefits including improved heart health and better digestion.</span>
                    </a>
                </div>
            </div>
        </body>
        </html>
        "#;

        let results = client.parse_html_results(complex_html, 5).unwrap();

        assert_eq!(results.len(), 2);

        // Test extraction from nested h2
        assert_eq!(results[0].title, "Apple Inc. Official Website");
        assert_eq!(results[0].url, "https://apple.com");
        assert!(results[0].description.contains("innovative world of Apple"));

        // Test extraction from h3 with nested spans
        assert_eq!(results[1].title, "Health Benefits of Apples");
        assert_eq!(results[1].url, "https://healthline.com/apple-benefits");
        assert!(results[1].description.contains("health benefits"));
    }

    #[test]
    fn test_html_parsing_no_results() {
        let client = DuckDuckGoClient::new();

        // HTML without matching selectors
        let empty_html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <div class="header">Search Results</div>
            <div class="no-results">No results found</div>
        </body>
        </html>
        "#;

        let result = client.parse_html_results(empty_html, 10);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DuckDuckGoError::NoResults));
    }

    #[test]
    fn test_extract_title_and_url_variations() {
        let client = DuckDuckGoClient::new();

        // Test h2 with direct link
        let html1 = r#"
        <div class="result body">
            <h2><a href="https://example.com">Example Title</a></h2>
        </div>
        "#;
        let document1 = scraper::Html::parse_document(html1);
        let element1 = document1.select(&scraper::Selector::parse("div").unwrap()).next().unwrap();
        let (title1, url1) = client.extract_title_and_url_simple(&element1).unwrap();
        assert_eq!(title1, "Example Title");
        assert_eq!(url1, "https://example.com");

        // Test h3 with link
        let html2 = r#"
        <div class="result body">
            <h3><a href="https://test.org">Test Page</a></h3>
        </div>
        "#;
        let document2 = scraper::Html::parse_document(html2);
        let element2 = document2.select(&scraper::Selector::parse("div").unwrap()).next().unwrap();
        let (title2, url2) = client.extract_title_and_url_simple(&element2).unwrap();
        assert_eq!(title2, "Test Page");
        assert_eq!(url2, "https://test.org");
    }

    #[test]
    fn test_description_extraction_variations() {
        let client = DuckDuckGoClient::new();

        // Test description from link elements
        let html = r#"
        <div class="result body">
            <h2><a href="https://example.com">Title</a></h2>
            <a href="https://example.com">This is a detailed description of the content from the link.</a>
            <a href="https://example.com">Additional context about the webpage content.</a>
        </div>
        "#;
        let document = scraper::Html::parse_document(html);
        let element = document.select(&scraper::Selector::parse("div").unwrap()).next().unwrap();
        let description = client.extract_description_simple(&element);
        
        assert!(description.contains("detailed description"));
        assert!(description.len() > 20);
    }
}
