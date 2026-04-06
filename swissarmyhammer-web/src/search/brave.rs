//! Brave Search client implementation
//!
//! Performs web searches by fetching Brave Search HTML results via reqwest
//! and parsing them with the scraper crate. No browser automation needed.

use crate::types::{ScoringConfig, SearchResult, WebSearchRequest};
use swissarmyhammer_common::{ErrorSeverity, Severity};

/// Brave Search client using direct HTTP requests
pub struct BraveSearchClient {
    scoring_config: ScoringConfig,
    http_client: reqwest::Client,
    /// Base URL override for testing; defaults to "https://search.brave.com"
    #[cfg(test)]
    base_url: String,
}

/// Errors that can occur during Brave search operations
#[derive(Debug, thiserror::Error)]
pub enum BraveSearchError {
    /// HTTP request error
    #[error("HTTP error: {0}")]
    Http(String),
    /// HTML parsing error
    #[error("Parse error: {0}")]
    Parse(String),
    /// Invalid search request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    /// Search completed but no results were found
    #[error("No results found")]
    NoResults,
}

impl Severity for BraveSearchError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            BraveSearchError::Http(_) => ErrorSeverity::Critical,
            BraveSearchError::Parse(_) => ErrorSeverity::Error,
            BraveSearchError::InvalidRequest(_) => ErrorSeverity::Error,
            BraveSearchError::NoResults => ErrorSeverity::Warning,
        }
    }
}

impl BraveSearchClient {
    /// Creates a new Brave Search client with default configuration
    pub fn new() -> Self {
        Self::with_scoring_config(ScoringConfig::default())
    }

    /// Creates a new Brave Search client with custom scoring configuration
    pub fn with_scoring_config(scoring_config: ScoringConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            scoring_config,
            http_client,
            #[cfg(test)]
            base_url: "https://search.brave.com".to_string(),
        }
    }

    /// Creates a client that sends requests to a custom base URL (for testing only)
    #[cfg(test)]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Performs a web search using Brave Search via direct HTTP request
    pub async fn search(
        &self,
        request: &WebSearchRequest,
    ) -> Result<Vec<SearchResult>, BraveSearchError> {
        tracing::debug!("Starting Brave search for: '{}'", request.query);

        let encoded_query = urlencoding::encode(&request.query);
        #[cfg(test)]
        let base = &self.base_url;
        #[cfg(not(test))]
        let base = "https://search.brave.com";
        let url = format!("{base}/search?q={encoded_query}&source=web");

        let response = self
            .http_client
            .get(&url)
            .header("Accept", "text/html")
            .send()
            .await
            .map_err(|e| BraveSearchError::Http(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(BraveSearchError::Http(format!(
                "Brave Search returned status {}",
                response.status()
            )));
        }

        let html_content = response
            .text()
            .await
            .map_err(|e| BraveSearchError::Http(format!("Failed to read response body: {e}")))?;

        tracing::debug!("Retrieved {} bytes from Brave Search", html_content.len());

        let max_results = request.results_count.unwrap_or(10);
        self.parse_html_results(&html_content, max_results)
    }

    /// Parses Brave Search HTML to extract search results
    fn parse_html_results(
        &self,
        html_content: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>, BraveSearchError> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html_content);

        // Brave uses data-pos attributes on result containers
        let snippet_sel = Selector::parse("[data-pos]")
            .map_err(|e| BraveSearchError::Parse(format!("Invalid selector: {e}")))?;

        let title_sel = Selector::parse("a .title").ok();
        let title_a_sel = Selector::parse("a[href]").ok();
        let desc_sel = Selector::parse(".snippet-description").ok();
        let desc_fallback_sel = Selector::parse("p").ok();

        let mut results = Vec::new();
        let mut seen_urls = std::collections::HashSet::new();

        for element in document.select(&snippet_sel) {
            if results.len() >= max_results {
                break;
            }

            // Extract title — try .title class inside a link first
            let mut title = String::new();
            let mut url = String::new();

            if let Some(ref sel) = title_sel {
                if let Some(title_el) = element.select(sel).next() {
                    title = title_el
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();
                    // Walk up to the <a> parent to get href
                    if let Some(ref a_sel) = title_a_sel {
                        if let Some(a_el) = element.select(a_sel).next() {
                            if let Some(href) = a_el.value().attr("href") {
                                if href.starts_with("http") {
                                    url = href.to_string();
                                }
                            }
                        }
                    }
                }
            }

            // Fallback: find first <a> with href starting with http
            if url.is_empty() {
                if let Some(ref a_sel) = title_a_sel {
                    for a_el in element.select(a_sel) {
                        if let Some(href) = a_el.value().attr("href") {
                            if href.starts_with("http") {
                                url = href.to_string();
                                if title.is_empty() {
                                    title = a_el
                                        .text()
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                        .trim()
                                        .to_string();
                                }
                                break;
                            }
                        }
                    }
                }
            }

            if title.is_empty() || url.is_empty() {
                continue;
            }

            // Deduplicate
            if seen_urls.contains(&url) {
                continue;
            }
            seen_urls.insert(url.clone());

            // Extract description
            let mut description = String::new();
            if let Some(ref sel) = desc_sel {
                if let Some(desc_el) = element.select(sel).next() {
                    description = desc_el
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();
                }
            }
            if description.is_empty() {
                if let Some(ref sel) = desc_fallback_sel {
                    for p_el in element.select(sel) {
                        let text = p_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                        if text.len() > 20 {
                            description = text;
                            break;
                        }
                    }
                }
            }

            results.push(SearchResult {
                title: html_escape::decode_html_entities(&title).to_string(),
                url,
                description: html_escape::decode_html_entities(&description).to_string(),
                score: self.calculate_result_score(results.len()),
                engine: "brave".to_string(),
                content: None,
            });
        }

        if results.is_empty() {
            tracing::warn!("No search results found in Brave HTML response");
            return Err(BraveSearchError::NoResults);
        }

        tracing::debug!("Parsed {} results from Brave Search", results.len());
        Ok(results)
    }

    /// Calculates result score based on position
    fn calculate_result_score(&self, index: usize) -> f64 {
        let config = &self.scoring_config;
        if config.exponential_decay {
            let score = config.base_score * (-config.decay_rate * index as f64).exp();
            score.max(config.min_score)
        } else {
            let score = config.base_score - (config.position_penalty * index as f64);
            score.max(config.min_score)
        }
    }
}

impl Default for BraveSearchClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new() {
        let client = BraveSearchClient::new();
        assert_eq!(client.scoring_config.base_score, 1.0);
    }

    #[test]
    fn test_scoring_configuration() {
        let client = BraveSearchClient::new();
        assert_eq!(client.calculate_result_score(0), 1.0);
        assert_eq!(client.calculate_result_score(1), 0.95);
        assert_eq!(client.calculate_result_score(2), 0.90);
    }

    #[test]
    fn test_parse_brave_html() {
        let client = BraveSearchClient::new();

        let html = r#"
        <html><body>
            <div data-pos="1">
                <a href="https://www.rust-lang.org/">
                    <span class="title">Rust Programming Language</span>
                </a>
                <p class="snippet-description">A systems programming language focused on safety and performance.</p>
            </div>
            <div data-pos="2">
                <a href="https://doc.rust-lang.org/book/">
                    <span class="title">The Rust Programming Language Book</span>
                </a>
                <p class="snippet-description">The official Rust book for learning the language.</p>
            </div>
        </body></html>
        "#;

        let results = client.parse_html_results(html, 10).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://www.rust-lang.org/");
        assert!(results[0].description.contains("safety and performance"));
        assert_eq!(results[0].engine, "brave");
        assert_eq!(results[0].score, 1.0);
        assert_eq!(results[1].score, 0.95);
    }

    #[test]
    fn test_parse_no_results() {
        let client = BraveSearchClient::new();
        let html = "<html><body><div>No results</div></body></html>";
        let result = client.parse_html_results(html, 10);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BraveSearchError::NoResults));
    }

    #[test]
    fn test_deduplicates_urls() {
        let client = BraveSearchClient::new();
        let html = r#"
        <html><body>
            <div data-pos="1">
                <a href="https://example.com"><span class="title">First</span></a>
            </div>
            <div data-pos="2">
                <a href="https://example.com"><span class="title">Duplicate</span></a>
            </div>
            <div data-pos="3">
                <a href="https://other.com"><span class="title">Other</span></a>
            </div>
        </body></html>
        "#;
        let results = client.parse_html_results(html, 10).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "First");
        assert_eq!(results[1].title, "Other");
    }

    #[test]
    fn test_error_severity() {
        use swissarmyhammer_common::Severity;

        assert_eq!(
            BraveSearchError::Http("fail".into()).severity(),
            ErrorSeverity::Critical
        );
        assert_eq!(
            BraveSearchError::Parse("fail".into()).severity(),
            ErrorSeverity::Error
        );
        assert_eq!(
            BraveSearchError::NoResults.severity(),
            ErrorSeverity::Warning
        );
    }

    #[test]
    fn test_title_fallback_from_anchor_text() {
        // When there is no .title span, the code should fall back to <a> text
        let client = BraveSearchClient::new();
        let html = r#"
        <html><body>
            <div data-pos="1">
                <a href="https://example.com/page">Example Page Title</a>
            </div>
        </body></html>
        "#;
        let results = client.parse_html_results(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example Page Title");
        assert_eq!(results[0].url, "https://example.com/page");
    }

    #[test]
    fn test_description_fallback_to_paragraph() {
        // When no .snippet-description exists, fall back to <p> with >20 chars
        let client = BraveSearchClient::new();
        let html = r#"
        <html><body>
            <div data-pos="1">
                <a href="https://example.com">
                    <span class="title">Example</span>
                </a>
                <p>Too short</p>
                <p>This is a longer paragraph that should be used as the description fallback.</p>
            </div>
        </body></html>
        "#;
        let results = client.parse_html_results(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("longer paragraph"));
        // The short <p> should be skipped (<=20 chars)
        assert!(!results[0].description.contains("Too short"));
    }

    #[test]
    fn test_exponential_decay_scoring() {
        let config = ScoringConfig {
            base_score: 1.0,
            position_penalty: 0.05,
            min_score: 0.01,
            exponential_decay: true,
            decay_rate: 0.5,
        };
        let client = BraveSearchClient::with_scoring_config(config);

        // Index 0: base_score * exp(-0.5 * 0) = 1.0 * 1.0 = 1.0
        let score0 = client.calculate_result_score(0);
        assert!((score0 - 1.0).abs() < 1e-10);

        // Index 1: 1.0 * exp(-0.5 * 1) = exp(-0.5) ≈ 0.6065
        let score1 = client.calculate_result_score(1);
        let expected = (-0.5_f64).exp();
        assert!(
            (score1 - expected).abs() < 1e-10,
            "expected {expected}, got {score1}"
        );

        // Index 2: 1.0 * exp(-0.5 * 2) = exp(-1.0) ≈ 0.3679
        let score2 = client.calculate_result_score(2);
        let expected2 = (-1.0_f64).exp();
        assert!(
            (score2 - expected2).abs() < 1e-10,
            "expected {expected2}, got {score2}"
        );

        // Scores should decrease monotonically
        assert!(score0 > score1);
        assert!(score1 > score2);
    }

    #[test]
    fn test_default_impl() {
        let client = BraveSearchClient::default();
        // Default should use the same config as new()
        assert_eq!(client.scoring_config.base_score, 1.0);
        assert!(!client.scoring_config.exponential_decay);
        assert_eq!(client.scoring_config.position_penalty, 0.05);
    }

    #[test]
    fn test_max_results_limiting() {
        let client = BraveSearchClient::new();
        let html = r#"
        <html><body>
            <div data-pos="1">
                <a href="https://first.com"><span class="title">First Result</span></a>
            </div>
            <div data-pos="2">
                <a href="https://second.com"><span class="title">Second Result</span></a>
            </div>
            <div data-pos="3">
                <a href="https://third.com"><span class="title">Third Result</span></a>
            </div>
        </body></html>
        "#;
        let results = client.parse_html_results(html, 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "First Result");
    }

    #[test]
    fn test_parse_result_elements_without_valid_links_are_skipped() {
        // data-pos elements that have no <a href> starting with http should be skipped (line 174)
        let client = BraveSearchClient::new();
        let html = r#"
        <html><body>
            <div data-pos="1">
                <span>No link here at all</span>
            </div>
            <div data-pos="2">
                <a href="/relative/path"><span class="title">Relative URL</span></a>
            </div>
            <div data-pos="3">
                <a href="https://valid.com"><span class="title">Valid Result</span></a>
            </div>
        </body></html>
        "#;
        // First two elements are skipped; only the third is valid
        let results = client.parse_html_results(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://valid.com");
    }

    #[test]
    fn test_parse_element_with_no_title_and_no_link_is_skipped() {
        // data-pos element with an <a> that has no text and no http href is skipped
        let client = BraveSearchClient::new();
        let html = r#"
        <html><body>
            <div data-pos="1">
                <a href="ftp://unsupported.com">FTP link</a>
            </div>
            <div data-pos="2">
                <a href="https://ok.com">OK Title</a>
            </div>
        </body></html>
        "#;
        let results = client.parse_html_results(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "OK Title");
    }

    #[tokio::test]
    async fn test_search_http_non_success_status() {
        // When the Brave API returns a non-2xx status, search() should return an Http error
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex("/search.*"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&mock_server)
            .await;

        let client = BraveSearchClient::new().with_base_url(mock_server.uri());
        let request = WebSearchRequest {
            query: "test query".to_string(),
            category: None,
            language: None,
            results_count: Some(5),
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        let result = client.search(&request).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BraveSearchError::Http(msg) => {
                assert!(msg.contains("429"), "Expected 429 in error, got: {msg}");
            }
            other => panic!("Expected Http error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_search_http_server_error() {
        // When the Brave API returns a 503 status, search() should return an Http error
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex("/search.*"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let client = BraveSearchClient::new().with_base_url(mock_server.uri());
        let request = WebSearchRequest {
            query: "rust programming".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        let result = client.search(&request).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BraveSearchError::Http(msg) => {
                assert!(
                    msg.contains("503"),
                    "Expected status 503 in error message, got: {msg}"
                );
            }
            other => panic!("Expected Http error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_search_successful_parse_with_results() {
        // Test the happy path: a successful HTTP response with valid HTML is parsed correctly
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let html = r#"
        <html><body>
            <div data-pos="1">
                <a href="https://example.com"><span class="title">Example Result</span></a>
                <p class="snippet-description">A description of the example result.</p>
            </div>
        </body></html>
        "#;

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex("/search.*"))
            .respond_with(ResponseTemplate::new(200).set_body_string(html))
            .mount(&mock_server)
            .await;

        let client = BraveSearchClient::new().with_base_url(mock_server.uri());
        let request = WebSearchRequest {
            query: "example".to_string(),
            category: None,
            language: None,
            results_count: Some(10),
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        let result = client.search(&request).await;
        assert!(result.is_ok(), "Expected Ok, got: {result:?}");
        let results = result.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example Result");
        assert_eq!(results[0].url, "https://example.com");
    }

    #[tokio::test]
    async fn test_search_returns_no_results_error_on_empty_html() {
        // When the server returns HTML with no parseable results, NoResults is returned
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let html = "<html><body><p>No search results found.</p></body></html>";

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex("/search.*"))
            .respond_with(ResponseTemplate::new(200).set_body_string(html))
            .mount(&mock_server)
            .await;

        let client = BraveSearchClient::new().with_base_url(mock_server.uri());
        let request = WebSearchRequest {
            query: "obscure query with no results".to_string(),
            category: None,
            language: None,
            results_count: Some(10),
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        let result = client.search(&request).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BraveSearchError::NoResults));
    }

    #[tokio::test]
    async fn test_search_uses_results_count_from_request() {
        // When results_count is None, it defaults to 10; test default is respected
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // Build HTML with 15 results
        let items: String = (1..=15)
            .map(|i| {
                format!(
                    r#"<div data-pos="{i}"><a href="https://result{i}.com"><span class="title">Result {i}</span></a></div>"#
                )
            })
            .collect();
        let html = format!("<html><body>{items}</body></html>");

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex("/search.*"))
            .respond_with(ResponseTemplate::new(200).set_body_string(html))
            .mount(&mock_server)
            .await;

        let client = BraveSearchClient::new().with_base_url(mock_server.uri());
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: None,
            results_count: None, // defaults to 10
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        let result = client.search(&request).await.unwrap();
        assert_eq!(result.len(), 10, "Expected 10 results (the default)");
    }
}
