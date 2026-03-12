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
        }
    }

    /// Performs a web search using Brave Search via direct HTTP request
    pub async fn search(
        &self,
        request: &WebSearchRequest,
    ) -> Result<Vec<SearchResult>, BraveSearchError> {
        tracing::debug!("Starting Brave search for: '{}'", request.query);

        let encoded_query = urlencoding::encode(&request.query);
        let url = format!("https://search.brave.com/search?q={encoded_query}&source=web");

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

        tracing::debug!(
            "Retrieved {} bytes from Brave Search",
            html_content.len()
        );

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
        let snippet_sel = Selector::parse("[data-pos]").map_err(|e| {
            BraveSearchError::Parse(format!("Invalid selector: {e}"))
        })?;

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
                    title = title_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
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
                                    title = a_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
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
                    description = desc_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
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
}
