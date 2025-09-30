# Create Rules Crate Structure and Basic Types

Refer to ideas/rules.md

## Goal

Create the `swissarmyhammer-rules` crate with basic structure and type definitions.

## Context

The rules crate will mirror the prompts crate structure but with rule-specific types. This is the foundation for all rule functionality.

## Implementation

1. Create `swissarmyhammer-rules/` directory
2. Create `Cargo.toml` copying workspace patterns from prompts
3. Create `src/lib.rs` with public API exports
4. Create basic module structure:
   - `lib.rs` - Public API
   - `rules.rs` - Rule struct and core types
   - `severity.rs` - Severity enum

5. Define `Severity` enum in `severity.rs`:
```rust
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}
```

6. Add crate to workspace `Cargo.toml` members list

## Testing

- Verify crate builds with `cargo build -p swissarmyhammer-rules`
- Verify crate is in workspace

## Success Criteria

- [ ] Crate directory and Cargo.toml created
- [ ] Basic module structure exists
- [ ] Severity enum defined
- [ ] Crate builds successfully
- [ ] Crate added to workspace
