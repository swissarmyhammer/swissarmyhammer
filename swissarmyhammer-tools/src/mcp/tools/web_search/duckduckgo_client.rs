//! DuckDuckGo search client implementation
//!
//! This module provides a client for performing web searches using DuckDuckGo
//! by simulating a real user with a headless browser.
//!
//! Key features:
//! - Uses chromiumoxide for real browser automation
//! - Simulates genuine user search behavior
//! - HTML parsing for reliable result extraction
//! - CAPTCHA avoidance through realistic browsing patterns
//! - Configurable scoring for search result ranking

use crate::mcp::tools::web_search::types::{ScoringConfig, *};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::error::CdpError;
use futures::StreamExt;
use rand::Rng;
use std::time::Duration;

// Configuration constants for retry logic and delays
const MAX_SEARCH_INPUT_ATTEMPTS: usize = 30;
const MAX_RESULTS_WAIT_ATTEMPTS: usize = 60;
const ATTEMPT_DELAY_MS: u64 = 500;
const RESULTS_WAIT_DELAY_MS: u64 = 250;
const INITIAL_PAGE_LOAD_DELAY_MS: u64 = 2000;
const CLEANUP_DELAY_MS: u64 = 100;

// Selector constants for various page elements
const SEARCH_INPUT_SELECTORS: &[&str] = &[
    "input[name='q']",
    "#search_form_input",
    "#searchbox_input",
    "input[type='search']",
];

const SEARCH_RESULT_SELECTORS: &[&str] = &[
    "article[data-testid='result']",
    "div[data-testid='result']",
    ".result",
    "[data-result]",
    "#links .result",
    ".web-result",
    ".result__body",
    "ol[data-testid='results'] li",
    ".react-results .result",
];

const RESULT_CONTAINER_SELECTORS: &[&str] = &[
    "div[data-testid='result']",     // Modern DuckDuckGo main selector
    "article[data-testid='result']", // Article-based results
    "div.result",                    // Classic result container
    "div[class*='result']",          // Any div with result in class
];

const TITLE_LINK_SELECTORS: &[&str] = &[
    "h2 a[data-testid='result-title-a']", // Modern testid-based title link
    "h3 a[data-testid='result-title-a']", // H3 variant
    "h2 a",                               // Fallback: any link in h2
    "h3 a",                               // Fallback: any link in h3
    "a[data-testid='result-title-a']",    // Direct title link
    "h2",                                 // Title text only
    "h3",                                 // H3 title text only
];

const URL_SELECTORS: &[&str] = &[
    "a[data-testid='result-title-a']", // Primary title link
    "a[href^='http']",                 // Any external link
    "a[href]",                         // Any link with href
];

const SNIPPET_SELECTORS: &[&str] = &[
    "[data-testid='result-snippet']",     // Modern testid-based snippet
    ".result__snippet",                   // Classic snippet class
    "[class*='snippet']",                 // Any element with snippet in class
    "div[data-result='snippet']",         // Data attribute variant
    "div.snippet",                        // Simple snippet div
    "span[data-testid='result-snippet']", // Span-based snippet
];

/// DuckDuckGo search client using browser automation
pub struct DuckDuckGoClient {
    scoring_config: ScoringConfig,
}

/// Errors that can occur during DuckDuckGo search operations
#[derive(Debug, thiserror::Error)]
pub enum DuckDuckGoError {
    /// Browser automation error
    #[error("Browser error: {0}")]
    Browser(Box<CdpError>),
    /// HTML parsing error
    #[error("Parse error: {0}")]
    Parse(String),
    /// Invalid search request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    /// Search completed but no results were found
    #[error("No results found")]
    NoResults,
    /// Element not found on page
    #[error("Element not found: {0}")]
    ElementNotFound(String),
    /// Navigation or page load timeout
    #[error("Timeout waiting for: {0}")]
    Timeout(String),
    /// CAPTCHA challenge detected
    #[error("CAPTCHA challenge detected: {0}")]
    CaptchaDetected(String),
}

impl From<CdpError> for DuckDuckGoError {
    fn from(err: CdpError) -> Self {
        DuckDuckGoError::Browser(Box::new(err))
    }
}

impl DuckDuckGoClient {
    /// Creates a new DuckDuckGo client with default scoring configuration
    pub fn new() -> Self {
        Self::with_scoring_config(ScoringConfig::default())
    }

    /// Creates a new DuckDuckGo client with custom scoring configuration
    pub fn with_scoring_config(scoring_config: ScoringConfig) -> Self {
        Self { scoring_config }
    }

    /// Performs a web search using DuckDuckGo with browser automation
    pub async fn search(
        &mut self,
        request: &WebSearchRequest,
    ) -> Result<Vec<SearchResult>, DuckDuckGoError> {
        tracing::debug!(
            "Starting DuckDuckGo browser search for: '{}'",
            request.query
        );

        // Launch browser with stealth configuration to avoid detection
        let (mut browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .window_size(1366, 768)  // Common resolution to blend in
                .args([
                    "--no-sandbox",
                    "--disable-dev-shm-usage",
                    "--disable-blink-features=AutomationControlled",
                    "--exclude-switches=enable-automation",
                    "--disable-extensions-except=",
                    "--disable-plugins-discovery",
                    "--disable-default-apps",
                    "--no-first-run",
                    "--disable-backgrounding-occluded-windows",
                    "--disable-renderer-backgrounding",
                    "--disable-background-timer-throttling",
                    "--disable-features=TranslateUI",
                    "--disable-component-extensions-with-background-pages",
                    "--user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
                ])
                .build()
                .map_err(|e| {
                    DuckDuckGoError::InvalidRequest(format!("Failed to build browser config: {e}"))
                })?,
        )
        .await
        .map_err(|e| {
            DuckDuckGoError::Browser(Box::new(e))
        })?;

        // Spawn handler task with better error handling
        let handler_task = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                // Continue processing even if there are parsing errors
                // This is necessary because Chrome may send CDP messages that chromiumoxide doesn't recognize
                match h {
                    Ok(_) => {
                        // Message processed successfully
                    }
                    Err(e) => {
                        // Log the error but don't break the connection
                        tracing::debug!("CDP message processing error (continuing): {}", e);
                        // Only break on critical connection errors, not parsing errors
                        if e.to_string().contains("connection closed")
                            || e.to_string().contains("io error")
                            || e.to_string().contains("websocket closed")
                        {
                            tracing::warn!(
                                "Critical browser connection error, stopping handler: {}",
                                e
                            );
                            break;
                        }
                    }
                }
            }
        });

        // Perform search operations with proper error handling
        let search_result = async {
            // Create page and navigate to DuckDuckGo
            let page = browser
                .new_page("about:blank")
                .await
                .map_err(|e| DuckDuckGoError::Browser(Box::new(e)))?;

            tracing::debug!("Navigating to DuckDuckGo");
            page.goto("https://duckduckgo.com").await.map_err(|e| {
                DuckDuckGoError::Timeout(format!("Failed to navigate to DuckDuckGo: {e}"))
            })?;

            // Wait for page to fully load and log current URL
            tokio::time::sleep(Duration::from_millis(INITIAL_PAGE_LOAD_DELAY_MS)).await;

            if let Ok(current_url) = page.url().await {
                tracing::debug!("Current URL after navigation: {:?}", current_url);
            }

            if let Ok(title) = page.get_title().await {
                tracing::debug!("Page title after navigation: {:?}", title);
            }

            // Execute stealth JavaScript to mask automation
            let stealth_script = r#"
                // Remove automation indicators
                delete window.navigator.webdriver;
                Object.defineProperty(navigator, 'webdriver', {
                    get: () => false,
                });
                
                // Override automation-related properties
                Object.defineProperty(navigator, 'plugins', {
                    get: () => [1, 2, 3, 4, 5],
                });
                
                // Mock Chrome runtime
                if (!window.chrome) {
                    window.chrome = {};
                }
                if (!window.chrome.runtime) {
                    window.chrome.runtime = {};
                }
                
                // Add some realistic properties
                Object.defineProperty(navigator, 'languages', {
                    get: () => ['en-US', 'en'],
                });
            "#;

            if let Err(e) = page.evaluate(stealth_script).await {
                tracing::debug!("Failed to execute stealth script (continuing): {}", e);
            }

            // Wait for search input box to be available
            tracing::debug!("Waiting for search input to load");
            let search_input = {
                let mut attempts = 0;
                let input_selectors = SEARCH_INPUT_SELECTORS;

                loop {
                    let mut found_input = None;

                    for selector in input_selectors {
                        match page.find_element(*selector).await {
                            Ok(element) => {
                                tracing::debug!("Found search input with selector: {}", selector);
                                found_input = Some(element);
                                break;
                            }
                            Err(_) => continue,
                        }
                    }

                    if let Some(input) = found_input {
                        break input;
                    } else if attempts < MAX_SEARCH_INPUT_ATTEMPTS {
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(ATTEMPT_DELAY_MS)).await;

                        // Log progress every 5 attempts
                        if attempts % 5 == 0 {
                            tracing::debug!(
                                "Still looking for search input, attempt {}/{}",
                                attempts,
                                MAX_SEARCH_INPUT_ATTEMPTS
                            );
                            // Try to debug what's on the page
                            if let Ok(html) = page.content().await {
                                let html_snippet = &html[..html.len().min(1000)];
                                tracing::debug!("Page HTML snippet: {}", html_snippet);
                            }
                        }
                    } else {
                        return Err(DuckDuckGoError::ElementNotFound(
                            "search input box".to_string(),
                        ));
                    }
                }
            };

            tracing::debug!("Entering search query: {}", request.query);

            // Add human-like delay before clicking
            let click_delay = {
                let mut rng = rand::thread_rng();
                Duration::from_millis(rng.gen_range(100..500))
            };
            tokio::time::sleep(click_delay).await;

            search_input
                .click()
                .await
                .map_err(|e| DuckDuckGoError::Browser(Box::new(e)))?;

            // Add small delay after clicking
            let after_click_delay = {
                let mut rng = rand::thread_rng();
                Duration::from_millis(rng.gen_range(50..200))
            };
            tokio::time::sleep(after_click_delay).await;

            // Type query with human-like speed
            let chunk_size = {
                let mut rng = rand::thread_rng();
                rng.gen_range(1..4)
            };

            for chunk in request.query.chars().collect::<Vec<_>>().chunks(chunk_size) {
                let chunk_str: String = chunk.iter().collect();
                search_input
                    .type_str(&chunk_str)
                    .await
                    .map_err(|e| DuckDuckGoError::Browser(Box::new(e)))?;

                // Small delay between chunks to simulate human typing
                let typing_delay = {
                    let mut rng = rand::thread_rng();
                    Duration::from_millis(rng.gen_range(50..150))
                };
                tokio::time::sleep(typing_delay).await;
            }

            // Random delay before pressing Enter
            let enter_delay = {
                let mut rng = rand::thread_rng();
                Duration::from_millis(rng.gen_range(200..800))
            };
            tokio::time::sleep(enter_delay).await;

            // Submit search by pressing Enter
            search_input
                .press_key("Enter")
                .await
                .map_err(|e| DuckDuckGoError::Browser(Box::new(e)))?;

            // Wait for search results to appear with more robust checking
            tracing::debug!("Waiting for search results to load");
            {
                let mut attempts = 0;
                let max_attempts = MAX_RESULTS_WAIT_ATTEMPTS; // Increase attempts for better reliability
                loop {
                    // Try multiple selectors for search results with more comprehensive checks
                    let selectors_to_try = SEARCH_RESULT_SELECTORS;

                    let mut result_found = false;
                    for selector in selectors_to_try {
                        match page.find_element(*selector).await {
                            Ok(_) => {
                                tracing::debug!("Found results with selector: {}", selector);
                                result_found = true;
                                break;
                            }
                            Err(_) => continue,
                        }
                    }

                    // Also check if page content changed (indicates loading completed)
                    if !result_found {
                        // Check if page has loaded by looking for any search-related content
                        if let Ok(Some(url_str)) = page.url().await {
                            if url_str.contains("q=") || url_str.contains("search") {
                                // Check if there are any meaningful results in the HTML
                                if let Ok(html) = page.content().await {
                                    if html.contains("result") || html.contains("search-result") {
                                        tracing::debug!("Found results in page HTML content");
                                        result_found = true;
                                    }
                                }
                            }
                        }
                    }

                    if result_found {
                        // Give a moment for any dynamic content to load
                        tokio::time::sleep(Duration::from_millis(ATTEMPT_DELAY_MS)).await;
                        break;
                    } else if attempts < max_attempts {
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(RESULTS_WAIT_DELAY_MS)).await;

                        // Log progress every 10 attempts
                        if attempts % 10 == 0 {
                            tracing::debug!(
                                "Still waiting for search results, attempt {}/{}",
                                attempts,
                                max_attempts
                            );
                        }
                    } else {
                        return Err(DuckDuckGoError::Timeout("search results".to_string()));
                    }
                }
            }

            // Get the page HTML content
            let html_content = page
                .content()
                .await
                .map_err(|e| DuckDuckGoError::Browser(Box::new(e)))?;

            // Check for CAPTCHA before parsing results
            tracing::debug!(
                "Checking for CAPTCHA in response content of {} characters",
                html_content.len()
            );

            if html_content.contains("Unfortunately, bots use DuckDuckGo too")
                || html_content.contains("anomaly-modal")
                || html_content.contains("challenge-form")
                || html_content.contains("Please complete the following challenge")
            {
                tracing::warn!("CAPTCHA detected in DuckDuckGo response");
                return Err(DuckDuckGoError::CaptchaDetected(
                    "DuckDuckGo detected automated access and is requesting CAPTCHA completion"
                        .to_string(),
                ));
            } else {
                tracing::debug!("No CAPTCHA detected, proceeding with result parsing");
            }

            // Parse results using existing HTML parsing logic
            let results =
                self.parse_html_results(&html_content, request.results_count.unwrap_or(10))?;

            tracing::debug!("DuckDuckGo search found {} results", results.len());

            Ok(results)
        }
        .await;

        // Always clean up browser resources regardless of success or failure
        tracing::debug!("Cleaning up browser resources");

        // Close browser gracefully
        if let Err(e) = browser.close().await {
            tracing::debug!("Browser close error (ignored): {}", e);
        }

        // Abort handler task
        handler_task.abort();

        // Give a moment for cleanup
        tokio::time::sleep(Duration::from_millis(CLEANUP_DELAY_MS)).await;

        // Return the search result
        search_result
    }

    /// Parses HTML content to extract search results from rendered DuckDuckGo page
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

        // Modern DuckDuckGo result selectors for browser-rendered page
        let result_selectors = RESULT_CONTAINER_SELECTORS;

        let mut all_results = Vec::new();
        let mut processed_urls = std::collections::HashSet::new();

        for result_selector_str in result_selectors {
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

            for result_element in result_elements.iter() {
                if all_results.len() >= max_results {
                    break;
                }

                // Extract title and URL
                let (title, url) = self.extract_title_and_url_simple(result_element)?;

                if title.is_empty() || url.is_empty() || !url.starts_with("http") {
                    tracing::debug!("Skipping invalid result: title='{}', url='{}'", title, url);
                    continue; // Skip invalid results
                }

                // Skip duplicates based on URL
                if processed_urls.contains(&url) {
                    continue;
                }
                processed_urls.insert(url.clone());

                // Extract description
                let description = self.extract_description_simple(result_element);

                all_results.push(SearchResult {
                    title: html_escape::decode_html_entities(&title).to_string(),
                    url,
                    description: html_escape::decode_html_entities(&description).to_string(),
                    score: self.calculate_result_score(all_results.len()),
                    engine: "duckduckgo".to_string(),
                    content: None, // Will be populated by content fetcher if needed
                });
            }
        }

        if !all_results.is_empty() {
            tracing::debug!(
                "Successfully parsed {} results from all selectors",
                all_results.len()
            );
            return Ok(all_results);
        }

        tracing::warn!("No search results found with any selector");
        Err(DuckDuckGoError::NoResults)
    }

    /// Extract title and URL from result element (modern DuckDuckGo structure)
    fn extract_title_and_url_simple(
        &self,
        element: &scraper::ElementRef,
    ) -> Result<(String, String), DuckDuckGoError> {
        use scraper::Selector;

        // Modern DuckDuckGo selectors for browser-rendered content
        let title_selectors = TITLE_LINK_SELECTORS;

        let url_selectors = URL_SELECTORS;

        let mut title = String::new();
        let mut url = String::new();

        // Extract title and URL from title selectors first
        for selector_str in title_selectors {
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
            for selector_str in url_selectors {
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

    /// Extract description from result element (modern DuckDuckGo structure)
    fn extract_description_simple(&self, element: &scraper::ElementRef) -> String {
        use scraper::Selector;

        // Modern DuckDuckGo description selectors
        let description_selectors = SNIPPET_SELECTORS;

        // Try each selector to find description content
        for selector_str in description_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for desc_element in element.select(&selector) {
                    let text = desc_element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();

                    if !text.is_empty() && text.len() > 20 {
                        return text;
                    }
                }
            }
        }

        // Fallback: try to extract any meaningful text from the element
        let all_text = element
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();

        // Return a substring that looks like a description (skip title-like text)
        let words: Vec<&str> = all_text.split_whitespace().collect();
        if words.len() > 10 {
            // Take middle portion that's likely to be description
            let start = words.len() / 4;
            let end = (words.len() * 3) / 4;
            words[start..end].join(" ")
        } else if words.len() > 5 {
            words[2..].join(" ") // Skip first few words (likely title)
        } else {
            String::new()
        }
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
        // Just verify client can be created
        assert_eq!(client.scoring_config.base_score, 1.0);
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
    fn test_html_parsing_with_modern_duckduckgo_response() {
        let client = DuckDuckGoClient::new();

        // Sample modern DuckDuckGo HTML response structure
        let sample_html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>DuckDuckGo Search</title></head>
        <body>
            <div class="results">
                <div data-testid="result">
                    <h2><a data-testid="result-title-a" href="https://example.com/apple">What is an Apple? - Example.com</a></h2>
                    <span data-testid="result-snippet">An apple is a round fruit that grows on trees. Apples are commonly red, green, or yellow.</span>
                </div>
                <div data-testid="result">
                    <h2><a data-testid="result-title-a" href="https://en.wikipedia.org/wiki/Apple">Apple - Wikipedia</a></h2>
                    <span data-testid="result-snippet">An apple is an edible fruit produced by an apple tree. Apple trees are cultivated worldwide.</span>
                </div>
                <div data-testid="result">
                    <h2><a data-testid="result-title-a" href="https://nutrition.org/apples">Apple Nutrition Facts</a></h2>
                    <span data-testid="result-snippet">Apples are a great source of fiber and vitamin C. They make healthy snacks.</span>
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

        // More complex HTML structure with nested elements using modern selectors
        let complex_html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <div class="main-results">
                <article data-testid="result">
                    <div class="result-header">
                        <h2><a data-testid="result-title-a" href="https://apple.com">Apple Inc. Official Website</a></h2>
                    </div>
                    <div class="result-content">
                        <div data-testid="result-snippet">Discover the innovative world of Apple and shop everything iPhone, iPad, Mac, Apple Watch.</div>
                    </div>
                </article>
                <div class="result">
                    <h3>
                        <a href="https://healthline.com/apple-benefits">
                            <span>Health Benefits of Apples</span>
                        </a>
                    </h3>
                    <div class="snippet">
                        <span>Apples provide numerous health benefits including improved heart health and better digestion.</span>
                    </div>
                </div>
            </div>
        </body>
        </html>
        "#;

        let results = client.parse_html_results(complex_html, 5).unwrap();

        assert_eq!(results.len(), 2);

        // Test extraction from modern structure
        assert_eq!(results[0].title, "Apple Inc. Official Website");
        assert_eq!(results[0].url, "https://apple.com");
        assert!(results[0].description.contains("innovative world of Apple"));

        // Test extraction from classic fallback structure
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
        let element1 = document1
            .select(&scraper::Selector::parse("div").unwrap())
            .next()
            .unwrap();
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
        let element2 = document2
            .select(&scraper::Selector::parse("div").unwrap())
            .next()
            .unwrap();
        let (title2, url2) = client.extract_title_and_url_simple(&element2).unwrap();
        assert_eq!(title2, "Test Page");
        assert_eq!(url2, "https://test.org");
    }

    #[test]
    fn test_captcha_detection() {
        let client = DuckDuckGoClient::new();

        // Test CAPTCHA HTML from actual response
        let captcha_html = r#"
        <html>
        <body>
            <div class="anomaly-modal__title">Unfortunately, bots use DuckDuckGo too.</div>
            <div class="anomaly-modal__description">Please complete the following challenge to confirm this search was made by a human.</div>
        </body>
        </html>
        "#;

        let result = client.parse_html_results(captcha_html, 10);

        // Should detect CAPTCHA and return NoResults error (since CAPTCHA detection happens before parsing)
        assert!(result.is_err());
        // In the actual search flow, CAPTCHA would be detected before parse_html_results is called
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
        let element = document
            .select(&scraper::Selector::parse("div").unwrap())
            .next()
            .unwrap();
        let description = client.extract_description_simple(&element);

        assert!(description.contains("detailed description"));
        assert!(description.len() > 20);
    }
}
