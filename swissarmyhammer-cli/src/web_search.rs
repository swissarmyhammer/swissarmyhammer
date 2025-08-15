//! Web search CLI commands
//!
//! This module provides CLI commands for web search functionality using the MCP web_search tool.
//! It enables users to perform web searches directly from the command line with the same
//! capabilities as the MCP tool.

use crate::cli::{OutputFormat, WebSearchCommands};
use crate::mcp_integration::CliToolContext;
use serde_json::json;
use std::error::Error;

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
                    _ => "", // Default to empty string (all time) for unknown values
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
/// table format similar to other CLI tools in the system.
///
/// # Arguments
///
/// * `result` - The JSON response from the MCP web_search tool
///
/// # Returns
///
/// * `Result<(), Box<dyn Error>>` - Success or error result
#[allow(clippy::uninlined_format_args)]
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

        // Calculate column widths for better formatting
        let max_title_width = results
            .iter()
            .filter_map(|r| r["title"].as_str())
            .map(|s| s.len())
            .max()
            .unwrap_or(20)
            .min(60); // Cap at 60 characters

        let max_desc_width = results
            .iter()
            .filter_map(|r| r["description"].as_str())
            .map(|s| s.len())
            .max()
            .unwrap_or(40)
            .min(80); // Cap at 80 characters

        // Print table header
        println!(
            "┌{:─<width$}┬{:─<8}┬{:─<12}┬{:─<desc_width$}┐",
            "",
            "",
            "",
            "",
            width = max_title_width + 2,
            desc_width = max_desc_width + 2
        );
        println!(
            "│{:^width$}│{:^8}│{:^12}│{:^desc_width$}│",
            "Title",
            "Score",
            "Engine",
            "Description",
            width = max_title_width + 2,
            desc_width = max_desc_width + 2
        );
        println!(
            "├{:─<width$}┼{:─<8}┼{:─<12}┼{:─<desc_width$}┤",
            "",
            "",
            "",
            "",
            width = max_title_width + 2,
            desc_width = max_desc_width + 2
        );

        // Print each result
        for (index, result_item) in results.iter().enumerate() {
            let title = result_item["title"].as_str().unwrap_or("Untitled");
            let url = result_item["url"].as_str().unwrap_or("");
            let description = result_item["description"].as_str().unwrap_or("");
            let score = result_item["score"].as_f64().unwrap_or(0.0);
            let engine = result_item["engine"].as_str().unwrap_or("unknown");

            // Truncate long text for table display
            let truncated_title = if title.len() > max_title_width {
                format!("{}...", &title[..max_title_width - 3])
            } else {
                title.to_string()
            };

            let truncated_desc = if description.len() > max_desc_width {
                format!("{}...", &description[..max_desc_width - 3])
            } else {
                description.to_string()
            };

            println!(
                "│ {:width$} │ {:>6.2} │ {:^10} │ {:desc_width$} │",
                truncated_title,
                score,
                engine,
                truncated_desc,
                width = max_title_width,
                desc_width = max_desc_width
            );

            // Show URL on next line
            println!(
                "│ {:width$} │        │            │ {:desc_width$} │",
                format!("🔗 {}", url),
                "",
                width = max_title_width,
                desc_width = max_desc_width
            );

            // Show content info if available
            if let Some(content_info) = result_item["content"].as_object() {
                if let (Some(word_count), Some(summary)) = (
                    content_info["word_count"].as_u64(),
                    content_info["summary"].as_str(),
                ) {
                    let content_summary = if summary.len() > max_desc_width {
                        format!("{}...", &summary[..max_desc_width - 3])
                    } else {
                        summary.to_string()
                    };

                    println!(
                        "│ {:width$} │        │            │ {:desc_width$} │",
                        format!("📄 {} words", word_count),
                        content_summary,
                        width = max_title_width,
                        desc_width = max_desc_width
                    );
                }
            }

            // Add separator line between results (except for last result)
            if index < results.len() - 1 {
                println!(
                    "├{:─<width$}┼{:─<8}┼{:─<12}┼{:─<desc_width$}┤",
                    "",
                    "",
                    "",
                    "",
                    width = max_title_width + 2,
                    desc_width = max_desc_width + 2
                );
            }
        }

        // Print table footer
        println!(
            "└{:─<width$}┴{:─<8}┴{:─<12}┴{:─<desc_width$}┘",
            "",
            "",
            "",
            "",
            width = max_title_width + 2,
            desc_width = max_desc_width + 2
        );

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
