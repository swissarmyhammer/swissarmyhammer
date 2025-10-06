# Move Parameter types to swissarmyhammer-common

## Location
`swissarmyhammer-prompts/src/prompts.rs:37`

## Current State
```rust
// TODO: Move these types to swissarmyhammer-common
```

## Description
Parameter types are currently defined in the prompts module but should be moved to swissarmyhammer-common to avoid circular dependencies and improve code organization.

## Requirements
- Move Parameter-related types from swissarmyhammer-prompts to swissarmyhammer-common
- Update all imports across the codebase
- Ensure no circular dependencies are introduced
- Update documentation

## Impact
This affects multiple crates and may require careful dependency management.