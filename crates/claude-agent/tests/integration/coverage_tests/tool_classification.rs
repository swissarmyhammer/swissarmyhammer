//! Coverage tests: Tool classification and title generation.

use claude_agent::tool_types::*;
use serde_json::json;

/// Tool name of the file read tool.
const READ_TEXT_FILE_TOOL: &str = "fs_read_text_file";

/// Tool name of the terminal create tool.
const TERMINAL_CREATE_TOOL: &str = "terminal_create";

/// Tool name of the code context tool.
const CODE_TOOL: &str = "mcp__sah__treesitter";

/// Title that the fallback produces for an unknown tool.
const UNKNOWN_TOOL_TITLE: &str = "Unknown tool";

/// Operation name of the semantic code search.
const SEARCH_CODE_OP: &str = "search code";

/// Argument key that names the code context operation.
const OP_KEY: &str = "op";

/// Tool name for a text file write.
const WRITE_TEXT_FILE_TOOL: &str = "fs_write_text_file";

/// Tool name for a search.
const SEARCH_TOOL: &str = "search";

/// JSON key that carries a file path.
const PATH_KEY: &str = "path";

/// JSON key that carries a search query.
const QUERY_KEY: &str = "query";

/// Search query that the title tests use.
const RETRY_LOGIC_QUERY: &str = "retry logic";

// ============================================================================
// Tool classification tests
// ============================================================================

#[test]
fn test_tool_kind_classify_read_operations() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool(READ_TEXT_FILE_TOOL, &args),
        ToolKind::Read
    );
    assert_eq!(ToolKind::classify_tool("fs_read", &args), ToolKind::Read);
    assert_eq!(ToolKind::classify_tool("read_file", &args), ToolKind::Read);
}

#[test]
fn test_tool_kind_classify_edit_operations() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool(WRITE_TEXT_FILE_TOOL, &args),
        ToolKind::Edit
    );
    assert_eq!(ToolKind::classify_tool("fs_write", &args), ToolKind::Edit);
    assert_eq!(ToolKind::classify_tool("edit_file", &args), ToolKind::Edit);
}

#[test]
fn test_tool_kind_classify_delete() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool("fs_delete", &args),
        ToolKind::Delete
    );
    assert_eq!(
        ToolKind::classify_tool("delete_file", &args),
        ToolKind::Delete
    );
}

#[test]
fn test_tool_kind_classify_move() {
    let args = json!({});
    assert_eq!(ToolKind::classify_tool("fs_move", &args), ToolKind::Move);
    assert_eq!(
        ToolKind::classify_tool("rename_file", &args),
        ToolKind::Move
    );
}

#[test]
fn test_tool_kind_classify_search() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool(SEARCH_TOOL, &args),
        ToolKind::Search
    );
    assert_eq!(ToolKind::classify_tool("grep", &args), ToolKind::Search);
    assert_eq!(ToolKind::classify_tool("find", &args), ToolKind::Search);
}

#[test]
fn test_tool_kind_classify_execute() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool(TERMINAL_CREATE_TOOL, &args),
        ToolKind::Execute
    );
    assert_eq!(ToolKind::classify_tool("execute", &args), ToolKind::Execute);
    assert_eq!(ToolKind::classify_tool("run", &args), ToolKind::Execute);
}

#[test]
fn test_tool_kind_classify_fetch() {
    let args = json!({});
    assert_eq!(ToolKind::classify_tool("fetch", &args), ToolKind::Fetch);
    assert_eq!(ToolKind::classify_tool("curl", &args), ToolKind::Fetch);
}

#[test]
fn test_tool_kind_classify_think() {
    let args = json!({});
    assert_eq!(ToolKind::classify_tool("think", &args), ToolKind::Think);
    assert_eq!(ToolKind::classify_tool("plan", &args), ToolKind::Think);
}

#[test]
fn test_tool_kind_classify_unknown() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool("unknown_tool", &args),
        ToolKind::Other
    );
}

#[test]
fn test_tool_kind_classify_mcp_tools() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool("mcp__server__read_file", &args),
        ToolKind::Read
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__write_data", &args),
        ToolKind::Edit
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__delete_item", &args),
        ToolKind::Delete
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__search_code", &args),
        ToolKind::Search
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__execute_cmd", &args),
        ToolKind::Execute
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__fetch_data", &args),
        ToolKind::Fetch
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__misc_op", &args),
        ToolKind::Other
    );
}

#[test]
fn test_tool_kind_classify_treesitter_by_op() {
    assert_eq!(
        ToolKind::classify_tool(CODE_TOOL, &json!({(OP_KEY): SEARCH_CODE_OP})),
        ToolKind::Search
    );
    assert_eq!(
        ToolKind::classify_tool(CODE_TOOL, &json!({(OP_KEY): "query ast"})),
        ToolKind::Read
    );
    assert_eq!(
        ToolKind::classify_tool(CODE_TOOL, &json!({(OP_KEY): "unknown op"})),
        ToolKind::Other
    );
}

// ============================================================================
// Tool title generation tests
// ============================================================================

#[test]
fn test_generate_title_fs_read() {
    let title = ToolCallReport::generate_title(
        READ_TEXT_FILE_TOOL,
        &json!({(PATH_KEY): "/home/user/test.rs"}),
    );
    assert_eq!(title, "Reading test.rs");
}

#[test]
fn test_generate_title_fs_read_no_path() {
    let title = ToolCallReport::generate_title(READ_TEXT_FILE_TOOL, &json!({}));
    assert_eq!(title, "Reading file");
}

#[test]
fn test_generate_title_fs_write() {
    let title = ToolCallReport::generate_title(
        WRITE_TEXT_FILE_TOOL,
        &json!({(PATH_KEY): "/tmp/output.txt"}),
    );
    assert_eq!(title, "Writing to output.txt");
}

#[test]
fn test_generate_title_terminal_create() {
    let title =
        ToolCallReport::generate_title(TERMINAL_CREATE_TOOL, &json!({"command": "cargo test"}));
    assert_eq!(title, "Running cargo test");
}

#[test]
fn test_generate_title_terminal_create_no_command() {
    let title = ToolCallReport::generate_title(TERMINAL_CREATE_TOOL, &json!({}));
    assert_eq!(title, "Creating terminal session");
}

#[test]
fn test_generate_title_search() {
    let title = ToolCallReport::generate_title(SEARCH_TOOL, &json!({"pattern": "TODO"}));
    assert_eq!(title, "Searching for 'TODO'");
}

#[test]
fn test_generate_title_mcp_tool() {
    let title = ToolCallReport::generate_title("mcp__server__read_data", &json!({}));
    // The prefix is dropped and each underscore becomes a space
    assert_eq!(title, "Server  read data");
}

#[test]
fn test_generate_title_unknown_tool() {
    let title = ToolCallReport::generate_title("custom_tool_name", &json!({}));
    assert_eq!(title, "Custom tool name");
}

#[test]
fn test_generate_title_empty_tool_name() {
    let title = ToolCallReport::generate_title("", &json!({}));
    assert_eq!(title, UNKNOWN_TOOL_TITLE);
}

#[test]
fn test_generate_title_treesitter_search_code_names_the_query() {
    let title = ToolCallReport::generate_title(
        CODE_TOOL,
        &json!({(OP_KEY): SEARCH_CODE_OP, (QUERY_KEY): RETRY_LOGIC_QUERY}),
    );
    assert_eq!(title, "Searching code for 'retry logic'");
}

#[test]
fn test_generate_title_treesitter_search_code_is_not_the_generic_fallback() {
    let title = ToolCallReport::generate_title(
        CODE_TOOL,
        &json!({(OP_KEY): SEARCH_CODE_OP, (QUERY_KEY): RETRY_LOGIC_QUERY}),
    );
    assert_ne!(title, "Code operation");
}

#[test]
fn test_generate_title_treesitter_search_code_without_query() {
    let title = ToolCallReport::generate_title(CODE_TOOL, &json!({(OP_KEY): SEARCH_CODE_OP}));
    assert_eq!(title, "Searching code");
}

#[test]
fn test_generate_title_mcp_prefix_without_a_name() {
    let title = ToolCallReport::generate_title("mcp__", &json!({}));
    assert_eq!(title, UNKNOWN_TOOL_TITLE);
}

#[test]
fn test_generate_title_name_of_only_underscores() {
    let title = ToolCallReport::generate_title("_", &json!({}));
    assert_eq!(title, UNKNOWN_TOOL_TITLE);
}
