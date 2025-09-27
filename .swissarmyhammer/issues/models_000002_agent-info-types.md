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