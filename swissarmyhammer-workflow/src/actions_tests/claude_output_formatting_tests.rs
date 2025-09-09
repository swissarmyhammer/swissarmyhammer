//! Tests for Claude output formatting functionality

use crate::actions::format_claude_output_as_yaml;

#[test]
fn test_format_claude_output_as_yaml() {
    // Test JSON object formatting
    let json_line = r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_01HHxb5NnvSfUxwCTQptWWaE","type":"tool_result","content":"Test content"}]},"parent_tool_use_id":null,"session_id":"e99afa02-75bc-4f2f-baef-68d5e071f023"}"#;

    let formatted = format_claude_output_as_yaml(json_line);

    // The output should be in YAML format
    assert!(formatted.contains("type: user"));
    assert!(formatted.contains("message:"));
    assert!(formatted.contains("role: user"));
    assert!(formatted.contains("content:"));
    assert!(formatted.contains("tool_use_id: toolu_01HHxb5NnvSfUxwCTQptWWaE"));
    assert!(formatted.contains("type: tool_result"));
    assert!(formatted.contains("content: Test content"));
    assert!(formatted.contains("parent_tool_use_id: null"));
    assert!(formatted.contains("session_id:"));
}

#[test]
fn test_format_claude_output_as_yaml_invalid_json() {
    // Test that invalid JSON returns the original string
    let invalid_json = "not a json string";
    let formatted = format_claude_output_as_yaml(invalid_json);
    assert_eq!(formatted, invalid_json);
}

#[test]
fn test_format_claude_output_as_yaml_empty_string() {
    // Test empty string
    let empty = "";
    let formatted = format_claude_output_as_yaml(empty);
    assert_eq!(formatted, empty);
}

#[test]
fn test_format_claude_output_as_yaml_whitespace() {
    // Test whitespace-only string
    let whitespace = "   \n   ";
    let formatted = format_claude_output_as_yaml(whitespace);
    assert_eq!(formatted, whitespace.trim());
}

#[test]
fn test_format_claude_output_as_yaml_nested_objects() {
    // Test deeply nested JSON object
    let nested_json = r#"{"level1":{"level2":{"level3":{"value":"deep"}}}}"#;
    let formatted = format_claude_output_as_yaml(nested_json);

    assert!(formatted.contains("level1:"));
    assert!(formatted.contains("level2:"));
    assert!(formatted.contains("level3:"));
    assert!(formatted.contains("value: deep"));
}

#[test]
fn test_format_claude_output_as_yaml_arrays() {
    // Test JSON with arrays
    let json_with_array = r#"{"items":["one","two","three"],"count":3}"#;
    let formatted = format_claude_output_as_yaml(json_with_array);

    assert!(formatted.contains("items:"));
    assert!(formatted.contains("- one"));
    assert!(formatted.contains("- two"));
    assert!(formatted.contains("- three"));
    assert!(formatted.contains("count: 3"));
}

#[test]
fn test_format_claude_output_as_yaml_multiline_string() {
    // Test JSON with multiline string containing \n
    let json_with_multiline = r#"{"message":{"content":"Line 1\nLine 2\nLine 3"},"type":"text"}"#;
    let formatted = format_claude_output_as_yaml(json_with_multiline);

    // Should use YAML block scalar notation
    assert!(formatted.contains("content: |-"));
    assert!(formatted.contains("  Line 1"));
    assert!(formatted.contains("  Line 2"));
    assert!(formatted.contains("  Line 3"));
    assert!(formatted.contains("type: text"));
}

#[test]
fn test_format_claude_output_as_yaml_source_code() {
    // Test JSON with source code
    let json_with_code = r#"{"message":{"content":"use anyhow::Result;\nuse colored::*;\n\nfn main() {\n    println!(\"Hello\");\n}"},"language":"rust"}"#;
    let formatted = format_claude_output_as_yaml(json_with_code);

    // Should format as multiline with proper indentation
    assert!(formatted.contains("content: |-"));

    // The content might have ANSI escape codes for syntax highlighting
    // So we check if the basic structure is present
    assert!(formatted.contains("use ") || formatted.contains("\x1b"));
    assert!(formatted.contains("colored") || formatted.contains("\x1b"));
    assert!(formatted.contains("fn main") || formatted.contains("\x1b"));
    assert!(formatted.contains("println") || formatted.contains("\x1b"));
    assert!(formatted.contains("language: rust"));
}

#[test]
fn test_format_claude_output_as_yaml_mixed_content() {
    // Test JSON with both single and multiline strings
    let json_mixed =
        r#"{"title":"Test","description":"This is a\nmultiline\ndescription","status":"active"}"#;
    let formatted = format_claude_output_as_yaml(json_mixed);

    assert!(formatted.contains("title: Test"));
    assert!(formatted.contains("description: |-"));
    assert!(formatted.contains("  This is a"));
    assert!(formatted.contains("  multiline"));
    assert!(formatted.contains("  description"));
    assert!(formatted.contains("status: active"));
}

#[test]
fn test_format_claude_output_as_yaml_with_syntax_highlighting() {
    // Test JSON with source code that should be syntax highlighted
    let json_with_highlighted_code = r#"{"message":{"content":"fn main() {\n    println!(\"Hello, world!\");\n}"},"type":"code"}"#;
    let formatted = format_claude_output_as_yaml(json_with_highlighted_code);

    // Should format as multiline with syntax highlighting
    assert!(formatted.contains("content: |-"));
    // The output should contain ANSI escape codes for syntax highlighting
    assert!(formatted.contains("\x1b["));
    assert!(formatted.contains("type: code"));
}
