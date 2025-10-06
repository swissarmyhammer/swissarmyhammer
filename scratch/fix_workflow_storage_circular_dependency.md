# Fix circular dependency in workflow storage

## Location
`swissarmyhammer-tools/src/mcp/server.rs:11`

## Current State
```rust
// TODO: Move workflow storage to swissarmyhammer-common to fix circular dependency
```

## Description
Workflow storage is currently in a location that creates circular dependencies with other modules. Moving it to swissarmyhammer-common would resolve this issue.

## Requirements
- Move workflow storage functionality to swissarmyhammer-common
- Update all imports and references
- Ensure no new circular dependencies are created
- Run all tests to verify the refactoring

## Related
Similar to the Parameter types issue - both involve improving module organization.