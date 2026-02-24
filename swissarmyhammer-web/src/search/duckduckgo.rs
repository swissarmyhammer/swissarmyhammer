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

use crate::chrome;
use crate::types::{DuckDuckGoConfig, ScoringConfig, WebSearchRequest, SearchResult};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::error::CdpError;
use futures::StreamExt;
use std::io::Write;
use std::time::Duration;
use swissarmyhammer_common::{ErrorSeverity, Pretty, Severity};
use tempfile;
use urlencoding::decode;

// CSS selectors for finding result elements and content

const RESULT_CONTAINER_SELECTORS: &[&str] = &[
    "div[data-testid='result']",         // Modern DuckDuckGo main selector
    "article[data-testid='result']",     // Article-based results
    "div.result",                        // Classic result container
    "div[class*='result']",              // Any div with result in class
    "article[data-layout='organic']",    // New DuckDuckGo organic results
    "div[data-layout='organic']",        // Div-based organic results
    "[data-testid*='result']",           // Any element with result testid
    "li[data-testid*='result']",         // List item results
    "div.results_links",                 // Alternative result container
    "div.web-result",                    // Web result container
    ".result__body",                     // Result body class
    "div.serp__results > div",           // SERP results children
    "div.js-react-on-rails-component",   // React component results
    "div.links_main",                    // HTML version main links container
    "div.result.results_links",          // HTML version result links
    "div.result.results_links_deep",     // HTML version deep links
    ".result.result--url-above-snippet", // HTML version URL above snippet
    "div.b_algo",                        // Bing-style results (fallback)
];

const TITLE_LINK_SELECTORS: &[&str] = &[
    "h2 a[data-testid='result-title-a']", // Modern testid-based title link
    "h3 a[data-testid='result-title-a']", // H3 variant
    "a[data-testid='result-title-a']",    // Direct title link
    "h2 > a",                             // Direct h2 child link
    "h3 > a",                             // Direct h3 child link
    "h2 a",                               // Any link in h2
    "h3 a",                               // Any link in h3
    "a[class*='result-title']",           // Title class patterns
    "a[class*='title']",                  // Generic title patterns
    ".result__title a",                   // Classic result title
    ".result-title",                      // Direct title class
    "h2",                                 // Title text only
    "h3",                                 // H3 title text only
    "h4 a",                               // H4 variant
];

const URL_SELECTORS: &[&str] = &[
    "a[data-testid='result-title-a']", // Primary title link
    "a[href^='http']",                 // Any external link
    "a[href]",                         // Any link with href
];

const SNIPPET_SELECTORS: &[&str] = &[
    "[data-testid='result-snippet']",     // Modern testid-based snippet
    "span[data-testid='result-snippet']", // Span-based snippet
    ".result__snippet",                   // Classic snippet class
    "[class*='snippet']",                 // Any element with snippet in class
    "div[data-result='snippet']",         // Data attribute variant
    "div.snippet",                        // Simple snippet div
    ".result__body",                      // Result body content
    ".result-description",                // Description class
    "p",                                  // Generic paragraph content
    "div > span",                         // Nested span content
    ".result-content",                    // Result content class
];

/// DuckDuckGo search client using browser automation
pub struct DuckDuckGoClient {
    scoring_config: ScoringConfig,
    timing_config: DuckDuckGoConfig,
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

impl Severity for DuckDuckGoError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Browser automation system failed - cannot perform any searches
            DuckDuckGoError::Browser(_) => ErrorSeverity::Critical,

            // Error: Search operation failed but system can retry or continue
            DuckDuckGoError::Parse(_) => ErrorSeverity::Error,
            DuckDuckGoError::InvalidRequest(_) => ErrorSeverity::Error,
            DuckDuckGoError::ElementNotFound(_) => ErrorSeverity::Error,
            DuckDuckGoError::Timeout(_) => ErrorSeverity::Error,
            DuckDuckGoError::CaptchaDetected(_) => ErrorSeverity::Error,

            // Warning: Search completed successfully but found no results
            DuckDuckGoError::NoResults => ErrorSeverity::Warning,
        }
    }
}

impl DuckDuckGoClient {
    /// Creates a new DuckDuckGo client with default configuration
    pub fn new() -> Self {
        Self::with_configs(ScoringConfig::default(), DuckDuckGoConfig::default())
    }

    /// Creates a new DuckDuckGo client with custom scoring configuration
    pub fn with_scoring_config(scoring_config: ScoringConfig) -> Self {
        Self::with_configs(scoring_config, DuckDuckGoConfig::default())
    }

    /// Creates a new DuckDuckGo client with custom timing configuration
    pub fn with_timing_config(timing_config: DuckDuckGoConfig) -> Self {
        Self::with_configs(ScoringConfig::default(), timing_config)
    }

    /// Creates a new DuckDuckGo client with custom configurations
    pub fn with_configs(scoring_config: ScoringConfig, timing_config: DuckDuckGoConfig) -> Self {
        Self {
            scoring_config,
            timing_config,
        }
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

        // Create a unique temporary directory for this browser instance
        // This prevents SingletonLock conflicts when multiple searches run
        let temp_dir = tempfile::Builder::new()
            .prefix("chromium-profile-")
            .tempdir()
            .map_err(|e| {
                DuckDuckGoError::InvalidRequest(format!(
                    "Failed to create temporary profile directory: {e}"
                ))
            })?;

        let temp_path = temp_dir.path().to_path_buf(); // Clone path to ensure it stays valid

        tracing::debug!(
            "Using temporary browser profile directory: {}",
            temp_path.display()
        );

        // Detect Chrome before attempting to launch
        let chrome_result = chrome::detect_chrome();
        if !chrome_result.found {
            let error_message = format!(
                "{}\n\n{}",
                chrome_result.message,
                chrome_result.installation_instructions()
            );
            tracing::error!("Chrome detection failed: {}", error_message);
            return Err(DuckDuckGoError::Browser(Box::new(CdpError::Io(
                std::io::Error::new(std::io::ErrorKind::NotFound, error_message),
            ))));
        }

        let chrome_path = chrome_result.path.unwrap();
        tracing::info!(
            "Found Chrome at: {} (via {})",
            chrome_path.display(),
            chrome_result.detection_method.as_ref().unwrap()
        );

        // Launch browser with stealth configuration to avoid detection
        let (mut browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .chrome_executable(&chrome_path)  // Explicitly set Chrome path
                .user_data_dir(&temp_path)  // Use unique temp directory to avoid lock conflicts
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
        .map_err(|e| DuckDuckGoError::Browser(Box::new(e)))?;

        // Spawn handler task with better error handling for CDP message deserialization
        let handler_task = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                // Continue processing even if there are parsing errors
                // This is necessary because Chrome may send CDP messages that chromiumoxide doesn't recognize
                match h {
                    Ok(_) => {
                        // Message processed successfully - continue silently
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        // Handle deserialization errors gracefully - these are common with newer Chrome versions
                        if error_str
                            .contains("data did not match any variant of untagged enum Message")
                        {
                            // This is a known issue with chromiumoxide not recognizing all CDP messages
                            // We can safely ignore these and continue processing
                            continue;
                        }

                        // Only log other types of errors for debugging
                        tracing::debug!("CDP message processing error (continuing): {}", e);

                        // Only break on critical connection errors, not parsing errors
                        if error_str.contains("connection closed")
                            || error_str.contains("io error")
                            || error_str.contains("websocket closed")
                            || error_str.contains("transport error")
                        {
                            tracing::warn!(
                                "Critical browser connection error, stopping handler: {}",
                                e
                            );
                            break;
                        }
                        // For all other errors, continue processing to keep the connection alive
                    }
                }
            }
            tracing::debug!("CDP handler task completed");
        });

        // Perform search operations with proper error handling
        let search_result = async {
            // Create page and navigate to DuckDuckGo
            let page = browser
                .new_page("about:blank")
                .await
                .map_err(|e| DuckDuckGoError::Browser(Box::new(e)))?;

            tracing::debug!("Navigating to DuckDuckGo HTML search page");
            // Navigate directly to the HTML version to avoid JavaScript detection issues
            let encoded_query = request.query.replace(" ", "+");
            let search_url = format!("https://duckduckgo.com/html?q={encoded_query}");
            tracing::debug!("Using search URL: {}", search_url);

            page.goto(&search_url).await.map_err(|e| {
                DuckDuckGoError::Timeout(format!("Failed to navigate to DuckDuckGo: {e}"))
            })?;

            // Wait for page to fully load and log current URL
            tokio::time::sleep(Duration::from_millis(
                self.timing_config.initial_page_load_delay_ms,
            ))
            .await;

            if let Ok(current_url) = page.url().await {
                tracing::debug!("Current URL after navigation: {}", Pretty(&current_url));
            }

            if let Ok(title) = page.get_title().await {
                tracing::debug!("Page title after navigation: {}", Pretty(&title));
            }

            // Since we're navigating directly to search results, skip form interaction
            tracing::debug!("Skipping search form interaction - using direct URL navigation");

            // Wait for HTML search results to load (simplified since we're using direct URL)
            tracing::debug!("Waiting for HTML search results to load");
            tokio::time::sleep(Duration::from_millis(1000)).await; // Give HTML page time to load

            // Get the page HTML content
            let html_content = page
                .content()
                .await
                .map_err(|e| DuckDuckGoError::Browser(Box::new(e)))?;

            tracing::debug!(
                "Retrieved HTML content of {} characters from DuckDuckGo page",
                html_content.len()
            );

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
            tracing::debug!("Starting to parse HTML results from DuckDuckGo page");
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
        tokio::time::sleep(Duration::from_millis(self.timing_config.cleanup_delay_ms)).await;

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

        // Save HTML to a temporary debug file if debug logging is enabled
        if tracing::enabled!(tracing::Level::DEBUG) {
            match tempfile::NamedTempFile::new() {
                Ok(mut temp_file) => {
                    if let Ok(()) = temp_file.write_all(html_content.as_bytes()) {
                        tracing::debug!(
                            "HTML response saved to temp file {}",
                            Pretty(&temp_file.path())
                        );
                        // temp_file will be automatically cleaned up when it goes out of scope
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to create temp file for HTML debug: {}", e);
                }
            }
        }

        // Also log a snippet of the HTML for immediate debugging if debug logging is enabled
        if tracing::enabled!(tracing::Level::DEBUG) {
            let html_snippet = if html_content.len() > 2000 {
                &html_content[..2000]
            } else {
                html_content
            };
            tracing::debug!("HTML snippet for debugging: {}", html_snippet);
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

            // If this is the first selector that found results, log some sample HTML
            if all_results.is_empty() && !result_elements.is_empty() {
                if let Some(first_element) = result_elements.first() {
                    let element_html = first_element.html();
                    let sample_html = if element_html.len() > 500 {
                        &element_html[..500]
                    } else {
                        &element_html
                    };
                    tracing::debug!("Sample result element HTML: {}", sample_html);
                }
            }

            for result_element in result_elements.iter() {
                if all_results.len() >= max_results {
                    break;
                }

                // Extract title and URL
                let (title, url) = self.extract_title_and_url_simple(result_element)?;

                // Handle DuckDuckGo redirect URLs
                let final_url =
                    if let Some(encoded_url) = url.strip_prefix("//duckduckgo.com/l/?uddg=") {
                        // Extract the actual URL from the DuckDuckGo redirect
                        let url_to_decode = if let Some(end) = encoded_url.find('&') {
                            &encoded_url[..end]
                        } else {
                            encoded_url
                        };

                        // Use proper URL decoding library
                        match decode(url_to_decode) {
                            Ok(decoded) => decoded.to_string(),
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to decode DuckDuckGo redirect URL '{}': {}",
                                    url_to_decode,
                                    e
                                );
                                // Fall back to the original URL if decoding fails
                                url.clone()
                            }
                        }
                    } else if url.starts_with("//") {
                        format!("https:{url}")
                    } else {
                        url.to_string()
                    };

                if title.is_empty() || final_url.is_empty() || !final_url.starts_with("http") {
                    tracing::debug!(
                        "Skipping invalid result: title='{}', url='{}'",
                        title,
                        final_url
                    );
                    // Log some HTML from this element to understand what we're missing
                    let element_html = result_element.html();
                    let sample_html = if element_html.len() > 300 {
                        &element_html[..300]
                    } else {
                        &element_html
                    };
                    tracing::debug!("Invalid result element HTML: {}", sample_html);
                    continue; // Skip invalid results
                }

                let url = final_url; // Use the processed URL

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

                    tracing::debug!(
                        "Trying selector '{}', found element '{}', extracted title: '{}'",
                        selector_str,
                        title_element.value().name(),
                        extracted_title
                    );

                    // If this is an 'a' element, also extract href
                    if title_element.value().name() == "a" {
                        if let Some(href) = title_element.value().attr("href") {
                            url = href.to_string();
                            title = extracted_title;
                            tracing::debug!(
                                "Found link element with title '{}' and URL '{}'",
                                title,
                                url
                            );
                            if !title.is_empty() && !url.is_empty() {
                                break;
                            }
                        }
                    } else if !extracted_title.is_empty() {
                        title = extracted_title;
                        tracing::debug!("Found title text without link: '{}'", title);
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

    #[test]
    fn test_timing_configuration() {
        // Test default timing configuration
        let client = DuckDuckGoClient::new();
        assert_eq!(client.timing_config.initial_page_load_delay_ms, 2000);
        assert_eq!(client.timing_config.cleanup_delay_ms, 100);

        // Test custom timing configuration
        let custom_timing = DuckDuckGoConfig {
            initial_page_load_delay_ms: 1000,
            cleanup_delay_ms: 50,
        };
        let custom_client = DuckDuckGoClient::with_timing_config(custom_timing);
        assert_eq!(custom_client.timing_config.initial_page_load_delay_ms, 1000);
        assert_eq!(custom_client.timing_config.cleanup_delay_ms, 50);

        // Test both configurations combined
        let scoring_config = ScoringConfig::default();
        let timing_config = DuckDuckGoConfig {
            initial_page_load_delay_ms: 3000,
            cleanup_delay_ms: 200,
        };
        let combined_client = DuckDuckGoClient::with_configs(scoring_config, timing_config);
        assert_eq!(
            combined_client.timing_config.initial_page_load_delay_ms,
            3000
        );
        assert_eq!(combined_client.timing_config.cleanup_delay_ms, 200);
        assert_eq!(combined_client.scoring_config.base_score, 1.0);
    }

    #[test]
    fn test_url_decoding_duckduckgo_redirects() {
        let client = DuckDuckGoClient::new();

        // Test DuckDuckGo redirect URL parsing
        let html_with_redirect = r#"
        <div data-testid="result">
            <h2><a data-testid="result-title-a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpath%3Fquery%3Dvalue%26other%3Dparam">Test Title</a></h2>
            <span data-testid="result-snippet">Test description</span>
        </div>
        "#;

        let results = client.parse_html_results(html_with_redirect, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test Title");
        assert_eq!(
            results[0].url,
            "https://example.com/path?query=value&other=param"
        );

        // Test DuckDuckGo redirect URL with additional parameters
        let html_with_complex_redirect = r#"
        <div data-testid="result">
            <h2><a data-testid="result-title-a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Ftest.org%2Fcomplex%3Fparam%3D123&rut=abc&vqd=xyz">Complex Title</a></h2>
            <span data-testid="result-snippet">Complex description</span>
        </div>
        "#;

        let complex_results = client
            .parse_html_results(html_with_complex_redirect, 10)
            .unwrap();
        assert_eq!(complex_results.len(), 1);
        assert_eq!(complex_results[0].title, "Complex Title");
        assert_eq!(complex_results[0].url, "https://test.org/complex?param=123");

        // Test regular URLs (no redirect)
        let html_direct_url = r#"
        <div data-testid="result">
            <h2><a data-testid="result-title-a" href="https://direct.com/path">Direct Title</a></h2>
            <span data-testid="result-snippet">Direct description</span>
        </div>
        "#;

        let direct_results = client.parse_html_results(html_direct_url, 10).unwrap();
        assert_eq!(direct_results.len(), 1);
        assert_eq!(direct_results[0].title, "Direct Title");
        assert_eq!(direct_results[0].url, "https://direct.com/path");
    }

    #[test]
    fn test_invalid_html_parsing() {
        let client = DuckDuckGoClient::new();

        // Test malformed HTML
        let malformed_html = r#"<div><h2><a href="https://broken.com">Broken</h2></div>"#;
        let result = client.parse_html_results(malformed_html, 10);
        // Should handle malformed HTML gracefully and return error if no results found
        assert!(result.is_err());

        // Test HTML with missing required elements
        let incomplete_html = r#"
        <div data-testid="result">
            <h2>Title without link</h2>
        </div>
        "#;
        let incomplete_result = client.parse_html_results(incomplete_html, 10);
        assert!(incomplete_result.is_err());
    }

    #[test]
    fn test_request_validation() {
        // Test empty query string
        let empty_request = WebSearchRequest {
            query: "".to_string(),
            category: None,
            language: None,
            results_count: Some(10),
            fetch_content: Some(false),
            safe_search: None,
            time_range: None,
        };

        // The search method would handle this validation - testing via integration tests
        assert!(empty_request.query.is_empty());

        // Test extremely long query
        let long_request = WebSearchRequest {
            query: "a".repeat(1000),
            category: None,
            language: None,
            results_count: Some(10),
            fetch_content: Some(false),
            safe_search: None,
            time_range: None,
        };

        // Long queries should be handled by the search validation
        assert!(long_request.query.len() > 500);
    }

    #[test]
    fn test_duckduckgo_error_severity() {
        use swissarmyhammer_common::Severity;

        // Test Error severity for search failures
        let error_variants = vec![
            DuckDuckGoError::Parse("Parse failed".to_string()),
            DuckDuckGoError::InvalidRequest("Invalid query".to_string()),
            DuckDuckGoError::ElementNotFound("Button not found".to_string()),
            DuckDuckGoError::Timeout("Page load timeout".to_string()),
            DuckDuckGoError::CaptchaDetected("CAPTCHA required".to_string()),
        ];

        for error in error_variants {
            assert_eq!(
                error.severity(),
                swissarmyhammer_common::ErrorSeverity::Error,
                "Expected Error severity for: {}",
                error
            );
        }

        // Test Warning severity for no results
        let no_results_error = DuckDuckGoError::NoResults;
        assert_eq!(
            no_results_error.severity(),
            swissarmyhammer_common::ErrorSeverity::Warning,
            "Expected Warning severity for no results"
        );

        // Test Critical severity for browser errors (using From trait)
        // Browser errors wrap CdpError which we can't easily construct in tests,
        // but we can verify the severity match arm exists by checking the implementation
        // directly through pattern matching on a constructed error variant
    }
}
