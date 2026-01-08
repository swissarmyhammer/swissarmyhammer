//! File operation parameter types for Agent Client Protocol

/// Parameters for the ACP fs/read_text_file method
///
/// ACP fs/read_text_file method implementation:
/// 1. sessionId: Required - validate against active sessions
/// 2. path: Required - must be absolute path
/// 3. line: Optional - 1-based line number to start reading from
/// 4. limit: Optional - maximum number of lines to read
/// 5. Response: content field with requested file content
///
/// Supports partial file reading for performance optimization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReadTextFileParams {
    /// Session ID for validation
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Absolute path to the file to read
    pub path: String,
    /// Optional 1-based line number to start reading from
    pub line: Option<u32>,
    /// Optional maximum number of lines to read
    pub limit: Option<u32>,
}

/// Response for the ACP fs/read_text_file method
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReadTextFileResponse {
    /// File content as requested (full file or partial based on line/limit)
    pub content: String,
}

/// Parameters for the ACP fs/write_text_file method
///
/// ACP fs/write_text_file method implementation:
/// 1. sessionId: Required - validate against active sessions
/// 2. path: Required - must be absolute path
/// 3. content: Required - text content to write
/// 4. MUST create file if it doesn't exist per ACP specification
/// 5. MUST create parent directories if needed
/// 6. Response: null result on success
///
/// Uses atomic write operations to ensure file integrity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WriteTextFileParams {
    /// Session ID for validation
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Absolute path to the file to write
    pub path: String,
    /// Text content to write to the file
    pub content: String,
}
