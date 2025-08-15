//! Web search tool for MCP operations
//!
//! This module provides the WebSearchTool for performing web searches through the MCP protocol.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::web_search::types::*;
use async_trait::async_trait;
use reqwest::Client;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde_json::Value;
use std::time::{Duration, Instant};
use url::Url;

/// Tool for performing web searches using SearXNG
#[derive(Default)]
pub struct WebSearchTool {
    client: Option<Client>,
}

impl WebSearchTool {
    /// Creates a new instance of the WebSearchTool
    pub fn new() -> Self {
        Self { client: None }
    }

    /// Gets or creates an HTTP client with appropriate configuration
    fn get_client(&mut self) -> &Client {
        if self.client.is_none() {
            self.client = Some(
                Client::builder()
                    .timeout(Duration::from_secs(30))
                    .user_agent("SwissArmyHammer/1.0 (Privacy-Focused Web Search)")
                    .build()
                    .unwrap_or_else(|_| Client::new()),
            );
        }
        self.client.as_ref().unwrap()
    }

    /// List of SearXNG instances to try
    /// For now using a hardcoded list, in future this could be dynamic
    fn get_searxng_instances() -> Vec<&'static str> {
        vec![
            "https://search.bus-hit.me",
            "https://searx.tiekoetter.com",
            "https://search.projectsegfau.lt",
            "https://searx.work",
            "https://search.sapti.me",
        ]
    }

    /// Performs a search using a SearXNG instance
    async fn perform_search(
        &mut self,
        instance: &str,
        request: &WebSearchRequest,
    ) -> Result<SearXngResponse, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client();

        let mut url = Url::parse(&format!("{instance}/search"))?;

        // Set query parameters
        url.query_pairs_mut()
            .append_pair("q", &request.query)
            .append_pair("format", "json")
            .append_pair("pageno", "1");

        if let Some(category) = &request.category {
            let category_str = match category {
                SearchCategory::General => "general",
                SearchCategory::Images => "images",
                SearchCategory::Videos => "videos",
                SearchCategory::News => "news",
                SearchCategory::Map => "map",
                SearchCategory::Music => "music",
                SearchCategory::It => "it",
                SearchCategory::Science => "science",
                SearchCategory::Files => "files",
            };
            url.query_pairs_mut()
                .append_pair("categories", category_str);
        }

        if let Some(language) = &request.language {
            url.query_pairs_mut().append_pair("language", language);
        }

        if let Some(safe_search) = request.safe_search {
            url.query_pairs_mut()
                .append_pair("safesearch", &(safe_search as u8).to_string());
        }

        if let Some(time_range) = &request.time_range {
            if !matches!(time_range, TimeRange::All) {
                let time_str = match time_range {
                    TimeRange::All => "",
                    TimeRange::Day => "day",
                    TimeRange::Week => "week",
                    TimeRange::Month => "month",
                    TimeRange::Year => "year",
                };
                url.query_pairs_mut().append_pair("time_range", time_str);
            }
        }

        tracing::debug!("Making search request to: {}", url);

        let response = client
            .get(url)
            .timeout(Duration::from_secs(15))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("SearXNG instance returned status: {}", response.status()).into());
        }

        let json: Value = response.json().await?;

        // Parse SearXNG response
        let results = json["results"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .take(request.results_count.unwrap_or(10))
            .filter_map(|result| {
                Some(SearchResult {
                    title: result["title"].as_str()?.to_string(),
                    url: result["url"].as_str()?.to_string(),
                    description: result["content"].as_str().unwrap_or("").to_string(),
                    score: 1.0, // SearXNG doesn't provide scores, use default
                    engine: result["engine"].as_str().unwrap_or("unknown").to_string(),
                    content: None, // Will be populated later if fetch_content is true
                })
            })
            .collect();

        let engines_used: Vec<String> = json["results"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|result| result["engine"].as_str().map(|s| s.to_string()))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Ok(SearXngResponse {
            results,
            engines_used,
            total_results: json["number_of_results"].as_u64().unwrap_or(0) as usize,
        })
    }

    /// Fetches content from a URL and converts it to markdown
    async fn fetch_content(
        &mut self,
        url: &str,
    ) -> Result<SearchResultContent, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client();
        let start_time = Instant::now();

        // For now, implement basic content fetching without markdowndown
        // In future iterations, we can integrate with markdowndown crate
        let response = client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to fetch content: HTTP {}", response.status()).into());
        }

        let html = response.text().await?;
        let fetch_time = start_time.elapsed();

        // Basic HTML to text conversion (simplified for now)
        let text = html
            .replace("<br>", "\n")
            .replace("</p>", "\n\n")
            .replace("</div>", "\n")
            .replace("</h1>", "\n")
            .replace("</h2>", "\n")
            .replace("</h3>", "\n");

        // Remove HTML tags (very basic)
        let text = regex::Regex::new(r"<[^>]*>")
            .unwrap()
            .replace_all(&text, "")
            .trim()
            .to_string();

        let word_count = text.split_whitespace().count();
        let summary = if word_count > 50 {
            text.split_whitespace()
                .take(50)
                .collect::<Vec<_>>()
                .join(" ")
                + "..."
        } else {
            text.clone()
        };

        Ok(SearchResultContent {
            markdown: text,
            word_count,
            fetch_time_ms: fetch_time.as_millis() as u64,
            summary,
        })
    }
}

/// Response from SearXNG API
struct SearXngResponse {
    results: Vec<SearchResult>,
    engines_used: Vec<String>,
    total_results: usize,
}

#[async_trait]
impl McpTool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(WebSearchRequest))
            .expect("Failed to generate schema")
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: WebSearchRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::info!(
            "Starting web search: '{}', results_count: {:?}, fetch_content: {:?}",
            request.query,
            request.results_count,
            request.fetch_content
        );

        if request.query.trim().is_empty() {
            return Err(McpError::invalid_request(
                "Search query cannot be empty",
                None,
            ));
        }

        let start_time = Instant::now();
        let mut search_tool = WebSearchTool::new();

        // Try each SearXNG instance until one works
        let instances = Self::get_searxng_instances();
        let mut last_error = None;
        let mut attempted_instances = Vec::new();

        for instance in instances {
            attempted_instances.push(instance.to_string());

            match search_tool.perform_search(instance, &request).await {
                Ok(mut searxng_response) => {
                    let search_time = start_time.elapsed();

                    // Optionally fetch content from each result
                    let mut content_fetch_stats = None;

                    if request.fetch_content.unwrap_or(true) {
                        let content_start = Instant::now();
                        let mut successful = 0;
                        let mut failed = 0;

                        for result in &mut searxng_response.results {
                            match search_tool.fetch_content(&result.url).await {
                                Ok(content) => {
                                    result.content = Some(content);
                                    successful += 1;
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to fetch content from {}: {}",
                                        result.url,
                                        e
                                    );
                                    failed += 1;
                                }
                            }
                        }

                        content_fetch_stats = Some(ContentFetchStats {
                            attempted: searxng_response.results.len(),
                            successful,
                            failed,
                            total_time_ms: content_start.elapsed().as_millis() as u64,
                        });
                    }

                    let response = WebSearchResponse {
                        results: searxng_response.results,
                        metadata: SearchMetadata {
                            query: request.query.clone(),
                            category: request.category.unwrap_or_default(),
                            language: request.language.unwrap_or_else(|| "en".to_string()),
                            results_count: request.results_count.unwrap_or(10),
                            search_time_ms: search_time.as_millis() as u64,
                            instance_used: instance.to_string(),
                            total_results: searxng_response.total_results,
                            engines_used: searxng_response.engines_used,
                            content_fetch_stats,
                            fetch_content: request.fetch_content.unwrap_or(true),
                        },
                    };

                    tracing::info!(
                        "Web search completed: found {} results for '{}' in {:?}",
                        response.results.len(),
                        response.metadata.query,
                        search_time
                    );

                    return Ok(BaseToolImpl::create_success_response(
                        serde_json::to_string_pretty(&response).map_err(|e| {
                            McpError::internal_error(
                                format!("Failed to serialize response: {e}"),
                                None,
                            )
                        })?,
                    ));
                }
                Err(e) => {
                    tracing::warn!("Search failed on instance {}: {}", instance, e);
                    last_error = Some(e);
                    continue;
                }
            }
        }

        // All instances failed
        let error = WebSearchError {
            error_type: "no_instances_available".to_string(),
            error_details: format!("All SearXNG instances failed. Last error: {last_error:?}"),
            attempted_instances,
            retry_after: Some(300), // Suggest retry after 5 minutes
        };

        Err(McpError::internal_error(
            serde_json::to_string_pretty(&error).unwrap_or_else(|_| "Search failed".to_string()),
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_web_search_tool_new() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_web_search_tool_schema() {
        let tool = WebSearchTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["results_count"].is_object());
        assert!(schema["properties"]["category"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["query"]));
    }

    #[tokio::test]
    async fn test_web_search_tool_execute_empty_query() {
        let tool = WebSearchTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_web_search_tool_execute_missing_query() {
        let tool = WebSearchTool::new();
        let context = create_test_context().await;

        let arguments = serde_json::Map::new(); // Missing query field

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_get_searxng_instances() {
        let instances = WebSearchTool::get_searxng_instances();
        assert!(!instances.is_empty());

        for instance in instances {
            assert!(instance.starts_with("https://"));
        }
    }

    #[test]
    fn test_web_search_request_parsing() {
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("test query".to_string()),
        );
        arguments.insert(
            "results_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5)),
        );
        arguments.insert("fetch_content".to_string(), serde_json::Value::Bool(false));

        let request: WebSearchRequest = BaseToolImpl::parse_arguments(arguments).unwrap();
        assert_eq!(request.query, "test query");
        assert_eq!(request.results_count, Some(5));
        assert_eq!(request.fetch_content, Some(false));
    }
}
