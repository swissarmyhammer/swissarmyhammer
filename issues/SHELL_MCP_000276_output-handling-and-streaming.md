# Output Handling and Streaming Implementation

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Implement advanced output handling features including output size limits, binary content handling, and real-time streaming support for long-running commands.

## Objective

Enhance the shell execution engine with robust output management, preventing memory issues from large outputs while providing comprehensive output capture and formatting.

## Requirements

### Output Size Management
- Implement configurable output size limits
- Truncate excessive output gracefully
- Preserve both stdout and stderr within limits
- Provide clear truncation indicators

### Binary Output Handling
- Detect binary vs text content
- Handle binary output safely without corruption
- Provide appropriate encoding for binary data
- Prevent binary content from breaking responses

### Output Streaming (Foundation)
- Prepare infrastructure for real-time output streaming
- Buffer output efficiently during execution
- Handle output ordering between stdout/stderr
- Maintain performance with large outputs

### Memory Management
- Prevent memory exhaustion from large command outputs
- Use efficient buffering strategies
- Clean up resources properly
- Handle resource limits gracefully

## Implementation Details

### Output Buffer Management
```rust
use std::io::{BufRead, BufReader};

struct OutputBuffer {
    max_size: usize,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    truncated: bool,
}

impl OutputBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            max_size,
            stdout: Vec::with_capacity(8192),
            stderr: Vec::with_capacity(8192),
            truncated: false,
        }
    }
    
    fn append_stdout(&mut self, data: &[u8]) {
        if self.total_size() + data.len() > self.max_size {
            self.truncated = true;
            // Truncate while preserving structure
        } else {
            self.stdout.extend_from_slice(data);
        }
    }
}
```

### Binary Content Detection
```rust
fn is_binary_content(data: &[u8]) -> bool {
    // Use heuristics to detect binary content
    data.iter().take(8192).any(|&b| b < 32 && b != b'\n' && b != b'\r' && b != b'\t')
}

fn format_output_content(data: &[u8]) -> String {
    if is_binary_content(data) {
        format!("[Binary content: {} bytes]", data.len())
    } else {
        String::from_utf8_lossy(data).to_string()
    }
}
```

### Enhanced Response Structure
```rust
pub struct ShellExecutionResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub execution_time_ms: u64,
    pub working_directory: PathBuf,
    pub output_truncated: bool,
    pub total_output_size: usize,
    pub binary_output_detected: bool,
}
```

### Configuration Integration
```rust
#[derive(Debug, Clone)]
pub struct OutputLimits {
    pub max_output_size: usize,      // Default: 10MB
    pub max_line_length: usize,      // Default: 2000 chars
    pub enable_streaming: bool,       // Default: false (future use)
}

impl Default for OutputLimits {
    fn default() -> Self {
        Self {
            max_output_size: 10 * 1024 * 1024,  // 10MB
            max_line_length: 2000,
            enable_streaming: false,
        }
    }
}
```

## Advanced Features

### Intelligent Truncation
- Preserve structure when truncating (don't cut mid-line)
- Keep beginning and end portions of output
- Add clear truncation markers
- Maintain readability of truncated content

### Output Analysis
- Detect common output patterns (JSON, logs, etc.)
- Provide summary information for large outputs
- Identify error patterns in output
- Extract key information from verbose output

### Performance Optimization
- Use efficient I/O operations
- Minimize memory copies
- Stream processing where possible
- Handle concurrent stdout/stderr properly

## Integration Points

### Configuration System
- Integrate with existing configuration framework
- Allow per-command output limits
- Support global default settings
- Enable runtime configuration updates

### Error Handling
- Handle I/O errors during output capture
- Manage resource exhaustion gracefully
- Provide clear error messages for output issues
- Integrate with existing error hierarchy

## Acceptance Criteria

- [ ] Output size limits prevent memory exhaustion
- [ ] Binary content handled without corruption
- [ ] Truncation indicators clear and informative
- [ ] Performance maintained with large outputs
- [ ] Memory usage stays within reasonable bounds
- [ ] Response metadata includes output information
- [ ] Configuration integration works properly
- [ ] Cross-platform output handling consistent

## Testing Requirements

- [ ] Tests with various output sizes (small, large, excessive)
- [ ] Binary content handling tests
- [ ] Memory usage tests with large outputs
- [ ] Performance benchmarks for output processing
- [ ] Truncation behavior tests
- [ ] Cross-platform output encoding tests

## Configuration Options

### Global Settings (for future steps)
```toml
[shell_tool]
max_output_size = "10MB"
max_line_length = 2000
truncate_strategy = "preserve_structure"
binary_detection = true
```

## Notes

- This step enhances the basic execution engine with production-ready output handling
- Focus on preventing resource exhaustion and memory issues
- Binary content handling is important for development tool outputs
- Streaming foundation prepares for future real-time output features
- Configuration integration enables deployment-specific tuning