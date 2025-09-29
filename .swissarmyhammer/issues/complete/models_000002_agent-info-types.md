# Step 2: Agent Information and Source Types

Refer to ideas/models.md

## Objective

Add core types for agent discovery and management, including agent source tracking and metadata structures.

## Tasks

### 1. Add Agent Source Enumeration
- Add `AgentSource` enum to `swissarmyhammer-config/src/agent.rs`
- Values: `Builtin`, `Project`, `User` 
- Derive `Debug`, `Clone`, `PartialEq`, `Eq`

### 2. Add Agent Information Structure
- Add `AgentInfo` struct to hold complete agent metadata
- Fields: `name`, `content`, `source`, `description` (optional)
- Derive appropriate traits

### 3. Add Agent Error Types
- Add `AgentError` enum for agent-specific errors
- Include cases: `NotFound`, `InvalidPath`, `IoError`, `ParseError`
- Implement `std::error::Error` and `Display` traits
- Use `thiserror` for clean error definitions

### 4. Add Description Parsing
- Add `parse_agent_description()` helper function
- Extract description from YAML front matter or comments
- Return `Option<String>` for optional descriptions

## Implementation Notes

- Place all new types in `swissarmyhammer-config/src/agent.rs`
- Follow existing error patterns in the codebase
- Keep description parsing simple - look for `description:` in YAML or `# Description:` comments
- Export new types in `lib.rs`

## Acceptance Criteria

- All new types compile and have proper trait implementations
- Error types integrate with existing error handling patterns
- Description parsing handles both YAML and comment formats
- Types are exported for use in other modules

## Files to Modify

- `swissarmyhammer-config/src/agent.rs`
- `swissarmyhammer-config/src/lib.rs` (exports)

## Proposed Solution

Based on the existing code patterns in `swissarmyhammer-config/src/agent.rs` and the specifications in `ideas/models.md`, I will implement:

### 1. AgentSource Enum
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSource {
    Builtin,
    Project, 
    User,
}
```

### 2. AgentInfo Struct
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub content: String,
    pub source: AgentSource,
    pub description: Option<String>,
}
```

### 3. AgentError Enum (using thiserror)
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Agent '{0}' not found")]
    NotFound(String),
    #[error("Invalid agent path: {0}")]
    InvalidPath(PathBuf),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    ParseError(#[from] serde_yaml::Error),
}
```

### 4. Description Parsing Function
```rust
pub fn parse_agent_description(content: &str) -> Option<String> {
    // Check for YAML front matter first
    if content.trim_start().starts_with("---") {
        // Parse YAML front matter for description field
    }
    
    // Fall back to comment-based description
    for line in content.lines() {
        if let Some(desc) = line.strip_prefix("# Description:").map(|s| s.trim()) {
            return Some(desc.to_string());
        }
    }
    
    None
}
```

### Implementation Strategy:
1. Follow existing patterns in the file for consistency
2. Add comprehensive unit tests for all new types
3. Use appropriate serde derives and attributes
4. Integrate with existing error handling patterns
5. Export new types in lib.rs

This approach maintains consistency with the existing codebase while adding the required functionality for agent discovery and management.
## Implementation Complete

All core types have been successfully implemented in `swissarmyhammer-config/src/agent.rs`:

### ✅ Completed Tasks

1. **AgentSource Enum** - Added with `Builtin`, `Project`, `User` variants
   - Derives: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`
   - Uses kebab-case serialization format (`"builtin"`, `"project"`, `"user"`)

2. **AgentInfo Struct** - Complete metadata container
   - Fields: `name: String`, `content: String`, `source: AgentSource`, `description: Option<String>`
   - Derives: `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`

3. **AgentError Enum** - Comprehensive error handling using thiserror
   - `NotFound(String)` - Agent not found by name
   - `InvalidPath(PathBuf)` - Invalid file path
   - `IoError` - File system errors (auto-conversion from std::io::Error)
   - `ParseError` - YAML parsing errors (auto-conversion from serde_yaml::Error)

4. **parse_agent_description() Function** - Dual-format parsing
   - YAML front matter support: `description: "value"` in `---` blocks
   - Comment format support: `# Description: value` lines
   - YAML takes precedence over comments
   - Proper whitespace trimming and empty string handling

5. **Library Exports** - All new types exported in `swissarmyhammer-config/src/lib.rs`
   - `AgentSource`, `AgentInfo`, `AgentError`, `parse_agent_description`

6. **Comprehensive Test Coverage** - 19 new unit tests added
   - AgentSource serialization/deserialization tests
   - AgentError display and conversion tests  
   - AgentInfo equality and serialization tests
   - parse_agent_description tests for all scenarios (YAML, comments, precedence, malformed content, whitespace handling)

### ✅ Verification Results

- **Build**: ✅ `cargo build` - successful compilation
- **Tests**: ✅ `cargo nextest run` - 2776 tests passed (including all new tests)
- **Format**: ✅ `cargo fmt --all` - code properly formatted
- **Integration**: ✅ All types properly integrated with existing error handling patterns

### Implementation Notes

- Followed existing code patterns for consistency (serde attributes, error handling, documentation style)
- Used thiserror for clean error definitions with automatic conversions
- Maintained backwards compatibility with existing AgentConfig types
- Added comprehensive documentation for all public types and functions
- Description parsing handles both YAML front matter and comment formats robustly

All acceptance criteria have been met and the implementation is ready for integration with the agent management system.

## Code Review Fixes Completed

During the code review process, I identified and fixed the following issues:

### Fixed Issues
1. **Clippy lint violation** in `swissarmyhammer-config/src/agent.rs:338`
   - Replaced manual string stripping (`content[3..]`) with idiomatic `strip_prefix` method
   - This resolves the `clippy::manual_strip` warning

2. **Empty line after doc comment** in `swissarmyhammer-workflow/src/action_parser.rs:592`
   - Removed empty line between doc comments to fix `clippy::empty-line-after-doc-comments`

### Verification Results
- ✅ `cargo clippy --all-targets -- -D warnings` - All clippy warnings resolved
- ✅ `cargo nextest run` - All 2776 tests passing
- ✅ Build system working correctly with agent generation

The implementation is now ready and complies with all lint requirements.