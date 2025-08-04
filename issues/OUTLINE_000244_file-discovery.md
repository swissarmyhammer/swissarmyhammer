# OUTLINE_000244: File Discovery and Glob Pattern Processing

Refer to ./specification/outline_tool.md

## Summary

Implement file discovery functionality that processes glob patterns and respects .gitignore files, building on existing search infrastructure. This creates the foundation for finding files to parse and outline.

## Context

The outline tool needs to discover files matching glob patterns while respecting gitignore rules, similar to the existing search indexing functionality. We can reuse much of the existing file discovery logic from the search module.

## Requirements

### 1. File Discovery Module
Create `src/outline/` module with file discovery capabilities:
```rust
// src/outline/mod.rs
pub mod file_discovery;
pub mod types;
```

### 2. Glob Pattern Processing
- Support multiple glob patterns: `["src/**/*.rs", "lib/**/*.ts"]`
- Handle complex patterns: `["**/*.{ts,js,rs,dart,py}"]`
- Validate patterns before processing
- Provide clear error messages for invalid patterns

### 3. Gitignore Integration
- Automatically respect .gitignore patterns
- Skip common build artifacts (target/, node_modules/, dist/)
- Skip generated files and dependencies
- Log skipped files for debugging

### 4. Language Detection
- Detect file language from extension
- Support languages: Rust (.rs), TypeScript (.ts), JavaScript (.js), Dart (.dart), Python (.py)
- Handle edge cases (multiple extensions, unknown files)
- Provide language filtering capabilities

## Technical Details

### File Discovery API
```rust
pub struct FileDiscovery {
    patterns: Vec<String>,
    gitignore: Option<GitignoreBuilder>,
}

impl FileDiscovery {
    pub fn new(patterns: Vec<String>) -> Result<Self>;
    pub fn discover_files(&self) -> Result<Vec<DiscoveredFile>>;
    pub fn with_gitignore(mut self, enable: bool) -> Self;
}

pub struct DiscoveredFile {
    pub path: PathBuf,
    pub language: Option<Language>,
    pub relative_path: String,
}
```

### Language Detection
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Dart,
    Python,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self>;
    pub fn tree_sitter_language(&self) -> tree_sitter::Language;
}
```

## Implementation Steps

1. Create `src/outline/` module structure
2. Implement basic file discovery with glob processing
3. Add gitignore integration using existing patterns
4. Implement language detection from file extensions
5. Create comprehensive error handling
6. Add logging for discovery process
7. Write unit and integration tests

## Integration Points

### Reuse Existing Infrastructure
- Leverage `glob` crate usage patterns from search module
- Use existing gitignore handling from search indexer
- Follow established error handling patterns
- Reuse file system utilities where appropriate

### MCP Tool Integration
```rust
// In src/mcp/tools/outline/generate/mod.rs
pub async fn handle_generate_outline(
    request: OutlineRequest,
) -> Result<OutlineResponse> {
    let discovery = FileDiscovery::new(request.patterns)?;
    let files = discovery.discover_files()?;
    
    // Continue to parsing phase...
}
```

## Testing Requirements

### Unit Tests
- Glob pattern validation
- Language detection accuracy  
- Error handling for invalid patterns
- Edge cases (empty patterns, non-existent directories)

### Integration Tests
- File discovery in test repositories
- Gitignore respect verification
- Performance with large file sets
- Cross-platform path handling

## Performance Considerations

- Lazy evaluation of file discovery
- Early filtering to avoid unnecessary I/O
- Parallel processing for large file sets
- Memory-efficient handling of file lists

## Error Handling

- Clear error messages for invalid glob patterns
- Graceful handling of permission denied errors
- Informative logging for debugging
- Recovery from partial failures

## Success Criteria

- Correctly processes all specified glob patterns
- Respects .gitignore files automatically
- Detects languages accurately for all supported extensions
- Handles edge cases gracefully
- Performance suitable for large codebases
- Comprehensive test coverage
- Clear error messages and logging

## Dependencies

- `glob` crate for pattern matching
- `ignore` crate for gitignore processing (if not already available)
- Existing file system utilities
- Standard library path handling

## Notes

This step builds the foundation for file processing. The file discovery should be efficient and robust as it will be used for every outline generation request. Consider caching mechanisms for repeated requests in the same directory.