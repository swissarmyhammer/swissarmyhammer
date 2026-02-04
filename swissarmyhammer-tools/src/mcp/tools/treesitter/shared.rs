//! Shared utilities for tree-sitter MCP tools

use rmcp::ErrorData as McpError;
use serde_json::{json, Value};
use std::path::PathBuf;
use swissarmyhammer_treesitter::{DuplicateCluster, QueryMatch, SimilarChunkResult, Workspace};

use crate::mcp::tool_registry::ToolContext;

/// Returns a JSON schema property definition for the workspace path parameter.
///
/// This is a common property used across all tree-sitter tools to specify
/// the workspace directory to operate on.
pub fn schema_workspace_path_property() -> Value {
    json!({
        "type": "string",
        "description": "Workspace path (default: current directory)"
    })
}

/// Builds a JSON schema object for MCP tool parameters.
///
/// Creates a standard JSON schema with "type": "object" and the provided
/// properties. Optionally includes a "required" array for mandatory fields.
///
/// # Arguments
/// * `properties` - Vector of (name, schema_value) tuples for each property
/// * `required` - Optional vector of property names that are required
pub fn build_tool_schema(properties: Vec<(&str, Value)>, required: Option<Vec<&str>>) -> Value {
    let mut props = serde_json::Map::new();
    for (name, value) in properties {
        props.insert(name.to_string(), value);
    }

    let mut schema = json!({
        "type": "object",
        "properties": props
    });

    if let Some(req) = required {
        schema["required"] = json!(req);
    }

    schema
}

/// Resolve the workspace path from an optional path parameter and tool context.
///
/// Returns the provided path if present, otherwise falls back to the context's
/// working directory or the current directory.
pub fn resolve_workspace_path(path: Option<&String>, context: &ToolContext) -> PathBuf {
    match path {
        Some(p) => PathBuf::from(p),
        None => context
            .working_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
    }
}

/// Open a tree-sitter workspace.
///
/// This uses the unified `Workspace` API which automatically handles leader
/// election. If no leader is running, this process becomes the leader.
/// If another process is already the leader, this connects as a client.
///
/// The returned `Workspace` can be used for queries regardless of whether
/// this process is the leader or a client.
pub async fn open_workspace(workspace_path: &PathBuf) -> Result<Workspace, McpError> {
    Workspace::open(workspace_path)
        .await
        .map_err(|e| McpError::internal_error(format!("Failed to open workspace: {}", e), None))
}

/// Format a code block with proper newline handling.
///
/// Ensures the code block has a trailing newline before the closing backticks.
fn format_code_block(text: &str) -> String {
    let mut block = String::from("```\n");
    block.push_str(text);
    if !text.ends_with('\n') {
        block.push('\n');
    }
    block.push_str("```\n\n");
    block
}

/// Format a chunk's file location as a string.
fn format_chunk_location(file: &std::path::Path, start_line: usize, end_line: usize) -> String {
    format!("{}:{}-{}", file.display(), start_line + 1, end_line + 1)
}

/// Generic formatter for result lists.
///
/// Handles the common pattern of: empty check, header with count, item formatting.
fn format_results<T, F>(items: &[T], empty_msg: &str, header_fmt: &str, format_item: F) -> String
where
    F: Fn(usize, &T) -> String,
{
    if items.is_empty() {
        return empty_msg.to_string();
    }

    let mut output = format!(
        "{}\n\n",
        header_fmt.replace("{count}", &items.len().to_string())
    );

    for (i, item) in items.iter().enumerate() {
        output.push_str(&format_item(i, item));
    }

    output
}

/// Format similar chunk results for display.
pub fn format_similar_chunks(results: &[SimilarChunkResult], header: &str) -> String {
    format_results(
        results,
        &format!("No {} found.", header),
        &format!("Found {{count}} {}:", header),
        |i, result| {
            let mut item = format!(
                "## Match {} (similarity: {:.2}%)\n",
                i + 1,
                result.similarity * 100.0
            );
            item.push_str(&format!(
                "**File:** {}\n",
                format_chunk_location(
                    &result.chunk.file,
                    result.chunk.start_line,
                    result.chunk.end_line
                )
            ));
            item.push_str(&format_code_block(&result.chunk.text));
            item
        },
    )
}

/// Format duplicate clusters for display.
pub fn format_duplicate_clusters(clusters: &[DuplicateCluster]) -> String {
    format_results(
        clusters,
        "No duplicate code clusters found.",
        "Found {count} duplicate code clusters:",
        |i, cluster| {
            let mut item = format!(
                "## Cluster {} ({} chunks, avg similarity: {:.2}%)\n\n",
                i + 1,
                cluster.chunks.len(),
                cluster.avg_similarity * 100.0
            );

            for (j, chunk) in cluster.chunks.iter().enumerate() {
                item.push_str(&format!(
                    "### Location {}: {}\n",
                    j + 1,
                    format_chunk_location(&chunk.file, chunk.start_line, chunk.end_line)
                ));
                item.push_str(&format_code_block(&chunk.text));
            }
            item
        },
    )
}

/// Format query match results for display.
pub fn format_query_matches(results: &[QueryMatch]) -> String {
    format_results(
        results,
        "No matches found for the query pattern.",
        "Found {count} matches:",
        |i, match_result| {
            let mut item = format!("## Match {}\n", i + 1);
            item.push_str(&format!("**File:** {}\n", match_result.file.display()));

            for capture in &match_result.captures {
                item.push_str(&format!(
                    "- **@{}** ({}): `{}` at lines {}-{}\n",
                    capture.name,
                    capture.kind,
                    capture.text.replace('\n', "\\n"),
                    capture.start_line + 1,
                    capture.end_line + 1
                ));
            }
            item.push('\n');
            item
        },
    )
}

#[cfg(test)]
pub mod test_helpers {
    use crate::mcp::tool_registry::McpTool;
    use crate::test_utils::create_test_context;
    use serde_json::json;

    /// Create a test context and temp directory for treesitter tool tests.
    pub async fn setup_test_env() -> (crate::mcp::tool_registry::ToolContext, tempfile::TempDir) {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        (context, temp_dir)
    }

    /// Create arguments with just a path pointing to the temp directory.
    pub fn args_with_path(
        temp_dir: &tempfile::TempDir,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "path".to_string(),
            json!(temp_dir.path().display().to_string()),
        );
        arguments
    }

    /// Assert basic tool properties: name matches expected and description contains keyword.
    pub fn assert_tool_basics<T: McpTool>(
        tool: &T,
        expected_name: &str,
        description_keyword: &str,
    ) {
        assert_eq!(McpTool::name(tool), expected_name);
        let desc = McpTool::description(tool);
        assert!(!desc.is_empty(), "Description should not be empty");
        assert!(
            desc.to_lowercase()
                .contains(&description_keyword.to_lowercase()),
            "Description '{}' should contain '{}'",
            desc,
            description_keyword
        );
    }

    /// Assert that the tool schema is a valid JSON object with properties.
    pub fn assert_schema_is_object<T: McpTool>(tool: &T) {
        let schema = McpTool::schema(tool);
        assert_eq!(schema["type"], "object", "Schema type should be 'object'");
        assert!(
            schema["properties"].is_object(),
            "Schema should have properties"
        );
    }

    /// Assert that the schema has specific required fields.
    pub fn assert_schema_has_required<T: McpTool>(tool: &T, required_fields: &[&str]) {
        let schema = McpTool::schema(tool);
        let required = schema["required"].as_array();
        assert!(required.is_some(), "Schema should have required array");
        let required = required.unwrap();
        for field in required_fields {
            assert!(
                required.contains(&json!(field)),
                "Required fields should include '{}'",
                field
            );
        }
    }

    /// Assert that the schema has specific property fields.
    pub fn assert_schema_has_properties<T: McpTool>(tool: &T, property_names: &[&str]) {
        let schema = McpTool::schema(tool);
        for name in property_names {
            assert!(
                schema["properties"][name].is_object(),
                "Schema should have property '{}'",
                name
            );
        }
    }

    /// Execute a tool with a temp directory path and optional extra arguments.
    ///
    /// Sets up the test environment, creates arguments with the temp path,
    /// merges any extra arguments, and executes the tool.
    pub async fn execute_tool_with_temp_path<T: McpTool>(
        tool: &T,
        extra_args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> (
        std::result::Result<rmcp::model::CallToolResult, rmcp::ErrorData>,
        tempfile::TempDir,
    ) {
        let (context, temp_dir) = setup_test_env().await;
        let mut arguments = args_with_path(&temp_dir);
        if let Some(extra) = extra_args {
            for (k, v) in extra {
                arguments.insert(k, v);
            }
        }
        let result = tool.execute(arguments, &context).await;
        (result, temp_dir)
    }

    /// Assert that tool execution succeeds on an empty workspace.
    ///
    /// With the Workspace API, opening a workspace makes the process
    /// a leader automatically, so tools should succeed (with empty results).
    pub async fn assert_execute_succeeds_on_empty_workspace<T: McpTool>(
        tool: &T,
        extra_args: Option<serde_json::Map<String, serde_json::Value>>,
    ) {
        let (result, _temp_dir) = execute_tool_with_temp_path(tool, extra_args).await;
        assert!(result.is_ok(), "Tool should succeed on empty workspace");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;
    use std::path::Path;
    use swissarmyhammer_treesitter::{query::Capture, ChunkResult};

    #[tokio::test]
    async fn test_resolve_workspace_path_with_explicit_path() {
        let context = create_test_context().await;
        let path = Some("/explicit/path".to_string());
        let result = resolve_workspace_path(path.as_ref(), &context);
        assert_eq!(result, PathBuf::from("/explicit/path"));
    }

    #[tokio::test]
    async fn test_resolve_workspace_path_from_context() {
        let mut context = create_test_context().await;
        context.working_dir = Some(PathBuf::from("/context/path"));
        let result = resolve_workspace_path(None, &context);
        assert_eq!(result, PathBuf::from("/context/path"));
    }

    #[tokio::test]
    async fn test_resolve_workspace_path_fallback() {
        let context = create_test_context().await;
        let result = resolve_workspace_path(None, &context);
        // Should fall back to current dir or default
        assert!(!result.as_os_str().is_empty() || result == PathBuf::default());
    }

    #[tokio::test]
    async fn test_open_workspace_auto_starts_leader() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let result = open_workspace(&temp_dir.path().to_path_buf()).await;

        // Should successfully open the index (becoming leader or connecting to one)
        assert!(result.is_ok(), "Should auto-start leader and connect");

        // Verify we got a working index by checking status
        let index = result.unwrap();
        let status = index.status().await;
        assert!(status.is_ok(), "Should be able to get status from index");
    }

    #[test]
    fn test_format_code_block_with_trailing_newline() {
        let output = format_code_block("code\n");
        assert_eq!(output, "```\ncode\n```\n\n");
    }

    #[test]
    fn test_format_code_block_without_trailing_newline() {
        let output = format_code_block("code");
        assert_eq!(output, "```\ncode\n```\n\n");
    }

    #[test]
    fn test_format_chunk_location() {
        let location = format_chunk_location(Path::new("/src/main.rs"), 0, 5);
        assert_eq!(location, "/src/main.rs:1-6");
    }

    #[test]
    fn test_format_chunk_location_same_line() {
        let location = format_chunk_location(Path::new("/test.rs"), 10, 10);
        assert_eq!(location, "/test.rs:11-11");
    }

    #[test]
    fn test_format_results_empty() {
        let items: Vec<i32> = vec![];
        let output = format_results(&items, "No items found.", "Found {count} items:", |_, _| {
            String::new()
        });
        assert_eq!(output, "No items found.");
    }

    #[test]
    fn test_format_results_with_items() {
        let items = vec!["a", "b"];
        let output = format_results(&items, "No items.", "Found {count} items:", |i, item| {
            format!("- Item {}: {}\n", i + 1, item)
        });
        assert!(output.contains("Found 2 items:"));
        assert!(output.contains("- Item 1: a"));
        assert!(output.contains("- Item 2: b"));
    }

    #[test]
    fn test_format_results_single_item() {
        let items = vec!["only"];
        let output = format_results(&items, "Empty.", "Got {count} thing:", |i, item| {
            format!("#{}: {}\n", i + 1, item)
        });
        assert!(output.contains("Got 1 thing:"));
        assert!(output.contains("#1: only"));
    }

    #[test]
    fn test_format_similar_chunks_empty() {
        let results: Vec<SimilarChunkResult> = vec![];
        let output = format_similar_chunks(&results, "similar code chunks");
        assert_eq!(output, "No similar code chunks found.");
    }

    #[test]
    fn test_format_similar_chunks_with_results() {
        let results = vec![SimilarChunkResult {
            chunk: ChunkResult {
                file: PathBuf::from("/test/file.rs"),
                text: "fn main() {}".to_string(),
                start_byte: 0,
                end_byte: 12,
                start_line: 0,
                end_line: 0,
            },
            similarity: 0.95,
        }];
        let output = format_similar_chunks(&results, "similar code chunks");
        assert!(output.contains("Found 1 similar code chunks"));
        assert!(output.contains("Match 1"));
        assert!(output.contains("95.00%"));
        assert!(output.contains("/test/file.rs"));
        assert!(output.contains("fn main() {}"));
    }

    #[test]
    fn test_format_similar_chunks_adds_newline() {
        let results = vec![SimilarChunkResult {
            chunk: ChunkResult {
                file: PathBuf::from("/test.rs"),
                text: "code without newline".to_string(),
                start_byte: 0,
                end_byte: 20,
                start_line: 0,
                end_line: 0,
            },
            similarity: 0.8,
        }];
        let output = format_similar_chunks(&results, "chunks");
        // Should add newline before closing backticks
        assert!(output.contains("code without newline\n```"));
    }

    #[test]
    fn test_format_query_matches_empty() {
        let results: Vec<QueryMatch> = vec![];
        let output = format_query_matches(&results);
        assert_eq!(output, "No matches found for the query pattern.");
    }

    #[test]
    fn test_format_query_matches_with_results() {
        let results = vec![QueryMatch {
            file: PathBuf::from("/src/main.rs"),
            captures: vec![Capture {
                name: "name".to_string(),
                kind: "identifier".to_string(),
                text: "main".to_string(),
                start_byte: 3,
                end_byte: 7,
                start_line: 0,
                end_line: 0,
            }],
        }];
        let output = format_query_matches(&results);
        assert!(output.contains("Found 1 matches"));
        assert!(output.contains("Match 1"));
        assert!(output.contains("/src/main.rs"));
        assert!(output.contains("@name"));
        assert!(output.contains("identifier"));
        assert!(output.contains("`main`"));
    }

    #[test]
    fn test_format_query_matches_escapes_newlines() {
        let results = vec![QueryMatch {
            file: PathBuf::from("/test.rs"),
            captures: vec![Capture {
                name: "body".to_string(),
                kind: "block".to_string(),
                text: "line1\nline2".to_string(),
                start_byte: 0,
                end_byte: 11,
                start_line: 0,
                end_line: 1,
            }],
        }];
        let output = format_query_matches(&results);
        assert!(output.contains("`line1\\nline2`"));
    }

    #[test]
    fn test_format_query_matches_line_numbers_one_indexed() {
        let results = vec![QueryMatch {
            file: PathBuf::from("/test.rs"),
            captures: vec![Capture {
                name: "fn".to_string(),
                kind: "function".to_string(),
                text: "test".to_string(),
                start_byte: 0,
                end_byte: 4,
                start_line: 0,
                end_line: 5,
            }],
        }];
        let output = format_query_matches(&results);
        // 0-indexed lines should become 1-indexed in output
        assert!(output.contains("lines 1-6"));
    }

    #[test]
    fn test_format_duplicate_clusters_empty() {
        let clusters: Vec<DuplicateCluster> = vec![];
        let output = format_duplicate_clusters(&clusters);
        assert_eq!(output, "No duplicate code clusters found.");
    }

    #[test]
    fn test_format_duplicate_clusters_with_results() {
        let clusters = vec![DuplicateCluster {
            chunks: vec![
                ChunkResult {
                    file: PathBuf::from("/a.rs"),
                    text: "duplicate code".to_string(),
                    start_byte: 0,
                    end_byte: 14,
                    start_line: 0,
                    end_line: 0,
                },
                ChunkResult {
                    file: PathBuf::from("/b.rs"),
                    text: "duplicate code".to_string(),
                    start_byte: 0,
                    end_byte: 14,
                    start_line: 5,
                    end_line: 5,
                },
            ],
            avg_similarity: 0.98,
        }];
        let output = format_duplicate_clusters(&clusters);
        assert!(output.contains("Found 1 duplicate code clusters"));
        assert!(output.contains("Cluster 1"));
        assert!(output.contains("2 chunks"));
        assert!(output.contains("98.00%"));
        assert!(output.contains("/a.rs"));
        assert!(output.contains("/b.rs"));
        assert!(output.contains("duplicate code"));
    }

    #[test]
    fn test_format_duplicate_clusters_line_numbers_one_indexed() {
        let clusters = vec![DuplicateCluster {
            chunks: vec![ChunkResult {
                file: PathBuf::from("/test.rs"),
                text: "code".to_string(),
                start_byte: 0,
                end_byte: 4,
                start_line: 0,
                end_line: 2,
            }],
            avg_similarity: 1.0,
        }];
        let output = format_duplicate_clusters(&clusters);
        // 0-indexed lines should become 1-indexed
        assert!(output.contains(":1-3"));
    }
}
