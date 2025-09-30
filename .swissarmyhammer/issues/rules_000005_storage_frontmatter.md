# Copy and Adapt Storage and Frontmatter Modules

Refer to ideas/rules.md

## Goal

Copy `storage.rs` and `frontmatter.rs` from prompts crate and adapt for rules.

## Context

These modules provide the foundation for loading rules from files. They need to be copied and adapted to work with Rule types instead of Prompt types.

## Implementation

1. Copy `swissarmyhammer-prompts/src/storage.rs` to `swissarmyhammer-rules/src/storage.rs`
2. Adapt `StorageBackend` trait for `Rule` type instead of `Prompt`
3. Adapt `MemoryStorage` and `FileStorage` implementations

4. Copy `swissarmyhammer-prompts/src/frontmatter.rs` to `swissarmyhammer-rules/src/frontmatter.rs`
5. This module is generic and should work as-is for parsing YAML frontmatter

6. Update `lib.rs` to export these modules

## Testing

- Unit tests for storage backends with Rule types
- Unit tests for frontmatter parsing with rule-specific fields (severity, auto_fix)

## Success Criteria

- [ ] storage.rs adapted for Rule type
- [ ] frontmatter.rs copied (works as-is)
- [ ] Unit tests for storage passing
- [ ] Unit tests for frontmatter passing
