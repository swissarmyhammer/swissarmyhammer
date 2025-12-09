//! File handling utilities for tests

use std::path::PathBuf;

/// Reads a test file and cleans it by removing metadata comments
pub fn read_and_clean_test_file(path: &PathBuf) -> String {
    let content =
        std::fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments (lines starting with //)
    content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Checks if a file should be skipped based on content
#[allow(dead_code)]
pub fn should_skip_file(content: &str, expected_prefix: &str) -> bool {
    if content.is_empty() {
        return true;
    }

    // Check if first meaningful line starts with expected prefix
    if let Some(first_line) = content.lines().next() {
        !first_line.trim().starts_with(expected_prefix)
    } else {
        true
    }
}

/// Checks if content contains unsupported syntax patterns
#[allow(dead_code)]
pub fn has_unsupported_syntax(content: &str) -> bool {
    // Common unsupported patterns found in various test files
    content.contains("rect ")
        || content.contains("linkStyle")
        || content.contains("classDef")
        || content.contains("@{")
        || content.contains("style ")
        || content.contains("click ")
        || content.contains("href ")
        || content.contains("callback ")
        || content.contains("call ")
        || content.contains("class ")
        || content.contains("direction ")
        || content.contains("subgraph ")
        || content.contains("end")
        || content.contains(":::")
        || content.contains("---")
        || content.contains("stroke")
        || content.contains("fill")
        || content.contains("color")
        || content.contains("graph ")
}

/// Checks if content contains complex flowchart syntax
#[allow(dead_code)]
pub fn has_complex_flowchart_syntax(content: &str) -> bool {
    content.contains("graph ") 
        || content.contains("direction ")
        || content.contains("subgraph ")
        || content.contains("end")
        || content.contains("click ")
        || content.contains("href ")
        || content.contains("style ")
        || content.contains("classDef")
        || content.contains("class ")
        || content.contains("linkStyle")
        || content.contains("<br>") 
        || content.contains("<br/>") 
        || content.contains("<br />")
        || content.contains("@{")  // Node styling syntax
        || content.contains("|")  // Edge labels (heuristic)
        || content.contains(":::")  // Class assignment syntax
        || content.contains(" & ")  // Multiple nodes syntax
        || content.contains("@-->")  // Edge IDs syntax
        || content.contains("o--o")  // Special edge types
        || content.contains("<-->")  // Bidirectional arrows
        || content.contains("x--x") // Cross edge types
}

/// Checks for sequence diagram unsupported syntax
#[allow(dead_code)]
pub fn has_unsupported_sequence_syntax(content: &str) -> bool {
    content.contains("rect ")
        || content.contains("alt ")
        || content.contains("else")
        || content.contains("opt ")
        || content.contains("loop ")
        || content.contains("par ")
        || content.contains("and ")
        || content.contains("critical ")
        || content.contains("option ")
        || content.contains("break ")
        || content.contains("autonumber")
        || content.contains("activate ")
        || content.contains("deactivate ")
        || content.contains("note ")
        || content.contains("box ")
        || content.contains("participant ")
        || content.contains("actor ")
        || content.contains("create ")
        || content.contains("destroy ")
        || content.contains("link ")
        || content.contains("links ")
        || content.contains("properties ")
        || content.contains("details ")
}
