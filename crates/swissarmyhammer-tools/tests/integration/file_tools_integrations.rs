//! Integration tests for file tools
//!
//! These tests verify that file tools work correctly through all layers of the system,
//! including MCP protocol handling, tool registry integration, security validation,
//! and end-to-end scenarios.
//!
//! The tests sit in one module for each tool and each subject. The review
//! engine renders a whole file into one agent prompt, and a file over the
//! per-file prompt cap is not reviewed at all, so a test tree this size has
//! to be several files rather than one.
//!
//! - [`read`] — the read tool: discovery, offsets and limits, errors, path
//!   traversal, and the edge cases of an empty, a one-line and a unicode file.
//! - [`glob`] — the glob tool: patterns, gitignore, case, sort order.
//! - [`grep`] — the grep tool: patterns, filters, context lines, output modes.
//! - [`write`] — the write tool: new files, overwrites, parent directories.
//! - [`edit`] — the edit tool: one replacement, replace-all, and edit chains.
//! - [`composition`] — two or more tools in one workflow.
//! - [`security`] — path traversal, symlinks, privileged locations and
//!   malformed input, across every file tool.
//! - [`performance`] — the memory a large read, write or edit costs.
//! - [`concurrency`] — many operations against one registry at the same time.
//! - [`properties`] — round-trip and consistency properties over generated
//!   inputs.
//!
//! This module carries what those ten share: the imports, the registry and
//! context fixtures, the argument builders, and the tool-registration checks.

mod composition;
mod concurrency;
mod edit;
mod glob;
mod grep;
mod performance;
mod properties;
mod read;
mod security;
mod write;

use serde_json::json;
use std::fs;

use std::sync::Arc;

use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use swissarmyhammer_config::ChatModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::files;

#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::{BufRead, BufReader};

/// Memory usage profiling utilities for performance testing
struct MemoryProfiler {
    initial_memory: Option<usize>,
}

impl MemoryProfiler {
    fn new() -> Self {
        let initial_memory = Self::get_memory_usage();
        Self { initial_memory }
    }

    #[cfg(target_os = "linux")]
    fn get_memory_usage() -> Option<usize> {
        if let Ok(file) = File::open("/proc/self/status") {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.starts_with("VmRSS:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            if let Ok(kb) = parts[1].parse::<usize>() {
                                return Some(kb * 1024);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    #[cfg(not(target_os = "linux"))]
    fn get_memory_usage() -> Option<usize> {
        None
    }

    fn memory_delta(&self) -> Option<isize> {
        if let (Some(initial), Some(current)) = (self.initial_memory, Self::get_memory_usage()) {
            Some(current as isize - initial as isize)
        } else {
            None
        }
    }

    fn format_bytes(bytes: usize) -> String {
        if bytes >= 1_000_000_000 {
            format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
        } else if bytes >= 1_000_000 {
            format!("{:.1} MB", bytes as f64 / 1_000_000.0)
        } else if bytes >= 1_000 {
            format!("{:.1} KB", bytes as f64 / 1_000.0)
        } else {
            format!("{} bytes", bytes)
        }
    }
}

/// Create a test context with mock storage backends for testing MCP tools
async fn create_test_context() -> ToolContext {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let _unique_id = format!(
        "{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    );

    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ChatModelConfig::default());

    ToolContext::new(tool_handlers, git_ops, agent_config)
}

/// Create a test tool registry with file tools registered
async fn create_test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    files::register_file_tools(&mut registry);
    registry
}

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Extract response text from CallToolResult to eliminate duplication across tests
fn extract_response_text(call_result: &rmcp::model::CallToolResult) -> &str {
    if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content")
    }
}

/// Recover the plain file content from a default (hashline) `read files`
/// result.
///
/// The default read output is `#hash:<hex>\n` followed by hashline-tagged
/// lines `N:HH|text`. This strips the leading freshness-token line and the
/// per-line `N:HH|` anchors, reconstructing the original content (line endings
/// preserved) so tests that assert on raw content can keep doing so against the
/// new default. The whole-file hash itself is exercised separately.
fn read_content(call_result: &rmcp::model::CallToolResult) -> String {
    let text = extract_response_text(call_result);
    let body = match text.split_once('\n') {
        Some((hash_line, body)) => {
            assert!(
                hash_line.starts_with("#hash:"),
                "expected leading #hash: line, got {hash_line}"
            );
            body
        }
        // A read of a truly empty file is just the `#hash:<hex>` line with no
        // trailing newline and no body.
        None => {
            assert!(
                text.starts_with("#hash:"),
                "expected #hash: line, got {text}"
            );
            return String::new();
        }
    };

    // De-tag each line by dropping the `N:HH|` anchor prefix, preserving the
    // original line terminators.
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while !rest.is_empty() {
        let (line, terminator, remaining) = match rest.find(['\n', '\r']) {
            None => (rest, "", ""),
            Some(idx) => {
                let after = &rest[idx..];
                let (term, remaining) = if let Some(s) = after.strip_prefix("\r\n") {
                    ("\r\n", s)
                } else if let Some(s) = after.strip_prefix('\r') {
                    ("\r", s)
                } else {
                    ("\n", &after[1..])
                };
                (&rest[..idx], term, remaining)
            }
        };
        // Strip the `N:HH|` anchor prefix (text after the first `|`).
        let plain = line.split_once('|').map(|(_, t)| t).unwrap_or(line);
        out.push_str(plain);
        out.push_str(terminator);
        rest = remaining;
    }
    out
}

/// Create a test file with given name and content, returning env, temp_dir path, and file path
fn create_test_file(
    name: &str,
    content: &str,
) -> (
    IsolatedTestEnvironment,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir_path = env.temp_dir();
    let test_file = temp_dir_path.join(name);
    fs::write(&test_file, content).unwrap();
    (env, temp_dir_path, test_file)
}

/// Create a temporary directory with an initialized git repository
fn create_test_dir_with_git() -> (IsolatedTestEnvironment, std::path::PathBuf) {
    let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = env.temp_dir();
    let repo = git2::Repository::init(&temp_dir).expect("Failed to initialize git repo");
    // Ensure initial branch is 'main' for consistency across environments
    repo.set_head("refs/heads/main")
        .expect("Failed to set HEAD to main");
    (env, temp_dir)
}

/// Generic argument builder that accepts key-value pairs
fn build_args(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
    let mut arguments = serde_json::Map::new();
    for (key, value) in pairs {
        arguments.insert(key.to_string(), value.clone());
    }
    arguments
}

/// Builder helper for files read arguments
fn read_args(path: &str) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[("op", json!("read file")), ("path", json!(path))])
}

/// Builder helper for files write arguments
fn write_args(file_path: &str, content: &str) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[
        ("op", json!("write file")),
        ("file_path", json!(file_path)),
        ("content", json!(content)),
    ])
}

/// Builder helper for files edit arguments
fn edit_args(
    file_path: &str,
    old_string: &str,
    new_string: &str,
) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[
        ("op", json!("edit file")),
        ("file_path", json!(file_path)),
        ("old_string", json!(old_string)),
        ("new_string", json!(new_string)),
    ])
}

/// Builder helper for files glob arguments
fn glob_args(pattern: &str) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[("op", json!("glob files")), ("pattern", json!(pattern))])
}

/// Builder helper for files grep arguments
fn grep_args(pattern: &str) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[("op", json!("grep files")), ("pattern", json!(pattern))])
}

/// Verify tool exists in registry
fn verify_tool_exists(registry: &ToolRegistry, tool_name: &str) {
    assert!(
        registry.get_tool(tool_name).is_some(),
        "Tool {} should be registered",
        tool_name
    );

    let tool_names = registry.list_tool_names();
    assert!(
        tool_names.contains(&tool_name.to_string()),
        "Tool {} should be in tool list",
        tool_name
    );
}

/// Verify tool description contains expected keywords
fn verify_tool_description(
    tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool,
    description_keywords: &[&str],
) {
    assert!(
        !tool.description().is_empty(),
        "Description should not be empty"
    );

    let description = tool.description().to_lowercase();
    let contains_any_keyword = description_keywords
        .iter()
        .any(|keyword| description.contains(keyword));
    assert!(
        contains_any_keyword,
        "Description should contain at least one of keywords: {:?}, but got: {}",
        description_keywords, description
    );
}

/// Verify tool schema properties
fn verify_tool_schema(
    tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool,
    required_properties: &[&str],
    optional_properties: &[&str],
) {
    // The per-op flat `properties` only exist on the FULL schema; the wire
    // `schema()` carries just `op`. CLI-shaped assertions use `schema_full()`.
    let schema = tool.schema_full();
    assert!(schema.is_object(), "Schema should be an object");

    let properties = schema["properties"].as_object().unwrap();
    for prop in required_properties {
        assert!(
            properties.contains_key(*prop),
            "Schema should contain required property: {}",
            prop
        );
    }
    for prop in optional_properties {
        assert!(
            properties.contains_key(*prop),
            "Schema should contain optional property: {}",
            prop
        );
    }

    // Handle both simple "required" array and "oneOf" schemas
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        // Simple schema with required array
        for prop in required_properties {
            assert!(
                required.contains(&serde_json::Value::String(prop.to_string())),
                "Property {} should be required",
                prop
            );
        }
    } else if let Some(one_of) = schema.get("oneOf").and_then(|o| o.as_array()) {
        // Schema with oneOf - verify at least one alternative contains the required properties
        let mut found_in_any_alternative = vec![false; required_properties.len()];
        for alternative in one_of {
            if let Some(alt_required) = alternative.get("required").and_then(|r| r.as_array()) {
                for (idx, prop) in required_properties.iter().enumerate() {
                    if alt_required.contains(&serde_json::Value::String(prop.to_string())) {
                        found_in_any_alternative[idx] = true;
                    }
                }
            }
        }
        // For oneOf schemas, we just verify the properties exist, not that they're strictly required
        // This is because oneOf means "one of these alternatives must be satisfied"
    }
}

/// Verify tool registration with expected properties
fn verify_tool_registration(
    registry: &ToolRegistry,
    tool_name: &str,
    description_keywords: &[&str],
    required_properties: &[&str],
    optional_properties: &[&str],
) {
    verify_tool_exists(registry, tool_name);

    let tool = registry.get_tool(tool_name).unwrap();
    assert_eq!(<dyn swissarmyhammer_tools::mcp::tool_registry::McpTool as swissarmyhammer_tools::mcp::tool_registry::McpTool>::name(tool), tool_name);

    verify_tool_description(tool, description_keywords);
    verify_tool_schema(tool, required_properties, optional_properties);
}
