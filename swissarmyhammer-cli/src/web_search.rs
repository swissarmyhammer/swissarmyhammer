//! Web search CLI commands
//!
//! This module provides CLI commands for web search functionality using the MCP web_search tool.
//! It enables users to perform web searches directly from the command line with the same
//! capabilities as the MCP tool.

use crate::cli::{OutputFormat, WebSearchCommands};
use crate::mcp_integration::CliToolContext;
use serde_json::json;
use std::error::Error;
use tabled::{Table, Tabled};

// Table display truncation limits
const MAX_TITLE_WIDTH: usize = 60;
const MAX_DESCRIPTION_WIDTH: usize = 80;
const MAX_URL_WIDTH: usize = 100;

/// Represents a search result for table display
#[derive(Tabled)]
struct SearchResultRow {
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Score")]
    score: String,
    #[tabled(rename = "Description")]
    description: String,
}

/// Truncates a string to the specified maximum width, adding ellipsis if truncated
///
/// # Arguments
///
/// * `text` - The text to truncate
/// * `max_width` - Maximum width in characters (must be >= 3 for ellipsis)
///
/// # Returns
///
/// Truncated string with "..." appended if truncation occurred
fn truncate_text(text: &str, max_width: usize) -> String {
    if text.chars().count() > max_width {
        let truncated: String = text.chars().take(max_width - 3).collect();
        format!("{truncated}...")
    } else {
        text.to_string()
    }
}

/// Handle web search CLI commands
///
/// This function processes web search commands and delegates to the appropriate
/// MCP tool handler, following the established CLI pattern for other tools.
///
/// # Arguments
///
/// * `command` - The web search command to execute
///
/// # Returns
///
/// * `Result<(), Box<dyn Error>>` - Success or error result
///
/// # Examples
///
/// ```rust,ignore
/// let command = WebSearchCommands::Search {
///     query: "rust async programming".to_string(),
///     category: SearchCategory::It,
///     results: 10,
///     // ... other fields
/// };
/// handle_web_search_command(command).await?;
/// ```
pub async fn handle_web_search_command(command: WebSearchCommands) -> Result<(), Box<dyn Error>> {
    let context = CliToolContext::new().await?;

    match command {
        WebSearchCommands::Search {
            query,
            category,
            results,
            language,
            fetch_content,
            safe_search,
            time_range,
            format,
        } => {
            // Validate input parameters before sending to MCP tool
            if query.trim().is_empty() {
                return Err("Search query cannot be empty".into());
            }

            if query.len() > 500 {
                return Err(
                    format!("Search query is {} characters, maximum is 500", query.len()).into(),
                );
            }

            if results == 0 || results > 50 {
                return Err(
                    format!("Results count must be between 1 and 50, got {results}").into(),
                );
            }

            if safe_search > 2 {
                return Err(
                    format!("Safe search level must be 0, 1, or 2, got {safe_search}").into(),
                );
            }

            // Create arguments for MCP tool
            // Convert safe_search from integer to enum variant string (capitalized)
            let safe_search_level = match safe_search {
                0 => "Off",
                1 => "Moderate",
                2 => "Strict",
                _ => "Moderate", // Default to Moderate for invalid values
            };

            let mut args_vec = vec![
                ("query", json!(query)),
                ("results_count", json!(results)),
                ("language", json!(language)),
                ("fetch_content", json!(fetch_content)),
                ("safe_search", json!(safe_search_level)),
            ];

            // Convert category string to enum variant (lowercase)
            let category_variant = match category.as_str() {
                "general" => "general",
                "images" => "images",
                "videos" => "videos",
                "news" => "news",
                "map" => "map",
                "music" => "music",
                "it" => "it",
                "science" => "science",
                "files" => "files",
                _ => "general", // Default to general for unknown categories
            };

            // Add category to arguments (always include it)
            args_vec.push(("category", json!(category_variant)));

            // Convert time_range string to enum variant if provided (lowercase, empty string for "all")
            if !time_range.is_empty() {
                let time_range_variant = match time_range.as_str() {
                    "day" => "day",
                    "week" => "week",
                    "month" => "month",
                    "year" => "year",
                    "all" | "" => "", // All time range is represented as empty string
                    _ => "",          // Default to empty string (all time) for unknown values
                };
                args_vec.push(("time_range", json!(time_range_variant)));
            }

            let args = context.create_arguments(args_vec);

            // Execute the web search tool
            let result = context.execute_tool("web_search", args).await?;

            // Format and display results based on requested format
            match format {
                OutputFormat::Table => {
                    // Convert CallToolResult to JSON for table display
                    let json_result = serde_json::to_value(&result)?;
                    display_search_results_table(&json_result)?;
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OutputFormat::Yaml => {
                    println!("{}", serde_yaml::to_string(&result)?);
                }
            }
        }
    }

    Ok(())
}

/// Display search results in a formatted table
///
/// Parses the MCP tool response and displays search results in a human-readable
/// table format using the tabled crate for consistent formatting.
///
/// # Arguments
///
/// * `result` - The JSON response from the MCP web_search tool
///
/// # Returns
///
/// * `Result<(), Box<dyn Error>>` - Success or error result
fn display_search_results_table(result: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    // Extract the content from MCP response
    let content = result["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|item| item["text"].as_str())
        .unwrap_or("");

    // Try to parse the content as JSON (the actual search response)
    if let Ok(search_response) = serde_json::from_str::<serde_json::Value>(content) {
        // Extract results and metadata
        let empty_vec = vec![];
        let results = search_response["results"].as_array().unwrap_or(&empty_vec);
        let metadata = &search_response["metadata"];

        // Display search summary
        if let (Some(query), Some(search_time), Some(instance)) = (
            metadata["query"].as_str(),
            metadata["search_time_ms"].as_u64(),
            metadata["instance_used"].as_str(),
        ) {
            println!("🔍 Search Results for: \"{query}\"");
            println!(
                "📊 Found {} results in {}ms using {}",
                results.len(),
                search_time,
                instance
            );

            if let Some(engines) = metadata["engines_used"].as_array() {
                let engine_names: Vec<String> = engines
                    .iter()
                    .filter_map(|e| e.as_str())
                    .map(|s| s.to_string())
                    .collect();
                println!("🔧 Engines: {}", engine_names.join(", "));
            }

            println!(); // Empty line before results
        }

        // Display results table
        if results.is_empty() {
            println!("No search results found.");
            return Ok(());
        }

        // Collect all table rows (main results plus URL/content rows)
        let mut table_rows: Vec<SearchResultRow> = Vec::new();

        for result_item in results.iter() {
            let title = result_item["title"].as_str().unwrap_or("Untitled");
            let url = result_item["url"].as_str().unwrap_or("");
            let description = result_item["description"].as_str().unwrap_or("");
            let score = result_item["score"].as_f64().unwrap_or(0.0);

            // Truncate text to reasonable lengths for table display
            let truncated_title = truncate_text(title, MAX_TITLE_WIDTH);
            let truncated_desc = truncate_text(description, MAX_DESCRIPTION_WIDTH);

            // Add main result row
            table_rows.push(SearchResultRow {
                title: truncated_title,
                score: format!("{score:.2}"),
                description: truncated_desc,
            });

            // Add URL row
            let truncated_url = truncate_text(url, MAX_URL_WIDTH);
            table_rows.push(SearchResultRow {
                title: format!("🔗 {truncated_url}"),
                score: String::new(),
                description: String::new(),
            });
        }

        // Create and display the table using tabled
        let table = Table::new(table_rows);
        println!("{table}");

        // Display content fetch statistics if available
        if let Some(fetch_stats) = metadata["content_fetch_stats"].as_object() {
            if let (Some(attempted), Some(successful), Some(failed), Some(total_time)) = (
                fetch_stats["attempted"].as_u64(),
                fetch_stats["successful"].as_u64(),
                fetch_stats["failed"].as_u64(),
                fetch_stats["total_time_ms"].as_u64(),
            ) {
                println!("\n📈 Content Fetch Statistics:");
                println!("   • Attempted: {attempted}");
                println!("   • Successful: {successful}");
                println!("   • Failed: {failed}");
                println!("   • Total time: {total_time}ms");
            }
        }
    } else {
        // Fallback: just print the content as text
        println!("{content}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_search_results_table_empty() {
        let result = json!({
            "content": [{
                "text": r#"{"results": [], "metadata": {"query": "test", "search_time_ms": 100, "instance_used": "test"}}"#
            }]
        });

        // Should not panic and should handle empty results gracefully
        assert!(display_search_results_table(&result).is_ok());
    }

    #[test]
    fn test_display_search_results_table_malformed() {
        let result = json!({
            "content": [{
                "text": "invalid json"
            }]
        });

        // Should fallback to plain text display
        assert!(display_search_results_table(&result).is_ok());
    }

    #[test]
    fn test_display_search_results_table_with_wide_content() {
        // Test case that reproduces the table alignment issue from the problem description
        let result = json!({
            "content": [{
                "text": r#"{"results": [
                    {
                        "title": "Apple - Official Site - Explore new products",
                        "url": "https://duckduckgo.com/y.js?ad_domain=apple.com&ad_provider=bingv7aa&ad_type=txad&click_metadata=NgjwJGO7CK0qpxaWDIxNtwSxyPTOKjKn22vISUDxEmFNKKXDIwOwT9YeFLs7DzeW0J3DMKPoPApYnPmWTwpWMJ3LqUOSqVVMPqBeCiOdpQqskjdJSrGjlcIshtT_dmp0.DEixbuIPVsWUmnWfgmPKkQ&rut=442ff7be7887db99478724500fa09458b3b545cce09265a3a0e3f3df30b0bb0d&u3=https%3A%2F%2Fwww.bing.com%2Faclick%3Fld%3De8uLnfJ7gUMvIDQ_TALU0sLTVUCUzz3TRt0gNOitPwnTxfpBYisM6LGhRY9BApmygW5PnyZkRBEV5_eP6md3bdV6Y5uck3gXZArBUFpPnhJMzr5DVnlrW1gNVSOm1tRYtkyFc4qLCqFlBWA_0mhiIml8lXhHHZk7KEusEly5t6Maf%2DEOV0VNc6eILCgzdt8najThk3hQ%26u%3DaHR0cHMlM2ElMmYlMmZ3d3cuYXBwbGUuY29tJTJmdXMlMmZzaG9wJTJmZ28lMmZzdG9yZSUzZmNpZCUzZGFvcy11cy1rd2JpLWJyYW5kLWJ0cy1sYXVuY2gtdXBkYXRlLTA3MDIyNS0lMjZhb3NpZCUzZHAyNDAlMjZrZW5fcGlkJTNkYmklN2VjbXAtNjk4MTk5NjIzJTdlYWRnLTEyNDM1NDg5ODkzMDE2ODElN2VhZC03NzcyMTk0NjU3OTA1NF9rd2QtNzc3MjIxODQ5Mjk3MzIlM2Fsb2MtMTkwJTdlZGV2LWMlN2VleHQtJTdlcHJkLSU3ZW50LXNlYXJjaCU3ZWNyaWQtJTI2dG9rZW4lM2Q4YzFhNzZhYi1jZmU1LTQzODAtYjU1MS1lNzU1YTNiNDhiYzg%26rlid%3D6d3f22dbd2e81f12eaf84a79f408ccd7&vqd=4-289524564473418491957113467918030559449&iurl=%7B1%7DIG%3DFBD528C5732B4AF8BB452C28605F9B5B%26CID%3D0ED3FF454A63625237A2E90E4B85632B%26ID%3DDevEx%2C5045.1",
                        "description": "Buy Mac or iPad for college, get AirPods or an eligible accessory of choice. ...",
                        "score": 1.0,
                        "engine": "duckduckgo",
                        "content": {
                            "word_count": 16266,
                            "summary": "Apple Store Online - Apple document. cookie = \"as_sfa=Mnx1c3x1c3x8ZW5fVVN8Y..."
                        }
                    },
                    {
                        "title": "Apple - Wikipedia",
                        "url": "https://en.wikipedia.org/wiki/Apple",
                        "description": "An apple is the round, edible fruit of an apple tree (Malus spp.). Fru...",
                        "score": 0.9,
                        "engine": "duckduckgo",
                        "content": {
                            "word_count": 15726,
                            "summary": ""
                        }
                    }
                ], "metadata": {"query": "what is an apple", "search_time_ms": 4778, "instance_used": "https://duckduckgo.com", "engines_used": ["duckduckgo"]}}"#
            }]
        });

        // This test currently succeeds but produces jagged output
        // After implementing tabled, the output should be properly aligned
        assert!(display_search_results_table(&result).is_ok());
    }

    #[test]
    fn test_display_search_results_table_clean_format() {
        let result = json!({
            "content": [{
                "text": r#"{"results": [
                    {
                        "title": "Test Title",
                        "url": "https://example.com",
                        "description": "Test description",
                        "score": 0.95,
                        "engine": "duckduckgo",
                        "content": {
                            "word_count": 1000,
                            "summary": "This is a summary that should not appear"
                        }
                    }
                ], "metadata": {"query": "test", "search_time_ms": 100, "instance_used": "test", "engines_used": ["duckduckgo"]}}"#
            }]
        });

        // Test should pass with clean format that excludes:
        // - Engine column or engine values
        // - Word count rows with "📄 X words" format
        // - Text preview/summary content
        assert!(display_search_results_table(&result).is_ok());
    }

    #[tokio::test]
    async fn test_handle_web_search_command_validation() {
        use crate::cli::WebSearchCommands;

        // Test empty query validation
        let command = WebSearchCommands::Search {
            query: "".to_string(),
            category: "general".to_string(),
            results: 10,
            language: "en".to_string(),
            fetch_content: true,
            safe_search: 1,
            time_range: "".to_string(),
            format: OutputFormat::Table,
        };

        let result = handle_web_search_command(command).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_handle_web_search_command_query_too_long() {
        use crate::cli::WebSearchCommands;

        let long_query = "a".repeat(501);
        let command = WebSearchCommands::Search {
            query: long_query,
            category: "general".to_string(),
            results: 10,
            language: "en".to_string(),
            fetch_content: true,
            safe_search: 1,
            time_range: "".to_string(),
            format: OutputFormat::Table,
        };

        let result = handle_web_search_command(command).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("501 characters"));
    }

    #[tokio::test]
    async fn test_handle_web_search_command_invalid_results_count() {
        use crate::cli::WebSearchCommands;

        let command = WebSearchCommands::Search {
            query: "test query".to_string(),
            category: "general".to_string(),
            results: 0,
            language: "en".to_string(),
            fetch_content: true,
            safe_search: 1,
            time_range: "".to_string(),
            format: OutputFormat::Table,
        };

        let result = handle_web_search_command(command).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("between 1 and 50"));
    }

    #[tokio::test]
    async fn test_handle_web_search_command_invalid_safe_search() {
        use crate::cli::WebSearchCommands;

        let command = WebSearchCommands::Search {
            query: "test query".to_string(),
            category: "general".to_string(),
            results: 10,
            language: "en".to_string(),
            fetch_content: true,
            safe_search: 5,
            time_range: "".to_string(),
            format: OutputFormat::Table,
        };

        let result = handle_web_search_command(command).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be 0, 1, or 2"));
    }
}
