//! Infrastructure types and utilities for shell command execution
//!
//! This module contains the core data types, output buffering, error types,
//! and configuration used by the shell tool.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use swissarmyhammer_common::{ErrorSeverity, Severity};

/// Default shell configuration providing hardcoded sensible defaults
///
/// This struct provides default configuration values for shell command execution,
/// replacing the previous configurable `ShellToolConfig` system with hardcoded
/// constants. The chosen defaults balance security, performance, and usability
/// for typical shell operations.
///
/// # Design Rationale
///
/// Shell configuration uses hardcoded defaults. The `sah_config` module is not used.
/// This design simplifies the system while maintaining essential safety limits:
///
/// - **Output Size Limit**: 10MB prevents memory exhaustion from commands that
///   produce massive output (e.g., `cat large_file.log`, `find /`)
/// - **Line Length Limit**: 2000 characters handles most real-world command output
///   while preventing single-line memory issues
///
/// # Default Values
///
/// | Setting | Value | Reason |
/// |---------|-------|--------|
/// | Max Output | 10MB | Generous limit for build logs, test output |
/// | Max Line Length | 2000 chars | Handles verbose tool output |
///
/// # Examples
///
/// Default configuration values (struct is private, examples cannot be tested):
/// - `max_output_size()`: 10,485,760 bytes (10MB)
/// - `max_line_length()`: 2000 characters
///
/// # Migration from sah_config
///
/// Previously, these values were configurable through the `sah_config` system:
/// ```toml
/// # Old sah.toml configuration (no longer supported)
/// [shell]
/// max_output_size = "50MB"
/// ```
///
/// The new approach uses compile-time constants, trading configurability for
/// simplicity and reliability. If different limits are needed, they should be
/// implemented as environment variables or command-line arguments.
pub(crate) struct DefaultShellConfig;

impl DefaultShellConfig {
    /// Maximum output size in bytes (10MB = 10,485,760 bytes)
    ///
    /// This limit prevents memory exhaustion from commands that produce
    /// massive output. When exceeded, output is truncated with a clear
    /// indication to the user.
    const MAX_OUTPUT_SIZE: usize = 10 * 1024 * 1024;

    /// Maximum line length in characters (2000)
    ///
    /// Individual lines longer than this limit are truncated. This prevents
    /// single lines from consuming excessive memory while allowing most
    /// real-world command output to pass through unchanged.
    const MAX_LINE_LENGTH: usize = 2000;

    /// Maximum output size in bytes (10MB)
    ///
    /// # Examples
    /// Returns 10,485,760 bytes (10MB limit)
    pub(crate) fn max_output_size() -> usize {
        Self::MAX_OUTPUT_SIZE
    }

    /// Maximum line length in characters (2000)
    ///
    /// # Examples
    /// Returns 2000 characters (2KB line limit)
    pub(crate) fn max_line_length() -> usize {
        Self::MAX_LINE_LENGTH
    }
}

/// Request structure for shell command execution
#[derive(Debug, Deserialize)]
pub(crate) struct ShellExecuteRequest {
    /// The shell command to execute
    pub(crate) command: String,

    /// Timeout in seconds before killing the command
    pub(crate) timeout: Option<u64>,

    /// Max output lines returned to agent (default: 200, -1 for all, 0 for status-only)
    pub(crate) max_lines: Option<i64>,

    /// Optional working directory for command execution
    pub(crate) working_directory: Option<String>,

    /// Optional environment variables as JSON string
    pub(crate) environment: Option<String>,
}

/// Result structure for shell command execution
#[derive(Debug, Serialize)]
pub struct ShellExecutionResult {
    /// Command ID for referencing in get_lines, search, grep operations
    pub command_id: usize,
    /// The command that was executed
    pub command: String,
    /// Exit code returned by the command
    pub exit_code: i32,
    /// Standard output captured from the command
    pub stdout: String,
    /// Standard error output captured from the command
    pub stderr: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Working directory where the command was executed
    pub working_directory: PathBuf,
    /// Whether output was truncated due to size limits
    pub output_truncated: bool,
    /// Total size of output before any truncation
    pub total_output_size: usize,
    /// Whether binary content was detected in the output
    pub binary_output_detected: bool,
}

/// Configuration for output limits and handling
///
/// Controls how command output is captured, processed, and limited to prevent
/// memory exhaustion and handle large outputs gracefully.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer_tools::mcp::tools::shell::OutputLimits;
///
/// // Use default limits (10MB max output, 2000 char lines)
/// let limits = OutputLimits::default();
///
/// // Custom limits for memory-constrained environments
/// let custom_limits = OutputLimits {
///     max_output_size: 1024 * 1024, // 1MB limit
///     max_line_length: 1000,         // Shorter lines
///     enable_streaming: false,       // Future streaming support
/// };
///
/// // High-throughput configuration for CI/build systems
/// let build_limits = OutputLimits {
///     max_output_size: 50 * 1024 * 1024, // 50MB for build outputs
///     max_line_length: 4000,              // Longer lines for verbose tools
///     enable_streaming: false,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct OutputLimits {
    /// Maximum total output size in bytes (default: 10MB)
    pub max_output_size: usize,
    /// Maximum length of individual lines (default: 2000 chars)
    pub max_line_length: usize,
    /// Enable streaming output for future use (default: false)
    pub enable_streaming: bool,
}

impl Default for OutputLimits {
    fn default() -> Self {
        Self {
            max_output_size: 10 * 1024 * 1024, // 10MB
            max_line_length: 2000,
            enable_streaming: false,
        }
    }
}

impl OutputLimits {
    /// Create OutputLimits with default configuration
    pub fn with_defaults() -> Result<Self, String> {
        Ok(Self {
            max_output_size: DefaultShellConfig::max_output_size(),
            max_line_length: DefaultShellConfig::max_line_length(),
            enable_streaming: false, // Reserved for future use
        })
    }
}

/// Buffer for managing output capture with size limits and binary detection
///
/// Provides intelligent output buffering that prevents memory exhaustion while
/// preserving output structure and detecting binary content. Handles concurrent
/// stdout/stderr streams with configurable size limits and graceful truncation.
///
/// # Key Features
///
/// - **Size Limiting**: Enforces maximum output size to prevent memory issues
/// - **Binary Detection**: Automatically detects and safely formats binary content
/// - **Structure Preservation**: Truncates at line boundaries when possible
/// - **Concurrent Streams**: Handles stdout and stderr independently
/// - **Metadata Tracking**: Records truncation status and total bytes processed
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer_tools::mcp::tools::shell::OutputBuffer;
///
/// // Create buffer with 1MB limit
/// let mut buffer = OutputBuffer::new(1024 * 1024);
///
/// // Append output data
/// let bytes_written = buffer.append_stdout(b"Hello, world!\n");
/// assert_eq!(bytes_written, 14);
///
/// // Check buffer status
/// assert!(!buffer.is_truncated());
/// assert_eq!(buffer.current_size(), 14);
///
/// // Get formatted output
/// let stdout = buffer.get_stdout();
/// assert_eq!(stdout, "Hello, world!\n");
///
/// // Example with binary content detection
/// let mut bin_buffer = OutputBuffer::new(1000);
/// bin_buffer.append_stdout(&[0x00, 0x01, 0x02, 0xFF]); // Binary data
/// assert!(bin_buffer.has_binary_content());
///
/// let output = bin_buffer.get_stdout();
/// assert!(output.starts_with("[Binary content:"));
/// ```
///
/// # Memory Management
///
/// The buffer enforces strict size limits during capture, not after:
/// - Output is processed in streaming fashion
/// - Size limits are checked for each append operation
/// - Truncation occurs at line boundaries when possible
/// - Memory usage stays constant regardless of total output size
pub struct OutputBuffer {
    /// Maximum allowed total size
    max_size: usize,
    /// Buffer for stdout data
    stdout_buffer: Vec<u8>,
    /// Buffer for stderr data
    stderr_buffer: Vec<u8>,
    /// Whether output has been truncated
    truncated: bool,
    /// Whether binary content has been detected
    binary_detected: bool,
    /// Total bytes processed (including truncated)
    total_bytes_processed: usize,
}

/// Helper function to find a safe point to truncate data (preferably at line boundary)
fn find_safe_truncation_point(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }

    // Look for the last newline in the data to preserve line structure
    for i in (0..data.len()).rev() {
        if data[i] == b'\n' {
            return i + 1; // Include the newline
        }
    }

    // If no newline found, truncate at a reasonable boundary (not mid-UTF8 sequence)
    // Look backwards for a safe UTF-8 boundary
    for i in (0..data.len()).rev() {
        // Check if this byte could be a valid UTF-8 start
        if data[i] & 0x80 == 0 || data[i] & 0xC0 == 0xC0 {
            return i;
        }
    }

    // Fallback: return the full length (should not happen with reasonable data)
    data.len()
}

/// Shared implementation for appending data to a buffer with size limit enforcement
///
/// This helper function eliminates the code duplication between append_stdout and append_stderr
/// by extracting the common logic while working around Rust's borrow checker limitations.
fn append_to_buffer_impl(
    data: &[u8],
    buffer: &mut Vec<u8>,
    total_bytes_processed: &mut usize,
    binary_detected: &mut bool,
    truncated: &mut bool,
    max_size: usize,
    current_size: usize,
) -> usize {
    *total_bytes_processed += data.len();

    // Check for binary content in this chunk
    if !*binary_detected && is_binary_content(data) {
        *binary_detected = true;
    }

    // Calculate how much we can append without exceeding limit
    let available_space = max_size.saturating_sub(current_size);

    if available_space == 0 {
        *truncated = true;
        return 0;
    }

    let bytes_to_append = std::cmp::min(data.len(), available_space);

    if bytes_to_append < data.len() {
        *truncated = true;
    }

    // Try to truncate at line boundaries to preserve readability
    let actual_bytes = if bytes_to_append < data.len() {
        find_safe_truncation_point(&data[..bytes_to_append])
    } else {
        bytes_to_append
    };

    buffer.extend_from_slice(&data[..actual_bytes]);
    actual_bytes
}

impl OutputBuffer {
    /// Create a new output buffer with specified size limit
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            stdout_buffer: Vec::with_capacity(8192),
            stderr_buffer: Vec::with_capacity(8192),
            truncated: false,
            binary_detected: false,
            total_bytes_processed: 0,
        }
    }

    /// Get current total size of buffered data
    pub fn current_size(&self) -> usize {
        self.stdout_buffer.len() + self.stderr_buffer.len()
    }

    /// Check if buffer has reached size limit
    pub fn is_at_limit(&self) -> bool {
        self.current_size() >= self.max_size
    }

    /// Append data to stdout buffer with size limit enforcement
    ///
    /// This wrapper method provides explicit, type-safe access to the stdout buffer.
    /// It delegates to the shared `append_to_buffer_impl` function which handles the
    /// common logic for size limiting, binary detection, and truncation.
    ///
    /// # Design Rationale: Wrapper Methods vs Macros
    ///
    /// While a macro could generate both `append_stdout` and `append_stderr`, the explicit
    /// wrapper approach is preferred for several reasons:
    ///
    /// - **Clarity**: Each method is explicitly visible in the source code, making the API
    ///   surface clear and easy to understand without macro expansion
    /// - **Documentation**: Each method can have its own dedicated documentation that appears
    ///   in rustdoc and IDE tooltips
    /// - **Type Safety**: The compiler can provide better error messages without macro indirection
    /// - **IDE Support**: Better autocomplete, go-to-definition, and refactoring support
    /// - **Explicit Control**: The public interface is explicitly defined rather than generated,
    ///   making API changes more intentional and visible in code review
    /// - **Debugging**: Stack traces and error messages reference the actual method names rather
    ///   than macro expansion contexts
    ///
    /// The minimal code duplication (two short wrapper methods) is a worthwhile trade-off for
    /// these benefits.
    pub fn append_stdout(&mut self, data: &[u8]) -> usize {
        let current_size = self.current_size();
        append_to_buffer_impl(
            data,
            &mut self.stdout_buffer,
            &mut self.total_bytes_processed,
            &mut self.binary_detected,
            &mut self.truncated,
            self.max_size,
            current_size,
        )
    }

    /// Append data to stderr buffer with size limit enforcement
    ///
    /// This wrapper method provides explicit, type-safe access to the stderr buffer.
    /// It delegates to the shared `append_to_buffer_impl` function which handles the
    /// common logic for size limiting, binary detection, and truncation.
    ///
    /// See `append_stdout` documentation for the detailed rationale on why wrapper methods
    /// are preferred over a macro-based approach for generating these methods.
    pub fn append_stderr(&mut self, data: &[u8]) -> usize {
        let current_size = self.current_size();
        append_to_buffer_impl(
            data,
            &mut self.stderr_buffer,
            &mut self.total_bytes_processed,
            &mut self.binary_detected,
            &mut self.truncated,
            self.max_size,
            current_size,
        )
    }

    /// Get stdout as formatted string with binary content handling
    pub fn get_stdout(&self) -> String {
        format_output_content(&self.stdout_buffer, self.binary_detected)
    }

    /// Get stderr as formatted string with binary content handling
    pub fn get_stderr(&self) -> String {
        format_output_content(&self.stderr_buffer, self.binary_detected)
    }

    /// Check if output was truncated
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// Check if binary content was detected
    pub fn has_binary_content(&self) -> bool {
        self.binary_detected
    }

    /// Get total bytes processed (including truncated data)
    pub fn total_bytes_processed(&self) -> usize {
        self.total_bytes_processed
    }

    /// Truncate buffer to line boundary for cleaner output
    fn truncate_to_line_boundary(buffer: &mut Vec<u8>) {
        while !buffer.is_empty() && buffer[buffer.len() - 1] != b'\n' {
            buffer.pop();
        }
    }

    /// Make room in buffer by truncating to accommodate marker
    fn make_room_for_marker(&mut self, needed_space: usize) {
        if !self.stdout_buffer.is_empty() {
            let to_remove = std::cmp::min(needed_space, self.stdout_buffer.len());
            self.stdout_buffer
                .truncate(self.stdout_buffer.len() - to_remove);
            Self::truncate_to_line_boundary(&mut self.stdout_buffer);
        } else if !self.stderr_buffer.is_empty() {
            let to_remove = std::cmp::min(needed_space, self.stderr_buffer.len());
            self.stderr_buffer
                .truncate(self.stderr_buffer.len() - to_remove);
            Self::truncate_to_line_boundary(&mut self.stderr_buffer);
        }
    }

    /// Append marker to appropriate buffer
    fn append_marker(&mut self, marker: &[u8]) {
        if !self.stdout_buffer.is_empty() {
            self.stdout_buffer.extend_from_slice(marker);
        } else if !self.stderr_buffer.is_empty() {
            self.stderr_buffer.extend_from_slice(marker);
        } else {
            self.stdout_buffer.extend_from_slice(marker);
        }
    }

    /// Add truncation marker to indicate data was truncated
    pub fn add_truncation_marker(&mut self) {
        if !self.truncated {
            return;
        }

        let marker = b"\n[Output truncated - exceeded size limit]";
        let available = self.max_size.saturating_sub(self.current_size());

        if available < marker.len() {
            let needed_space = marker.len() - available;
            self.make_room_for_marker(needed_space);
        }

        let available = self.max_size.saturating_sub(self.current_size());
        if available >= marker.len() {
            self.append_marker(marker);
        }
    }
}

/// Detect if data contains binary content using heuristics
pub fn is_binary_content(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }

    // Check first 8KB for binary markers to avoid scanning huge text files
    let sample = &data[..std::cmp::min(data.len(), 8192)];

    for &byte in sample {
        // Early exit if we find definitive binary content
        if byte == 0 {
            return true; // Null bytes are definitive
        }
    }

    false
}

/// Format output content with binary detection and safe string conversion
pub fn format_output_content(data: &[u8], binary_detected: bool) -> String {
    if binary_detected || is_binary_content(data) {
        format!("[Binary content: {} bytes]", data.len())
    } else {
        String::from_utf8_lossy(data).to_string()
    }
}

/// Comprehensive error types for shell command execution
#[derive(Debug)]
pub enum ShellError {
    /// Failed to spawn the command process
    CommandSpawnError {
        /// The command that failed to spawn
        command: String,
        /// The underlying IO error
        source: std::io::Error,
    },

    /// Runtime execution failure
    ExecutionError {
        /// The command that failed to execute
        command: String,
        /// Error message describing the failure
        message: String,
    },

    /// Invalid command provided
    InvalidCommand {
        /// Error message describing why the command is invalid
        message: String,
    },

    /// System-level error
    SystemError {
        /// Error message describing the system error
        message: String,
    },

    /// Working directory error
    WorkingDirectoryError {
        /// Error message describing the working directory issue
        message: String,
    },
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::CommandSpawnError { command, source } => {
                write!(f, "Failed to spawn command '{command}': {source}")
            }
            ShellError::ExecutionError { command, message } => {
                write!(f, "Command '{command}' execution failed: {message}")
            }

            ShellError::InvalidCommand { message } => {
                write!(f, "Invalid command: {message}")
            }
            ShellError::SystemError { message } => {
                write!(f, "System error during command execution: {message}")
            }
            ShellError::WorkingDirectoryError { message } => {
                write!(f, "Working directory error: {message}")
            }
        }
    }
}

impl std::error::Error for ShellError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShellError::CommandSpawnError { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl Severity for ShellError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: System-level failures that prevent shell from functioning
            ShellError::CommandSpawnError { .. } => ErrorSeverity::Critical,
            ShellError::SystemError { .. } => ErrorSeverity::Critical,

            // Error: Command execution failures but system remains functional
            ShellError::ExecutionError { .. } => ErrorSeverity::Error,
            ShellError::InvalidCommand { .. } => ErrorSeverity::Error,
            ShellError::WorkingDirectoryError { .. } => ErrorSeverity::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to verify buffer state after append operation
    ///
    /// This reduces duplication in OutputBuffer test assertions.
    fn assert_buffer_state(
        buffer: &OutputBuffer,
        expected_written: usize,
        actual_written: usize,
        should_be_truncated: bool,
        max_size: usize,
    ) {
        assert_eq!(actual_written, expected_written, "Written bytes mismatch");
        assert_eq!(
            buffer.is_truncated(),
            should_be_truncated,
            "Truncation state mismatch"
        );
        assert!(
            buffer.current_size() <= max_size,
            "Buffer size {} exceeds max {}",
            buffer.current_size(),
            max_size
        );
    }

    #[test]
    fn test_output_buffer_size_limits() {
        let mut buffer = OutputBuffer::new(100); // 100 byte limit

        // Add data that doesn't exceed limit
        let small_data = b"hello world\n";
        let written = buffer.append_stdout(small_data);
        assert_buffer_state(&buffer, small_data.len(), written, false, 100);
        assert_eq!(buffer.current_size(), small_data.len());

        // Add data that would exceed limit
        let large_data = vec![b'x'; 200]; // 200 bytes
        let written = buffer.append_stdout(&large_data);
        assert!(written < large_data.len()); // Should be truncated
        assert_buffer_state(&buffer, written, written, true, 100);
    }

    #[test]
    fn test_output_buffer_comprehensive_size_limits() {
        let mut buffer = OutputBuffer::new(50); // Very small limit for testing

        // Test exact limit boundary
        let exact_data = vec![b'a'; 50];
        let written = buffer.append_stdout(&exact_data);
        assert_buffer_state(&buffer, 50, written, false, 50);
        assert_eq!(buffer.current_size(), 50);
        assert!(buffer.is_at_limit());

        // Try to add one more byte - should be rejected
        let one_byte = b"x";
        let written = buffer.append_stdout(one_byte);
        assert_buffer_state(&buffer, 0, written, true, 50);
        assert_eq!(buffer.current_size(), 50); // Size shouldn't change
    }

    #[test]
    fn test_output_buffer_mixed_stdout_stderr() {
        let mut buffer = OutputBuffer::new(100);

        // Add data to both streams
        let stdout_data = b"stdout content\n";
        let stderr_data = b"stderr content\n";

        let stdout_written = buffer.append_stdout(stdout_data);
        let stderr_written = buffer.append_stderr(stderr_data);

        assert_buffer_state(&buffer, stdout_data.len(), stdout_written, false, 100);
        assert_buffer_state(&buffer, stderr_data.len(), stderr_written, false, 100);
        assert_eq!(buffer.current_size(), stdout_data.len() + stderr_data.len());

        // Verify content is preserved correctly
        let stdout_result = buffer.get_stdout();
        let stderr_result = buffer.get_stderr();
        assert!(stdout_result.contains("stdout content"));
        assert!(stderr_result.contains("stderr content"));
    }

    #[test]
    fn test_output_buffer_mixed_stream_truncation() {
        let mut buffer = OutputBuffer::new(30); // Small limit

        // Fill most of buffer with stdout
        let stdout_data = b"stdout data here\n"; // 17 bytes
        buffer.append_stdout(stdout_data);
        assert_eq!(buffer.current_size(), 17);

        // Add stderr that would exceed limit
        let stderr_data = b"long stderr content that exceeds remaining space\n"; // 49 bytes
        let written = buffer.append_stderr(stderr_data);

        // Should only write what fits (30 - 17 = 13 bytes)
        assert!(written <= 13);
        assert_buffer_state(&buffer, written, written, true, 30);

        // Verify both streams have content
        assert!(!buffer.get_stdout().is_empty());
        assert!(!buffer.get_stderr().is_empty());
    }

    #[test]
    fn test_output_buffer_zero_size_limit() {
        let mut buffer = OutputBuffer::new(0);

        // Any data should be rejected immediately
        let data = b"hello";
        let written = buffer.append_stdout(data);
        assert_buffer_state(&buffer, 0, written, true, 0);
        assert_eq!(buffer.current_size(), 0);
        assert!(buffer.is_at_limit());
    }

    #[test]
    fn test_output_buffer_incremental_writes() {
        let mut buffer = OutputBuffer::new(50);

        // Add data incrementally
        for i in 0..10 {
            let data = format!("{i}\n");
            let data_bytes = data.as_bytes();
            let written = buffer.append_stdout(data_bytes);

            if buffer.current_size() + data_bytes.len() <= 50 {
                assert_buffer_state(&buffer, data_bytes.len(), written, false, 50);
            } else {
                // Should truncate or reject when limit is reached
                assert!(written <= data_bytes.len());
                assert!(buffer.is_truncated() || buffer.is_at_limit());
                break;
            }
        }

        assert!(buffer.current_size() <= 50);
    }

    #[test]
    fn test_output_buffer_utf8_boundary_handling() {
        let mut buffer = OutputBuffer::new(20);

        // Create UTF-8 data that might be truncated at boundary
        let utf8_data = "Hello 世界 测试"; // Mix of ASCII and UTF-8

        // Try to add more data than the buffer can hold
        let large_utf8 = utf8_data.repeat(10); // Much larger than 20 bytes
        let written = buffer.append_stdout(large_utf8.as_bytes());

        assert!(written > 0);
        assert_buffer_state(&buffer, written, written, true, 20);

        // Verify the output is still valid UTF-8 or handled gracefully
        let output = buffer.get_stdout();
        assert!(!output.is_empty());
        // The output should be valid UTF-8 due to safe truncation
    }

    #[test]
    fn test_output_buffer_total_bytes_tracking() {
        let mut buffer = OutputBuffer::new(20);

        // Add data that exceeds limit
        let data1 = b"first chunk of data\n"; // 20 bytes
        let data2 = b"second chunk that exceeds\n"; // 26 bytes

        let written1 = buffer.append_stdout(data1);
        let written2 = buffer.append_stdout(data2);

        // Total processed should include all attempted data
        let total = buffer.total_bytes_processed();
        assert_eq!(total, data1.len() + data2.len());

        // But current size should be limited
        assert_buffer_state(&buffer, written1 + written2, written1 + written2, true, 20);
    }

    #[test]
    fn test_output_buffer_binary_detection() {
        let mut buffer = OutputBuffer::new(1000);

        // Add normal text data
        let text_data = b"hello world\n";
        buffer.append_stdout(text_data);
        assert!(!buffer.has_binary_content());

        // Add binary data
        let binary_data = vec![0u8, 1u8, 2u8, 255u8];
        buffer.append_stderr(&binary_data);
        assert!(buffer.has_binary_content());
    }

    #[test]
    fn test_output_buffer_comprehensive_binary_detection() {
        // Test various types of binary content

        // Pure text - should not be detected as binary
        let mut text_buffer = OutputBuffer::new(1000);
        text_buffer.append_stdout(b"Normal ASCII text with numbers 123\n");
        text_buffer.append_stdout(b"Tab\ttab and newlines\n\r\n");
        assert!(!text_buffer.has_binary_content());

        // Null bytes - definitive binary content
        let mut null_buffer = OutputBuffer::new(1000);
        null_buffer.append_stdout(b"text with\x00null byte");
        assert!(null_buffer.has_binary_content());

        // Mixed content - binary should be detected
        let mut mixed_buffer = OutputBuffer::new(1000);
        mixed_buffer.append_stdout(b"normal text\n");
        mixed_buffer.append_stderr(b"stderr with\x00binary");
        assert!(mixed_buffer.has_binary_content());
    }

    #[test]
    fn test_output_buffer_binary_content_formatting() {
        let mut buffer = OutputBuffer::new(1000);

        // Add binary data to both streams
        let binary_stdout = vec![0x00, 0x01, 0xFF, b'a', b'b'];
        let binary_stderr = vec![0x02, 0x03, 0xFE, b'c', b'd'];

        buffer.append_stdout(&binary_stdout);
        buffer.append_stderr(&binary_stderr);

        assert!(buffer.has_binary_content());

        // Check that formatted output indicates binary content
        let stdout_formatted = buffer.get_stdout();
        let stderr_formatted = buffer.get_stderr();

        assert!(stdout_formatted.contains("Binary content"));
        assert!(stdout_formatted.contains("bytes"));
        assert!(stderr_formatted.contains("Binary content"));
        assert!(stderr_formatted.contains("bytes"));
    }

    #[test]
    fn test_binary_content_detection_function() {
        // Test normal text
        assert!(!is_binary_content(b"hello world"));
        assert!(!is_binary_content(b"hello\nworld\t"));
        assert!(!is_binary_content(b"hello\r\nworld"));

        // Test binary content
        assert!(is_binary_content(&[0u8, 1u8, 2u8])); // null bytes
        assert!(is_binary_content(b"hello\x00world")); // embedded null
    }

    #[test]
    fn test_format_output_content() {
        // Test normal text formatting
        let text_data = b"hello world";
        let result = format_output_content(text_data, false);
        assert_eq!(result, "hello world");

        // Test binary content formatting
        let binary_data = vec![0u8, 1u8, 2u8];
        let result = format_output_content(&binary_data, true);
        assert!(result.contains("Binary content"));
        assert!(result.contains("3 bytes"));

        // Test binary detection in function
        let result = format_output_content(&binary_data, false);
        assert!(result.contains("Binary content"));
    }

    #[test]
    fn test_output_buffer_truncation_marker() {
        let mut buffer = OutputBuffer::new(50);

        // Fill buffer to capacity
        let data = vec![b'a'; 60];
        let written = buffer.append_stdout(&data);
        assert_buffer_state(&buffer, written, written, true, 50);

        // Add truncation marker
        buffer.add_truncation_marker();

        let stdout = buffer.get_stdout();
        assert!(stdout.contains("Output truncated"));
    }

    #[test]
    fn test_output_buffer_line_boundary_truncation() {
        let mut buffer = OutputBuffer::new(20); // Small limit

        // Add data with line boundaries
        let data = b"line1\nline2\nline3\n";
        let written = buffer.append_stdout(data);
        assert_buffer_state(&buffer, written, written, data.len() > 20, 20);

        let output = buffer.get_stdout();
        // Should preserve line structure where possible
        assert!(output.ends_with('\n') || buffer.is_truncated());
    }

    #[test]
    fn test_output_limits_default() {
        let limits = OutputLimits::default();
        assert_eq!(limits.max_output_size, 10 * 1024 * 1024); // 10MB
        assert_eq!(limits.max_line_length, 2000);
        assert!(!limits.enable_streaming);
    }

    #[test]
    fn test_memory_management_large_buffer() {
        // This test verifies that our output buffer doesn't consume excessive memory
        // even when processing large amounts of data

        let small_limit = 1024; // 1KB limit
        let mut buffer = OutputBuffer::new(small_limit);

        // Simulate processing a large amount of data in chunks
        let chunk_size = 100;
        let chunk = vec![b'A'; chunk_size];

        let mut total_written = 0;
        let mut iterations = 0;

        // Keep adding data until we hit the limit
        while !buffer.is_at_limit() && iterations < 50 {
            let written = buffer.append_stdout(&chunk);
            total_written += written;
            iterations += 1;

            // Verify we don't exceed our limit
            assert!(buffer.current_size() <= small_limit);
        }

        // Should have written some data but not exceed limit
        assert!(total_written > 0);
        assert!(buffer.current_size() <= small_limit);
        assert!(buffer.is_truncated());

        // Verify truncation marker can be added
        buffer.add_truncation_marker();
        let output = buffer.get_stdout();
        assert!(output.contains("Output truncated"));

        println!(
            "Memory test: wrote {} bytes, buffer size: {}, truncated: {}",
            total_written,
            buffer.current_size(),
            buffer.is_truncated()
        );
    }

    #[test]
    fn test_default_config_values() {
        // Test DefaultShellConfig methods use valid defaults
        assert_eq!(DefaultShellConfig::max_output_size(), 10 * 1024 * 1024);
        assert_eq!(DefaultShellConfig::max_line_length(), 2000);
    }

    /// Helper function to assert error severity
    ///
    /// This eliminates duplication in error severity test assertions.
    fn assert_error_severity(error: ShellError, expected: ErrorSeverity, description: &str) {
        assert_eq!(error.severity(), expected, "{}", description);
    }

    #[test]
    fn test_shell_error_severity_critical() {
        // Test system-level failures are Critical
        assert_error_severity(
            ShellError::CommandSpawnError {
                command: "test".to_string(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "command not found"),
            },
            ErrorSeverity::Critical,
            "CommandSpawnError should be Critical",
        );

        assert_error_severity(
            ShellError::SystemError {
                message: "system failure".to_string(),
            },
            ErrorSeverity::Critical,
            "SystemError should be Critical",
        );
    }

    #[test]
    fn test_shell_error_severity_error() {
        // Test execution/validation failures are Error level
        assert_error_severity(
            ShellError::ExecutionError {
                command: "test".to_string(),
                message: "execution failed".to_string(),
            },
            ErrorSeverity::Error,
            "ExecutionError should be Error",
        );

        assert_error_severity(
            ShellError::InvalidCommand {
                message: "invalid syntax".to_string(),
            },
            ErrorSeverity::Error,
            "InvalidCommand should be Error",
        );

        assert_error_severity(
            ShellError::WorkingDirectoryError {
                message: "directory not found".to_string(),
            },
            ErrorSeverity::Error,
            "WorkingDirectoryError should be Error",
        );
    }

    #[test]
    fn test_all_shell_errors_have_severity() {
        // Ensure all ShellError variants have severity assigned
        let errors = vec![
            ShellError::CommandSpawnError {
                command: "test".to_string(),
                source: std::io::Error::other("test"),
            },
            ShellError::ExecutionError {
                command: "test".to_string(),
                message: "test".to_string(),
            },
            ShellError::InvalidCommand {
                message: "test".to_string(),
            },
            ShellError::SystemError {
                message: "test".to_string(),
            },
            ShellError::WorkingDirectoryError {
                message: "test".to_string(),
            },
        ];

        for error in errors {
            // This will fail to compile if Severity is not implemented
            let _severity = error.severity();
        }
    }
}
