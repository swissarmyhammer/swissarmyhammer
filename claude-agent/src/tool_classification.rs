// TODO - this looks to be only used from testing and is a useless reformatting fo existing data

//! Tool classification and title generation for ACP compliance
//!
//! This module provides functionality to classify tools by kind and generate
//! human-readable titles based on tool names and parameters.

use crate::tool_types::{ToolCallReport, ToolKind};

impl ToolKind {
    /// Classify a tool by its name and parameters to determine the appropriate kind
    pub fn classify_tool(tool_name: &str, arguments: &serde_json::Value) -> Self {
        // ACP requires comprehensive tool call reporting with rich metadata:
        // 1. toolCallId: Unique identifier for correlation across updates
        // 2. title: Human-readable description of tool operation
        // 3. kind: Classification for UI optimization and icon selection
        // 4. status: Lifecycle state (pending, in_progress, completed, failed)
        // 5. content: Output content produced by tool execution
        // 6. locations: File paths for follow-along features
        // 7. rawInput/rawOutput: Detailed I/O data for debugging
        //
        // Complete reporting enables rich client experiences and debugging.

        match tool_name {
            // File system read operations
            "fs_read_text_file" | "fs_read" | "read_file" => ToolKind::Read,

            // File system write and modification operations
            "fs_write_text_file" | "fs_write" | "write_file" | "fs_edit" | "edit_file" => {
                ToolKind::Edit
            }

            // File deletion operations
            "fs_delete" | "delete_file" | "remove_file" => ToolKind::Delete,

            // File move and rename operations
            "fs_move" | "move_file" | "rename_file" => ToolKind::Move,

            // Search and grep operations
            "fs_search" | "search" | "grep" | "find" => ToolKind::Search,

            // Terminal and command execution
            "terminal_create" | "terminal_write" | "terminal_read" | "execute" | "run" => {
                ToolKind::Execute
            }

            // External data fetching
            "fetch" | "http_get" | "download" | "curl" | "wget" => ToolKind::Fetch,

            // Internal reasoning and planning tools
            "think" | "reason" | "plan" | "analyze_approach" | "generate_strategy" => {
                ToolKind::Think
            }

            // Operation-based tools - classify by op argument
            "files" => Self::classify_by_op(arguments),

            // MCP tools - classify by prefix pattern or op argument
            tool if tool.contains("mcp__") => {
                // Check for operation-based tools (e.g., mcp__sah__files)
                if tool.ends_with("__files") || tool.ends_with("__treesitter") {
                    return Self::classify_by_op(arguments);
                }

                if tool.contains("read")
                    || tool.contains("get")
                    || tool.contains("show")
                    || tool.contains("list")
                {
                    ToolKind::Read
                } else if tool.contains("write")
                    || tool.contains("create")
                    || tool.contains("edit")
                    || tool.contains("update")
                {
                    ToolKind::Edit
                } else if tool.contains("delete") || tool.contains("remove") {
                    ToolKind::Delete
                } else if tool.contains("search") || tool.contains("grep") || tool.contains("find")
                {
                    ToolKind::Search
                } else if tool.contains("execute")
                    || tool.contains("shell")
                    || tool.contains("terminal")
                {
                    ToolKind::Execute
                } else if tool.contains("fetch") || tool.contains("web") || tool.contains("http") {
                    ToolKind::Fetch
                } else {
                    ToolKind::Other
                }
            }

            // Default fallback for unknown tools
            _ => ToolKind::Other,
        }
    }

    /// Classify an operation-based tool by inspecting the `op` argument
    fn classify_by_op(arguments: &serde_json::Value) -> Self {
        let op = arguments
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match op {
            "read file" => ToolKind::Read,
            "write file" | "edit file" => ToolKind::Edit,
            "glob files" => ToolKind::Read,
            "grep files" | "search code" => ToolKind::Search,
            "query ast" => ToolKind::Read,
            _ => ToolKind::Other,
        }
    }
}

/// Tool title generation system for human-readable descriptions
impl ToolCallReport {
    /// Generate a context-aware human-readable title based on tool name and parameters
    pub fn generate_title(tool_name: &str, arguments: &serde_json::Value) -> String {
        match tool_name {
            "fs_read_text_file" | "fs_read" => {
                if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
                    format!(
                        "Reading {}",
                        std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path)
                    )
                } else {
                    "Reading file".to_string()
                }
            }
            "fs_write_text_file" | "fs_write" => {
                if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
                    format!(
                        "Writing to {}",
                        std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path)
                    )
                } else {
                    "Writing file".to_string()
                }
            }
            "terminal_create" => {
                if let Some(command) = arguments.get("command").and_then(|v| v.as_str()) {
                    format!("Running {}", command)
                } else {
                    "Creating terminal session".to_string()
                }
            }
            "fs_delete" => {
                if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
                    format!(
                        "Deleting {}",
                        std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path)
                    )
                } else {
                    "Deleting file".to_string()
                }
            }
            "search" | "grep" => {
                if let Some(pattern) = arguments.get("pattern").and_then(|v| v.as_str()) {
                    format!("Searching for '{}'", pattern)
                } else {
                    "Searching files".to_string()
                }
            }
            // Operation-based tools - generate title from op and arguments
            "files" => Self::generate_op_title(arguments),

            // MCP tools - generate titles based on tool name and parameters
            tool if tool.starts_with("mcp__") => {
                // Check for operation-based tools (e.g., mcp__sah__files)
                if tool.ends_with("__files") || tool.ends_with("__treesitter") {
                    return Self::generate_op_title(arguments);
                }

                let clean_name = tool.strip_prefix("mcp__").unwrap_or(tool).replace('_', " ");

                // Capitalize first letter
                let mut chars = clean_name.chars();
                match chars.next() {
                    None => tool.to_string(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
            // Default case - convert snake_case to Title Case
            _ => {
                let clean_name = tool_name.replace('_', " ");
                let mut chars = clean_name.chars();
                match chars.next() {
                    None => "Unknown tool".to_string(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        }
    }

    /// Generate a title for operation-based tools by inspecting the `op` argument
    fn generate_op_title(arguments: &serde_json::Value) -> String {
        let op = arguments
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let path = arguments
            .get("path")
            .or_else(|| arguments.get("file_path"))
            .and_then(|v| v.as_str());
        let filename = path.map(|p| {
            std::path::Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        });

        match op {
            "read file" => match filename {
                Some(f) => format!("Reading {}", f),
                None => "Reading file".to_string(),
            },
            "write file" => match filename {
                Some(f) => format!("Writing to {}", f),
                None => "Writing file".to_string(),
            },
            "edit file" => match filename {
                Some(f) => format!("Editing {}", f),
                None => "Editing file".to_string(),
            },
            "glob files" => {
                if let Some(pattern) = arguments.get("pattern").and_then(|v| v.as_str()) {
                    format!("Globbing '{}'", pattern)
                } else {
                    "Globbing files".to_string()
                }
            }
            "grep files" => {
                if let Some(pattern) = arguments.get("pattern").and_then(|v| v.as_str()) {
                    format!("Searching for '{}'", pattern)
                } else {
                    "Searching files".to_string()
                }
            }
            "query ast" => match filename {
                Some(f) => format!("Querying AST of {}", f),
                None => "Querying AST".to_string(),
            },
            _ => "Files operation".to_string(),
        }
    }
}
