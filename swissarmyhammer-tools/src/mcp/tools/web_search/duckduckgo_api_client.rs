//! DuckDuckGo Instant Answer API client implementation
//!
//! This module provides a client for DuckDuckGo's official Instant Answer API.
//! Unlike the web scraping approach, this uses DuckDuckGo's legitimate API
//! for factual queries, definitions, calculations, and instant information.
//!
//! Note: DuckDuckGo's API is designed for instant answers, not comprehensive
//! web search results. For full web search functionality, consider using
//! third-party services that provide DuckDuckGo integration.

use crate::mcp::tools::web_search::privacy::PrivacyManager;
use crate::mcp::tools::web_search::types::*;
use reqwest::{Client, Error as ReqwestError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

/// Configuration for the DuckDuckGo API client
#[derive(Debug, Clone)]
pub struct DuckDuckGoApiConfig {
    /// Base API URL for DuckDuckGo instant answers
    pub api_url: String,
    /// Request timeout duration
    pub timeout: Duration,
}

impl Default for DuckDuckGoApiConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.duckduckgo.com".to_string(),
            timeout: Duration::from_secs(10),
        }
    }
}

/// DuckDuckGo Instant Answer API client
pub struct DuckDuckGoApiClient {
    client: Client,
    config: DuckDuckGoApiConfig,
}

/// Errors that can occur during DuckDuckGo API operations
#[derive(Debug, thiserror::Error)]
pub enum DuckDuckGoApiError {
    /// Network connectivity error
    #[error("Network error: {0}")]
    Network(#[from] ReqwestError),
    /// JSON parsing error
    #[error("Parse error: {0}")]
    Parse(String),
    /// Invalid search request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    /// No instant answer available for this query
    #[error("No instant answer available for this query")]
    NoInstantAnswer,
    /// API request limit reached
    #[error("DuckDuckGo API request limit reached. Please try again later.")]
    RateLimited,
}

/// DuckDuckGo Instant Answer API response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuckDuckGoApiResponse {
    /// The instant answer abstract text
    #[serde(rename = "Abstract")]
    pub abstract_text: String,

    /// Source of the abstract
    #[serde(rename = "AbstractSource")]
    pub abstract_source: String,

    /// Abstract text content
    #[serde(rename = "AbstractText")]
    pub abstract_text_content: String,

    /// URL for the abstract source
    #[serde(rename = "AbstractURL")]
    pub abstract_url: String,

    /// Entity information
    #[serde(rename = "Entity")]
    pub entity: String,

    /// Heading text
    #[serde(rename = "Heading")]
    pub heading: String,

    /// Image URL
    #[serde(rename = "Image")]
    pub image: String,

    /// Image height
    #[serde(rename = "ImageHeight")]
    pub image_height: serde_json::Value,

    /// Image width  
    #[serde(rename = "ImageWidth")]
    pub image_width: serde_json::Value,

    /// Whether image is a logo
    #[serde(rename = "ImageIsLogo")]
    pub image_is_logo: serde_json::Value,

    /// Infobox content
    #[serde(rename = "Infobox")]
    pub infobox: String,

    /// Related topics (can be nested)
    #[serde(rename = "RelatedTopics")]
    pub related_topics: Vec<RelatedTopicItem>,

    /// Definition text
    #[serde(rename = "Definition")]
    pub definition: String,

    /// Definition source
    #[serde(rename = "DefinitionSource")]
    pub definition_source: String,

    /// Definition URL
    #[serde(rename = "DefinitionURL")]
    pub definition_url: String,

    /// Answer text (for calculations, conversions, etc.)
    #[serde(rename = "Answer")]
    pub answer: String,

    /// Answer type
    #[serde(rename = "AnswerType")]
    pub answer_type: String,

    /// Type of response
    #[serde(rename = "Type")]
    pub response_type: String,

    /// Redirect URL if applicable
    #[serde(rename = "Redirect")]
    pub redirect: String,

    /// Results array (usually empty for instant answers)
    #[serde(rename = "Results")]
    pub results: Vec<serde_json::Value>,

    /// Metadata
    #[serde(rename = "meta")]
    pub meta: Option<serde_json::Value>,
}

/// Related topic from DuckDuckGo API - handles both individual topics and categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedTopicItem {
    /// First URL for the topic (only present for individual topics)
    #[serde(rename = "FirstURL")]
    pub first_url: Option<String>,

    /// Icon information (only present for individual topics)
    #[serde(rename = "Icon")]
    pub icon: Option<serde_json::Value>,

    /// HTML result with link (only present for individual topics)
    #[serde(rename = "Result")]
    pub result: Option<String>,

    /// Plain text description (only present for individual topics)
    #[serde(rename = "Text")]
    pub text: Option<String>,

    /// Name of the category (only present for categories)
    #[serde(rename = "Name")]
    pub name: Option<String>,

    /// Topics within this category (only present for categories)
    #[serde(rename = "Topics")]
    pub topics: Option<Vec<RelatedTopicItem>>,
}

impl DuckDuckGoApiClient {
    /// Creates a new DuckDuckGo API client with default configuration
    pub fn new() -> Self {
        Self::with_config(DuckDuckGoApiConfig::default())
    }

    /// Creates a new DuckDuckGo API client with the specified configuration
    pub fn with_config(config: DuckDuckGoApiConfig) -> Self {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client, config }
    }

    /// Performs an instant answer search using DuckDuckGo's API
    pub async fn search_instant_answer(
        &self,
        request: &WebSearchRequest,
        privacy_manager: &PrivacyManager,
    ) -> Result<Vec<SearchResult>, DuckDuckGoApiError> {
        tracing::info!(
            "Starting DuckDuckGo Instant Answer API search for: '{}'",
            request.query
        );

        // Build the API URL
        let api_url = self.build_api_url(request)?;
        tracing::info!("DuckDuckGo API URL: {}", api_url);

        // Apply request jitter for privacy
        privacy_manager.apply_jitter().await;

        // Build the request with privacy headers
        let mut request_builder = self.client.get(&api_url);

        // Apply User-Agent from privacy manager
        if let Some(user_agent) = privacy_manager.get_user_agent() {
            request_builder = request_builder.header("User-Agent", user_agent);
        } else {
            // Use a simple User-Agent for API requests
            request_builder = request_builder.header(
                "User-Agent",
                "swissarmyhammer-search/1.0 (https://github.com/wballard/sah-search)",
            );
        }

        // Apply privacy headers
        request_builder = privacy_manager.apply_privacy_headers(request_builder);

        // Explicitly request uncompressed content
        request_builder = request_builder.header("Accept-Encoding", "identity");

        let response = request_builder
            .send()
            .await
            .map_err(DuckDuckGoApiError::Network)?;

        if !response.status().is_success() {
            return match response.status().as_u16() {
                429 => Err(DuckDuckGoApiError::RateLimited),
                _ => Err(DuckDuckGoApiError::InvalidRequest(format!(
                    "API returned status: {}",
                    response.status()
                ))),
            };
        }

        // Use response.json() which should handle decompression automatically
        let api_response: DuckDuckGoApiResponse = response
            .json()
            .await
            .map_err(|e| DuckDuckGoApiError::Parse(format!("Failed to parse JSON: {e}")))?;

        // Convert the API response to SearchResult format
        let results = self.convert_api_response_to_search_results(
            &api_response,
            request.results_count.unwrap_or(10),
        );

        tracing::debug!("DuckDuckGo API search found {} results", results.len());

        if results.is_empty() {
            Err(DuckDuckGoApiError::NoInstantAnswer)
        } else {
            Ok(results)
        }
    }

    /// Builds the API URL with proper parameters
    fn build_api_url(&self, request: &WebSearchRequest) -> Result<String, DuckDuckGoApiError> {
        let mut url = Url::parse(&self.config.api_url)
            .map_err(|e| DuckDuckGoApiError::InvalidRequest(format!("Invalid API URL: {e}")))?;

        {
            let mut query_pairs = url.query_pairs_mut();

            // Required search query
            query_pairs.append_pair("q", &request.query);

            // Format as JSON
            query_pairs.append_pair("format", "json");

            // Skip HTML for cleaner responses
            query_pairs.append_pair("no_html", "1");

            // Skip redirects to get direct answers
            query_pairs.append_pair("no_redirect", "1");
        }

        Ok(url.to_string())
    }

    /// Converts DuckDuckGo API response to SearchResult format
    fn convert_api_response_to_search_results(
        &self,
        api_response: &DuckDuckGoApiResponse,
        max_results: usize,
    ) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let mut result_count = 0;

        // Add instant answer if available
        if !api_response.answer.is_empty() {
            results.push(SearchResult {
                title: "Instant Answer".to_string(),
                url: String::new(),
                description: api_response.answer.clone(),
                score: 1.0,
                engine: "duckduckgo-api".to_string(),
                content: None,
            });
            result_count += 1;
        }

        // Add definition if available
        if !api_response.definition.is_empty() && result_count < max_results {
            results.push(SearchResult {
                title: "Definition".to_string(),
                url: api_response.definition_url.clone(),
                description: api_response.definition.clone(),
                score: 0.95,
                engine: "duckduckgo-api".to_string(),
                content: None,
            });
            result_count += 1;
        }

        // Add abstract if available
        if !api_response.abstract_text_content.is_empty() && result_count < max_results {
            results.push(SearchResult {
                title: if !api_response.heading.is_empty() {
                    format!("About: {}", api_response.heading)
                } else {
                    format!("About: {}", api_response.abstract_source)
                },
                url: api_response.abstract_url.clone(),
                description: api_response.abstract_text_content.clone(),
                score: 0.9,
                engine: "duckduckgo-api".to_string(),
                content: None,
            });
            result_count += 1;
        }

        // Add related topics (handling both individual topics and categories)
        let mut topic_index = 0;
        for topic_item in &api_response.related_topics {
            if result_count >= max_results {
                break;
            }

            // Check if this is a category (has name and topics fields)
            if let Some(_category_name) = &topic_item.name {
                if let Some(topics) = &topic_item.topics {
                    // Add topics from the category
                    for nested_topic in topics {
                        if result_count >= max_results {
                            break;
                        }
                        if let Some(search_result) =
                            self.convert_topic_item_to_result(nested_topic, topic_index)
                        {
                            results.push(search_result);
                            result_count += 1;
                            topic_index += 1;
                        }
                    }
                }
            } else {
                // This is an individual topic
                if let Some(search_result) =
                    self.convert_topic_item_to_result(topic_item, topic_index)
                {
                    results.push(search_result);
                    result_count += 1;
                    topic_index += 1;
                }
            }
        }

        results
    }

    /// Converts a topic item to a search result if valid
    fn convert_topic_item_to_result(
        &self,
        topic: &RelatedTopicItem,
        index: usize,
    ) -> Option<SearchResult> {
        let text = topic.text.as_ref()?;
        let first_url = topic.first_url.as_ref()?;

        if text.is_empty() || first_url.is_empty() {
            return None;
        }

        // Extract title from the text (first part before the dash usually)
        let title = text.split(" - ").next().unwrap_or(text).to_string();

        let description = text.split(" - ").skip(1).collect::<Vec<_>>().join(" - ");

        Some(SearchResult {
            title,
            url: first_url.clone(),
            description: if description.is_empty() {
                text.clone()
            } else {
                description
            },
            score: self.calculate_topic_score(index),
            engine: "duckduckgo-api".to_string(),
            content: None,
        })
    }

    /// Calculates score for related topics based on their position
    fn calculate_topic_score(&self, index: usize) -> f64 {
        // Start at 0.85 for first topic and decrease by 0.05 per position
        let score = 0.85 - (index as f64 * 0.05);
        // Round to 2 decimal places to avoid floating point precision issues
        let rounded_score = (score * 100.0).round() / 100.0;
        rounded_score.max(0.1) // Minimum score of 0.1
    }
}

impl Default for DuckDuckGoApiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duckduckgo_api_client_new() {
        let client = DuckDuckGoApiClient::new();
        assert_eq!(client.config.api_url, "https://api.duckduckgo.com");
    }

    #[test]
    fn test_duckduckgo_api_client_with_custom_config() {
        let config = DuckDuckGoApiConfig {
            api_url: "https://custom.api.com".to_string(),
            timeout: Duration::from_secs(5),
        };
        let client = DuckDuckGoApiClient::with_config(config.clone());
        assert_eq!(client.config.api_url, "https://custom.api.com");
        assert_eq!(client.config.timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_build_api_url_basic() {
        let client = DuckDuckGoApiClient::new();
        let request = WebSearchRequest {
            query: "test query".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };

        let url = client.build_api_url(&request).unwrap();
        assert!(url.contains("q=test+query"));
        assert!(url.contains("format=json"));
        assert!(url.contains("no_html=1"));
        assert!(url.contains("no_redirect=1"));
    }

    #[test]
    fn test_calculate_topic_score() {
        let client = DuckDuckGoApiClient::new();

        assert_eq!(client.calculate_topic_score(0), 0.85);
        assert_eq!(client.calculate_topic_score(1), 0.8);
        assert_eq!(client.calculate_topic_score(2), 0.75);

        // Test minimum score
        assert_eq!(client.calculate_topic_score(20), 0.1);
    }

    #[test]
    fn test_convert_api_response_basic() {
        let client = DuckDuckGoApiClient::new();
        let api_response = DuckDuckGoApiResponse {
            abstract_text: "Test abstract".to_string(),
            abstract_source: "Wikipedia".to_string(),
            abstract_text_content: "Test abstract text".to_string(),
            abstract_url: "https://wikipedia.org/test".to_string(),
            entity: String::new(),
            heading: "Test Heading".to_string(),
            image: String::new(),
            image_height: serde_json::Value::from(0),
            image_width: serde_json::Value::from(0),
            image_is_logo: serde_json::Value::from(0),
            infobox: String::new(),
            related_topics: vec![],
            definition: "Test definition".to_string(),
            definition_source: "Dictionary".to_string(),
            definition_url: "https://dict.com/test".to_string(),
            answer: "42".to_string(),
            answer_type: "calc".to_string(),
            response_type: "A".to_string(),
            redirect: String::new(),
            results: vec![],
            meta: None,
        };

        let results = client.convert_api_response_to_search_results(&api_response, 10);

        // Should have instant answer, definition, and abstract
        assert_eq!(results.len(), 3);

        // Check instant answer
        assert_eq!(results[0].title, "Instant Answer");
        assert_eq!(results[0].description, "42");
        assert_eq!(results[0].score, 1.0);

        // Check definition
        assert_eq!(results[1].title, "Definition");
        assert_eq!(results[1].description, "Test definition");
        assert_eq!(results[1].score, 0.95);

        // Check abstract
        assert_eq!(results[2].title, "About: Test Heading");
        assert_eq!(results[2].description, "Test abstract text");
        assert_eq!(results[2].score, 0.9);
    }

    #[test]
    fn test_convert_api_response_with_related_topics() {
        let client = DuckDuckGoApiClient::new();
        let api_response = DuckDuckGoApiResponse {
            abstract_text: String::new(),
            abstract_source: String::new(),
            abstract_text_content: String::new(),
            abstract_url: String::new(),
            entity: String::new(),
            heading: String::new(),
            image: String::new(),
            image_height: serde_json::Value::from(0),
            image_width: serde_json::Value::from(0),
            image_is_logo: serde_json::Value::from(0),
            infobox: String::new(),
            related_topics: vec![
                RelatedTopicItem {
                    first_url: Some("https://example.com/1".to_string()),
                    icon: None,
                    result: Some(String::new()),
                    text: Some("Topic 1 - Description for topic 1".to_string()),
                    name: None,
                    topics: None,
                },
                RelatedTopicItem {
                    first_url: Some("https://example.com/2".to_string()),
                    icon: None,
                    result: Some(String::new()),
                    text: Some("Topic 2 - Description for topic 2".to_string()),
                    name: None,
                    topics: None,
                },
            ],
            definition: String::new(),
            definition_source: String::new(),
            definition_url: String::new(),
            answer: String::new(),
            answer_type: String::new(),
            response_type: "D".to_string(),
            redirect: String::new(),
            results: vec![],
            meta: None,
        };

        let results = client.convert_api_response_to_search_results(&api_response, 10);

        assert_eq!(results.len(), 2);

        // Check first topic
        assert_eq!(results[0].title, "Topic 1");
        assert_eq!(results[0].description, "Description for topic 1");
        assert_eq!(results[0].score, 0.85);

        // Check second topic
        assert_eq!(results[1].title, "Topic 2");
        assert_eq!(results[1].description, "Description for topic 2");
        assert_eq!(results[1].score, 0.8);
    }
}
