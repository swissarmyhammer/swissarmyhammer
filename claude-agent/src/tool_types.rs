// TODO - this has near duplicate code an types with similar structures ACP and MCP
// TODO - this whole module should go as it is just used from testing and we should test the natural ACP and MCP types
//! Tool call data structures and types for ACP compliance
//!
//! This module contains the core data structures used for tool call reporting
//! according to the Agent Client Protocol (ACP) specification.

use serde::{Deserialize, Serialize};

/// ACP-compliant tool call classification according to specification
///
/// Tool kinds help Clients choose appropriate icons and optimize how they display
/// tool execution progress. This enum matches the ACP specification exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    /// Reading files or data
    Read,
    /// Modifying files or content
    Edit,
    /// Removing files or data
    Delete,
    /// Moving or renaming files
    Move,
    /// Searching for information
    Search,
    /// Running commands or code
    Execute,
    /// Internal reasoning or planning
    Think,
    /// Retrieving external data
    Fetch,
    /// Other tool types (default)
    #[serde(other)]
    Other,
}

/// ACP-compliant tool call execution status
///
/// Tool calls progress through different statuses during their lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    /// The tool call hasn't started running yet because the input is either streaming or awaiting approval
    Pending,
    /// The tool call is currently running
    InProgress,
    /// The tool call completed successfully
    Completed,
    /// The tool call failed with an error
    Failed,
    /// The tool call was cancelled before completion
    Cancelled,
}

/// Content produced by a tool call execution
///
/// Tool calls can produce different types of content including regular content blocks,
/// file diffs, and embedded terminals for live command output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallContent {
    /// Standard content blocks like text, images, or resources
    Content {
        /// The actual content block
        content: agent_client_protocol::ContentBlock,
    },
    /// File modifications shown as diffs
    Diff {
        /// The absolute file path being modified
        path: String,
        /// The original content (null for new files)
        #[serde(rename = "oldText")]
        old_text: Option<String>,
        /// The new content after modification
        #[serde(rename = "newText")]
        new_text: String,
    },
    /// Live terminal output from command execution
    Terminal {
        /// The ID of a terminal created with terminal/create
        #[serde(rename = "terminalId")]
        terminal_id: String,
    },
}

/// File location affected by a tool call for "follow-along" features
///
/// Tool calls can report file locations they're working with, enabling Clients
/// to implement features that track which files the Agent is accessing or modifying.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallLocation {
    /// The absolute file path being accessed or modified
    pub path: String,
    /// Optional line number within the file
    pub line: Option<u64>,
}

/// Lightweight snapshot of tool call state for change detection
///
/// This structure tracks the last state sent in an update to enable
/// ACP-compliant partial updates that only include changed fields.
#[derive(Debug, Clone, PartialEq)]
struct ToolCallReportSnapshot {
    status: ToolCallStatus,
    title: String,
    kind: ToolKind,
    content_len: usize,
    locations_len: usize,
    raw_input_present: bool,
    raw_output_present: bool,
}

/// Complete ACP-compliant tool call report structure
///
/// This struct contains all metadata required by the ACP specification for
/// comprehensive tool call reporting with rich client experiences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallReport {
    /// Unique identifier for this tool call within the session
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
    /// Human-readable title describing what the tool is doing
    pub title: String,
    /// The category of tool being invoked
    pub kind: ToolKind,
    /// The current execution status
    pub status: ToolCallStatus,
    /// Content produced by the tool call
    #[serde(default)]
    pub content: Vec<ToolCallContent>,
    /// File locations affected by this tool call
    #[serde(default)]
    pub locations: Vec<ToolCallLocation>,
    /// The raw input parameters sent to the tool
    #[serde(rename = "rawInput", skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<serde_json::Value>,
    /// The raw output returned by the tool
    #[serde(rename = "rawOutput", skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<serde_json::Value>,
    /// The actual tool name (e.g., "Read", "Write", "Bash")
    #[serde(skip)]
    pub tool_name: String,
    /// Shadow copy of the last state sent in an update (for change detection)
    ///
    /// ACP partial updates optimize bandwidth by only sending changed fields.
    /// This tracks the previous state to enable efficient change detection.
    #[serde(skip)]
    previous_state: Option<Box<ToolCallReportSnapshot>>,
}

impl ToolCallReport {
    /// Create a new tool call report
    pub fn new(tool_call_id: String, title: String, kind: ToolKind, tool_name: String) -> Self {
        Self {
            tool_call_id,
            title,
            kind,
            status: ToolCallStatus::Pending,
            content: Vec::new(),
            locations: Vec::new(),
            raw_input: None,
            raw_output: None,
            tool_name,
            previous_state: None,
        }
    }

    /// Extract file locations from tool parameters for ACP file location tracking
    pub fn extract_file_locations(
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Vec<ToolCallLocation> {
        let mut locations = Vec::new();

        // Common file path parameter names across different tools
        let path_fields = [
            "path",
            "file_path",
            "filepath",
            "filename",
            "file",
            "source",
            "dest",
            "destination",
            "input",
            "output",
        ];

        // Extract file paths from common parameter structures
        match arguments {
            serde_json::Value::Object(obj) => {
                for field_name in &path_fields {
                    if let Some(path_value) = obj.get(*field_name) {
                        if let Some(path_str) = path_value.as_str() {
                            // Only add valid file paths (not URLs or other non-file paths)
                            if Self::is_file_path(path_str) {
                                locations.push(ToolCallLocation {
                                    path: Self::normalize_path(path_str),
                                    line: None,
                                });
                            }
                        }
                    }
                }

                // Handle array of paths (e.g., patterns in glob operations)
                if let Some(serde_json::Value::Array(pattern_array)) = obj.get("patterns") {
                    for pattern in pattern_array {
                        if let Some(pattern_str) = pattern.as_str() {
                            if Self::is_file_path(pattern_str) {
                                locations.push(ToolCallLocation {
                                    path: Self::normalize_path(pattern_str),
                                    line: None,
                                });
                            }
                        }
                    }
                }

                // Handle line number if present (for edit operations)
                let line_number = obj
                    .get("line")
                    .and_then(|v| v.as_u64())
                    .or_else(|| obj.get("line_number").and_then(|v| v.as_u64()))
                    .or_else(|| obj.get("offset").and_then(|v| v.as_u64()));

                // Add line number to the first location if available
                if let (Some(line), Some(first_location)) = (line_number, locations.first_mut()) {
                    first_location.line = Some(line);
                }
            }
            _ => {
                // Handle string parameters that might be file paths
                if let Some(path_str) = arguments.as_str() {
                    if Self::is_file_path(path_str) {
                        locations.push(ToolCallLocation {
                            path: Self::normalize_path(path_str),
                            line: None,
                        });
                    }
                }
            }
        }

        // Tool-specific location extraction
        match tool_name {
            // MCP file operations
            tool if tool.starts_with("mcp__files_") => {
                // These tools typically use standardized parameter names
                // Already handled by the generic extraction above
            }
            // Built-in file operations
            "Read" | "Write" | "Edit" | "Glob" | "Grep" => {
                // Already handled by the generic extraction above
            }
            _ => {
                // For unknown tools, the generic extraction should suffice
            }
        }

        locations
    }

    /// Check if a string represents a file path (not URL, command, etc.)
    fn is_file_path(s: &str) -> bool {
        // Skip URLs
        if s.starts_with("http://") || s.starts_with("https://") {
            return false;
        }

        // Skip commands or other non-path strings
        if s.contains(' ') && !s.starts_with('/') && !s.contains('\\') {
            return false;
        }

        // Handle glob patterns - these are definitely file patterns
        if s.contains('*') || s.contains('?') || s.contains('[') {
            return true;
        }

        // Must contain path separators or be absolute path
        s.contains('/')
            || s.contains('\\')
            || s.starts_with('/')
            || (cfg!(windows) && s.contains(':'))
    }

    /// Normalize file path to absolute form for consistency
    fn normalize_path(path: &str) -> String {
        // Don't normalize glob patterns - they should remain as-is
        if path.contains('*') || path.contains('?') || path.contains('[') {
            return path.to_string();
        }

        // Convert to absolute path if relative
        if path.starts_with("./") || path.starts_with("../") {
            // Try to resolve relative paths
            if let Ok(absolute) = std::fs::canonicalize(path) {
                absolute.to_string_lossy().to_string()
            } else {
                // If canonicalize fails, try to construct absolute path
                if let Ok(current_dir) = std::env::current_dir() {
                    current_dir.join(path).to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            }
        } else if !path.starts_with('/') && !path.contains(':') {
            // Relative path without ./ prefix - make absolute only for non-glob patterns
            if let Ok(current_dir) = std::env::current_dir() {
                current_dir.join(path).to_string_lossy().to_string()
            } else {
                path.to_string()
            }
        } else {
            // Already absolute path
            path.to_string()
        }
    }

    /// Update the status of this tool call
    pub fn update_status(&mut self, status: ToolCallStatus) {
        self.status = status;
    }

    /// Add content to this tool call
    pub fn add_content(&mut self, content: ToolCallContent) {
        self.content.push(content);
    }

    /// Add a file location to this tool call
    pub fn add_location(&mut self, location: ToolCallLocation) {
        self.locations.push(location);
    }

    /// Set the raw input parameters for this tool call
    pub fn set_raw_input(&mut self, input: serde_json::Value) {
        self.raw_input = Some(input);
    }

    /// Set the raw output for this tool call
    pub fn set_raw_output(&mut self, output: serde_json::Value) {
        self.raw_output = Some(output);
    }

    /// Capture current state as the baseline for future change detection
    ///
    /// This method should be called after sending a tool call update to mark
    /// the current state as "sent", enabling ACP-compliant partial updates
    /// that only include changed fields in subsequent updates.
    pub fn mark_state_sent(&mut self) {
        self.previous_state = Some(Box::new(ToolCallReportSnapshot {
            status: self.status,
            title: self.title.clone(),
            kind: self.kind,
            content_len: self.content.len(),
            locations_len: self.locations.len(),
            raw_input_present: self.raw_input.is_some(),
            raw_output_present: self.raw_output.is_some(),
        }));
    }

    /// Convert to agent_client_protocol::ToolCall for session notifications
    pub fn to_acp_tool_call(&self) -> agent_client_protocol::ToolCall {
        agent_client_protocol::ToolCall {
            id: agent_client_protocol::ToolCallId(self.tool_call_id.clone().into()),
            title: self.title.clone(),
            kind: self.kind.to_acp_kind(),
            status: self.status.to_acp_status(),
            content: self.content.iter().map(|c| c.to_acp_content()).collect(),
            locations: self.locations.iter().map(|l| l.to_acp_location()).collect(),
            raw_input: self.raw_input.clone(),
            raw_output: self.raw_output.clone(),
            meta: None,
        }
    }

    /// Convert to agent_client_protocol::ToolCallUpdate for status updates
    ///
    /// This method implements ACP-compliant partial updates by only including
    /// fields that have changed since the last update was sent. This optimizes
    /// bandwidth and processing efficiency as per the ACP specification:
    /// "All fields except toolCallId are optional in updates. Only the fields
    /// being changed need to be included."
    ///
    /// On the first update (when no previous state exists), all fields are included
    /// to ensure the client receives complete initial state.
    ///
    /// # Parameters
    /// * `include_context_fields` - If true, always include content and locations even if unchanged
    ///   (useful for final updates like completion/failure to provide full context)
    pub fn to_acp_tool_call_update_with_context(
        &self,
        include_context_fields: bool,
    ) -> agent_client_protocol::ToolCallUpdate {
        let fields = if let Some(prev) = &self.previous_state {
            // ACP partial update: only include fields that have changed since last update
            agent_client_protocol::ToolCallUpdateFields {
                status: if prev.status != self.status {
                    Some(self.status.to_acp_status())
                } else {
                    None
                },
                title: if prev.title != self.title {
                    Some(self.title.clone())
                } else {
                    None
                },
                kind: if prev.kind != self.kind {
                    Some(self.kind.to_acp_kind())
                } else {
                    None
                },
                // Note: Content and location changes are detected by length only
                // (intentional performance optimization - O(1) vs O(n) deep comparison).
                // This means modifying existing items without changing count won't trigger
                // an update, but add/remove operations are always detected.
                content: if prev.content_len != self.content.len()
                    || (include_context_fields && !self.content.is_empty())
                {
                    Some(self.content.iter().map(|c| c.to_acp_content()).collect())
                } else {
                    None
                },
                locations: if prev.locations_len != self.locations.len()
                    || (include_context_fields && !self.locations.is_empty())
                {
                    Some(self.locations.iter().map(|l| l.to_acp_location()).collect())
                } else {
                    None
                },
                raw_input: if prev.raw_input_present != self.raw_input.is_some() {
                    self.raw_input.clone()
                } else {
                    None
                },
                raw_output: if prev.raw_output_present != self.raw_output.is_some() {
                    self.raw_output.clone()
                } else {
                    None
                },
            }
        } else {
            // First update - send all fields to establish initial state
            agent_client_protocol::ToolCallUpdateFields {
                kind: Some(self.kind.to_acp_kind()),
                status: Some(self.status.to_acp_status()),
                title: Some(self.title.clone()),
                content: Some(self.content.iter().map(|c| c.to_acp_content()).collect()),
                locations: Some(self.locations.iter().map(|l| l.to_acp_location()).collect()),
                raw_input: self.raw_input.clone(),
                raw_output: self.raw_output.clone(),
            }
        };

        agent_client_protocol::ToolCallUpdate {
            id: agent_client_protocol::ToolCallId(self.tool_call_id.clone().into()),
            fields,
            meta: None,
        }
    }

    /// Convert to agent_client_protocol::ToolCallUpdate with default behavior
    ///
    /// This is a convenience method that calls `to_acp_tool_call_update_with_context(false)`
    /// for standard partial updates.
    pub fn to_acp_tool_call_update(&self) -> agent_client_protocol::ToolCallUpdate {
        self.to_acp_tool_call_update_with_context(false)
    }
}

impl ToolKind {
    /// Convert to agent_client_protocol::ToolKind
    pub fn to_acp_kind(&self) -> agent_client_protocol::ToolKind {
        match self {
            ToolKind::Read => agent_client_protocol::ToolKind::Read,
            ToolKind::Edit => agent_client_protocol::ToolKind::Edit,
            ToolKind::Delete => agent_client_protocol::ToolKind::Delete,
            ToolKind::Move => agent_client_protocol::ToolKind::Move,
            ToolKind::Search => agent_client_protocol::ToolKind::Search,
            ToolKind::Execute => agent_client_protocol::ToolKind::Execute,
            ToolKind::Think => agent_client_protocol::ToolKind::Think,
            ToolKind::Fetch => agent_client_protocol::ToolKind::Fetch,
            ToolKind::Other => agent_client_protocol::ToolKind::Other,
        }
    }
}

impl ToolCallStatus {
    /// Convert to agent_client_protocol::ToolCallStatus
    pub fn to_acp_status(&self) -> agent_client_protocol::ToolCallStatus {
        match self {
            ToolCallStatus::Pending => agent_client_protocol::ToolCallStatus::Pending,
            ToolCallStatus::InProgress => agent_client_protocol::ToolCallStatus::InProgress,
            ToolCallStatus::Completed => agent_client_protocol::ToolCallStatus::Completed,
            ToolCallStatus::Failed => agent_client_protocol::ToolCallStatus::Failed,
            // ACP doesn't have Cancelled status, map to Failed
            ToolCallStatus::Cancelled => agent_client_protocol::ToolCallStatus::Failed,
        }
    }
}

impl ToolCallContent {
    /// Convert to agent_client_protocol::ToolCallContent
    pub fn to_acp_content(&self) -> agent_client_protocol::ToolCallContent {
        match self {
            ToolCallContent::Content { content } => {
                agent_client_protocol::ToolCallContent::Content {
                    content: content.clone(),
                }
            }
            ToolCallContent::Diff {
                path,
                old_text,
                new_text,
            } => {
                // ACP expects a diff field with a Diff struct
                agent_client_protocol::ToolCallContent::Diff {
                    diff: agent_client_protocol::Diff {
                        path: path.clone().into(),
                        old_text: old_text.clone(),
                        new_text: new_text.clone(),
                        meta: None,
                    },
                }
            }
            ToolCallContent::Terminal { terminal_id } => {
                agent_client_protocol::ToolCallContent::Terminal {
                    terminal_id: agent_client_protocol::TerminalId(terminal_id.clone().into()),
                }
            }
        }
    }
}

impl ToolCallLocation {
    /// Convert to agent_client_protocol::ToolCallLocation
    pub fn to_acp_location(&self) -> agent_client_protocol::ToolCallLocation {
        agent_client_protocol::ToolCallLocation {
            path: self.path.clone().into(),
            line: self.line.map(|l| l as u32),
            meta: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_file_location_creation() {
        let location = ToolCallLocation {
            path: "/home/user/test.txt".to_string(),
            line: Some(42),
        };

        assert_eq!(location.path, "/home/user/test.txt");
        assert_eq!(location.line, Some(42));
    }

    #[test]
    fn test_file_location_to_acp_conversion() {
        let location = ToolCallLocation {
            path: "/home/user/test.txt".to_string(),
            line: Some(42),
        };

        let acp_location = location.to_acp_location();
        assert_eq!(acp_location.path.to_string_lossy(), "/home/user/test.txt");
        assert_eq!(acp_location.line, Some(42));
        assert!(acp_location.meta.is_none());
    }

    #[test]
    fn test_extract_file_locations_basic_path() {
        let args = json!({
            "path": "/home/user/document.txt"
        });

        let locations = ToolCallReport::extract_file_locations("fs_read", &args);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].path.ends_with("document.txt"));
        assert_eq!(locations[0].line, None);
    }

    #[test]
    fn test_extract_file_locations_with_line_number() {
        let args = json!({
            "file_path": "/home/user/code.rs",
            "line": 25
        });

        let locations = ToolCallReport::extract_file_locations("edit_file", &args);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].path.ends_with("code.rs"));
        assert_eq!(locations[0].line, Some(25));
    }

    #[test]
    fn test_extract_file_locations_multiple_paths() {
        let args = json!({
            "source": "/home/user/source.txt",
            "destination": "/home/user/dest.txt"
        });

        let locations = ToolCallReport::extract_file_locations("fs_move", &args);
        assert_eq!(locations.len(), 2);
        assert!(locations.iter().any(|l| l.path.ends_with("source.txt")));
        assert!(locations.iter().any(|l| l.path.ends_with("dest.txt")));
    }

    #[test]
    fn test_extract_file_locations_ignores_urls() {
        let args = json!({
            "path": "https://example.com/file.txt",
            "url": "http://test.com",
            "file": "/local/file.txt"
        });

        let locations = ToolCallReport::extract_file_locations("fetch", &args);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].path.ends_with("file.txt"));
        assert!(!locations[0].path.contains("http"));
    }

    #[test]
    fn test_extract_file_locations_ignores_commands() {
        let args = json!({
            "command": "ls -la /tmp",
            "path": "/actual/file.txt"
        });

        let locations = ToolCallReport::extract_file_locations("execute", &args);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].path.ends_with("file.txt"));
    }

    #[test]
    fn test_is_file_path_detection() {
        // Valid file paths
        assert!(ToolCallReport::is_file_path("/absolute/path.txt"));
        assert!(ToolCallReport::is_file_path("./relative/path.txt"));
        assert!(ToolCallReport::is_file_path("../parent/file.txt"));
        assert!(ToolCallReport::is_file_path("subdir/file.txt"));

        // Invalid file paths
        assert!(!ToolCallReport::is_file_path("https://example.com"));
        assert!(!ToolCallReport::is_file_path("http://test.com"));
        assert!(!ToolCallReport::is_file_path("ls -la directory"));
        assert!(!ToolCallReport::is_file_path("simple command"));
    }

    #[test]
    fn test_normalize_path_absolute() {
        let path = "/home/user/test.txt";
        let normalized = ToolCallReport::normalize_path(path);
        assert_eq!(normalized, path);
    }

    #[test]
    fn test_tool_call_report_add_location() {
        let mut report = ToolCallReport::new(
            "test_id".to_string(),
            "Test Tool".to_string(),
            ToolKind::Read,
            "Read".to_string(),
        );

        let location = ToolCallLocation {
            path: "/test/path.txt".to_string(),
            line: Some(10),
        };

        report.add_location(location);
        assert_eq!(report.locations.len(), 1);
        assert_eq!(report.locations[0].path, "/test/path.txt");
        assert_eq!(report.locations[0].line, Some(10));
    }

    #[test]
    fn test_tool_call_to_acp_includes_locations() {
        let mut report = ToolCallReport::new(
            "test_id".to_string(),
            "Reading file".to_string(),
            ToolKind::Read,
            "Read".to_string(),
        );

        report.add_location(ToolCallLocation {
            path: "/test/file.txt".to_string(),
            line: Some(5),
        });

        let acp_tool_call = report.to_acp_tool_call();
        assert_eq!(acp_tool_call.locations.len(), 1);
        assert_eq!(
            acp_tool_call.locations[0].path.to_string_lossy(),
            "/test/file.txt"
        );
        assert_eq!(acp_tool_call.locations[0].line, Some(5));
    }

    #[test]
    fn test_tool_call_update_includes_locations() {
        let mut report = ToolCallReport::new(
            "test_id".to_string(),
            "Writing file".to_string(),
            ToolKind::Edit,
            "test_tool".to_string(),
        );

        report.add_location(ToolCallLocation {
            path: "/test/output.txt".to_string(),
            line: None,
        });

        let acp_update = report.to_acp_tool_call_update();
        assert!(acp_update.fields.locations.is_some());
        let locations = acp_update.fields.locations.unwrap();
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].path.to_string_lossy(), "/test/output.txt");
        assert_eq!(locations[0].line, None);
    }

    #[test]
    fn test_extract_locations_mcp_tools() {
        let args = json!({
            "file_path": "/workspace/src/main.rs"
        });

        let locations = ToolCallReport::extract_file_locations("mcp__files_read", &args);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].path.ends_with("main.rs"));
    }

    #[test]
    fn test_extract_locations_empty_args() {
        let args = json!({});

        let locations = ToolCallReport::extract_file_locations("unknown_tool", &args);
        assert_eq!(locations.len(), 0);
    }

    #[test]
    fn test_extract_locations_string_arg() {
        let args = json!("/single/file/path.txt");

        let locations = ToolCallReport::extract_file_locations("tool", &args);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].path.ends_with("path.txt"));
    }

    #[test]
    fn test_tool_call_content_serialization_content_variant() {
        let content = ToolCallContent::Content {
            content: agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Test content".to_string(),
                    annotations: None,
                    meta: None,
                },
            ),
        };

        let json = serde_json::to_value(&content).expect("Should serialize");
        assert_eq!(json["type"], "content");
        assert!(json.get("content").is_some());
    }

    #[test]
    fn test_tool_call_content_serialization_diff_variant() {
        let content = ToolCallContent::Diff {
            path: "/test/file.rs".to_string(),
            old_text: Some("old content".to_string()),
            new_text: "new content".to_string(),
        };

        let json = serde_json::to_value(&content).expect("Should serialize");
        assert_eq!(json["type"], "diff");
        assert_eq!(json["path"], "/test/file.rs");
        assert_eq!(json["oldText"], "old content");
        assert_eq!(json["newText"], "new content");
    }

    #[test]
    fn test_tool_call_content_serialization_diff_variant_new_file() {
        let content = ToolCallContent::Diff {
            path: "/test/new_file.rs".to_string(),
            old_text: None,
            new_text: "new file content".to_string(),
        };

        let json = serde_json::to_value(&content).expect("Should serialize");
        assert_eq!(json["type"], "diff");
        assert_eq!(json["path"], "/test/new_file.rs");
        assert!(json["oldText"].is_null());
        assert_eq!(json["newText"], "new file content");
    }

    #[test]
    fn test_tool_call_content_serialization_terminal_variant() {
        let content = ToolCallContent::Terminal {
            terminal_id: "term_abc123xyz".to_string(),
        };

        let json = serde_json::to_value(&content).expect("Should serialize");
        assert_eq!(json["type"], "terminal");
        assert_eq!(json["terminalId"], "term_abc123xyz");
    }

    #[test]
    fn test_tool_call_content_deserialization_content_variant() {
        let json = json!({
            "type": "content",
            "content": {
                "type": "text",
                "text": "Deserialized content"
            }
        });

        let content: ToolCallContent = serde_json::from_value(json).expect("Should deserialize");
        match content {
            ToolCallContent::Content { content } => {
                if let agent_client_protocol::ContentBlock::Text(text) = content {
                    assert_eq!(text.text, "Deserialized content");
                } else {
                    panic!("Expected text content");
                }
            }
            _ => panic!("Expected Content variant"),
        }
    }

    #[test]
    fn test_tool_call_content_deserialization_diff_variant() {
        let json = json!({
            "type": "diff",
            "path": "/src/main.rs",
            "oldText": "fn main() { }",
            "newText": "fn main() {\n    println!(\"Hello\");\n}"
        });

        let content: ToolCallContent = serde_json::from_value(json).expect("Should deserialize");
        match content {
            ToolCallContent::Diff {
                path,
                old_text,
                new_text,
            } => {
                assert_eq!(path, "/src/main.rs");
                assert_eq!(old_text.unwrap(), "fn main() { }");
                assert!(new_text.contains("println"));
            }
            _ => panic!("Expected Diff variant"),
        }
    }

    #[test]
    fn test_tool_call_content_deserialization_terminal_variant() {
        let json = json!({
            "type": "terminal",
            "terminalId": "term_987654321"
        });

        let content: ToolCallContent = serde_json::from_value(json).expect("Should deserialize");
        match content {
            ToolCallContent::Terminal { terminal_id } => {
                assert_eq!(terminal_id, "term_987654321");
            }
            _ => panic!("Expected Terminal variant"),
        }
    }

    #[test]
    fn test_tool_call_content_to_acp_content_variant() {
        let content = ToolCallContent::Content {
            content: agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "ACP test".to_string(),
                    annotations: None,
                    meta: None,
                },
            ),
        };

        let acp_content = content.to_acp_content();
        match acp_content {
            agent_client_protocol::ToolCallContent::Content { content } => {
                if let agent_client_protocol::ContentBlock::Text(text) = content {
                    assert_eq!(text.text, "ACP test");
                } else {
                    panic!("Expected text content");
                }
            }
            _ => panic!("Expected Content variant"),
        }
    }

    #[test]
    fn test_tool_call_content_to_acp_diff_variant() {
        let content = ToolCallContent::Diff {
            path: "/workspace/config.json".to_string(),
            old_text: Some(r#"{"debug": false}"#.to_string()),
            new_text: r#"{"debug": true}"#.to_string(),
        };

        let acp_content = content.to_acp_content();
        match acp_content {
            agent_client_protocol::ToolCallContent::Diff { diff } => {
                assert_eq!(diff.path.to_string_lossy(), "/workspace/config.json");
                assert_eq!(diff.old_text.unwrap(), r#"{"debug": false}"#);
                assert_eq!(diff.new_text, r#"{"debug": true}"#);
                assert!(diff.meta.is_none());
            }
            _ => panic!("Expected Diff variant"),
        }
    }

    #[test]
    fn test_tool_call_content_to_acp_terminal_variant() {
        let content = ToolCallContent::Terminal {
            terminal_id: "term_unique_id_123".to_string(),
        };

        let acp_content = content.to_acp_content();
        match acp_content {
            agent_client_protocol::ToolCallContent::Terminal { terminal_id } => {
                assert_eq!(terminal_id.0.as_ref(), "term_unique_id_123");
            }
            _ => panic!("Expected Terminal variant"),
        }
    }

    #[test]
    fn test_tool_call_report_with_multiple_content_types() {
        let mut report = ToolCallReport::new(
            "test_multi_content".to_string(),
            "Multi-content test".to_string(),
            ToolKind::Edit,
            "test_tool".to_string(),
        );

        report.add_content(ToolCallContent::Content {
            content: agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Starting operation".to_string(),
                    annotations: None,
                    meta: None,
                },
            ),
        });

        report.add_content(ToolCallContent::Diff {
            path: "/test/file.txt".to_string(),
            old_text: Some("before".to_string()),
            new_text: "after".to_string(),
        });

        report.add_content(ToolCallContent::Terminal {
            terminal_id: "term_operation_123".to_string(),
        });

        assert_eq!(report.content.len(), 3);

        let acp_call = report.to_acp_tool_call();
        assert_eq!(acp_call.content.len(), 3);

        match &acp_call.content[0] {
            agent_client_protocol::ToolCallContent::Content { .. } => {}
            _ => panic!("First content should be Content variant"),
        }

        match &acp_call.content[1] {
            agent_client_protocol::ToolCallContent::Diff { .. } => {}
            _ => panic!("Second content should be Diff variant"),
        }

        match &acp_call.content[2] {
            agent_client_protocol::ToolCallContent::Terminal { .. } => {}
            _ => panic!("Third content should be Terminal variant"),
        }
    }

    #[test]
    fn test_diff_content_with_multiline_text() {
        let old_content = "line 1\nline 2\nline 3";
        let new_content = "line 1\nmodified line 2\nline 3\nline 4";

        let content = ToolCallContent::Diff {
            path: "/src/multi.txt".to_string(),
            old_text: Some(old_content.to_string()),
            new_text: new_content.to_string(),
        };

        let json = serde_json::to_value(&content).expect("Should serialize multiline");
        assert!(json["oldText"].as_str().unwrap().contains('\n'));
        assert!(json["newText"].as_str().unwrap().contains('\n'));

        let acp_content = content.to_acp_content();
        match acp_content {
            agent_client_protocol::ToolCallContent::Diff { diff } => {
                assert!(diff.old_text.unwrap().contains('\n'));
                assert!(diff.new_text.contains('\n'));
            }
            _ => panic!("Expected Diff variant"),
        }
    }

    #[test]
    fn test_diff_content_with_unicode() {
        let content = ToolCallContent::Diff {
            path: "/docs/unicode.txt".to_string(),
            old_text: Some("Hello 世界".to_string()),
            new_text: "Hello 世界! 🌍".to_string(),
        };

        let json = serde_json::to_value(&content).expect("Should serialize unicode");
        assert!(json["newText"].as_str().unwrap().contains('🌍'));

        let deserialized: ToolCallContent =
            serde_json::from_value(json).expect("Should deserialize unicode");
        match deserialized {
            ToolCallContent::Diff { new_text, .. } => {
                assert!(new_text.contains('🌍'));
                assert!(new_text.contains('世'));
            }
            _ => panic!("Expected Diff variant"),
        }
    }

    #[test]
    fn test_empty_content_list_serialization() {
        let report = ToolCallReport::new(
            "test_empty_content".to_string(),
            "No content test".to_string(),
            ToolKind::Think,
            "test_tool".to_string(),
        );

        let acp_call = report.to_acp_tool_call();
        assert_eq!(acp_call.content.len(), 0);

        let json = serde_json::to_value(&report).expect("Should serialize empty content");
        assert!(json["content"].is_array());
        assert_eq!(json["content"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_partial_update_first_update_includes_all_fields() {
        // First update should include all fields since there's no previous state
        let report = ToolCallReport::new(
            "test_001".to_string(),
            "Test Tool".to_string(),
            ToolKind::Read,
            "test_tool".to_string(),
        );

        let update = report.to_acp_tool_call_update();

        // All fields should be present in first update
        assert!(update.fields.status.is_some());
        assert!(update.fields.title.is_some());
        assert!(update.fields.kind.is_some());
        assert!(update.fields.content.is_some());
        assert!(update.fields.locations.is_some());
        assert_eq!(update.fields.title.unwrap(), "Test Tool");
    }

    #[test]
    fn test_partial_update_status_only() {
        // Test that only status field is included when only status changes
        let mut report = ToolCallReport::new(
            "test_002".to_string(),
            "Test Tool".to_string(),
            ToolKind::Read,
            "test_tool".to_string(),
        );

        // Mark initial state as sent
        report.mark_state_sent();

        // Now change only the status
        report.update_status(ToolCallStatus::InProgress);

        let update = report.to_acp_tool_call_update();

        // Only status should be present
        assert!(update.fields.status.is_some());
        assert_eq!(
            update.fields.status.unwrap(),
            agent_client_protocol::ToolCallStatus::InProgress
        );

        // All other fields should be None (omitted in serialization)
        assert!(update.fields.title.is_none());
        assert!(update.fields.kind.is_none());
        assert!(update.fields.content.is_none());
        assert!(update.fields.locations.is_none());
        assert!(update.fields.raw_input.is_none());
        assert!(update.fields.raw_output.is_none());
    }

    #[test]
    fn test_partial_update_content_only() {
        // Test that only content field is included when only content changes
        let mut report = ToolCallReport::new(
            "test_003".to_string(),
            "Test Tool".to_string(),
            ToolKind::Execute,
            "test_tool".to_string(),
        );

        // Mark initial state as sent
        report.mark_state_sent();

        // Now add content
        report.add_content(ToolCallContent::Content {
            content: agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Progress update".to_string(),
                    annotations: None,
                    meta: None,
                },
            ),
        });

        let update = report.to_acp_tool_call_update();

        // Only content should be present
        assert!(update.fields.content.is_some());
        assert_eq!(update.fields.content.as_ref().unwrap().len(), 1);

        // All other fields should be None
        assert!(update.fields.status.is_none());
        assert!(update.fields.title.is_none());
        assert!(update.fields.kind.is_none());
        assert!(update.fields.locations.is_none());
        assert!(update.fields.raw_input.is_none());
        assert!(update.fields.raw_output.is_none());
    }

    #[test]
    fn test_partial_update_multiple_fields() {
        // Test that multiple changed fields are included
        let mut report = ToolCallReport::new(
            "test_004".to_string(),
            "Test Tool".to_string(),
            ToolKind::Edit,
            "test_tool".to_string(),
        );

        // Mark initial state as sent
        report.mark_state_sent();

        // Change multiple fields
        report.update_status(ToolCallStatus::Completed);
        report.add_content(ToolCallContent::Content {
            content: agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Done".to_string(),
                    annotations: None,
                    meta: None,
                },
            ),
        });
        report.set_raw_output(serde_json::json!({"result": "success"}));

        let update = report.to_acp_tool_call_update();

        // Changed fields should be present
        assert!(update.fields.status.is_some());
        assert!(update.fields.content.is_some());
        assert!(update.fields.raw_output.is_some());

        // Unchanged fields should be None
        assert!(update.fields.title.is_none());
        assert!(update.fields.kind.is_none());
        assert!(update.fields.locations.is_none());
        assert!(update.fields.raw_input.is_none());
    }

    #[test]
    fn test_partial_update_no_changes() {
        // Test that no fields are included when nothing changes
        let mut report = ToolCallReport::new(
            "test_005".to_string(),
            "Test Tool".to_string(),
            ToolKind::Search,
            "test_tool".to_string(),
        );

        // Mark initial state as sent
        report.mark_state_sent();

        // Don't change anything
        let update = report.to_acp_tool_call_update();

        // All fields should be None since nothing changed
        assert!(update.fields.status.is_none());
        assert!(update.fields.title.is_none());
        assert!(update.fields.kind.is_none());
        assert!(update.fields.content.is_none());
        assert!(update.fields.locations.is_none());
        assert!(update.fields.raw_input.is_none());
        assert!(update.fields.raw_output.is_none());
    }

    #[test]
    fn test_partial_update_location_changes() {
        // Test that locations field is included when locations change
        let mut report = ToolCallReport::new(
            "test_006".to_string(),
            "Test Tool".to_string(),
            ToolKind::Read,
            "test_tool".to_string(),
        );

        // Mark initial state as sent
        report.mark_state_sent();

        // Add a location
        report.add_location(ToolCallLocation {
            path: "/test/file.txt".to_string(),
            line: Some(42),
        });

        let update = report.to_acp_tool_call_update();

        // Only locations should be present
        assert!(update.fields.locations.is_some());
        assert_eq!(update.fields.locations.as_ref().unwrap().len(), 1);

        // All other fields should be None
        assert!(update.fields.status.is_none());
        assert!(update.fields.title.is_none());
        assert!(update.fields.kind.is_none());
        assert!(update.fields.content.is_none());
        assert!(update.fields.raw_input.is_none());
        assert!(update.fields.raw_output.is_none());
    }

    #[test]
    fn test_partial_update_title_change() {
        // Test that title field is included when title changes
        let mut report = ToolCallReport::new(
            "test_007".to_string(),
            "Original Title".to_string(),
            ToolKind::Edit,
            "test_tool".to_string(),
        );

        // Mark initial state as sent
        report.mark_state_sent();

        // Change the title
        report.title = "Updated Title".to_string();

        let update = report.to_acp_tool_call_update();

        // Only title should be present
        assert!(update.fields.title.is_some());
        assert_eq!(update.fields.title.as_ref().unwrap(), "Updated Title");

        // All other fields should be None
        assert!(update.fields.status.is_none());
        assert!(update.fields.kind.is_none());
        assert!(update.fields.content.is_none());
        assert!(update.fields.locations.is_none());
        assert!(update.fields.raw_input.is_none());
        assert!(update.fields.raw_output.is_none());
    }

    #[test]
    fn test_partial_update_serialization_omits_none_fields() {
        // Test that None fields are actually omitted in JSON serialization
        let mut report = ToolCallReport::new(
            "test_008".to_string(),
            "Test Tool".to_string(),
            ToolKind::Execute,
            "test_tool".to_string(),
        );

        // Mark initial state as sent
        report.mark_state_sent();

        // Change only status
        report.update_status(ToolCallStatus::InProgress);

        let update = report.to_acp_tool_call_update();
        let json = serde_json::to_value(&update.fields).expect("Should serialize");

        // Only status should be in the JSON
        assert!(json.get("status").is_some());

        // These fields should not exist in the JSON at all
        assert!(json.get("title").is_none());
        assert!(json.get("kind").is_none());
        assert!(json.get("content").is_none());
        assert!(json.get("locations").is_none());
        assert!(json.get("rawInput").is_none());
        assert!(json.get("rawOutput").is_none());
    }

    #[test]
    fn test_partial_update_with_context_flag() {
        // Test that context flag includes content/locations even when unchanged
        let mut report = ToolCallReport::new(
            "test_009".to_string(),
            "Test Tool".to_string(),
            ToolKind::Execute,
            "test_tool".to_string(),
        );

        // Add content
        report.add_content(ToolCallContent::Terminal {
            terminal_id: "term_123".to_string(),
        });

        // Mark state as sent (simulating terminal embedding update)
        report.mark_state_sent();

        // Now change status (simulating completion)
        report.update_status(ToolCallStatus::Completed);

        // Generate update with context flag true
        let update = report.to_acp_tool_call_update_with_context(true);

        // Content should be included even though length didn't change
        assert!(
            update.fields.content.is_some(),
            "Content should be included with context flag"
        );
        assert_eq!(update.fields.content.as_ref().unwrap().len(), 1);

        // Status should also be included since it changed
        assert!(update.fields.status.is_some());
        assert_eq!(
            update.fields.status.unwrap(),
            agent_client_protocol::ToolCallStatus::Completed
        );
    }
}
